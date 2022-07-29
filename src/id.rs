//! Ids, lists of Ids, and various iterators.

use crate::event::*;
use crate::prelude_lib::*;
use std::cell::RefCell;
use std::fmt;
use std::ops::{Range, RangeInclusive, Add, Sub};
use std::iter::Peekable;
use std::hash;
use std::cmp::Ordering;

type Run<M> = (Id<M>, Id<M>);

pub trait Raw
where
    Self: 'static + Send + Sync,
    Self: Ord + Copy + fmt::Debug + hash::Hash,
    Self: serde::Serialize + serde::de::DeserializeOwned,
    Self: Add<Output=Self> + Sub<Output=Self>,
    Self: self::raw_impl::Sealed,
{
    fn to_usize(self) -> usize;
    fn from_usize(x: usize) -> Self;
    fn offset(self, d: i8) -> Self;
    const ZERO: Self;
    const LAST: Self;
}
mod raw_impl {
    /// Forbid non-primitives from being put into a SyncRef.
    pub trait Sealed {}
    use super::Raw;
    macro_rules! imp {
        ($($ty:ident),*) => {$(
            impl Sealed for $ty {}
            impl Raw for $ty {
                #[inline]
                fn to_usize(self) -> usize { self as usize }
                #[inline]
                fn from_usize(x: usize) -> Self { x as _ }
                #[inline]
                #[allow(clippy::cast_lossless)]
                fn offset(self, d: i8) -> Self {
                    i64::from(self as i64 + i64::from(d)) as $ty
                }
                const ZERO: Self = 0;
                const LAST: Self = std::$ty::MAX;
            }
        )*};
    }
    imp! { u8, u16, u32, u64 }
    // u128? Absurd.
}

/// A strongly typed row id.
#[derive(Copy, Clone)]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(bound = "M: 'static")]
#[serde(transparent)]
pub struct Id<M: TableMarker>(pub M::RawId);
impl<M: TableMarker> Default for Id<M> {
    fn default() -> Self {
        Self(<M::RawId as Raw>::LAST)
    }
}
impl<M: TableMarker> PartialEq for Id<M> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<M: TableMarker> hash::Hash for Id<M> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        hash::Hash::hash(&self.0, state);
    }
}
impl<M: TableMarker> Eq for Id<M> {}
impl<M: TableMarker> PartialOrd for Id<M> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl<M: TableMarker> Ord for Id<M> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}
impl<M: TableMarker> fmt::Debug for Id<M> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}[{:?}]", M::NAME, self.0)
    }
}
impl<M: TableMarker> Id<M> {
    #[inline]
    pub fn new(i: M::RawId) -> Self {
        Self(i)
    }
    #[inline]
    pub fn from_usize(x: usize) -> Self {
        Id(M::RawId::from_usize(x))
    }
    #[inline]
    pub fn to_usize(self) -> usize {
        M::RawId::to_usize(self.0)
    }
    #[inline]
    pub fn step(self, d: i8) -> Self {
        Id(self.0.offset(d))
    }
    #[inline]
    pub fn next(self) -> Self { self.step(1) }
    #[inline]
    pub fn zero() -> Self { Id(M::RawId::ZERO) }
    #[inline]
    pub fn last() -> Self { Id(M::RawId::LAST) }
}

/// An `Id` that is known to be in-bounds on the given table.
/// You should check the Id if you'll be doing a lot of indexing.
// Hmm, unsound if the columns have inconsistent lengths.
#[derive(Copy, Clone)]
pub struct CheckedId<'a, M: TableMarker> {
    table: PhantomData<&'a M>,
    id: Id<M>,
}
impl<'a, M: TableMarker> Eq for CheckedId<'a, M> {}
impl<'a, M: TableMarker> PartialEq for CheckedId<'a, M> {
    fn eq(&self, other: &Self) -> bool {
        self.id.0.eq(&other.id.0)
    }
}
impl<'a, M: TableMarker> PartialOrd for CheckedId<'a, M> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.id.0.partial_cmp(&other.id.0)
    }
}
impl<'a, M: TableMarker> Ord for CheckedId<'a, M> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.0.cmp(&other.id.0)
    }
}
impl<'a, M: TableMarker> fmt::Debug for CheckedId<'a, M> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}[{:?}]", M::NAME, self.id.0)
    }
}
pub unsafe trait Check: Copy + Ord + fmt::Debug {
    type M: TableMarker;
    unsafe fn check_from_len<'a>(
        self,
        table: PhantomData<&'a Self::M>,
        max: usize,
    ) -> CheckedId<'a, Self::M> {
        // unsafe because you mustn't lie about `max`.
        let i = self.to_usize();
        if i >= max {
            panic!("OOB");
        }
        CheckedId {
            table,
            id: Id::from_usize(i),
        }
    }
    fn uncheck(&self) -> Id<Self::M> {
        Id(self.to_raw())
    }
    unsafe fn step(self, d: i8) -> Self;
    fn to_usize(&self) -> usize;
    unsafe fn from_usize(i: usize) -> Self;
    fn to_raw(&self) -> <Self::M as TableMarker>::RawId;
}
unsafe impl<'a, M: TableMarker> Check for CheckedId<'a, M> {
    type M = M;
    #[inline]
    fn to_usize(&self) -> usize {
        self.id.to_usize()
    }
    #[inline]
    unsafe fn from_usize(i: usize) -> Self {
        CheckedId {
            table: PhantomData,
            id: Id::from_usize(i),
        }
    }
    #[inline]
    unsafe fn step(self, d: i8) -> Self {
        CheckedId {
            id: self.id.step(d),
            ..self
        }
    }
    #[cfg(release)]
    #[inline]
    fn check<'b>(&self, _id_list: &'b IdList<Self::M>) -> CheckedId<'b, Self::M> {
        *self
    }
    #[cfg(release)]
    #[inline]
    unsafe fn check_from_len(
        &self,
        _table: PhantomData<&'a Self::M>,
        _max: usize,
    ) -> CheckedId<'a, Self::M> {
        *self
    }
    #[inline]
    fn to_raw(&self) -> <Self::M as TableMarker>::RawId { self.id.0 }
}
unsafe impl<'a, M: TableMarker> Check for Id<M> {
    type M = M;
    #[inline]
    fn uncheck(&self) -> Id<M> {
        *self
    }
    #[inline]
    fn to_usize(&self) -> usize {
        self.0.to_usize()
    }
    #[inline]
    unsafe fn from_usize(i: usize) -> Self {
        Id::from_usize(i)
    }
    #[inline]
    unsafe fn step(self, d: i8) -> Self {
        Id(self.0.offset(d))
    }
    #[inline]
    fn to_raw(&self) -> <Self::M as TableMarker>::RawId { self.0 }
}
impl<'a, M: TableMarker> Into<Id<M>> for CheckedId<'a, M> {
    fn into(self) -> Id<M> {
        self.id
    }
}
impl<M: TableMarker> From<usize> for Id<M> {
    #[inline]
    fn from(i: usize) -> Self {
        Id(M::RawId::from_usize(i))
    }
}

