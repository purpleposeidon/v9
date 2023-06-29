//! Ids, lists of Ids, and various iterators.

use crate::event::*;
use crate::prelude_lib::*;
use std::fmt;
use std::ops::{Range, RangeInclusive};
use std::hash;
use std::cmp::Ordering;

use crate::event::lifestage;

mod serializers {
    macro_rules! for_set {
        ($atr:meta, $($set:tt)*) => {
            #[cfg($atr)]
            pub trait Serializable: $($set)* {}
            #[cfg($atr)]
            impl<X: $($set)*> Serializable for X {}
        };
    }
    for_set!(all(    feature = "serde" ,     feature = "bincode" ), serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode);
    for_set!(all(not(feature = "serde"),     feature = "bincode" ), bincode::Encode + bincode::Decode);
    for_set!(all(    feature = "serde" , not(feature = "bincode")), serde::Serialize + serde::de::DeserializeOwned);
    for_set!(all(not(feature = "serde"), not(feature = "bincode")), );
}


pub trait Raw: self::serializers::Serializable + runlist::Id + self::raw_impl::Sealed {
    const ZERO: Self = <Self as runlist::Id>::ZERO;
    const ONE: Self = <Self as runlist::Id>::ONE;
    const TWO: Self = <Self as runlist::Id>::TWO;
    const LAST: Self = <Self as runlist::Id>::MAX;
    fn to_usize(self) -> usize {
        <Self as runlist::Id>::to_usize(self)
    }
    /// Panics if out of range.
    fn from_usize(i: usize) -> Self {
        <Self as runlist::Id>::from_usize(i)
    }
    fn offset(self, d: i8) -> Self {
        <Self as runlist::Id>::offset(self, d)
    }
}
impl<X: self::serializers::Serializable + runlist::Id + self::raw_impl::Sealed> Raw for X {}

mod raw_impl {
    /// Forbid non-primitives from being put into a SyncRef.
    pub trait Sealed {}
    impl Sealed for u8 {}
    impl Sealed for u16 {}
    impl Sealed for u32 {}
    impl Sealed for u64 {}
    // u128? Absurd.
}

