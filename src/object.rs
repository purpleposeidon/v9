//! The `Universe`, and interacting with it as a data structure.

use crate::prelude_lib::*;
use std::collections::hash_map::Entry as MapEntry;
use std::collections::HashMap;
use std::sync::{Mutex, Condvar};
use ezty::AnyDebug;

// FIXME: impl Extract for Universe.

// FIXME: Implement a property wrapper. Probably called `Val` instead of `Property`.

/// The star of our show! The god object that holds everything.
#[derive(Default)]
pub struct Universe {
    pub(crate) objects: Mutex<HashMap<Ty, Box<Locked>>>,
    pub(crate) condvar: Condvar,
    pub(crate) frozen: bool,
}

unsafe impl Send for Universe {}
unsafe impl Sync for Universe {}
// I'm working off of metaphor by RwLock here.
// RwLock has these bounds if T does.
// Since Obj is our T, it needs to have those bounds as well.

impl Universe {
    pub fn new() -> Self {
        Self::default()
    }
    fn insert(map: &mut HashMap<Ty, Box<Locked>>, ty: Ty, obj: Box<Locked>) {
        match map.entry(ty) {
            MapEntry::Occupied(_) => {
                panic!("object inserted twice: {:?}", ty)
            },
            MapEntry::Vacant(e) => e.insert(obj),
        };
    }
    pub fn add<T: AnyDebug>(&self, key: Ty, obj: T) {
        assert!(!self.frozen);
        let map = &mut *self.objects.lock().unwrap();
        Universe::insert(map, key, Locked::new(Box::new(obj), std::any::type_name::<T>()));
    }
    pub fn add_mut<T: AnyDebug>(&mut self, key: Ty, obj: T) {
        assert!(!self.frozen);
        let map = &mut *self.objects.get_mut().unwrap();
        let obj = Locked::new(Box::new(obj), std::any::type_name::<T>());
        Universe::insert(map, key, obj);
    }
    pub fn remove<T: AnyDebug>(&self, key: Ty) -> Option<Box<dyn AnyDebug>> {
        assert!(!self.frozen);
        self.objects
            .lock()
            .unwrap()
            .remove(&key)
            .map(|l| l.into_inner())
    }
    pub fn remove_mut<T: AnyDebug>(&mut self, key: Ty) -> Option<Box<dyn AnyDebug>> {
        assert!(!self.frozen);
        self.objects
            .get_mut()
            .unwrap()
            .remove(&key)
            .map(|l| l.into_inner())
    }
    /// Disable further modification to the structure of the Universe.
    pub fn freeze(&mut self) {
        self.frozen = true;
    }
    pub fn has<T: AnyDebug>(&self) -> bool {
        self.has_ty(Ty::of::<T>())
    }
    pub fn has_ty(&self, ty: Ty) -> bool {
        self.objects
            .lock()
            .unwrap()
            .get(&ty)
            .is_some()
    }
}

impl Universe {
    pub fn all_mut(&mut self, mut each: impl FnMut(/*marker:*/ Ty, /*obj:*/ &mut dyn AnyDebug)) {
        let mut objs = self.objects.lock().unwrap();
        for (marker, lock) in objs.iter_mut() {
            unsafe {
                let mut lock = lock.write();
                let obj: &mut dyn AnyDebug = &mut *lock;
                each(*marker, obj);
            }
        }
    }
    pub fn all_ref(&self, mut each: impl FnMut(/*marker:*/ Ty, /*obj:*/ &dyn AnyDebug)) {
        let mut objs = self.objects.lock().unwrap();
        for (marker, lock) in objs.iter_mut() {
            unsafe {
                let lock = lock.read(/* mut. Awkard. */);
                let obj: &dyn AnyDebug = &*lock;
                each(*marker, obj);
            }
        }
    }
}

