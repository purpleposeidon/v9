//! Columns and their extractions.

use crate::event::*;
use crate::prelude_lib::*;
use std::hint::unreachable_unchecked;
use crate::linkage::LiftColumn;

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Column<M: TableMarker, T: AnyDebug> {
    #[cfg_attr(feature = "serde", serde(skip))]
    pub table_marker: M,
    // NB: This is unsafe to access. You could make the columns have different lengths.
    #[doc(hidden)]
    pub data: Vec<T>,
}
impl<M: TableMarker, T: AnyDebug> Default for Column<M, T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<M: TableMarker, T: AnyDebug> Column<M, T> {
    pub fn new() -> Self {
        Column {
            table_marker: Default::default(),
            data: vec![],
        }
    }
    #[inline(always)] pub fn data(&self) -> &Vec<T> { &self.data }
    #[inline(always)] pub unsafe fn data_mut(&mut self) -> &mut Vec<T> { &mut self.data }
    #[inline(always)] pub fn set_data(&mut self, d: Vec<T>) { self.data = d }
}

pub type FastEdit<'a, C> = FastEditColumn<
    'a,
    <C as LiftColumn>::M,
    <C as LiftColumn>::T,
>;

pub struct ReadColumn<'a, M: TableMarker, T: AnyDebug> {
    pub col: &'a Column<M, T>,
}
pub struct FastEditColumn<'a, M: TableMarker, T: AnyDebug> {
    col: &'a mut Column<M, T>,
}
/// You can change the values in this column, but not the length.
/// Changes may be logged. Because of this, you must access items in increasing order.
// FIXME: Maybe we could work around this. What if we saved a copy of the original to the log?
// HashSet?
pub struct EditColumn<'a, M: TableMarker, T: AnyDebug>
where
    T: Clone,
{
    #[doc(hidden)]
    pub col: &'a mut Column<M, T>,
    must_log: bool,
    log: &'a mut Vec<(Id<M>, T)>,
}
pub struct WriteColumn<'a, M: TableMarker, T: AnyDebug> {
    pub col: MutButRef<'a, Column<M, T>>,
}

