use crate::event::*;
use crate::prelude_lib::*;
use std::cell::RefCell;
use std::fmt;

pub trait Raw: 'static + Copy + fmt::Debug + Ord + Send + Sync + serde::Serialize + serde::de::DeserializeOwned {
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
                fn to_usize(self) -> usize { self as usize }
                fn from_usize(x: usize) -> Self { x as _ }
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
    pub fn step(self, d: i8) -> Self {
        Id(self.0.offset(d))
    }
}

/// An `Id` that is known to be in-bounds on the given table. You may want to check your ID if you
/// will be working with a lot of columns or indices.
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
        Id::from_usize(self.to_usize())
    }
    unsafe fn step(self, d: i8) -> Self;
    fn to_usize(&self) -> usize;
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

#[derive(Clone, Copy)]
pub struct IdRange<'a, I: Check<'a>> {
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

#[derive(Default)]
pub struct IdList<M: TableMarker> {
    pub free: Vec<Id<M>>,
    // FIXME: It'd be nice to use a RunList.
    pub deleting: SyncRef<Vec<Id<M>>>,
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
        let ids = mem::replace(self.deleting.get_mut(), vec![]);
        let mut deleted = Deleted { ids };
        universe.submit_event(&mut deleted);
        mem::swap(&mut deleted.ids, self.deleting.get_mut());
    }
    pub fn write_deletions(&mut self) {
        // This uses timsort, which WP says has special handling for runs.
        // Merging together two sorted runs is the typical case.
        // Theoretically, an unstable sort will be slower in this case,
        // but I haven't tested this.
        self.free.extend(self.deleting.get_mut().drain(..));
        self.free.sort();
        // We could implement this in a better way by merging back-to-front.
        // We'd extend `free` with !0's, merge backwards.
    }
    pub fn iter(&self) -> CheckedIter<M> {
        unsafe {
            CheckedIter::new(self.len, &self.free[..])
        }
    }
    pub fn delete(&mut self, id: Id<M>) {
        let deleting = self.deleting.get_mut();
        deleting.push(id);
    }
    pub fn removing(&mut self) -> ListRemoving<'static, M> {
        unsafe {
            // FIXME: WHY WHY WHY
            ListRemoving {
                range: IdRange::to(Id::from_usize(self.len)),
                exclude: mem::transmute(&self.free[..]),
                deleting: mem::transmute(&self.deleting),
            }
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
    range: UncheckedIdRange<M>,
    exclude: &'a [Id<M>],
    deleting: &'a SyncRef<Vec<Id<M>>>,
    // FIXME: Make it &mut Vec :(
}
impl<'a, M: TableMarker> Iterator for ListRemoving<'a, M> {
    type Item = RmId<'a, M>;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(id) = self.range.step() {
            while *self.exclude.first().unwrap_or(&id) < id {
                self.exclude = &self.exclude[1..];
            }
            if self.exclude.first().cloned() == Some(id) {
                self.exclude = &self.exclude[1..];
                continue;
            }
            let deleting = unsafe { self.deleting.as_cell_unsafe() };
            return Some(RmId { id, deleting });
        }
        None
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
    fn finish(universe: &Universe, owned: Self::Owned) {
        if owned.needed {
            owned.flush(universe);
        }
        owned.write_deletions();
    }
}

/// An `Id` with a method for removing the row.
#[derive(Copy, Clone)]
pub struct RmId<'a, M: TableMarker> {
    id: Id<M>,
    deleting: &'a RefCell<Vec<Id<M>>>,
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
    unsafe fn step(self, d: i8) -> Self {
        RmId {
            id: self.id.step(d),
            ..self
        }
    }
}

pub struct CheckedIter<'a, M: TableMarker> {
    free: &'a [Id<M>],
    id: Id<M>,
    end: Id<M>,
}
impl<'a, M: TableMarker> CheckedIter<'a, M> {
    pub unsafe fn new(len: usize, free: &'a [Id<M>]) -> Self {
        CheckedIter {
            free,
            id: Id::from_usize(0),
            end: Id::from_usize(len),
        }
    }
}
impl<'a, M: TableMarker> Iterator for CheckedIter<'a, M> {
    type Item = CheckedId<'a, M>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.id.to_usize() >= self.end.to_usize() {
                return None;
            }
            let ret = CheckedId {
                table: PhantomData,
                id: self.id,
            };
            self.id = self.id.step(1);
            let next_free = *self.free.first().unwrap_or(&self.id);
            match ret.id.cmp(&next_free) {
                Ordering::Less => return Some(ret),
                Ordering::Equal => {
                    self.free = &self.free[1..];
                    continue;
                }
                Ordering::Greater => {
                    unimplemented!("CheckedIter handling id greater free list's id")
                }
            }
        }
    }
}

/// Stores `Id`s with great efficiency.
/// Runs are stored like a `Range`.
/// (In the case of a single run, zero allocation is needed.)
/// Non-contiguous `Id`s have the same memory overhead as a `Vec`.
/// However, the `Id`s must be pushed in order.
///
/// If you are iterating over the rows in a table,
/// it's easiest to use the table's `Read`, `Write`, or `Edit` `context!`.
/// Otherwise you will need to take `&$table::Id` or `&mut $table::Id` as an argument to the
/// `Kernel`.
#[derive(Clone, Default)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct RunList<M: TableMarker> {
    data: smallvec::SmallVec<[(Id<M>, Id<M>); 2]>,
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
    pub fn clear(&mut self) { self.data.clear() }
    pub fn iter(&self) -> RunListIter<M> {
        self.into_iter()
    }
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
    fn check(x: impl Iterator<Item = u8>) {
        let mut c = Checker::default();
        for x in x {
            c.push(Id(x));
        }
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
}