/// This is an exclusive range, just like `std::ops::Range`.
#[derive(Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(bound = "I: serde::Serialize + serde::de::DeserializeOwned")]
pub struct IdRange<'a, I: Check> {
    #[serde(skip)]
    pub(crate) _a: PhantomData<&'a ()>,
    pub start: I,
    pub end: I,
}
impl<'a, I: Check> fmt::Debug for IdRange<'a, I> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}[{}..{}]",
            I::M::NAME,
            self.start.to_usize(),
            self.end.to_usize()
        )
    }
}
impl<'a, I: Check> IdRange<'a, I> {
    pub fn step(&mut self) -> Option<I> {
        unsafe {
            if self.start >= self.end {
                return None;
            }
            let ret = self.start;
            self.start = self.start.step(1);
            Some(ret)
        }
    }
    pub fn contains<'b, O>(&self, i: O) -> bool
    where
        O: 'b + Check<M=I::M>,
    {
        let start = self.start.to_raw();
        let end = self.end.to_raw();
        let i = i.to_raw();
        start <= i && i < end
    }
    pub fn len(&self) -> usize {
        let start = self.start.to_raw().to_usize();
        let end = self.end.to_raw().to_usize();
        end - start
    }
    pub fn is_empty(&self) -> bool { self.start == self.end }
    pub fn offset(&self, i: usize) -> Option<I> {
        unsafe {
            if i >= self.len() {
                None
            } else {
                Some(I::from_usize(self.start.to_usize() + i))
            }
        }
    }
    pub fn inner_index<'b, O>(&self, i: O) -> Option<<I::M as TableMarker>::RawId>
    where
    O: 'b + Check<M=I::M>,
    {
        if self.contains(i) {
            Some(i.to_raw() - self.start.to_raw())
        } else {
            None
        }
    }
}
impl<M: TableMarker> IdRange<'static, Id<M>> {
    pub fn new(start: Id<M>, end: Id<M>) -> Self {
        IdRange {
            _a: PhantomData,
            start,
            end,
        }
    }
    pub fn on(id: Id<M>) -> Self {
        IdRange {
            _a: PhantomData,
            start: id,
            end: id.next(),
        }
    }
    pub fn to(end: Id<M>) -> Self {
        IdRange {
            _a: PhantomData,
            start: Id::from_usize(0),
            end,
        }
    }
    pub fn empty() -> Self {
        IdRange {
            _a: PhantomData,
            start: Id::from_usize(0),
            end: Id::from_usize(0),
        }
    }
    /// If you want to iterate over a checked `IdRange`, use `table.ids().range()`.
    pub fn iter(self) -> IdRangeIter<'static, Id<M>> {
        IdRangeIter {
            range: self,
            _a: PhantomData,
        }
    }
}
pub struct IdRangeIter<'a, I: Check> {
    range: IdRange<'a, I>,
    _a: PhantomData<&'a ()>,
}
impl<'a, I: Check> IntoIterator for IdRange<'a, I> {
    type Item = I;
    type IntoIter = IdRangeIter<'a, I>;
    fn into_iter(self) -> Self::IntoIter {
        IdRangeIter {
            range: self,
            _a: PhantomData,
        }
    }
}
impl<'a, I: Check> Iterator for IdRangeIter<'a, I>
where
    I: Clone,
{
    type Item = I;
    fn next(&mut self) -> Option<I> {
        unsafe {
            if self.range.start >= self.range.end {
                return None;
            }
            let ret = Some(self.range.start);
            self.range.start = self.range.start.step(1);
            ret
        }
    }
}
pub type UncheckedIdRange<M> = IdRange<'static, Id<M>>;
impl<M: TableMarker> From<Range<Id<M>>> for UncheckedIdRange<M> {
    fn from(r: Range<Id<M>>) -> Self {
        IdRange {
            _a: PhantomData,
            start: r.start,
            end: r.end,
        }
    }
}