impl Universe {
    pub fn clone_value<T: AnyDebug + Clone>(&self) -> T {
        self.with(T::clone)
    }
    pub fn with<T: AnyDebug, R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.with_obj(Ty::of::<T>(), |obj| {
            let obj = obj.downcast_ref().expect("type mismatch");
            f(obj)
        })
    }
    pub fn with_mut<T: AnyDebug, R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        self.with_obj_mut(Ty::of::<T>(), |obj| {
            let obj = obj.downcast_mut().expect("type mismatch");
            f(obj)
        })
    }
    pub fn with_obj<R>(&self, ty: Ty, f: impl FnOnce(&dyn AnyDebug) -> R) -> R {
        let mut f = Some(f);
        let mut ret = Option::None;
        self.with_access(ty, Access::Read, &mut |obj: *mut dyn AnyDebug| unsafe {
            let obj = &*obj;
            ret = Some((f.take().unwrap_unchecked())(obj));
        });
        unsafe { ret.unwrap_unchecked() }
    }
    pub fn with_obj_mut<R>(&self, ty: Ty, f: impl FnOnce(&mut dyn AnyDebug) -> R) -> R {
        let mut f = Some(f);
        let mut ret = Option::None;
        self.with_access(ty, Access::Write, &mut |obj: *mut dyn AnyDebug| unsafe {
            let obj = &mut *obj;
            ret = Some((f.take().unwrap_unchecked())(obj));
        });
        unsafe { ret.unwrap_unchecked() }
    }
    fn with_access(
        &self,
        ty: Ty,
        access: Access,
        f: &mut dyn FnMut(*mut dyn AnyDebug),
    ) {
        let objects = self.objects.lock().unwrap();
        let mut objects = self.condvar.wait_while(objects, |objects| {
            let obj = objects
                .get_mut(&ty)
                .unwrap_or_else(|| panic!("type not found: {:?}", ty));
            !obj.can(access)
        }).expect("with_var condvar wait failed");
        let obj = objects
            .get_mut(&ty)
            .unwrap_or_else(|| panic!("type not found: {:?}", ty));
        obj.acquire(access);
        let obj = unsafe { obj.contents() };
        mem::drop(objects);
        let _cleanup = {
            struct Defer<T: FnMut()>(T);
            impl<T: FnMut()> Drop for Defer<T> {
                fn drop(&mut self) {
                    (self.0)()
                }
            }
            Defer(move || {
                let mut objects = self.objects.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                let obj = objects
                    .get_mut(&ty)
                    .unwrap_or_else(|| panic!("type lost while in use: {:?}", ty));
                obj.release(access);
                self.condvar.notify_all();
            })
        };
        f(obj);
    }
    pub fn lock_state_dump(&self) {
        let objects = self.objects.lock().unwrap();
        for (ty, val) in objects.iter() {
            println!("    {:?}\t{:?}", ty, val.state);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fmt::Write;

    unsafe impl<'a> Extract for &'a mut String {
        fn each_resource(f: &mut dyn FnMut(Ty, Access)) {
            f(Ty::of::<String>(), Access::Write);
        }
        type Owned = Self;
        unsafe fn extract(_universe: &Universe, rez: &mut Rez) -> Self {
            rez.take_mut_downcast()
        }
        unsafe fn convert(_universe: &Universe, owned: *mut Self::Owned) -> Self {
            *(&mut *owned)
        }
        type Cleanup = ();
    }

    #[test]
    fn construction() {
        let _universe = Universe::new();
    }

    #[test]
    fn single_string() {
        let mut universe = Universe::new();
        let key = Ty::of::<String>();
        universe.add_mut(key, format!("Hello"));
    }

    #[test]
    #[should_panic]
    fn conflicting_strings() {
        let mut universe = Universe::new();
        let key = Ty::of::<String>();
        universe.add_mut(key, format!("oh no"));
        universe.add_mut(key, format!("o noez"));
    }

    #[test]
    fn look_string() {
        let mut universe = Universe::new();
        universe.add_mut(Ty::of::<String>(), format!("Hello"));
        universe.kmap(|text: &mut String| {
            println!("Hey!");
            println!("We've got: {:?}", text);
        });
    }

    #[test]
    #[should_panic]
    fn look_for_missing_string() {
        let universe = Universe::new();
        universe.kmap(|_: &mut String| {});
    }

    #[test]
    fn change_string() {
        let mut universe = Universe::new();
        universe.add_mut(Ty::of::<String>(), format!("Hello"));
        universe.kmap(|text: &mut String| {
            println!("We've got: {:?}", text);
            write!(text, " World").ok();
        });
        universe.kmap(|text: &mut String| {
            assert_eq!(text, "Hello World");
        });
    }

    #[test]
    fn universe_claims_to_be_threadsafe() {
        fn assert<T: Send + Sync>() {}
        assert::<Universe>();
    }
}

/// Extract many things at once.
///
/// Several kernels might have a set of arguments in common. Furthermore, sets of these things
/// might be passed off to functions. This macro allows you to dry up your code.
///
/// Each field of the struct must be `Extract`, *AND* its kind must be `type T<'a> = …`.
///
/// The macro adds a lifetime to everything, so in the example the declared item comes out
/// `struct MyContext<'a>`.
///
/// # Example
/// ```
/// # use v9::prelude::*;
/// #
/// # #[v9::table]
/// # struct my_table {
/// #     pub foo: i32,
/// # }
/// #
/// #[v9::context]
/// pub struct MyContext {
///     hi: self::my_table::Edit,
/// }
/// # fn main() {}
/// ```
///
// We could mention that it adds a module, but that hardly seems necessary with paste. :D
#[macro_export]
macro_rules! decl_context {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$cmeta:meta])*
                $cvis:vis $cn:ident
                    $(: &mut $cty_mut:ty,)?
                    $(: &$cty_ref:ty,)?
                    $(: $cty_path:path,)?
            )*
        }
    ) => {
        $crate::paste::item! {
            #[allow(unused_imports)]
            $vis use self::[<_v9_impl_ $name>]::$name;
            #[allow(non_snake_case)]
            mod [<_v9_impl_ $name>] {
                // trickery to convert $:path to other things.
                #[allow(non_camel_case_types)]
                mod path {
                    use super::super::*;
                    $(
                        pub type [<_v9_ctx_ $name _ $cn>]<'a> =
                            $(&'a mut $cty_mut)?
                            $(&'a $cty_ref)?
                            $($cty_path<'a>)?
                        ;
                    )*
                }
                #[allow(non_camel_case_types)]
                mod cn {
                    $(pub type $cn<'a> = super::path::[<_v9_ctx_ $name _ $cn>]<'a>;)*
                }
                #[allow(non_camel_case_types)]
                mod owned {
                    $(pub type $cn = <super::cn::$cn<'static> as super::Extract>::Owned;)*
                }
                use $crate::prelude_macro::*;
                $(#[$meta])*
                pub struct $name<'a> {
                    $(
                        $(#[$cmeta])*
                        $cvis $cn: self::cn::$cn<'a>,
                    )*
                }
                pub struct __OwnedContext {
                    $($cn: self::owned::$cn,)*
                }
                unsafe impl<'a> Extract for $name<'a> {
                    fn each_resource(f: &mut dyn FnMut(Ty, Access)) {
                        $(<self::cn::$cn<'static> as Extract>::each_resource(f);)*
                    }
                    type Owned = __OwnedContext;
                    unsafe fn extract(universe: &Universe, rez: &mut Rez) -> Self::Owned {
                        __OwnedContext {
                            $($cn: <self::cn::$cn<'static> as Extract>::extract(universe, rez),)*
                        }
                    }
                    unsafe fn convert(universe: &Universe, owned: *mut Self::Owned) -> Self {
                        let owned: &mut __OwnedContext = &mut *owned;
                        $name {
                            $($cn: <self::cn::$cn<'static> as Extract>::convert(universe, &mut owned.$cn),)*
                        }
                    }
                    type Cleanup = __OwnedCleanup;
                }
                pub struct __OwnedCleanup {
                    $($cn: <self::cn::$cn<'static> as Extract>::Cleanup,)*
                }
                unsafe impl<'a> Cleaner<$name<'a>> for __OwnedCleanup {
                    fn pre_cleanup(owned: __OwnedContext, universe: &Universe) -> Self {
                        Self {
                            $($cn: {
                                type T = self::cn::$cn<'static>;
                                <<T as Extract>::Cleanup as Cleaner<T>>::pre_cleanup(owned.$cn, universe)
                            },)*
                        }
                    }
                    fn post_cleanup(self, universe: &Universe) {
                        $(Cleaner::<self::cn::$cn<'static>>::post_cleanup(self.$cn, universe);)*
                    }
                }
            }
        }
    };
}

/// This trait is implemented by macros such as `decl_table!`. It provides a common means for
/// adding types to the [`Universe`].
pub trait Register {
    fn register(universe: &mut Universe);
}

/// Allows accessing a `Universe` from within a kernel. Best avoided if you use schedulers.
// Which is why we don't just impl Extract for &Universe.
#[repr(transparent)]
pub struct UniverseRef<'a> {
    universe: &'a Universe,
}
unsafe impl<'a> Send for UniverseRef<'a> {}
unsafe impl<'a> Sync for UniverseRef<'a> {}
impl<'a> Deref for UniverseRef<'a> {
    type Target = Universe;
    fn deref(&self) -> &Universe { self.universe }
}
unsafe impl<'a> Extract for UniverseRef<'a> {
    fn each_resource(_f: &mut dyn FnMut(Ty, Access)) {}
    type Owned = ();
    unsafe fn extract(_universe: &Universe, _rez: &mut Rez) -> Self::Owned {}
    unsafe fn convert(universe: &Universe, _owned: *mut Self::Owned) -> Self {
        UniverseRef {
            universe: /*unsafe*/ {
                // This is safe because the only way to construct a UniverseRef is via our kernel
                // stuff. The contract of Extract says that universe outlives Self.
                // So you can only get this in to an argument to a closure.
                // And Rust won't let you send stuff from closure arguments to outside the closure?
                // Phew! (See StaticStuffShouldntCompile.)
                &*(universe as *const _)
            },
        }
    }
    type Cleanup = ();
}

/// ```compile_fail
/// use v9::prelude_lib::*;
/// fn static_stuff_shouldnt_compile() {
///     let mut dude = Option::<&Universe>::None;
///     let u = Universe::new();
///     u.eval(|verse: UniverseRef<'static>| {
///         dude = Some(&verse);
///     });
///     std::mem::drop(u);
///     dude.unwrap().eval(|_verse: UniverseRef| {
///         panic!();
///     });
/// }
/// ```
/// ```compile_fail
/// use v9::prelude_lib::*;
/// use v9::kernel::KernelArg;
/// v9::decl_property! {
///     pub FOO: ~[u8; 4] = [1, 2, 3, 4];
/// }
/// fn other_static_stuff_shouldnt_compile() {
///     let mut u = Universe::new();
///     FOO::register(&mut u);
///     let mut foop = &mut [5, 6, 7, 8];
///     u.eval(|foo: &'static mut FOO| {
///         let foo: &mut [u8; 4] = &mut *foo;
///         foop = foo;
///     });
///     println!("{:?}", foop);
///     {u};
///     println!("{:?}", foop);
/// }
/// fn main() {}
/// ```
#[cfg(doctest)]
struct SoundnessChecks;
