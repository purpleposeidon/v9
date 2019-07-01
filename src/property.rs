use crate::prelude_lib::*;
use std::any::Any;

#[derive(Debug)]
#[derive(serde::Serialize, serde::Deserialize)]
#[repr(transparent)]
pub struct Property<T: Any + Send + Sync> {
    pub inner: T,
}
impl<T: Any + Send + Sync> Deref for Property<T> {
    type Target = T;
    fn deref(&self) -> &T { &self.inner }
}
impl<T: Any + Send + Sync> DerefMut for Property<T> {
    fn deref_mut(&mut self) -> &mut T { &mut self.inner }
}
impl<T: Any + Send + Sync> Obj for Property<T> {}
impl<T: Any + Send + Sync> Obj for &'static Property<T> {}
impl<T: Any + Send + Sync> Obj for &'static mut Property<T> {}
unsafe impl<'a, T: Any + Send + Sync> ExtractOwned for &'a Property<T> {
    type Ty = Property<T>;
    const ACC: Access = Access::Read;
    unsafe fn extract(_universe: &Universe, rez: &mut Rez) -> Self {
        rez.take_ref_downcast()
    }
}
unsafe impl<'a, T: Any + Send + Sync> ExtractOwned for &'a mut Property<T> {
    type Ty = Property<T>;
    const ACC: Access = Access::Write;
    unsafe fn extract(_universe: &Universe, rez: &mut Rez) -> Self {
        rez.take_mut_downcast()
    }
}

pub trait PropertyMarker {
    const NAME: Name;
    fn header() -> PropertyHeader;
}

#[derive(Debug)]
pub struct PropertyHeader {
    pub name: Name,
    pub property_type: TypeId,
    pub inner_type: TypeId,
}

#[macro_export]
macro_rules! property {
    // Default-initialized property
    (
        $(#[$meta:meta])*
        $vis:vis type $name:ident: $type:ty
    ) => {
        property! {
            $(#[$meta])*
            $vis type $name: $type = Default::default();
        }
    };

    // expression-initialized properties
    (
        $(#[$meta:meta])*
        $vis:vis $name:ident: $type:ty = $init:expr;
    ) => {
        $(#[$meta])*
        #[allow(non_camel_case_types)]
        $vis type $name = $crate::property::Property<$type>;
        impl $crate::object::Register for $name {
            fn register(universe: &mut $crate::object::Universe) {
                universe.add_mut(
                    TypeId::of::<Self>(),
                    Self { inner: $init },
                );
            }
        }
        impl $crate::property::PropertyMarker for $name {
            const NAME: &'static str = stringify!($name);
            fn header() -> $crate::property::PropertyHeader {
                $crate::property::PropertyHeader {
                    name: Self::NAME,
                    property_type: TypeId::of::<Self>(),
                    inner_type: TypeId::of::<$type>(),
                }
            }
        }
    };
}

#[cfg(test)]
mod test {
    use crate::prelude_lib::*;

    #[derive(Debug)]
    struct MyProperty {
        val: i32,
    }

    property! {
        MY_PROPERTY: MyProperty = MyProperty {
            val: 27,
        };
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
