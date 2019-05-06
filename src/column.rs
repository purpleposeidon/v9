//! Columns and their extractions.

use crate::event::*;
use crate::prelude_lib::*;
use std::hint::unreachable_unchecked;

#[derive(Debug)]
pub struct Column<M: TableMarker, T> {
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
}
pub struct EditColumn<'a, M: TableMarker, T>
where
    T: Clone,
{
    #[doc(hidden)]
    pub col: &'a mut Column<M, T>,
    must_log: bool,
    log: &'a mut Vec<(Id<M>, T)>,
}
pub struct WriteColumn<'a, M: TableMarker, T> {
    pub col: MutButRef<'a, Column<M, T>>,
}

impl<'a, I, M: TableMarker, T> Index<I> for ReadColumn<'a, M, T>
where
    T: Clone,
    I: Check<'a, M = M>,
{
    type Output = T;
    fn index(&self, i: I) -> &T {
        unsafe {
            let i = i.check(PhantomData, self.col.data.len());
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
            let i = i.check(PhantomData, self.col.data.len());
            if let Some((prev, dude)) = self.log.last() {
                match prev.cmp(&i.uncheck()) {
                    Ordering::Less => panic!("disordered column access"),
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
            let i = i.check(PhantomData, self.col.data.len());
            if !self.must_log {
                return self.col.data.get_unchecked_mut(i.to_usize());
            }
            let prev = self.log.last().map(|(i, _)| i);
            if let Some(prev) = prev {
                match prev.cmp(&i.uncheck()) {
                    Ordering::Less => panic!("disordered column access"),
                    Ordering::Equal => (),
                    Ordering::Greater => {
                        let val = self.col.data.get_unchecked(i.to_usize()).clone();
                        self.log.push((i.uncheck(), val))
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
            let i = i.check(PhantomData, self.col.data.len());
            self.col.data.get_unchecked(i.to_usize())
        }
    }
}
// WriteColumn is append-only, so IndexMut is not provided.

impl<'a, M: TableMarker, T> WriteColumn<'a, M, T> {
    pub fn borrow(&self) -> ReadColumn<M, T> {
        ReadColumn { col: &*self.col }
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
            col: obj.downcast_ref().unwrap(),
        }
    }
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
    type Owned = (
        &'a mut Column<M, T>, // col
        bool,                 // must_log
        Vec<(Id<M>, T)>,      // log
    );
    unsafe fn extract(universe: &Universe, rez: &mut Rez) -> Self::Owned {
        let col: &mut Column<M, T> = rez.take_mut_downcast();
        let must_log = universe.has::<Tracker<EditColumn<M, T>>>();
        let log = vec![];
        (col, must_log, log)
    }
    unsafe fn convert(_universe: &Universe, owned: *mut Self::Owned) -> Self {
        let owned: &mut Self::Owned = &mut *owned;
        let &mut (ref mut col, must_log, ref mut log) = owned;
        EditColumn { col, must_log, log }
    }
    fn finish(universe: &Universe, (col, must_log, log): Self::Owned) {
        if !must_log {
            return;
        }
        if log.is_empty() {
            return;
        }
        let log = {
            let col: &'static Column<M, T> = unsafe { mem::transmute(&*col) };
            let ev = Edited { col, new: log };
            universe.submit_event(&ev);
            ev.new
        };
        for (id, new) in log.into_iter() {
            col.data[id.0.to_usize()] = new;
        }
    }
}
unsafe impl<'a, M, T> Extract for WriteColumn<'a, M, T>
where
    M: TableMarker,
    T: 'static + Send + Sync,
{
    fn each_resource(f: &mut dyn FnMut(TypeId, Access)) {
        f(TypeId::of::<Column<M, T>>(), Access::Write)
    }
    // (col, must_log, old_len)
    type Owned = (&'a mut Column<M, T>, bool, usize);
    unsafe fn extract(universe: &Universe, rez: &mut Rez) -> Self::Owned {
        let must_log = universe.has::<Tracker<Pushed<M>>>();
        let col: &mut Column<M, T> = rez.take_mut_downcast();
        let old_len = col.data.len();
        (col, must_log, old_len)
    }
    unsafe fn convert(_universe: &Universe, owned: *mut Self::Owned) -> Self {
        let owned: &mut Self::Owned = &mut *owned;
        WriteColumn {
            col: MutButRef::new(owned.0),
        }
    }
    fn finish(universe: &Universe, (col, must_log, old_len): Self::Owned) {
        if !must_log {
            return;
        }
        let new_len = col.data.len();
        if old_len == new_len {
            return;
        }
        universe.submit_event(&Pushed::<M> {
            range: IdRange {
                _a: PhantomData,
                start: Id::from_usize(old_len),
                end: Id::from_usize(new_len),
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