#[derive(Default, Debug)]
pub struct IdList<M: TableMarker> {
    pub free: RunList<M>,
    pushing: RunList<M>,
    deleting: SyncRef<RunList<M>>,
    outer_capacity: usize,
    // We only use SyncRef because elsehwere needs a &mut V, but this is unusable.
}
impl<M: TableMarker> IdList<M> {
    #[inline]
    pub fn len(&self) -> usize {
        let free_len = self.free.len();
        if cfg!(test) {
            assert_eq!(free_len, self.free.iter().count());
        }
        self.outer_capacity - free_len
    }
    #[inline]
    pub fn outer_capacity(&self) -> usize { self.outer_capacity }
    #[inline]
    pub unsafe fn set_outer_capacity(&mut self, outer_capacity: usize) { self.outer_capacity = outer_capacity }
    pub fn exists(&self, id: Id<M>) -> bool {
        id.to_usize() < self.outer_capacity && !self.free.iter_runs().any(|run| {
            run.contains(&id)
        })
    }
    pub fn flush(&mut self, universe: &Universe, tracked_events: u8) {
        if !self.pushing.is_empty() {
            if tracked_events & TRACK_PUSH != 0 {
                let ids = mem::replace(&mut self.pushing, RunList::default());
                let mut pushed = Pushed { ids };
                universe.submit_event(&mut pushed);
                mem::swap(&mut pushed.ids, &mut self.pushing);
                if !pushed.ids.is_empty() {
                    panic!("changed during flush");
                }
            }
            self.pushing.clear();
        }
        if !self.deleting.get_mut().is_empty() {
            if tracked_events & TRACK_DELETE != 0 {
                let ids = mem::replace(self.deleting.get_mut(), RunList::default());
                let mut deleted = Deleted { ids };
                universe.submit_event(&mut deleted);
                mem::swap(&mut deleted.ids, self.deleting.get_mut());
                if !deleted.ids.is_empty() {
                    panic!("changed during flush");
                }
            }
            self.write_deletions();
        }
    }
    pub fn write_deletions(&mut self) {
        // This uses timsort, which WP says has special handling for runs.
        // Merging together two sorted runs is the typical case.
        // Theoretically, an unstable sort will be slower in this case,
        // but I haven't tested this.
        let deleting = self.deleting.get_mut();
        deleting.len = 0;
        self.free.data.extend(deleting.data.drain());
        self.free.sort();
        // We could implement this in a better way by merging back-to-front.
        // We'd extend `free` with !0's, merge backwards.
    }
    #[inline]
    pub fn iter(&self) -> CheckedIter<M> {
        unsafe {
            CheckedIter::new(self.outer_capacity, &self.free)
        }
    }
    #[inline]
    pub fn range(&self, range: UncheckedIdRange<M>) -> CheckedIter<M> {
        unsafe {
            assert!(range.start <= range.end);
            CheckedIter::over(range, &self.free)
        }
    }
    #[inline]
    pub fn delete(&mut self, id: Id<M>) {
        self.deleting.get_mut().push(id);
    }
    pub fn delete_extend(&mut self, i: impl Iterator<Item=Id<M>>) {
        self.deleting.get_mut().extend(i);
    }
    pub fn delete_extend_ranges(&mut self, i: impl Iterator<Item=RangeInclusive<Id<M>>>) {
        let deleting = self.deleting.get_mut();
        deleting.data.reserve(i.size_hint().0);
        for run in i {
            deleting.push_run(run);
        }
    }
    #[inline]
    pub fn removing(&mut self) -> ListRemoving<'static, M> {
        // NB: This is unsound. This is done intentionally.
        // It makes usage nicer. Don't do weird things.
        // See `removing2()` for the sound version. It's a pain.
        // The crux of the issue is:
        //    Checkable
        //          Wants the Id and the Column to share a lifetime
        //    vs RmId
        //          Stores a reference to self.free
        //    vs impl IndexMut for Column
        //          Entangles the lifetime of the indexed value with &mut self
        // FIXME: Could we sneak an abort in somehow?
        unsafe {
            ListRemoving {
                checked: {
                    // Passing in self.len
                    CheckedIter::new(self.outer_capacity, mem::transmute(&self.free))
                },
                deleting: mem::transmute(&self.deleting),
            }
        }
        // (Also it'd be nice to just transmute from removing2(),
        // but I get some BS error about varying size.)
        // What if we did
        //    fn removing<'a>(&mut self, &'a ()) -> ListRemoving<'a, M>
        // Maybe we could pass in &mut true, and it set it to false?
    }
    #[doc(hidden)]
    pub fn removing2<'a, 'b>(&'a mut self) -> ListRemoving<'b, M>
    where
        'a: 'b,
    {
        ListRemoving {
            checked: unsafe {
                // Passing in self.len
                CheckedIter::new(self.outer_capacity, &self.free)
            },
            deleting: &self.deleting,
        }
    }
    /// Creates a new Id, or returns a previously deleted Id.
    /// This function is unsafe because it does not push anything to the column's Vecs.
    pub unsafe fn recycle_id(&mut self) -> Result<Id<M>, Id<M>> {
        if let Some(id) = self.free.pop() {
            self.pushing.push(id);
            Ok(id)
        } else {
            let id = Id::from_usize(self.outer_capacity);
            self.outer_capacity += 1;
            self.pushing.push(id);
            Err(id)
        }
    }
    /// Note: This method is `O(self.free.data.len())`
    pub unsafe fn recycle_id_contiguous(&mut self, n: usize) -> Result<UncheckedIdRange<M>, UncheckedIdRange<M>> {
        if n == 0 {
            return Err(UncheckedIdRange::empty());
        }
        if n == 1 {
            // Special handling required because a (b, a) case would be skipped.
            let id = self.recycle_id();
            return id.map(UncheckedIdRange::on).map_err(UncheckedIdRange::on);
        }
        let mut remove = None;
        let mut ret = None;
        for (i, (a, b)) in self.free.data.iter_mut().enumerate().rev(/* remove from end */) {
            // FIXME: This isn't a very good algorithm, especially if you want to push more than
            // one range. Rather than doing weird heuristicy stuff, we should change the API to
            // take a Vec of ranges, sort by large-to-small, and get it done in O(n).
            // But for now, let's just hope you don't actually end up here!
            if a >= b { continue; }
            let d = b.to_usize() - a.to_usize();
            match d.cmp(&n) {
                Ordering::Less => continue,
                Ordering::Equal => {
                    remove = Some(i);
                    ret = Some(IdRange::new(*a, *b));
                    break;
                },
                Ordering::Greater => {
                    let a2 = Id::from_usize(a.to_usize() + n);
                    ret = Some(IdRange::new(*a, a2));
                    *a = a2;
                },
            }
        }
        if let Some(i) = remove {
            self.free.data.remove(i);
        }
        if let Some(ret) = ret {
            return Ok(ret);
        }
        let a = self.outer_capacity;
        self.outer_capacity += n;
        let b = self.outer_capacity;
        let a = Id::from_usize(a);
        let b = Id::from_usize(b);
        self.pushing.push_run(a..=b.step(-1));
        Err(IdRange::new(a, b))
    }
    /// The next Id that will be used for the next call to push. Be aware that calling this
    /// multiple times will return the same ID.
    pub fn next(&self) -> Id<M> {
        // Idea: What if there wasa  'future IDs' iterator?
        self.free.last()
            .unwrap_or_else(|| {
                Id::from_usize(self.outer_capacity)
            })
    }
    pub fn check<'a, 'b>(&'a self, i: impl Check<M=M> + 'b) -> CheckedId<'a, M> {
        unsafe {
            i.check_from_len(
                PhantomData::<&'a M>,
                self.outer_capacity,
            )
        }
    }
    pub fn erase_events(&mut self) {
        self.pushing.clear();
        self.deleting.get_mut().clear();
    }
}
impl<'a, M: TableMarker> IntoIterator for &'a IdList<M> {
    type Item = CheckedId<'a, M>;
    type IntoIter = CheckedIter<'a, M>;
    fn into_iter(self) -> Self::IntoIter { self.iter() }
}
pub struct ListRemoving<'a, M: TableMarker> {
    checked: CheckedIter<'a, M>,
    deleting: &'a SyncRef<RunList<M>>,
    // FIXME: Make it &mut Vec :(
}
impl<'a, M: TableMarker> Iterator for ListRemoving<'a, M> {
    type Item = RmId<'a, M>;
    fn next(&mut self) -> Option<Self::Item> {
        self.checked.next()
            .map(|id| {
                RmId {
                    id: id.uncheck(),
                    deleting: unsafe { self.deleting.as_cell_unsafe() },
                }
            })
    }
}
unsafe impl<'a, M: TableMarker> ExtractOwned for &'a IdList<M> {
    type Ty = IdList<M>;
    const ACC: Access = Access::Read;
    unsafe fn extract(_universe: &Universe, rez: &mut Rez) -> Self {
        let got: &dyn Any = rez.take_ref();
        got.downcast_ref().unwrap()
    }
}
unsafe impl<'a, M: TableMarker> Extract for &'a mut IdList<M> {
    fn each_resource(f: &mut dyn FnMut(TypeId, Access)) {
        f(TypeId::of::<IdList<M>>(), Access::Write)
    }
    type Owned = Self;
    unsafe fn extract(_universe: &Universe, rez: &mut Rez) -> Self::Owned {
        rez.take_mut_downcast()
    }
    unsafe fn convert(_universe: &Universe, owned: *mut Self::Owned) -> Self {
        &mut *owned
    }
    type Cleanup = IdListCleanup;
}
pub const TRACK_PUSH: u8 = 1;
pub const TRACK_DELETE: u8 = 2;
#[doc(hidden)]
pub struct IdListCleanup {
    tracked_events: u8,
    nothing: bool,
}
unsafe impl<'a, M: TableMarker> Cleaner<&'a mut IdList<M>> for IdListCleanup {
    fn pre_cleanup(owned: &'a mut IdList<M>, universe: &Universe) -> Self {
        let tracked_events = {
            let p = universe.has::<Tracker<Pushed<M>>>();
            let d = universe.has::<Tracker<Deleted<M>>>();
            (if p { TRACK_PUSH } else { 0 }) | (if d { TRACK_DELETE } else { 0 })
        };
        let nothing = owned.pushing.is_empty() && owned.deleting.get_mut().is_empty();
        IdListCleanup { tracked_events, nothing }
    }
    fn post_cleanup(self, universe: &Universe) {
        if self.nothing { return; }
        // FIXME: this needs to happen without any other thread having the opportunity to acquire
        // locks. We could have a bit of state on 'verse that says "you can only release locks",
        // and we can set it in the cleanup() closure, and temporarily release it here.
        // Otherwise there is a legitimate risk that another thread will snatch something we've
        // locked before we're done cleaning up.
        // FIXME: In the meanwhile, we could assert that `pushing` & `deleting` are empty?
        // Would a "reentrant lock" help here?
        // Possibly the problem is that any arbitrary dang thing can have a dependence hanging off
        // of the event being processed. We can't even look ahead! And it could be very recursive!
        universe.with_mut(|owned: &mut IdList<M>| {
            owned.flush(universe, self.tracked_events);
        });
    }
}

