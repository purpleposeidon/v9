//! Columns and their extractions.

use crate::event::*;
use crate::prelude_lib::*;
use std::hint::unreachable_unchecked;

pub type NoSend = PhantomData<*mut ()>;

#[derive(Debug)]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Column<M: TableMarker, T> {
    #[serde(skip)]
    pub table_marker: M,
    pub data: Vec<T>,
}
impl<M: TableMarker, T> Default for Column<M, T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<M: TableMarker, T: 'static + Send + Sync> Obj for Column<M, T> {}
impl<M: TableMarker, T> Column<M, T> {
    pub fn new() -> Self {
        Column {
            table_marker: Default::default(),
            data: vec![],
        }
    }
}

pub struct ReadColumn<'a, M: TableMarker, T> {
    pub col: &'a Column<M, T>,
    pub no_send: NoSend,
}
/// You can change the values in this column, but not the length.
/// Changes may be logged. Because of this, you must access items in increasing order.
// FIXME: Maybe we could work around this. What if we saved a copy of the original to the log?
// HashSet?
pub struct EditColumn<'a, M: TableMarker, T>
where
    T: Clone,
{
    #[doc(hidden)]
    pub col: &'a mut Column<M, T>,
    must_log: bool,
    log: &'a mut Vec<(Id<M>, T)>,
    // FIXME: pub no_send: NoSend,
}
pub struct WriteColumn<'a, M: TableMarker, T> {
    pub col: MutButRef<'a, Column<M, T>>,
    pub no_send: NoSend,
}

#[cold]
fn disordered_column_access() -> ! { panic!("disordered column access") }
impl<'a, I, M: TableMarker, T> Index<I> for ReadColumn<'a, M, T>
where
    I: Check<'a, M = M>,
{
    type Output = T;
    fn index(&self, i: I) -> &T {
        unsafe {
            let i = i.check_from_len(PhantomData, self.col.data.len());
            self.col.data.get_unchecked(i.to_usize())
        }
    }
}
impl<'a, I, M: TableMarker, T> Index<I> for EditColumn<'a, M, T>
where
    T: Clone,
    I: Check<'a, M = M>,
{
    type Output = T;
    fn index(&self, i: I) -> &T {
        unsafe {
            let i = i.check_from_len(PhantomData, self.col.data.len());
            if let Some((prev, dude)) = self.log.last() {
                match prev.cmp(&i.uncheck()) {
                    Ordering::Less => disordered_column_access(),
                    Ordering::Equal => dude,
                    Ordering::Greater => self.col.data.get_unchecked(i.to_usize()),
                }
            } else {
                self.col.data.get_unchecked(i.to_usize())
            }
        }
    }
}
impl<'a, I, M: TableMarker, T> IndexMut<I> for EditColumn<'a, M, T>
where
    T: Clone,
    I: Check<'a, M = M>,
{
    fn index_mut(&mut self, i: I) -> &mut T {
        unsafe {
            let i = i.check_from_len(PhantomData, self.col.data.len());
            let i = i.uncheck();
            if !self.must_log {
                return self.col.data.get_unchecked_mut(i.to_usize());
            }
            let prev = self.log.last().map(|(i, _)| i);
            if let Some(prev) = prev {
                match prev.cmp(&i) {
                    Ordering::Less => disordered_column_access(),
                    Ordering::Equal => (),
                    Ordering::Greater => {
                        let val = self.col.data.get_unchecked(i.to_usize()).clone();
                        self.log.push((i, val))
                    }
                }
            }
            if let Some((_i, v)) = self.log.last_mut() {
                v
            } else {
                unreachable_unchecked()
            }
        }
    }
}
impl<'a, M: TableMarker, T, I> Index<I> for WriteColumn<'a, M, T>
where
    I: Check<'a, M = M>,
{
    type Output = T;
    fn index(&self, i: I) -> &T {
        unsafe {
            let i = i.check_from_len(PhantomData, self.col.data.len());
            self.col.data.get_unchecked(i.to_usize())
        }
    }
}
// WriteColumn is append-only, so IndexMut is not provided.

impl<'a, M: TableMarker, T> WriteColumn<'a, M, T> {
    pub fn borrow(&self) -> ReadColumn<M, T> {
        ReadColumn { no_send: PhantomData, col: &*self.col }
    }
}

