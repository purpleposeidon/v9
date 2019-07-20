//! Ids, lists of Ids, and various iterators.

use crate::event::*;
use crate::prelude_lib::*;
use std::cell::RefCell;
use std::fmt;
use std::ops::{Range, RangeInclusive};
use std::iter::Peekable;
use std::hash;

type Run<M> = (Id<M>, Id<M>);

pub trait Raw: 'static + Copy + fmt::Debug + Ord + Send + Sync + hash::Hash + serde::Serialize + serde::de::DeserializeOwned {
    fn to_usize(self) -> usize;
    fn from_usize(x: usize) -> Self;
    fn offset(self, d: i8) -> Self;
    const ZERO: Self;
    const LAST: Self;
}
mod raw_impl {
    use super::Raw;
    macro_rules! imp {
        ($($ty:ident),*) => {$(
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
pub unsafe trait Check<'a>: Copy + Ord + fmt::Debug {
    type M: TableMarker;
    fn check(&self, id_list: &'a IdList<Self::M>) -> CheckedId<'a, Self::M> {
        unsafe {
            self.check_from_len(
                PhantomData::<&'a Self::M>,
                id_list.len,
            )
        }
    }
    unsafe fn check_from_len(
        &self,
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
    fn to_raw(&self) -> <Self::M as TableMarker>::RawId;
}
unsafe impl<'a, M: TableMarker> Check<'a> for CheckedId<'a, M> {
    type M = M;
    fn to_usize(&self) -> usize {
        self.id.to_usize()
    }
    unsafe fn step(self, d: i8) -> Self {
        CheckedId {
            id: self.id.step(d),
            ..self
        }
    }
    #[cfg(release)]
    fn check(&self, _id_list: &'a IdList<Self::M>) -> CheckedId<'a, Self::M> {
        *self
    }
    #[cfg(release)]
    unsafe fn check_from_len(
        &self,
        _table: PhantomData<&'a Self::M>,
        _max: usize,
    ) -> CheckedId<'a, Self::M> {
        *self
    }
    fn to_raw(&self) -> <Self::M as TableMarker>::RawId { self.id.0 }
}
unsafe impl<'a, M: TableMarker> Check<'a> for Id<M> {
    type M = M;
    fn uncheck(&self) -> Id<M> {
        *self
    }
    fn to_usize(&self) -> usize {
        self.0.to_usize()
    }
    unsafe fn step(self, d: i8) -> Self {
        Id(self.0.offset(d))
    }
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

#[derive(Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(bound = "I: serde::Serialize + serde::de::DeserializeOwned")]
pub struct IdRange<'a, I: Check<'a>> {
    #[serde(skip)]
    pub(crate) _a: PhantomData<&'a ()>,
    pub start: I,
    pub end: I,
}
impl<'a, I: Check<'a>> fmt::Debug for IdRange<'a, I> {
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
impl<'a, I: Check<'a>> IdRange<'a, I> {
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
        O: Check<'b, M=I::M>,
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
}
impl<M: TableMarker> IdRange<'static, Id<M>> {
    pub fn on(start: Id<M>, end: Id<M>) -> Self {
        IdRange {
            _a: PhantomData,
            start,
            end,
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
}
pub struct IdRangeIter<'a, I: Check<'a>> {
    range: IdRange<'a, I>,
    _a: PhantomData<&'a ()>,
}
impl<'a, I: Check<'a>> IntoIterator for IdRange<'a, I> {
    type Item = I;
    type IntoIter = IdRangeIter<'a, I>;
    fn into_iter(self) -> Self::IntoIter {
        IdRangeIter {
            range: self,
            _a: PhantomData,
        }
    }
}
impl<'a, I: Check<'a>> Iterator for IdRangeIter<'a, I>
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

#[derive(Default)]
pub struct IdList<M: TableMarker> {
    pub free: RunList<M>,
    pub deleting: SyncRef<RunList<M>>,
    pub needed: bool,
    len: usize,
    // We only use SyncRef because elsehwere needs a &mut V, but this is unusable.
}
impl<M: TableMarker> Obj for IdList<M> {}
impl<M: TableMarker> Drop for IdList<M> {
    fn drop(&mut self) {
        if !self.deleting.get_mut().is_empty() {
            panic!("unflushed IdList");
        }
    }
}
impl<M: TableMarker> IdList<M> {
    pub fn len(&self) -> usize { self.len }
    pub fn flush(&mut self, universe: &Universe) {
        let ids = mem::replace(self.deleting.get_mut(), RunList::default());
        let mut deleted = Deleted { ids };
        universe.submit_event(&mut deleted);
        let tmp = self.deleting.get_mut();
        mem::swap(&mut deleted.ids, tmp);
        assert!(deleted.ids.is_empty(), "additional deletions occured during deletion flush");
    }
    pub fn write_deletions(&mut self) {
        // This uses timsort, which WP says has special handling for runs.
        // Merging together two sorted runs is the typical case.
        // Theoretically, an unstable sort will be slower in this case,
        // but I haven't tested this.
        self.free.data.extend(self.deleting.get_mut().data.drain());
        self.free.sort();
        // We could implement this in a better way by merging back-to-front.
        // We'd extend `free` with !0's, merge backwards.
    }
    pub fn iter(&self) -> CheckedIter<M> {
        unsafe {
            CheckedIter::new(self.len, &self.free)
        }
    }
    pub fn delete(&mut self, id: Id<M>) {
        let deleting = self.deleting.get_mut();
        deleting.push(id);
    }
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
        unsafe {
            ListRemoving {
                checked: {
                    // Passing in self.len
                    CheckedIter::new(self.len, mem::transmute(&self.free))
                },
                deleting: mem::transmute(&self.deleting),
            }
        }
        // (Also it'd be nice to just transmute from removing2(),
        // but I get some BS error about varying size.)
    }
    #[doc(hidden)]
    pub fn removing2<'a, 'b>(&'a mut self) -> ListRemoving<'b, M>
    where
        'a: 'b,
    {
        ListRemoving {
            checked: unsafe {
                // Passing in self.len
                CheckedIter::new(self.len, &self.free)
            },
            deleting: &self.deleting,
        }
    }
    pub unsafe fn recycle_id(&mut self) -> Result<Id<M>, Id<M>> {
        if let Some(id) = self.free.pop() {
            Ok(id)
        } else {
            let i = self.len;
            self.len += 1;
            Err(Id::from_usize(i))
        }
    }
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
        let got: &Obj = rez.take_ref();
        got.downcast_ref().unwrap()
    }
}
unsafe impl<'a, M: TableMarker> Extract for &'a mut IdList<M> {
    fn each_resource(f: &mut dyn FnMut(TypeId, Access)) {
        f(TypeId::of::<IdList<M>>(), Access::Write)
    }
    type Owned = Self;
    unsafe fn extract(universe: &Universe, rez: &mut Rez) -> Self::Owned {
        let me: Self = rez.take_mut_downcast();
        me.needed = universe.has::<Tracker<Deleted<M>>>();
        me
    }
    unsafe fn convert(_universe: &Universe, owned: *mut Self::Owned) -> Self {
        &mut *owned
    }
    type Cleanup = IdListCleanup;
}
#[doc(hidden)]
pub struct IdListCleanup {
    anything: bool,
}
unsafe impl<'a, M: TableMarker> Cleaner<&'a mut IdList<M>> for IdListCleanup {
    fn pre_cleanup(owned: <&'a mut IdList<M> as Extract>::Owned, _universe: &Universe) -> Self {
        IdListCleanup {
            anything: owned.needed | !owned.deleting.get_mut().is_empty(),
        }
    }
    fn post_cleanup(self, universe: &Universe) {
        if !self.anything { return; }
        // FIXME: this needs to happen without any other thread having the opportunity to acquire
        // locks. We could have a bit of state on 'verse that says "you can only release locks",
        // and we can set it in the cleanup() closure, and temporarily release it here.
        universe.with_mut(|owned: &mut IdList<M>| {
            if owned.needed {
                owned.flush(universe);
            }
            owned.write_deletions();
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
unsafe impl<'a, M: TableMarker> Check<'a> for RmId<'a, M> {
    type M = M;
    fn to_usize(&self) -> usize {
        self.id.to_usize()
    }
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
    pub unsafe fn new(len: usize, free: &'a RunList<M>) -> Self {
        CheckedIter {
            range: IdRange::to(Id::from_usize(len)),
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
}

/// Stores `Id`s with great efficiency.
/// Runs are stored like a `Range`.
/// (In the case of a single run, zero allocation is needed.)
/// Non-contiguous `Id`s have the same memory overhead as a `Vec`.
/// However, the `Id`s must be pushed in order.
///
/// If you are iterating over the rows in a table,
/// it's easiest to use the table's `Read`, `Write`, or `Edit` `decl_context!`.
/// Otherwise you will need to take `&$table::Id` or `&mut $table::Id` as an argument to the
/// `Kernel`.
#[derive(Clone, Default)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct RunList<M: TableMarker> {
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
    pub fn is_empty(&self) -> bool { self.iter().next().is_none() }
    pub fn push(&mut self, id: Id<M>) {
        // (a < b) --> a..=b
        // (a = b) --> [a]
        // (a > b) --> [b, a]
        let new = if let Some(old) = self.data.pop() {
            let (a, b) = old;
            assert!(
                a < id && b < id,
                "RunList entries must be in increasing order"
            );
            // We could possibly relax the ordering requirement.
            // In this case we should provide `defrag()` method.
            match a.cmp(&b) {
                Ordering::Less => {
                    if b.step(1) == id {
                        (a, id)
                    } else {
                        self.data.push(old);
                        (id, id)
                    }
                }
                Ordering::Equal => {
                    if b.step(1) == id {
                        (a, id)
                    } else {
                        (id, a)
                    }
                }
                Ordering::Greater => {
                    self.data.push(old);
                    (id, id)
                }
            }
        } else {
            (id, id)
        };
        self.data.push(new);
    }
    pub fn push_run(&mut self, l: Id<M>, h: Id<M>) {
        if let Some((a, b)) = self.data.last_mut() {
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
    }
    pub fn pop(&mut self) -> Option<Id<M>> {
        if let Some((a, b)) = self.data.pop() {
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
        }
    }
    pub fn clear(&mut self) { self.data.clear() }
    pub fn iter(&self) -> RunListIter<M> {
        self.into_iter()
    }
    pub fn extend_from(&mut self, new: Self) {
        self.data.reserve(new.data.len());
        self.data.extend(new.data);
    }
    pub fn extend(&mut self, iter: impl Iterator<Item=Id<M>>) {
        for id in iter {
            self.push(id);
        }
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
        for run in runs.into_iter() {
            if run.start() == run.end() {
                self.push(*run.start());
            } else {
                self.push_run(*run.start(), *run.end());
            }
        }
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
    fn next(&mut self) -> Option<Self::Item> {
        let buff = if let Some(buff) = self.buffer.take() {
            buff
        } else if let Some((&x, xs)) = self.data.split_first() {
            self.buffer = Some(x);
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
                self.buffer = Some((a.step(1), b));
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
    }
    impl Checker {
        fn push(&mut self, i: I) {
            self.slow.push(i);
            self.fast.push(i);
            assert_eq!(self.slow.iter().count(), self.fast.iter().count());
            for (s, f) in self.slow.iter().zip(self.fast.iter()) {
                assert_eq!(*s, f);
            }
        }
    }
    fn check(x: impl Iterator<Item = u8>) -> usize {
        let mut c = Checker::default();
        for x in x {
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
    #[should_panic]
    fn doubled() {
        checks(&[1, 2, 4, 4, 5, 6]);
    }
    #[test]
    #[should_panic]
    fn bad_order() {
        checks(&[1, 0]);
    }
    #[test]
    #[should_panic]
    fn bad_order2() {
        checks(&[1, 2, 0]);
    }
    // #[test]
    // fn efficient_when_backwards() {
    //     let got_len = check((0..20).rev());
    //     assert_eq!(got_len, 1);
    // }
}