/// An `Id` with a method for removing the row.
#[derive(Copy, Clone)]
pub struct RmId<'a, M: TableMarker> {
    id: Id<M>,
    deleting: &'a RefCell<RunList<M>>,
}
impl<'a, M: TableMarker> fmt::Debug for RmId<'a, M> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.id)
    }
}
impl<'a, M: TableMarker> PartialEq for RmId<'a, M> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl<'a, M: TableMarker> Eq for RmId<'a, M> {}
impl<'a, M: TableMarker> PartialOrd for RmId<'a, M> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.id.partial_cmp(&other.id)
    }
}
impl<'a, M: TableMarker> Ord for RmId<'a, M> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}
impl<'a, M: TableMarker> RmId<'a, M> {
    pub fn remove(self) {
        self.deleting.borrow_mut().push(self.id);
    }
}
unsafe impl<'a, M: TableMarker> Check for RmId<'a, M> {
    type M = M;
    fn to_usize(&self) -> usize {
        self.id.to_usize()
    }
    unsafe fn from_usize(_i: usize) -> Self { unimplemented!() }
    fn to_raw(&self) -> <Self::M as TableMarker>::RawId { self.id.0 }
    unsafe fn step(self, d: i8) -> Self {
        RmId {
            id: self.id.step(d),
            ..self
        }
    }
}

