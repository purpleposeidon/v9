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
/// let mut owned: Self::Owned = Self::extract(&mut resources);
/// // (any other Extraction instance…)
/// {
///     let converted: Self = Self::convert(&mut owned);
///     // (…)
///     kernel_call(converted, …);
/// }
/// Self::finish(universe, owned);
/// // (…)
/// ```
// I've put crazy amounts of time into trying to get this working w/o unsafe.
// It's impossible. Any lifetime that you can name is valid for the duration of the function, and
// we'd need to be able to name lifetimes for something *shorter* than a function.
// You might want to see if you can get it working with recursive closures or something,
// but I suspect it'd be kinda nasty.
pub unsafe trait Extract: Sized {
    /// List the type & access requirement needed to do the extraction.
    /// This function must have constant behavior; it is unsound otherwise.
    fn each_resource(f: &mut dyn FnMut(TypeId, Access));
    type Owned;
    unsafe fn extract(universe: &Universe, rez: &mut Rez) -> Self::Owned;
    unsafe fn convert(universe: &Universe, owned: *mut Self::Owned) -> Self;
    fn finish(_universe: &Universe, _owned: Self::Owned) {}
}
// FIXME: It'd be nice to have impls of Extract for tuples; up to, say, 5.

/// Helper trait.
pub unsafe trait ExtractOwned {
    type Ty: Obj;
    const ACC: Access;
    unsafe fn extract(universe: &Universe, rez: &mut Rez) -> Self;
}
unsafe impl<X> Extract for X
where
    X: ExtractOwned,
{
    fn each_resource(f: &mut dyn FnMut(TypeId, Access)) {
        f(TypeId::of::<X::Ty>(), X::ACC)
    }
    type Owned = Option<X>;
    unsafe fn extract(universe: &Universe, rez: &mut Rez) -> Self::Owned {
        Some(X::extract(universe, rez))
    }
    unsafe fn convert(_universe: &Universe, owned: *mut Self::Owned) -> X {
        (*owned).take().unwrap()
    }
}

/// Produces the objects asked for by `Extract`.
#[derive(Debug)]
pub struct Rez {
    vals: &'static [(*mut dyn Obj, Access)],
}
impl Rez {
    pub(crate) fn new(vals: &'static [(*mut dyn Obj, Access)]) -> Self {
        Rez { vals }
    }
    pub unsafe fn take_ref<'a>(&mut self) -> &'a dyn Obj {
        let (v, a): (*mut dyn Obj, Access) = self.vals[0];
        assert_eq!(a, Access::Read, "asked for Access::Write but used take_ref");
        self.vals = &self.vals[1..];
        &mut *v
    }
    pub unsafe fn take_mut<'a>(&mut self) -> &'a mut dyn Obj {
        let (v, a): (*mut dyn Obj, Access) = self.vals[0];
        assert_eq!(a, Access::Write, "asked for Access::Read but used take_mut");
        self.vals = &self.vals[1..];
        &mut *v
    }
    pub unsafe fn take_ref_downcast<'a, T: Obj>(&mut self) -> &'a T {
        let got: &Obj = self.take_ref();
        got.downcast_ref().unwrap()
    }
    pub unsafe fn take_mut_downcast<'a, T: Obj>(&mut self) -> &'a mut T {
        let got: &mut Obj = self.take_mut();
        got.downcast_mut().unwrap()
    }
    // FIXME: Explain why we use the 'static lie.
    // FIXME: Couldn't these methods be made safe if we stuck an 'a on Rez?
}