/// A strongly typed row id.
#[derive(Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(bound = "M: 'static"))]
#[cfg_attr(feature = "serde", serde(transparent))]
// #[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[repr(transparent)]
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
    unsafe fn check_from_capacity<'a>(
        &self,
        table: PhantomData<&'a Self::M>,
        max: usize,
    ) -> CheckedId<'a, Self::M> {
        // unsafe because you mustn't lie about `max`.
        let i = self.to_usize();
        if i >= max {
            oob(i, max);
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
    unsafe fn step(mut self, d: i8) -> Self {
        self.id = self.id.step(d);
        self
    }
    #[cfg(release)]
    #[inline]
    fn check<'b>(&self, _id_list: &'b IdList<Self::M>) -> CheckedId<'b, Self::M> {
        *self
    }
    #[cfg(release)]
    #[inline]
    unsafe fn check_from_capacity(
        &self,
        _table: PhantomData<&'a Self::M>,
        _max: usize,
    ) -> CheckedId<'a, Self::M> {
        *self
    }
    #[inline]
    fn to_raw(&self) -> <Self::M as TableMarker>::RawId { self.id.0 }
}
unsafe impl<M: TableMarker> Check for Id<M> {
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
impl<'a, M: TableMarker> From<CheckedId<'a, M>> for Id<M> {
    fn from(v: CheckedId<'a, M>) -> Self {
        v.id
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(bound = "I: serde::Serialize + serde::de::DeserializeOwned"))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
pub struct IdRange<'a, I: Check> {
    // We don't use TableMarker so that we can be flexible.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub(crate) _a: PhantomData<&'a ()>,
    pub start: I,
    pub end: I,
}
impl<'a, M: TableMarker> Default for IdRange<'a, Id<M>> {
    fn default() -> Self {
        IdRange {
            _a: PhantomData,
            start: Id::from_usize(0),
            end: Id::from_usize(0),
        }
    }
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
impl<'a, I: Check> Into<Range<I>> for IdRange<'a, I> {
    fn into(self) -> Range<I> {
        self.start .. self.end
    }
}
impl<'a, I: Check> Into<RangeInclusive<I>> for IdRange<'a, I> {
    fn into(self) -> RangeInclusive<I> {
        assert!(!self.is_empty());
        self.start ..= unsafe { self.end.step(-1) }
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
#[derive(Clone)]
pub struct IdRangeIter<'a, I: Check> {
    range: IdRange<'a, I>,
    _a: PhantomData<&'a ()>,
}
impl<'a, I: Copy + Check> IntoIterator for IdRange<'a, I> {
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
impl<M: TableMarker> From<RangeInclusive<Id<M>>> for UncheckedIdRange<M> {
    fn from(r: RangeInclusive<Id<M>>) -> Self {
        IdRange {
            _a: PhantomData,
            start: *r.start(),
            end: r.end().step(1),
        }
    }
}

#[derive(Default, Debug, Clone)]
#[repr(C)]
pub struct IdList<M: TableMarker> {
    inner: runlist::IdList<M::RawId>,
    event_commitment: EventCommitment,
    load_events: bool,
}
impl<M: TableMarker> IdList<M> {
    pub fn validate(&self) { self.inner.assert().unwrap(); }
    #[inline] pub fn len(&self) -> usize { self.inner.len() }
    #[inline] pub fn is_empty(&self) -> bool { self.inner.is_empty() }
    #[inline] pub fn outer_capacity(&self) -> usize { M::RawId::to_usize(self.inner.outer_capacity()) }
    #[inline] pub fn exists(&self, id: Id<M>) -> bool { self.inner.exists(id.0) }
    pub fn flush(&mut self, universe: &Universe) {
        if let EventCommitment::None = self.event_commitment { return; }
        self.event_commitment = EventCommitment::None;
        let (track_push, track_delete) = (
            {
                false
                    || universe.is_tracked::<Push<M, lifestage::MEMORY>>()
                    || universe.is_tracked::<Push<M, lifestage::LOGICAL>>()
                    || universe.is_tracked::<Push<M, lifestage::LOAD>>()
            }, {
                false
                    || universe.is_tracked::<Delete<M, lifestage::MEMORY>>()
                    || universe.is_tracked::<Delete<M, lifestage::LOGICAL>>()
                    || universe.is_tracked::<Delete<M, lifestage::LOAD>>()
            }
        );
        use runlist::FlushResult;
        match self.inner.flush(track_push, track_delete) {
            FlushResult::Nothing => (),
            FlushResult::Pushed(ids) => if !ids.is_empty() {
                let ids = RunList::<M> { inner: ids };
                let mut event = Push { lifestage: unsafe { Unsafe::new(lifestage::MEMORY) }, ids };
                universe.submit_event(&mut event);
                let ids = event.ids;
                if self.load_events {
                    self.load_events = false;
                    let mut event = Push { lifestage: unsafe { Unsafe::new(lifestage::LOAD) }, ids };
                    universe.submit_event(&mut event);
                } else {
                    let mut event = Push { lifestage: unsafe { Unsafe::new(lifestage::LOGICAL) }, ids };
                    universe.submit_event(&mut event);
                }
            },
            FlushResult::Deleted(ids) => if !ids.is_empty() {
                let ids = RunList::<M> { inner: ids };
                let ids = if self.load_events {
                    self.load_events = false;
                    let mut event = Delete { lifestage: unsafe { Unsafe::new(lifestage::LOAD) }, ids };
                    universe.submit_event(&mut event);
                    event.ids
                } else {
                    let mut event = Delete { lifestage: unsafe { Unsafe::new(lifestage::LOGICAL) }, ids };
                    universe.submit_event(&mut event);
                    event.ids
                };
                let mut event = Delete { lifestage: unsafe { Unsafe::new(lifestage::MEMORY) }, ids };
                universe.submit_event(&mut event);
            },
        }
    }
    #[inline]
    pub fn iter(&self) -> CheckedIter<M> {
        CheckedIter {
            inner: self.inner.iter_singles(),
        }
    }
    #[inline]
    pub fn delete(&mut self, id: Id<M>) {
        self.event_commitment.put(EventCommitment::Delete { event: true });
        self.inner.delete(id.0);
    }
    pub fn delete_extend(&mut self, i: impl Iterator<Item=Id<M>> + Clone) {
        self.event_commitment.put(EventCommitment::Delete { event: true });
        self.inner.delete_ids(i.map(|i| {
            let i = i.to_raw();
            i..=i
        }));
    }
    pub fn delete_extend_ranges(&mut self, i: impl Iterator<Item=RangeInclusive<Id<M>>> + Clone) {
        self.event_commitment.put(EventCommitment::Delete { event: true });
        self.inner.delete_ids(i.map(|i| {
            i.start().to_raw()..=i.end().to_raw()
        }));
    }
    pub fn removing<'this, 'iter>(&'this mut self) -> ListRemoving<'iter, M>
    where
        'this: 'iter,
    {
        self.event_commitment.half_commit(false);
        // We need to return a self-borrowing iterator, lol? Uh-oh.
        let (iter, deleter) = self.inner.iter_singles_deleting();
        ListRemoving {
            _m: PhantomData,
            iter,
            deleter,
            event_commitment: &mut self.event_commitment as *mut _,
        }
    }
    /// Creates a new Id, or returns a previously deleted Id.
    ///
    /// # Safety
    /// This function is unsafe because it does not push anything to the tables's column vectors.
    pub unsafe fn recycle_id_no_event(&mut self) -> Result<Id<M>, Id<M>> {
        self.event_commitment.put(EventCommitment::Push { event: false });
        match self.inner.recycle_id() {
            Ok(id) => Ok(Id(id)),
            Err(id) => Err(Id(id)),
        }
    }
    /// Returns a list of IDs in an arbitrary order.
    /// # Safety
    /// This function is unsafe because it does not push anything to the tables's column vectors.
    pub unsafe fn recycle_ids_no_event(&mut self, n: usize) -> Recycle<M> {
        self.event_commitment.put(EventCommitment::Push { event: false });
        let n = M::RawId::from_usize(n);
        let recycle = self.inner.recycle_ids_sparse(n);
        Recycle {
            replace: RunList { inner: recycle.replace },
            extend: M::RawId::to_usize(recycle.extend),
            extension: IdRange {
                _a: PhantomData,
                start: Id(recycle.extension.start),
                end: Id(recycle.extension.end),
            },
        }
    }
    /// Note: This method is `O(self.free.data.len())`
    /// # Safety
    /// This function is unsafe because it does not push anything to the tables's column vectors.
    pub unsafe fn recycle_ids_contiguous_no_event(&mut self, n: usize) -> Recycle<M> {
        self.event_commitment.put(EventCommitment::Push { event: false });
        let n = M::RawId::from_usize(n);
        let recycle = self.inner.recycle_ids_contiguous(n);
        Recycle {
            replace: RunList { inner: recycle.replace },
            extend: M::RawId::to_usize(recycle.extend),
            extension: IdRange {
                _a: PhantomData,
                start: Id(recycle.extension.start),
                end: Id(recycle.extension.end),
            },
        }
    }
    pub fn check<'a, 'b>(&'a self, i: impl Check<M=M> + 'b) -> CheckedId<'a, M> {
        unsafe {
            i.check_from_capacity(
                PhantomData::<&'a M>,
                self.outer_capacity(),
            )
        }
    }
}
impl<'a, M: TableMarker> IntoIterator for &'a IdList<M> {
    type Item = CheckedId<'a, M>;
    type IntoIter = CheckedIter<'a, M>;
    fn into_iter(self) -> Self::IntoIter { self.iter() }
}
pub struct ListRemoving<'a, M: TableMarker> {
    _m: PhantomData<&'a mut IdList<M>>,
    iter: runlist::IterIdsSingles<'a, M::RawId>,
    deleter: runlist::Deleter<'a, M::RawId>,
    event_commitment: *mut EventCommitment,
}
impl<'a, M: TableMarker> Iterator for ListRemoving<'a, M> {
    type Item = RmId<'a, M>;
    fn next(&mut self) -> Option<Self::Item> {
        let deleter = &mut self.deleter as *mut _;
        self.iter.next().map(move |id| RmId {
            id: Id(id),
            deleter,
            event_commitment: self.event_commitment,
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum EventCommitment {
    None,
    Push { event: bool },
    Delete { event: bool },
}
impl Default for EventCommitment { fn default() -> Self { EventCommitment::None } }
impl EventCommitment {
    pub fn half_commit(&self, push: bool) {
        match (push, self) {
            (_, EventCommitment::None) => (),
            (true, EventCommitment::Push { .. }) => (),
            (false, EventCommitment::Push { .. }) => (),
            _ => panic!("half-commit failed. Already {:?}, want push = {:?}", self, push),
        }
    }
    pub fn put(&mut self, new: EventCommitment) {
        if let EventCommitment::None = self {
            assert!(new != EventCommitment::None);
            *self = new;
        } else {
            assert!(*self == new, "Can't mix event commitments: existing was {:?}, new is {:?}", self, new);
        }
    }
}

unsafe impl<'a, M: TableMarker> ExtractOwned for &'a IdList<M> {
    type Ty = IdList<M>;
    const ACC: Access = Access::Read;
    unsafe fn extract(_universe: &Universe, rez: &mut Rez) -> Self {
        let got: &dyn AnyDebug = rez.take_ref();
        got.downcast_ref().unwrap()
    }
}
unsafe impl<'a, M: TableMarker> Extract for &'a mut IdList<M> {
    fn each_resource(f: &mut dyn FnMut(Ty, Access)) {
        f(Ty::of::<IdList<M>>(), Access::Write)
    }
    type Owned = Self;
    unsafe fn extract(_universe: &Universe, rez: &mut Rez) -> Self::Owned {
        rez.take_mut_downcast()
    }
    unsafe fn convert(_universe: &Universe, owned: *mut Self::Owned) -> Self {
        *owned
    }
    type Cleanup = IdListCleanup;
}
pub const TRACK_PUSH: u8 = 1;
pub const TRACK_DELETE: u8 = 2;
#[doc(hidden)]
pub struct IdListCleanup;
unsafe impl<'a, M: TableMarker> Cleaner<&'a mut IdList<M>> for IdListCleanup {
    fn pre_cleanup(_owned: &'a mut IdList<M>, _universe: &Universe) -> Self {
        IdListCleanup
    }
    fn post_cleanup(self, universe: &Universe) {
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
            owned.flush(universe);
        });
    }
}

#[derive(Debug)]
#[must_use]
pub struct Recycle<M: TableMarker> {
    pub replace: RunList<M>,
    pub extend: usize,
    pub extension: UncheckedIdRange<M>,
}
impl<M: TableMarker> Recycle<M> {
    pub fn count(&self) -> usize {
        self.extend + self.replace.len()
    }
}

/// An `Id` with a method for removing the row.
pub struct RmId<'a, M: TableMarker> {
    pub id: Id<M>,
    deleter: *mut runlist::Deleter<'a, M::RawId>,
    event_commitment: *mut EventCommitment,
}
impl<'a, M: TableMarker> RmId<'a, M> {
    pub fn id(&self) -> Id<M> {
        self.id
    }
    pub fn remove(self) {
        unsafe { &mut *self.event_commitment }.put(EventCommitment::Delete { event: true });
        let deleter = unsafe { &mut *self.deleter };
        deleter.delete(self.id.to_raw());
    }
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

#[derive(Debug, Clone)]
pub struct CheckedIter<'a, M: TableMarker> {
    // NB: Soundness requires these be private.
    inner: runlist::IterIdsSingles<'a, M::RawId>,
}
impl<'a, M: TableMarker> Iterator for CheckedIter<'a, M> {
    type Item = CheckedId<'a, M>;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|id| CheckedId {
            table: PhantomData,
            id: Id(id),
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

/// Stores `Id`s with great efficiency. Runs are stored like a `RangeInclusive`. (In the case of a
/// single run, zero allocation is needed.) Non-contiguous `Id`s have the same memory overhead as a
/// `Vec`.
///
/// If you are iterating over the rows in a table, it's easiest to use the `$table::Read`, `Write`,
/// or `Edit` contexts. Otherwise you will need to take `&$table::Ids` or `&mut $table::Ids`
/// as an argument to the `Kernel`.
#[derive(Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RunList<M: TableMarker> {
    inner: runlist::RunList<M::RawId>,
}
impl<M: TableMarker> fmt::Debug for RunList<M> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{:?}]", self.inner)
    }
}
impl<M: TableMarker, X: Into<runlist::Run<M::RawId>>> From<X> for RunList<M> {
    fn from(run: X) -> RunList<M> {
        RunList {
            inner: runlist::RunList::on(run),
        }
    }
}
impl<M: TableMarker + Check> From<UncheckedIdRange<M>> for RunList<M> {
    fn from(run: UncheckedIdRange<M>) -> Self {
        let mut inner = runlist::RunList::<M::RawId>::default();
        if !run.is_empty() {
            use std::convert::TryInto;
            let run: runlist::Run::<M::RawId> = (run.start.0 .. run.end.0).try_into().unwrap();
            inner.push(run);
        }
        RunList { inner }
    }
}
impl<M: TableMarker> RunList<M> {
    pub fn new() -> Self { Self::default() }
    pub fn on(id: Id<M>) -> Self {
        Self { inner: runlist::RunList::on(id.0) }
    }
    pub fn get_data(&self) -> &[(Id<M>, Id<M>)] {
        let data: &[runlist::Run<M::RawId>] = self.inner.data();
        unsafe { std::mem::transmute(data) }
    }
    pub fn from_raw_data(len: usize, data: Vec<runlist::Run<M::RawId>>) -> Result<Self, String> {
        let inner = runlist::RunList::from_data(data)?;
        let actual = inner.len();
        if actual != len {
            return Err(format!("RunList length not as advertised: actual = {}, given = {}", actual, len));
        }
        Ok(RunList { inner })
    }
    pub fn validate_data(&self) -> Result<(), String> { self.inner.assert() }
    #[inline] pub fn len(&self) -> usize { self.inner.len() }
    #[inline] pub fn is_empty(&self) -> bool { self.inner.is_empty() }
    #[inline] pub fn push(&mut self, i: Id<M>) { self.inner.push(i.0); }
    #[inline] pub fn push_run(&mut self, r: RangeInclusive<Id<M>>) { self.inner.push(r.start().0 ..= r.end().0); }
    #[inline] pub fn pop(&mut self) -> Option<Id<M>> { self.inner.pop_arbitrary().map(Id::<M>) }
    #[inline] pub fn clear(&mut self) { self.inner.clear(); }
    #[inline] pub fn iter(&self) -> RunListIterSingles<M> { RunListIterSingles(self.inner.iter_singles()) }
    #[inline] pub fn contains(&self, id: Id<M>) -> bool { self.inner.contains(id.to_raw()) }
    #[inline] pub fn iter_runs(&self) -> RunListIterRanges<M> { RunListIterRanges(self.inner.iter_ranges()) }
    pub fn extend(&mut self, iter: impl Iterator<Item=Id<M>>) {
        // Reserve isn't possible.
        for id in iter {
            self.inner.push(id.to_raw());
        }
    }
    // FIXME: fn merge(&mut self, other: &Self);
}
// FIXME: Ugh! IntoIterator for RunList. Do I want it? I actually don't use RunList directly very often...
impl<'a, M: TableMarker> IntoIterator for &'a RunList<M> {
    type Item = Id<M>;
    type IntoIter = RunListIterSingles<'a, M>;
    fn into_iter(self) -> Self::IntoIter { self.iter() }
}
#[derive(Debug)]
pub struct RunListIterSingles<'a, M: TableMarker>(runlist::IterSingles<'a, M::RawId>);
impl<'a, M: TableMarker> Clone for RunListIterSingles<'a, M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<'a, M: TableMarker> Iterator for RunListIterSingles<'a, M> {
    type Item = Id<M>;
    #[inline] fn next(&mut self) -> Option<Self::Item> { self.0.next().map(Id::<M>::new) }
    #[inline] fn size_hint(&self) -> (usize, Option<usize>) { self.0.size_hint() }
}
#[derive(Debug, Clone)]
pub struct RunListIterRanges<'a, M: TableMarker>(runlist::IterRanges<'a, M::RawId>);
impl<'a, M: TableMarker> Iterator for RunListIterRanges<'a, M> {
    type Item = IdRange<'static, Id<M>>;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|run: RangeInclusive<M::RawId>| -> IdRange<'static, Id<M>> {
                IdRange {
                    _a: PhantomData,
                    start: Id::new(*run.start()),
                    end: Id::new(*run.end()).step(1),
                }
            })
    }
    #[inline] fn size_hint(&self) -> (usize, Option<usize>) { self.0.size_hint() }
}