pub struct CheckedIter<'a, M: TableMarker> {
    range: UncheckedIdRange<M>,
    free: Peekable<RunListIter<'a, M>>,
}
impl<'a, M: TableMarker> CheckedIter<'a, M> {
    pub unsafe fn new(outer_capacity: usize, free: &'a RunList<M>) -> Self {
        CheckedIter {
            range: IdRange::to(Id::from_usize(outer_capacity)),
            free: free.iter().peekable(),
        }
    }
    pub unsafe fn over(range: UncheckedIdRange<M>, free: &'a RunList<M>) -> Self {
        CheckedIter {
            range,
            free: free.iter().peekable(),
        }
    }
}
impl<'a, M: TableMarker> Iterator for CheckedIter<'a, M> {
    type Item = CheckedId<'a, M>;
    fn next(&mut self) -> Option<Self::Item> {
        // FIXME: range=1 billion to 1 billion+1, exclude=0 to 1 billion
        while let Some(id) = self.range.step() {
            loop {
                match self.free.peek().map(|e| id.cmp(e)).unwrap_or(Ordering::Less) {
                    Ordering::Equal => break,
                    Ordering::Greater => self.free.next(),
                    Ordering::Less => return Some(CheckedId { table: PhantomData, id }),
                };
            }
        }
        None
    }
    // FIXME: impl a size_hint
}

