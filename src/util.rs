use std::cell::RefCell;
use std::ops::Deref;
use crate::prelude_lib::RunList;
use crate::table::TableMarker;

/// A `Sync`able `RefCell`.
#[derive(Default, Debug, Clone)]
pub struct SyncRef<T: TableMarker> {
    val: RefCell<RunList<T>>,
}
impl<T: TableMarker> SyncRef<T> {
    pub fn new(val: RunList<T>) -> Self {
        SyncRef {
            val: RefCell::new(val),
        }
    }
    pub fn get_mut(&mut self) -> &mut RunList<T> {
        self.val.get_mut()
    }
    pub fn as_cell(&mut self) -> &RefCell<RunList<T>> {
        &self.val
    }
    pub unsafe fn as_cell_unsafe(&self) -> &RefCell<RunList<T>> {
        &self.val
    }
}
// Trying to impl Deref/DerefMut provokes odd curiosities.
unsafe impl<T: TableMarker> Send for SyncRef<T> {}
unsafe impl<T: TableMarker> Sync for SyncRef<T> {}
// FIXME: Ugh, this is probably unsound.

/// ```compile_fail
/// use std::cell::Cell;
/// use v9::util::SyncRef;
///
/// fn main() {
///     let sync_ref = SyncRef::new(Cell::new(0));
///     fn check<T: Send + Sync>(_: T) {}
///     check(sync_ref);
/// }
/// ```
#[cfg(doctest)]
struct SyncRefSyncless;

/// A `&mut T` that pretends it's a `&T`.
pub struct MutButRef<'a, T>(&'a mut T);
impl<'a, T> MutButRef<'a, T> {
    pub fn new(t: &'a mut T) -> Self {
        Self(t)
    }
    pub unsafe fn get_mut(&mut self) -> &mut T {
        // Obviously nothing unsafe is happening here,
        // but users of MutButRef require the guarantee.
        self.0
    }
}
impl<'a, T> Deref for MutButRef<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.0
    }
}


pub mod die {
    pub static BAD_ITER_LEN: &str = "Iterator must know its exact Id length";
}