#[cold]
fn disordered_column_access() -> ! {
    panic!("disordered column access")
}
impl<'a, 'b, I, M: TableMarker, T: AnyDebug> Index<I> for ReadColumn<'a, M, T>
where
    I: 'b + Check<M = M>,
{
    type Output = T;
    fn index(&self, i: I) -> &T {
        unsafe {
            let i = i.check_from_len(PhantomData, self.col.data.len());
            self.col.data.get_unchecked(i.to_usize())
        }
    }
}
impl<'a, 'b, I, M: TableMarker, T: AnyDebug> Index<I> for FastEditColumn<'a, M, T>
where
    I: 'b + Check<M = M>,
{
    type Output = T;
    fn index(&self, i: I) -> &T {
        unsafe {
            let i = i.check_from_len(PhantomData, self.col.data.len());
            self.col.data.get_unchecked(i.to_usize())
        }
    }
}
impl<'a, 'b, I, M: TableMarker, T: AnyDebug> IndexMut<I> for FastEditColumn<'a, M, T>
where
    I: 'b + Check<M = M>,
{
    fn index_mut(&mut self, i: I) -> &mut T {
        unsafe {
            let i = i.check_from_len(PhantomData, self.col.data.len());
            self.col.data.get_unchecked_mut(i.to_usize())
        }
    }
}
impl<'a, 'b, I, M: TableMarker, T: AnyDebug> Index<I> for EditColumn<'a, M, T>
where
    T: Clone,
    I: 'b + Check<M = M>,
{
    type Output = T;
    fn index(&self, i: I) -> &T {
        unsafe {
            let i = i.check_from_len(PhantomData, self.col.data.len());
            if let Some((prev, dude)) = self.log.last() {
                match i.uncheck().cmp(prev) {
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
impl<'a, 'b, I, M: TableMarker, T: AnyDebug> IndexMut<I> for EditColumn<'a, M, T>
where
    T: Clone,
    I: 'b + Check<M = M>,
{
    fn index_mut(&mut self, i: I) -> &mut T {
        unsafe {
            let i = i.check_from_len(PhantomData, self.col.data.len());
            let i = i.uncheck();
            if !self.must_log {
                return self.col.data.get_unchecked_mut(i.to_usize());
            }
            let prev = self.log.last().map(|(i, _)| i);
            let prev = prev.map(|prev| i.cmp(prev));
            let prev = prev.unwrap_or(Ordering::Greater);
            match prev {
                Ordering::Less => disordered_column_access(),
                Ordering::Equal => (),
                Ordering::Greater => {
                    let val = self.col.data.get_unchecked(i.to_usize()).clone();
                    self.log.push((i, val))
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
impl<'a, 'b, M: TableMarker, T: AnyDebug, I> Index<I> for WriteColumn<'a, M, T>
where
    I: 'b + Check<M = M>,
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

impl<'a, M: TableMarker, T: AnyDebug> WriteColumn<'a, M, T> {
    pub fn borrow(&self) -> ReadColumn<M, T> {
        ReadColumn { col: &*self.col }
    }
}
impl<'a, M: TableMarker, T: AnyDebug> EditColumn<'a, M, T>
where
    T: Clone,
{
    pub fn borrow(&self) -> ReadColumn<M, T> {
        assert!(self.log.is_empty());
        ReadColumn { col: &*self.col }
    }
}

unsafe impl<'a, M, T: AnyDebug> ExtractOwned for ReadColumn<'a, M, T>
where
    M: TableMarker,
    T: 'static,
{
    type Ty = Column<M, T>;
    const ACC: Access = Access::Read;
    unsafe fn extract(_universe: &Universe, rez: &mut Rez) -> Self {
        let obj: &'static dyn AnyDebug = rez.take_ref();
        ReadColumn {
            col: obj.downcast_ref().unwrap(),
        }
    }
}
unsafe impl<'a, M, T: AnyDebug> ExtractOwned for FastEditColumn<'a, M, T>
where
    M: TableMarker,
    T: 'static,
{
    type Ty = Column<M, T>;
    const ACC: Access = Access::Write;
    unsafe fn extract(universe: &Universe, rez: &mut Rez) -> Self {
        let obj: &'static mut dyn AnyDebug = rez.take_mut();
        assert!(!universe.is_tracked::<Edited<M, T>>(), "FastEditColumn used on a tracked column");
        FastEditColumn {
            col: obj.downcast_mut().unwrap(),
        }
    }
}
#[doc(hidden)]
pub struct EditColumnOwned<'a, M: TableMarker, T: AnyDebug> {
    col: &'a mut Column<M, T>,
    must_log: bool,
    log: Vec<(Id<M>, T)>,
}
unsafe impl<'a, M, T> Extract for EditColumn<'a, M, T>
where
    M: TableMarker,
    T: 'static + Send + Sync,
    T: Clone,
    T: AnyDebug,
{
    fn each_resource(f: &mut dyn FnMut(Ty, Access)) {
        f(Ty::of::<Column<M, T>>(), Access::Write)
    }
    type Owned = EditColumnOwned<'a, M, T>;
    unsafe fn extract(universe: &Universe, rez: &mut Rez) -> Self::Owned {
        let col: &mut Column<M, T> = rez.take_mut_downcast();
        let must_log = universe.is_tracked::<Edited<M, T>>();
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
pub struct EditColumnCleanup<M: TableMarker, T: AnyDebug> {
    must_log: bool,
    log: Vec<(Id<M>, T)>,
}
unsafe impl<'a, M, T> Cleaner<EditColumn<'a, M, T>> for EditColumnCleanup<M, T>
where
    M: TableMarker,
    T: 'static + Send + Sync,
    T: Clone,
    T: AnyDebug,
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
unsafe impl<'a, M, T> ExtractOwned for WriteColumn<'a, M, T>
where
    M: TableMarker,
    T: 'static + Send + Sync,
    T: AnyDebug,
{
    type Ty = Column<M, T>;
    const ACC: Access = Access::Write;
    unsafe fn extract(_universe: &Universe, rez: &mut Rez) -> Self {
        WriteColumn {
            col: MutButRef::new(rez.take_mut_downcast()),
        }
    }
}

pub unsafe trait ColumnInfo<M: TableMarker> {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
unsafe impl<M: TableMarker, T: AnyDebug> ColumnInfo<M> for Column<M, T> {
    fn len(&self) -> usize {
        self.data.len()
    }
}
unsafe impl<'a, M: TableMarker, T: AnyDebug> ColumnInfo<M> for ReadColumn<'a, M, T> {
    fn len(&self) -> usize {
        self.col.data.len()
    }
}
unsafe impl<'a, M: TableMarker, T: AnyDebug + Clone> ColumnInfo<M> for EditColumn<'a, M, T> {
    fn len(&self) -> usize {
        self.col.data.len()
    }
}
unsafe impl<'a, M: TableMarker, T: AnyDebug> ColumnInfo<M> for WriteColumn<'a, M, T> {
    fn len(&self) -> usize {
        self.col.data.len()
    }
}