/// Stores `Id`s with great efficiency.
/// Runs are stored like a `Range`.
/// (In the case of a single run, zero allocation is needed.)
/// Non-contiguous `Id`s have the same memory overhead as a `Vec`.
///
/// If you are iterating over the rows in a table,
/// it's easiest to use the table's `Read`, `Write`, or `Edit` `decl_context!`.
/// Otherwise you will need to take `&$table::Id` or `&mut $table::Id` as an argument to the
/// `Kernel`.
#[derive(Clone, Default)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct RunList<M: TableMarker> {
    len: usize,
    data: smallvec::SmallVec<[(Id<M>, Id<M>); 2]>,
    // FIXME: is_sorted: bool,
}
impl<M: TableMarker> fmt::Debug for RunList<M> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[")?;
        let mut first = true;
        for (a, b) in &self.data {
            if first {
                first = false;
            } else {
                write!(f, ", ")?;
            }
            match a.cmp(&b) {
                Ordering::Less => write!(f, "{:?}..={:?}", a, b),
                Ordering::Equal => write!(f, "{:?}", a),
                Ordering::Greater => write!(f, "{:?}, {:?}", b, a),
            }?;
        }
        write!(f, "]")
    }
}
impl<M: TableMarker> RunList<M> {
    pub fn new() -> Self { Self::default() }
    #[inline(always)]
    fn validate(&self) {
        if cfg!(test) {
            let actual = self.iter().count();
            if actual != self.len {
                panic!("bad RunList.\nlen={}\ndata={:?}\nraw={:?}", self.len, self, self.data);
            }
        }
    }
    #[inline]
    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.iter().next().is_none() }
    pub fn push(&mut self, i: Id<M>) {
        self.validate();
        self.len += 1;
        let last = M::RawId::LAST;
        // (a < b) --> a..=b
        // (a = b) --> [a]
        // (a > b) --> [b, a]
        let new = if let Some(old) = self.data.pop() {
            let (a, b) = old;
            //if i == a || i == b { return; }
            match a.cmp(&b) {
                Ordering::Less => {
                    if b.0 != last && b.step(1) == i {
                        (a, i)
                    } else if i.0 != last && i.step(1) == a {
                        (i, b)
                    } else {
                        self.data.push(old);
                        (i, i)
                    }
                }
                Ordering::Equal => {
                    if a.0 != last && a.step(1) == i {
                        (a, i)
                    } else if i.0 != last && i.step(1) == a {
                        (i, a)
                    } else if i > a {
                        (i, a)
                    } else if a == i {
                        self.len -= 1;
                        (a, a)
                    } else {
                        (a, i)
                    }
                }
                Ordering::Greater => {
                    // Two separate, [b, a].
                    let s = {
                        // known: a > b
                        // so output starts [b, a]. But where does i go?
                        if i < b {
                            [i, b, a]
                        } else if i < a {
                            [b, i, a]
                        } else {
                            [b, a, i]
                        }
                    };
                    if cfg!(test) {
                        let mut g = [a, b, i];
                        g.sort();
                        assert_eq!(s, g);
                    }
                    // All these items are separate.
                    // OR ARE THEY? 0,1 might merge. And 1,2 might merge. And 0,1,2 might merge
                    // also.
                    // Another issue: If we have [8, 8, 14], then self.len is wrong.
                    if s[0].0 != last && s[0].step(1) == s[1] {
                        if s[1].0 != last && s[1].step(1) == s[2] {
                            // Filled in a hole.
                            (s[0], s[2])
                        } else {
                            self.data.push((s[0], s[1]));
                            (s[2], s[2])
                        }
                    } else if s[1].0 != last && s[1].step(1) == s[2] {
                        self.data.push((s[0], s[0]));
                        (s[1], s[2])
                    } else if s[0] == s[1] || s[1] == s[2] {
                        // Nothing happened.
                        self.len -= 1;
                        (s[2], s[0])
                    } else {
                        self.data.push((s[1], s[0]));
                        (s[2], s[2])
                    }
                }
            }
        } else {
            (i, i)
        };
        self.data.push(new);
        self.validate();
    }
    pub fn push_run(&mut self, r: RangeInclusive<Id<M>>) {
        self.validate();
        // FIXME: if r.is_empty() { return; }
        let l = *r.start();
        let h = *r.end();
        assert!(h >= l);
        self.len += 1 + h.to_usize() - l.to_usize();
        if let Some((a, b)) = self.data.last_mut() {
            // We're just gonna assume that r is not inclusive with any existing run.
            // So we handle [0..=5], [6..=9], but not [0..=5], [5..=9] or [0..=5], [0..=9].
            if a <= b && b.step(1) == l {
                *b = h;
                return;
            }
            // A possibility:
            // if a > b && a+1 == l:
            //     [b], a..=h
            // No immediate benefit to this atm.
        }
        self.data.push((l, h));
        self.validate();
    }
    pub fn pop(&mut self) -> Option<Id<M>> {
        self.validate();
        let r = if let Some((a, b)) = self.data.pop() {
            self.len -= 1;
            Some(match a.cmp(&b) {
                Ordering::Greater => {
                    self.data.push((b, b));
                    a
                },
                Ordering::Equal => a,
                Ordering::Less => {
                    self.data.push((a, b.step(-1)));
                    b
                },
            })
        } else {
            None
        };
        self.validate();
        r
    }
    pub fn last(&self) -> Option<Id<M>> {
        if let Some((a, b)) = self.data.last().cloned() {
            Some(match a.cmp(&b) {
                Ordering::Greater => a,
                Ordering::Equal => a,
                Ordering::Less => b,
            })
        } else {
            None
        }
    }
    pub fn clear(&mut self) {
        self.validate();
        self.len = 0;
        self.data.clear();
        self.validate();
    }
    pub fn iter(&self) -> RunListIter<M> {
        self.into_iter()
    }
    pub fn extend_from(&mut self, new: Self) {
        self.data.extend(new.data);
        self.len += new.len;
        self.validate();
    }
    pub fn extend(&mut self, iter: impl Iterator<Item=Id<M>>) {
        for id in iter {
            self.push(id);
        }
        self.validate();
    }
    pub fn iter_runs<'a>(&'a self) -> impl Iterator<Item=RangeInclusive<Id<M>>> + 'a {
        struct Iter<'a, M: TableMarker> {
            buff: Option<RangeInclusive<Id<M>>>,
            data: &'a [Run<M>],
        }
        impl<'a, M: TableMarker> Iterator for Iter<'a, M> {
            type Item = RangeInclusive<Id<M>>;

            fn next(&mut self) -> Option<Self::Item> {
                if self.buff.is_some() {
                    self.buff.take()
                } else if let Some(head) = self.data.first() {
                    self.data = &self.data[1..];
                    Some(if head.1 < head.0 {
                        self.buff = Some(head.0..=head.0);
                        head.1..=head.1
                    } else {
                        head.0..=head.1
                    })
                } else {
                    None
                }
            }
        }
        Iter {
            buff: None,
            data: self.data.as_slice()
        }
    }
    pub fn sort(&mut self) {
        // FIXME: This could be more efficient, but that is, WOW, that's complicated.
        let mut runs = Vec::with_capacity(self.data.len() * 2);
        runs.extend(self.iter_runs());
        runs.sort_by_key(|run| *run.start());
        self.data.clear();
        self.data.reserve(runs.len());
        self.len = 0;
        for run in runs.into_iter() {
            self.push_run(run);
        }
        self.validate();
    }
    // FIXME: fn compact(&mut self);
    // FIXME: fn merge(&mut self, other: &Self);
}
impl<'a, M: TableMarker> IntoIterator for &'a RunList<M> {
    type Item = Id<M>;
    type IntoIter = RunListIter<'a, M>;
    fn into_iter(self) -> Self::IntoIter {
        RunListIter {
            buffer: None,
            data: &self.data[..],
        }
    }
}
pub struct RunListIter<'a, M: TableMarker> {
    buffer: Option<(Id<M>, Id<M>)>,
    data: &'a [(Id<M>, Id<M>)],
}
impl<'a, M: TableMarker> Iterator for RunListIter<'a, M> {
    type Item = Id<M>;
    // FIXME: impl size_hint
    fn next(&mut self) -> Option<Self::Item> {
        let buff = if let Some(buff) = self.buffer.take() {
            buff
        } else if let Some((&x, xs)) = self.data.split_first() {
            self.data = xs;
            x
        } else {
            return None;
        };
        let (a, b) = buff;
        // (a < b) --> a..=b
        // (a = b) --> [a]
        // (a > b) --> [b, a]
        match a.cmp(&b) {
            Ordering::Less => {
                let buff = (a.step(1), b);
                if buff.0 <= buff.1 {
                    self.buffer = Some(buff);
                }
                Some(a)
            }
            Ordering::Equal => {
                self.buffer = None;
                Some(a)
            }
            Ordering::Greater => {
                self.buffer = Some((a, a));
                Some(b)
            }
        }
    }
}

#[cfg(test)]
mod test_run_list {
    use super::*;
    use std::collections::*;
    #[derive(Debug, Copy, Clone, Default)]
    struct M;
    impl TableMarker for M {
        const NAME: Name = "M";
        type RawId = u8;
        fn header() -> TableHeader {
            unimplemented!()
        }
    }
    impl Register for M {
        fn register(_universe: &mut Universe) {
            unimplemented!()
        }
    }
    type I = Id<M>;
    #[derive(Debug, Clone, Default)]
    struct Checker {
        slow: Vec<I>,
        fast: RunList<M>,
        seen_slow: HashMap<I, usize>,
        seen_fast: HashMap<I, usize>,
    }
    impl Checker {
        fn push(&mut self, i: I) {
            self.seen_slow.clear();
            self.seen_fast.clear();
            self.slow.push(i);
            self.fast.push(i);
            println!("{:?}", self.fast.data);
            // assert_eq!(self.slow.iter().count(), self.fast.iter().count());
            for f in self.fast.iter() {
                println!("    {}", f.0);
                *self.seen_fast.entry(f).or_insert(0) += 1;
            }
            for &s in self.slow.iter() {
                *self.seen_slow.entry(s).or_insert(0) += 1;
            }
            for (f, &nf) in &self.seen_fast {
                let ns = self.seen_slow[f];
                assert!(nf <= ns);
            }
        }
    }
    fn check(x: impl Iterator<Item = u8>) -> usize {
        let mut c = Checker::default();
        for x in x {
            println!("{}", x);
            c.push(Id(x));
        }
        c.fast.data.len()
    }
    fn checks(x: &[u8]) {
        check(x.iter().map(|&x| x));
    }
    #[test]
    fn test() {
        checks(&[]);
        checks(&[1]);
        checks(&[0, 1]);
        checks(&[0, 1, 2, 3, 4, 5]);
        check((0..4).chain(10..20));
        check((0..20).skip(1));
        check((0..20).skip(2));
        check((1..20).skip(2));
        check((1..20).skip(1));
        checks(&[0, 1, 3, 4, 6]);
    }
    #[test]
    fn efficient_when_backwards() {
        let got_len = check((0..20).rev());
        assert_eq!(got_len, 1);
    }
    #[test]
    fn on_iter_is_some() {
        let r = UncheckedIdRange::<M>::on(Id::<M>::from_usize(3));
        assert_eq!(1, r.iter().count());
    }

