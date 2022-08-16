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
pub unsafe trait Extract: Sized {
    /// List the type & access requirement needed to do the extraction.
    /// This function must have constant behavior; it is unsound otherwise.
    fn each_resource(f: &mut dyn FnMut(Ty, Access));
    type Owned;
    unsafe fn extract(universe: &Universe, rez: &mut Rez) -> Self::Owned;
    unsafe fn convert(universe: &Universe, owned: *mut Self::Owned) -> Self;
    /// Default is `()`, which does nothing.
    type Cleanup: Cleaner<Self>;
}
// FIXME: It'd be nice to have impls of Extract for tuples; up to, say, 5.

pub unsafe trait Cleaner<E: Extract> {
    fn pre_cleanup(owned: E::Owned, universe: &Universe) -> Self;
    fn post_cleanup(self, universe: &Universe);
}
unsafe impl<E: Extract> Cleaner<E> for () {
    fn pre_cleanup(_owned: E::Owned, _universe: &Universe) -> Self {}
    fn post_cleanup(self, _universe: &Universe) {}
}


/// Helper trait.
pub unsafe trait ExtractOwned {
    type Ty: AnyDebug;
    const ACC: Access;
    unsafe fn extract(universe: &Universe, rez: &mut Rez) -> Self;
}
unsafe impl<X> Extract for X
where
    X: ExtractOwned,
{
    fn each_resource(f: &mut dyn FnMut(Ty, Access)) {
        f(Ty::of::<X::Ty>(), X::ACC)
    }
    type Owned = Option<X>;
    unsafe fn extract(universe: &Universe, rez: &mut Rez) -> Self::Owned {
        Some(X::extract(universe, rez))
    }
    unsafe fn convert(_universe: &Universe, owned: *mut Self::Owned) -> X {
        (*owned).take().unwrap()
    }
    type Cleanup = ();
}

/// Produces the objects asked for by `Extract`.
#[derive(Debug)]
pub struct Rez {
    // FIXME: We don't actually need 'static on this, right?
    vals: &'static [(*mut dyn AnyDebug, Access)],
}
impl Rez {
    pub(crate) fn new(vals: &'static [(*mut dyn AnyDebug, Access)]) -> Self {
        Rez { vals }
    }
    pub unsafe fn take_ref<'b>(&mut self) -> &'b dyn AnyDebug {
        let (v, a): (*mut dyn AnyDebug, Access) = self.vals[0];
        assert_eq!(a, Access::Read, "asked for Access::Write but used take_ref");
        self.vals = &self.vals[1..];
        &mut *v
    }
    pub unsafe fn take_mut<'b>(&mut self) -> &'b mut dyn AnyDebug {
        let (v, a): (*mut dyn AnyDebug, Access) = self.vals[0];
        assert_eq!(a, Access::Write, "asked for Access::Read but used take_mut");
        self.vals = &self.vals[1..];
        &mut *v
    }
    pub unsafe fn take_ref_downcast<'b, T: AnyDebug>(&mut self) -> &'b T {
        let got: &dyn AnyDebug = self.take_ref();
        got.downcast_ref().unwrap()
    }
    pub unsafe fn take_mut_downcast<'b, T: AnyDebug>(&mut self) -> &'b mut T {
        let got: &mut dyn AnyDebug = self.take_mut();
        got.downcast_mut().unwrap()
    }
    // FIXME: Explain why we use the 'static lie.
    // FIXME: Couldn't these methods be made safe if we stuck an 'b on Rez?
}