unsafe impl<'a, M, T: Send + Sync> ExtractOwned for ReadColumn<'a, M, T>
where
    M: TableMarker,
    T: 'static,
{
    type Ty = Column<M, T>;
    const ACC: Access = Access::Read;
    unsafe fn extract(_universe: &Universe, rez: &mut Rez) -> Self {
        let obj: &'static dyn Obj = rez.take_ref();
        ReadColumn {
            no_send: PhantomData,
            col: obj.downcast_ref().unwrap(),
        }
    }
}
#[doc(hidden)]
pub struct EditColumnOwned<'a, M, T>
where
    M: TableMarker,
{
    col: &'a mut Column<M, T>,
    must_log: bool,
    log: Vec<(Id<M>, T)>,
}
unsafe impl<'a, M, T> Extract for EditColumn<'a, M, T>
where
    M: TableMarker,
    T: 'static + Send + Sync,
    T: Clone,
{
    fn each_resource(f: &mut dyn FnMut(TypeId, Access)) {
        f(TypeId::of::<Column<M, T>>(), Access::Write)
    }
    type Owned = EditColumnOwned<'a, M, T>;
    unsafe fn extract(universe: &Universe, rez: &mut Rez) -> Self::Owned {
        let col: &mut Column<M, T> = rez.take_mut_downcast();
        let must_log = universe.has::<Tracker<EditColumn<M, T>>>();
        let log = vec![];
        EditColumnOwned { col, must_log, log }
    }
    unsafe fn convert(_universe: &Universe, owned: *mut Self::Owned) -> Self {
        let EditColumnOwned { col, must_log, log } = &mut *owned;
        EditColumn { col, must_log: *must_log, log }
    }
    type Cleanup = EditColumnCleanup<M, T>;
}
#[doc(hidden)]
pub struct EditColumnCleanup<M: TableMarker, T> {
    must_log: bool,
    log: Vec<(Id<M>, T)>,
}
unsafe impl<'a, M, T> Cleaner<EditColumn<'a, M, T>> for EditColumnCleanup<M, T>
where
    M: TableMarker,
    T: 'static + Send + Sync,
    T: Clone,
    // or `EditColumn<>: Extract`?
{
    fn pre_cleanup(eco: EditColumnOwned<'a, M, T>, _universe: &Universe) -> Self {
        Self {
            must_log: eco.must_log,
            log: eco.log,
        }
    }
    fn post_cleanup(self, universe: &Universe) {
        if !self.must_log || self.log.is_empty() {
            return;
        }
        let log = universe.with(move |col: &Column<M, T>| {
            let col = col as *const _;
            let mut ev = Edited { col, new: self.log };
            universe.submit_event(&mut ev);
            ev.new
        });
        universe.with_mut(move |col: &mut Column<M, T>| {
            for (id, new) in log.into_iter() {
                col.data[id.0.to_usize()] = new;
            }
        });
    }
}
#[doc(hidden)]
pub struct WriteColLog<M, T>
where
    M: TableMarker,
{
    col: *mut Column<M, T>,
    must_log: bool,
    old_len: usize,
}
unsafe impl<'a, M, T> Extract for WriteColumn<'a, M, T>
where
    M: TableMarker,
    T: 'static + Send + Sync,
{
    fn each_resource(f: &mut dyn FnMut(TypeId, Access)) {
        f(TypeId::of::<Column<M, T>>(), Access::Write)
    }
    type Owned = WriteColLog<M, T>;
    unsafe fn extract(universe: &Universe, rez: &mut Rez) -> Self::Owned {
        let must_log = universe.has::<Tracker<Pushed<M>>>();
        let col: &mut Column<M, T> = rez.take_mut_downcast();
        let len = col.data.len();
        WriteColLog {
            col: col as *mut _,
            must_log,
            old_len: len,
        }
    }
    unsafe fn convert(_universe: &Universe, owned: *mut Self::Owned) -> Self {
        let owned: &mut Column<M, T> = &mut *((*owned).col);
        WriteColumn {
            col: MutButRef::new(owned),
            no_send: PhantomData,
        }
    }
    type Cleanup = WriteColCleanup<M, T>;
}
#[doc(hidden)]
pub struct WriteColCleanup<M, T>
where
    M: TableMarker,
{
    marker: PhantomData<(M, T)>,
    must_log: bool,
    old_len: usize,
    new_len: usize,
}
unsafe impl<'a, M, T> Cleaner<WriteColumn<'a, M, T>> for WriteColCleanup<M, T>
where
    M: TableMarker,
    T: 'static + Send + Sync,
{
    fn pre_cleanup(owned: WriteColLog<M, T>, _universe: &Universe) -> Self {
        let new_len = unsafe { (*owned.col).len() };
        Self {
            marker: PhantomData,
            must_log: owned.must_log,
            old_len: owned.old_len,
            new_len,
        }
    }
    fn post_cleanup(self, universe: &Universe) {
        if !self.must_log || self.new_len == self.old_len {
            return;
        }
        universe.submit_event(&mut Pushed::<M> {
            range: IdRange {
                _a: PhantomData,
                start: Id::from_usize(self.old_len),
                end: Id::from_usize(self.new_len),
            },
        });
    }
}

pub unsafe trait ColumnInfo<M: TableMarker> {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
unsafe impl<M: TableMarker, T> ColumnInfo<M> for Column<M, T> {
    fn len(&self) -> usize {
        self.data.len()
    }
}
unsafe impl<'a, M: TableMarker, T> ColumnInfo<M> for ReadColumn<'a, M, T> {
    fn len(&self) -> usize {
        self.col.data.len()
    }
}
unsafe impl<'a, M: TableMarker, T: Clone> ColumnInfo<M> for EditColumn<'a, M, T> {
    fn len(&self) -> usize {
        self.col.data.len()
    }
}
unsafe impl<'a, M: TableMarker, T> ColumnInfo<M> for WriteColumn<'a, M, T> {
    fn len(&self) -> usize {
        self.col.data.len()
    }
}