    #[test]
    fn unordered_ranges() {
        let mut l = RunList::<M>::default();
        for i in 20..30 {
            l.push(Id(i));
        }
        for i in 0..10 {
            l.push(Id(i));
        }
    }

    #[test]
    fn random_insertions() {
        let some_random_fucking_numbers_jesus_christ_how_fucking_hard_is_this_gonna_have_to_be_fuck_off_for_fucks_sake_fuckity_fuckity_fuck_fuck_fuck_you = &[
16, 3, 16, 1, 17, 18, 11, 10, 1, 17, 13, 15, 9, 12, 16, 2, 10, 8, 14, 19, 9, 2, 10, 3, 14, 6, 9, 9, 19, 3, 11, 6, 4, 7, 2, 20, 7, 15, 20, 1, 9, 4, 14, 8, 1, 18, 0, 17, 8, 15,
13, 4, 13, 3, 20, 10, 16, 12, 8, 12, 10, 6, 0, 10, 17, 18, 19, 4, 14, 2, 10, 13, 3, 20, 4, 14, 2, 8, 8, 3, 5, 17, 8, 0, 0, 11, 7, 6, 3, 0, 0, 20, 8, 4, 20, 15, 19, 2, 13, 0,
1, 3, 18, 12, 0, 16, 16, 14, 4, 0, 18, 19, 8, 6, 12, 16, 14, 14, 5, 1, 14, 4, 13, 2, 4, 4, 0, 19, 7, 13, 0, 1, 18, 15, 14, 0, 0, 17, 0, 16, 15, 19, 14, 17, 15, 1, 2, 12, 9, 19,
12, 20, 15, 2, 6, 9, 14, 3, 18, 20, 9, 15, 15, 13, 5, 9, 7, 9, 19, 19, 9, 17, 5, 5, 14, 15, 20, 0, 19, 4, 12, 1, 2, 18, 12, 10, 11, 16, 11, 13, 8, 8, 19, 4, 5, 13, 17, 4, 13, 4,
15, 17, 1, 15, 11, 15, 18, 12, 8, 15, 8, 15, 18, 20, 1, 3, 4, 18, 10, 4, 4, 18, 11, 15, 13, 2, 13, 4, 3, 4, 7, 9, 18, 7, 11, 11, 8, 11, 16, 11, 4, 11, 15, 7, 8, 15, 8, 14, 3, 19,
12, 3, 1, 14, 8, 18, 19, 20, 17, 19, 4, 1, 16, 2, 6, 13, 3, 10, 1, 4, 9, 19, 1, 3, 9, 5, 13, 7, 2, 16, 3, 6, 3, 2, 6, 11, 8, 7, 2, 9, 4, 11, 0, 11, 19, 17, 16, 17, 13, 10,
7, 0, 9, 18, 11, 2, 13, 20, 7, 12, 14, 1, 20, 16, 11, 15, 6, 3, 5, 6, 12, 16, 16, 0, 16, 8, 5, 10, 15, 2, 18, 2, 11, 12, 20, 4, 17, 14, 3, 20, 9, 20, 7, 6, 16, 5, 2, 1, 3, 17,
12, 1, 10, 8, 14, 8, 17, 7, 20, 17, 19, 7, 20, 15, 13, 7, 2, 8, 16, 15, 8, 20, 19, 4, 11, 9, 15, 2, 7, 18, 18, 9, 9, 8, 10, 13, 6, 9, 7, 5, 16, 20, 1, 17, 16, 8, 4, 17, 18, 13,
18, 5, 11, 19, 6, 11, 7, 9, 1, 7, 0, 4, 13, 0, 14, 17, 14, 2, 5, 8, 2, 18, 7, 1, 12, 12, 6, 5, 19, 12, 17, 5, 2, 20, 0, 10, 15, 0, 8, 12, 2, 7, 11, 2, 4, 6, 11, 0, 2, 3,
1, 2, 11, 20, 12, 13, 14, 6, 1, 6, 1, 6, 11, 11, 19, 13, 13, 6, 9, 11, 16, 7, 5, 2, 6, 5, 11, 9, 18, 9, 18, 13, 6, 8, 18, 15, 6, 3, 20, 7, 20, 13, 15, 17, 11, 0, 15, 16, 14, 17,
16, 14, 13, 7, 2, 7, 0, 14, 20, 0, 9, 5, 19, 6, 18, 10, 5, 13, 0, 14, 19, 1, 1, 17, 18, 15, 15, 4, 6, 11, 6, 13, 13, 3, 12, 2, 14, 20, 17, 8, 9, 20, 13, 1, 19, 4, 8, 8, 4, 7,
9, 14, 13, 12, 10, 7, 4, 0, 6, 15, 13, 2, 17, 4, 9, 15, 3, 13, 13, 5, 16, 17, 5, 13, 20, 20, 16, 13, 13, 20, 7, 0, 17, 9, 4, 14, 14, 14, 19, 6, 18, 4, 14, 3, 18, 10, 1, 12, 4, 5,
6, 18, 20, 7, 8, 17, 17, 11, 1, 18, 9, 3, 0, 0, 8, 12, 0, 6, 8, 11, 14, 16, 13, 10, 8, 3, 7, 9, 0, 10, 13, 20, 13, 13, 10, 6, 1, 4, 6, 10, 3, 14, 9, 16, 8, 18, 17, 17, 6, 5,
18, 18, 13, 19, 15, 18, 5, 0, 18, 8, 16, 12, 18, 16, 11, 12, 18, 5, 12, 10, 14, 14, 4, 19, 6, 13, 5, 10, 13, 0, 6, 13, 0, 16, 18, 1, 20, 8, 13, 9, 9, 19, 4, 19, 12, 19, 17, 3, 13, 14,
2, 18, 19, 12, 11, 17, 9, 17, 5, 2, 6, 13, 15, 10, 20, 19, 12, 16, 18, 8, 1, 9, 14, 18, 15, 5, 11, 10, 15, 9, 13, 7, 8, 2, 9, 2, 11, 0, 3, 18, 9, 13, 13, 18, 14, 7, 2, 11, 2, 3,
16, 1, 0, 13, 10, 17, 6, 2, 19, 8, 14, 11, 3, 2, 18, 18, 14, 1, 10, 20, 18, 7, 13, 13, 11, 8, 20, 15, 16, 5, 17, 12, 15, 17, 17, 10, 8, 14, 7, 14, 14, 6, 10, 19, 11, 9, 10, 5, 15, 5,
1, 0, 0, 15, 4, 7, 12, 18, 1, 7, 9, 10, 18, 5, 16, 0, 8, 3, 18, 6, 10, 11, 4, 8, 12, 4, 20, 7, 5, 16, 2, 20, 17, 8, 12, 17, 15, 15, 14, 18, 13, 8, 8, 20, 10, 10, 1, 0, 12, 15,
2, 14, 5, 14, 14, 18, 8, 7, 19, 20, 3, 2, 12, 18, 14, 6, 6, 5, 3, 17, 17, 11, 1, 6, 16, 19, 1, 9, 12, 1, 11, 15, 0, 16, 19, 10, 19, 2, 20, 19, 2, 2, 11, 14, 9, 2, 16, 7, 5, 13,
2, 10, 16, 16, 18, 8, 2, 10, 5, 20, 12, 10, 12, 10, 2, 18, 10, 2, 9, 1, 4, 10, 14, 4, 11, 17, 17, 18, 1, 7, 20, 19, 3, 1, 5, 16, 4, 12, 16, 15, 5, 16, 6, 8, 7, 20, 11, 3, 14, 2,
7, 10, 17, 2, 8, 15, 10, 19, 2, 8, 15, 9, 3, 14, 16, 9, 12, 15, 0, 7, 4, 18, 17, 14, 11, 20, 13, 17, 2, 9, 12, 18, 17, 11, 19, 13, 17, 20, 10, 4, 19, 11, 14, 6, 5, 3, 4, 15, 0, 14,
        ];
        let mut rng = some_random_fucking_numbers_jesus_christ_how_fucking_hard_is_this_gonna_have_to_be_fuck_off_for_fucks_sake_fuckity_fuckity_fuck_fuck_fuck_you.iter().cycle().cloned();
        for _ in 0..200 {
            let n = rng.next().unwrap();
            let test = (0..n).map(|_| rng.next().unwrap());
            check(test);
        }
    }

