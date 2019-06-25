use std::cell::RefCell;
use std::ops::Deref;

/// A `Sync`able `RefCell`.
#[derive(Default, Debug, Clone)]
pub struct SyncRef<T> {
    val: RefCell<T>,
}
impl<T> SyncRef<T> {
    pub fn new(val: T) -> Self {
        SyncRef {
            val: RefCell::new(val),
        }
    }
    pub fn get_mut(&mut self) -> &mut T {
        self.val.get_mut()
    }
    pub fn as_cell(&mut self) -> &RefCell<T> {
        &self.val
    }
    pub unsafe fn as_cell_unsafe(&self) -> &RefCell<T> {
        &self.val
    }
}
// Trying to impl Deref/DerefMut provokes odd curiosities.
unsafe impl<T: Send> Send for SyncRef<T> {}
unsafe impl<T> Sync for SyncRef<T> {}
// FIXME: Ugh, this is probably unsound.

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