#[cfg(feature = "bincode")]
mod bincode_impls {
    use super::{Id, RunList, TableMarker};
    use bincode::enc::{Encoder, Encode};
    use bincode::de::{Decoder, Decode};
    use bincode::error::{EncodeError, DecodeError};
    impl<M: TableMarker> Encode for Id<M> {
        fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
            self.0.encode(encoder)
        }
    }
    impl<M: TableMarker> Decode for Id<M> {
        fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
            Ok(Id(<M::RawId as Decode>::decode(decoder)?))
        }
    }
    impl<M: TableMarker> Encode for RunList<M> {
        fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
            self.inner.len().encode(encoder)?;
            let pairs = self.inner.data().len();
            pairs.encode(encoder)?;
            for pair in self.inner.data() {
                pair.data().encode(encoder)?;
            }
            Ok(())
        }
    }
    impl<M: TableMarker> Decode for RunList<M> {
        fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
            let _len = usize::decode(decoder)?;
            let pairs: usize = usize::decode(decoder)?;
            type Data<M> = [<M as TableMarker>::RawId; 2];
            let mut data = Vec::<runlist::Run<M::RawId>>::with_capacity(pairs);
            for _ in 0..pairs {
                let run = Data::<M>::decode(decoder)?;
                let run = runlist::Run::<M::RawId>::from_data(run);
                data.push(run);
            }
            match runlist::RunList::from_data(data) {
                Ok(inner) => Ok(RunList { inner }),
                Err(e) => Err(DecodeError::OtherString(e)),
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
            println!("{:?}", self.fast);
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
        c.fast.len()
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
    #[should_panic]
    fn backwards() {
        let got_len = check((0..20).rev());
        assert_eq!(got_len, 1);
    }
    #[test]
    fn on_iter_is_some() {
        let r = UncheckedIdRange::<M>::on(Id::<M>::from_usize(3));
        assert_eq!(1, r.iter().count());
    }

    #[test]
    #[should_panic]
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
                    l.flush(u);
                    fn r<R>(r: Result<R, R>) -> R {
                        match r {
                            Ok(r) => r,
                            Err(r) => r,
                        }
                    }
                    let mut pushed = vec![];
                    for _ in 0..x {
                        let id = r(l.recycle_id_no_event());
                        pushed.push(id);
                        l.len();
                    }
                    l.len();
                    l.flush(u);
                    for _ in 0..y {
                        if let Some(id) = pushed.pop() {
                            l.delete(id);
                            l.len();
                        }
                    }
                    l.flush(u);
                    l.len();
                }
            }
        }
    }

    #[test]
    fn runlist_ordered() {
        let mut l = RunList::<M>::default();
        l.push(Id(8));
        l.push(Id(14));
        l.push(Id(17));
    }
    #[test]
    #[should_panic]
    fn runlist_duplicates() {
        let mut l = RunList::<M>::default();
        l.push(Id(8));
        l.push(Id(14));
        l.push(Id(8));
    }
    #[test]
    #[should_panic]
    fn runlist_disordered() {
        let mut l = RunList::<M>::default();
        l.push(Id(7));
        l.push(Id(8));
        l.push(Id(9));
        l.push(Id(3));
    }


    #[test]
    fn dude1() {
        unsafe {
            let mut l = IdList::<M>::default();
            let u = &Universe::new();
            l.flush(u);
            fn r<R>(r: Result<R, R>) -> R {
                match r {
                    Ok(r) => r,
                    Err(r) => r,
                }
            }
            println!("\npush");
            let a = r(l.recycle_id_no_event());
            { l.len(); l.flush(u); l.len(); }

            println!("{:?}", l);
            println!("\ndelete 1");
            l.delete(a);
            println!("{:?}", l);
            { l.len(); l.flush(u); l.len(); }

            println!("\nresurect");
            let a2 = r(l.recycle_id_no_event());
            { l.len(); l.flush(u); l.len(); }
            assert_eq!(a, a2);

            println!("\ndelete 2");
            l.delete(a2);
            { l.len(); l.flush(u); l.len(); }
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

#[cold]
fn oob(i: usize, max: usize) -> ! {
    panic!("OOB: i:{} >= max:{}", i, max)
}
