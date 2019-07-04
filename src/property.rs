use crate::prelude_lib::*;

pub trait PropertyMarker: Register {
    const NAME: Name;
    fn header() -> PropertyHeader;
}

#[derive(Debug)]
pub struct PropertyHeader {
    pub name: Name,
    pub property_type: TypeId,
    pub inner_type: TypeId,
}


/// Declares a singleton property.
///
/// # Example
/// ```
/// # #[macro_use] extern crate v9;
/// # use v9::prelude::*;
/// property! {
///     /// Documentation can go here.
///     /// This property is initialized via `Default`.
///     #[derive(Clone)] // Derives also.
///     pub MY_PROPERTY: ~i32
/// }
///
/// property! {
///     // You can also initialize with an expression. Note the trailing semicolon.
///     pub EXPLICIT_INIT: ~i32 = 237;
/// }
///
/// fn main() {
///     let mut universe = Universe::new();
///     MY_PROPERTY::register(&mut universe);
///     EXPLICIT_INIT::register(&mut universe);
///     universe.kmap(|a: &mut MY_PROPERTY, b: &EXPLICIT_INIT| {
///         **a += **b;
///     });
/// }
/// ```
///
/// # `impl doesn't use types inside crate`
/// This is an error you'll get if you try to make a property out of a type you don't own.
/// You can get around this by putting a `~` in front of the type, as is done in the example here.
/// They'll be slightly less pleasant to use... as you can see in the example here.
// Maybe this `non_localtype` thing isn't worthwhile. Maybe your types should always be local?
// We could also have a macro to create a wrapper? Hmm? `property_wrapper!` ?
//
// Well, anyways. This macro has the notion of 'non-local' types. Put a ~ in front, and it'll
// generate a wrapper for you. It makes the macro a bit weird.
// Since we don't have anything like `$($ty:ty $| ~ $ty:ty)|`, we emulate it by using `$()?`.
// It makes life a little weird. We have to pull in the preceding token to satisfy the
// future-proofing rules. Also we can't expand $meta when we have $[non]local_type selected,
// because it whines about lockstepping.
#[macro_export]
macro_rules! property {
    // Default-initialized property
    (
        $(#[$meta:meta])*
        $vis:vis $name:ident
            $(: $local_type:ty)?
            $(: ~$nonlocal_type:ty)?
    ) => {
        property! {
            $(#[$meta])*
            $vis $name
                $(: $local_type)?
                $(:~$nonlocal_type)?
                = Default::default();
        }
    };

    // expression-initialized properties
    (
        $(#[$meta:meta])*
        $vis:vis $name:ident
            $(: $local_type:ty)?
            $(: ~$nonlocal_type:ty)?
            = $init:expr;
    ) => {
        $crate::paste::item! {
            // Stabilize $_:ty's. paste lets us use a nice alternative to the
            //     mod thing { use super::super::* }
            // pattern. I think it's more robust too!
            // The namespacing spam is a bit unfortunate.
            $(
                #[doc(hidden)]
                #[allow(non_camel_case_types)]
                type [<_v9_property_type_ $name>] = $local_type;
            )?
            $(
                #[doc(hidden)]
                #[allow(non_camel_case_types)]
                type [<_v9_property_type_ $name>] = $nonlocal_type;
            )?
            #[doc(hidden)]
            #[allow(non_snake_case)]
            fn [<_v9_property_init_ $name>]() -> [<_v9_property_type_ $name>] {
                $init
            }

            $vis use self::[<_v9_property_mod_ $name>]::Prop as $name;

            #[doc(hidden)]
            #[allow(non_snake_case, dead_code, non_camel_case_types)]
            mod [<_v9_property_mod_ $name>] {
                use $crate::property::prelude::*;
                use super::[<_v9_property_type_ $name>] as Type;
                use super::[<_v9_property_init_ $name>] as init_fn;

                $crate::property!(@wrap_nonlocal $($nonlocal_type)*; $(#[$meta])*);
                $(
                    $crate::property!(@if $local_type);
                    pub type Prop = Type;
                    use self::init_fn as localized_init_fn;
                    impl Obj for Prop {}
                    unsafe impl Property for Prop {
                        // FIXME: Boy does this feel dirty!
                        // Like, you've given me this thing...
                        // I have no clue what it is...
                        // And I'm going to implement Extract on it.
                    }
                    impl AssertLocalType for Prop {}
                    //$("attributes don't make sense here" $meta)*
                    // ... is what I'd like to say. But it's not worth fixing. Ugh!
                )?

                impl Register for Prop {
                    fn register(universe: &mut Universe) {
                        universe.add_mut(
                            TypeId::of::<Prop>(),
                            localized_init_fn(),
                        );
                    }
                }
                impl PropertyMarker for Prop {
                    const NAME: Name = stringify!($name);
                    fn header() -> PropertyHeader {
                        PropertyHeader {
                            name: Self::NAME,
                            property_type: TypeId::of::<Self>(),
                            inner_type: TypeId::of::<Type>(),
                        }
                    }
                }
            }
        }
    };

    // Matches on a 'branch' without producing any tokens.
    (@if $tt:tt) => {};

    // Work-around for lockstep issue.
    (@wrap_nonlocal ; $(#[$meta:meta])*) => {};
    (@wrap_nonlocal $nonlocal_type:ty; $(#[$meta:meta])*) => {
        $(#[$meta])*
        #[repr(transparent)]
        #[derive(Debug, Default)]
        pub struct PropGeneric<T> {
            // We have no idea if `Type` is debug or not.
            // Unfortunately, Rust also has no idea if we have any idea if `Type` is Debugor
            // not. If it happens to not be, then if we had `inner: Type`, deriving Debug
            // would crash. So we have to convince Rust that we don't know.
            pub inner: T,
        }
        // ...and that was super easy! We don't have to worry about it now.
        pub type Prop = PropGeneric<Type>;
        fn localized_init_fn() -> Prop {
            Prop { inner: init_fn() }
        }
        impl Deref for Prop {
            type Target = Type;
            fn deref(&self) -> &Type { &self.inner }
        }
        impl DerefMut for Prop {
            fn deref_mut(&mut self) -> &mut Type { &mut self.inner }
        }
        impl Obj for Prop {}
        unsafe impl Property for Prop {}
    };
}

pub unsafe trait Property: Obj {}
unsafe impl<'a, X: Property> ExtractOwned for &'a X {
    type Ty = X;
    const ACC: Access = Access::Read;
    unsafe fn extract(_universe: &Universe, rez: &mut Rez) -> Self {
        rez.take_ref_downcast()
    }
}
unsafe impl<'a, X: Property> ExtractOwned for &'a mut X {
    type Ty = X;
    const ACC: Access = Access::Write;
    unsafe fn extract(_universe: &Universe, rez: &mut Rez) -> Self {
        rez.take_mut_downcast()
    }
}

#[doc(hidden)]
pub mod prelude {
    pub use crate::prelude_lib::{Deref, DerefMut, Name, Obj, TypeId, Universe};
    pub use crate::prelude_lib::{Property, PropertyHeader, PropertyMarker, Register};

    #[doc(hidden)]
    #[allow(non_camel_case_types)]
    pub trait AssertLocalType {}
}

#[cfg(test)]
mod test {
    use crate::prelude_lib::*;

    #[derive(Debug)]
    pub struct MyProperty {
        val: i32,
    }

    property! {
        MY_PROPERTY: MyProperty = MyProperty {
            val: 27,
        };
    }

    property! {
        pub SHORT_PROPERTY: ~i32
    }

    #[test]
    fn property() {
        let mut universe = Universe::new();
        MY_PROPERTY::register(&mut universe);
        universe.kmap(|prop: &MY_PROPERTY| {
            println!("{:?}", prop);
        });
    }
}


#[cfg(test)]
#[allow(unused_imports)]
mod test_compiles {
    #[allow(dead_code)]
    pub struct Meh {
        val: i32,
    }

    property! { MY_PROPERTY: Meh = Meh { val: 42 }; }

    context! {
        #[allow(dead_code)]
        struct Stuff {
            test: &MY_PROPERTY,
            test2: &mut NON_LOCAL_PROPERTY,
        }
    }

    property! { NON_LOCAL_PROPERTY: ~i32 }
}