    #[test]
    fn random_found() {
        check([3, 16, 1].iter().cloned());
    }

    #[test]
    fn short_range() {
        let mut l = RunList::<M>::default();
        l.push_run(Id(0)..=Id(0));
        let mut it = l.iter();
        assert_eq!(it.next(), Some(Id(0)));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn id_list() {
        unsafe {
            for x in 1..5 {
                for y in 1..x {
                    let mut l = IdList::<M>::default();
                    let u = &Universe::new();
                    l.flush(u, 0);
                    fn r<R>(r: Result<R, R>) -> R {
                        match r {
                            Ok(r) => r,
                            Err(r) => r,
                        }
                    }
                    let mut pushed = vec![];
                    for _ in 0..x {
                        let id = r(l.recycle_id());
                        pushed.push(id);
                        l.len();
                    }
                    l.len();
                    l.flush(u, 0);
                    for _ in 0..y {
                        if let Some(id) = pushed.pop() {
                            l.delete(id);
                            l.len();
                        }
                    }
                    l.flush(u, 0);
                    l.len();
                }
            }
        }
    }

    #[test]
    fn thing() {
        let mut l = RunList::<M>::default();
        l.push(Id(8));
        l.push(Id(14));
        l.push(Id(8));
    }

    #[test]
    fn dude() {
        unsafe {
            let mut l = IdList::<M>::default();
            let u = &Universe::new();
            l.flush(u, 0);
            fn r<R>(r: Result<R, R>) -> R {
                match r {
                    Ok(r) => r,
                    Err(r) => r,
                }
            }
            println!("\npush");
            let a = r(l.recycle_id());
            { l.len(); l.flush(u, 0); l.len(); }

            println!("\ndelete");
            l.delete(a);
            { l.len(); l.flush(u, 0); l.len(); }

            println!("\nresurect");
            let a2 = r(l.recycle_id());
            { l.len(); l.flush(u, 0); l.len(); }
            assert_eq!(a, a2);

            println!("\ndelete");
            l.delete(a2);
            { l.len(); l.flush(u, 0); l.len(); }
        }
    }

    #[test]
    fn dude2() {
        let mut l = RunList::<M>::default();
        l.push(Id(0));
        l.pop();
        l.push(Id(0));
        l.pop();
    }
}
