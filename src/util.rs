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

pub struct MutButRef<'a, T>(&'a mut T);
impl<'a, T> MutButRef<'a, T> {
    pub fn new(t: &'a mut T) -> Self {
        Self(t)
    }
    pub unsafe fn get_mut(&mut self) -> &mut T {
        self.0
    }
}
impl<'a, T> Deref for MutButRef<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.0
    }
}
