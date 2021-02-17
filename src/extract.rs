//! Extracting values from the `Universe`.
use crate::prelude_lib::*;

#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(u8)]
pub enum Access {
    Read,
    Write,
}

/// A type that can be used as an argument to a `Kernel`.
///
/// Unfortunately the lifetime requirements of this trait can't be expressed in Rust at this
/// time. It is quite simple however; the following sort of pattern needs to be valid:
/// ```no_compile
/// universe.acquire_locks(&mut resources);
/// let mut owned: Self::Owned = Self::extract(universe, &mut resources);
/// // (any other Extraction instance…)
/// {
///     let converted: Self = Self::convert(universe, &mut owned);
///     // (…)
///     kernel_call(converted, …);
/// }
///
/// let cleaner = Self::Cleanup::pre_cleanup(owned, universe);
/// // (…)
/// universe.release_locks(&mut resources);
/// cleaner.post_cleanup(universe);
/// // (…)
/// ```
// I've put crazy amounts of time into trying to get this working w/o unsafe.
// It's impossible. Any lifetime that you can name is valid for the duration of the function,
// and what we need is to name lifetimes for something *shorter* than a function.
// It might work with recursive closures or something, but I suspect it'd be way too nasty.
pub unsafe trait Extract<'a>: Sized {
    /// List the type & access requirement needed to do the extraction.
    /// This function must have constant behavior; it is unsound otherwise.
    fn each_resource(f: &mut dyn FnMut(TypeId, Access));
    type Owned: 'a;
    unsafe fn extract<'u: 'a>(universe: &'u Universe, rez: &mut Rez<'u>) -> Self::Owned;
    unsafe fn convert(universe: &Universe, owned: *mut Self::Owned) -> Self;
    /// Default is `()`, which does nothing.
    type Cleanup: Cleaner<'a, Self>;
}
// FIXME: It'd be nice to have impls of Extract for tuples; up to, say, 5.

pub unsafe trait Cleaner<'a, E: Extract<'a>> {
    fn pre_cleanup(owned: E::Owned, universe: &'a Universe) -> Self;
    fn post_cleanup(self, universe: &'a Universe);
}
unsafe impl<'a, E: Extract<'a>> Cleaner<'a, E> for () {
    fn pre_cleanup(_owned: E::Owned, _universe: &'a Universe) -> Self {}
    fn post_cleanup(self, _universe: &'a Universe) {}
}


/// Helper trait.
pub unsafe trait ExtractOwned<'a> {
    type Ty: Any;
    const ACC: Access;
    unsafe fn extract<'u: 'a>(universe: &'u Universe, rez: &mut Rez<'u>) -> Self;
}
unsafe impl<'a, X> Extract<'a> for X
where
    X: 'a,
    X: ExtractOwned<'a>,
{
    fn each_resource(f: &mut dyn FnMut(TypeId, Access)) {
        f(TypeId::of::<X::Ty>(), X::ACC)
    }
    type Owned = Option<X>;
    unsafe fn extract<'u: 'a>(universe: &'u Universe, rez: &mut Rez<'u>) -> Self::Owned {
        Some(X::extract(universe, rez))
    }
    unsafe fn convert(_universe: &Universe, owned: *mut Self::Owned) -> X {
        (*owned).take().unwrap()
    }
    type Cleanup = ();
}

/// Produces the objects asked for by `Extract`.
#[derive(Debug)]
pub struct Rez<'a> {
    vals: &'a [(*mut dyn Any, Access)],
    // FIXME: We could use Result<&Any, &mut Any>
    // Ooh! enum Mr<T> { }, it'd be great.
}
impl<'a> Rez<'a> {
    /// # Safety
    /// `vals` must not alias, must not outlive `'a`, `Access` must be accurate.
    pub(crate) unsafe fn new(vals: &'a [(*mut dyn Any, Access)]) -> Self {
        Rez { vals }
    }
    pub fn take_ref(&mut self) -> &'a dyn Any {
        let (v, a): (*mut dyn Any, Access) = self.vals[0];
        assert_eq!(a, Access::Read, "asked for Access::Write but used take_ref");
        self.vals = &self.vals[1..];
        unsafe { &mut *v }
    }
    pub fn take_mut(&mut self) -> &'a mut dyn Any {
        let (v, a): (*mut dyn Any, Access) = self.vals[0];
        assert_eq!(a, Access::Write, "asked for Access::Read but used take_mut");
        self.vals = &self.vals[1..];
        unsafe { &mut *v }
    }
    pub fn take_ref_downcast<T: Any>(&mut self) -> &'a T {
        let got: &dyn Any = self.take_ref();
        got.downcast_ref().unwrap()
    }
    pub fn take_mut_downcast<T: Any>(&mut self) -> &'a mut T {
        let got: &mut dyn Any = self.take_mut();
        got.downcast_mut().unwrap()
    }
}
