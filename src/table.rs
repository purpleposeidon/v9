use crate::prelude_lib::*;

/// Generic information about a table.
// Doesn't include len tho. :(
#[derive(Debug)]
pub struct TableHeader {
    pub name: Name,
    pub marker: TypeId,
    pub columns: Vec<TypeId>,
}
impl Obj for TableHeader {}
pub trait TableMarker: 'static + Default + Copy + Send + Sync + Register + fmt::Debug {
    const NAME: Name;
    type RawId: Raw;
    fn header() -> TableHeader;
}

/// Defines a table. This is the most important item in the crate!
///
/// # Usage
/// ```
/// // Declare a couple tables.
/// v9::table! {
///     pub struct cheeses {
///         pub quantity: f64,
///         // NOTE: Absolute paths should be used.
///         pub warehouse: crate::warehouses::Id,
///         pub stinky: bool,
///     }
/// }
///
/// v9::table! {
///     pub struct warehouses {
///         pub coordinates: (i32, i32),
///         pub on_fire: bool,
///     }
/// }
///
/// fn main() {
///     // We create a new Universe. We put everything we can in it!
///     use v9::prelude::Universe;
///     let mut universe = Universe::new();
///
///     // But it doesn't know about the tables, so we must register them.
///     use v9::prelude::Register;
///     cheeses::Marker::register(&mut universe);
///     warehouses::Marker::register(&mut universe);
///
///     // Let's print out an inventory.
///     use v9::kernel::Kernel;
///     let mut print_inventory = Kernel::new(|cheeses: cheeses::Read, warehouses: warehouses::Read| {
///         println!("Warehouses:");
///         for id in warehouses.iter() {
///             println!("{:?}", warehouses.ref_row(id));
///         }
///         println!("Cheeses:");
///         for id in cheeses.iter() {
///             println!("{:?}", cheeses.ref_row(id));
///         }
///     });
///     // The kernel holds our closure, and keeps track of all of the objects it requires.
///     universe.run(&mut print_inventory);
///     // It's empty... we should add some things.
///     universe.kmap(|mut warehouses: warehouses::Write, mut cheeses: cheeses::Write| {
///         warehouses.reserve(3);
///         let w0 = warehouses.push(warehouses::Row {
///             coordinates: (0, 0),
///             on_fire: true,
///         });
///         let w1 = warehouses.push(warehouses::Row {
///             coordinates: (4, 9),
///             on_fire: false,
///         });
///         let w2 = warehouses.push(warehouses::Row {
///             coordinates: (8, 4),
///             on_fire: true,
///         });
///         for wid in &[w0, w1, w2] {
///             cheeses.reserve(10);
///             for _ in 0..10 {
///                 cheeses.push(cheeses::Row {
///                     quantity: 237.0,
///                     warehouse: *wid,
///                     stinky: true,
///                 });
///             }
///         }
///     });
///     // v9 is somewhat low-level. Because a Kernel does allocation (and we HATE allocation)
///     // you are expected to implement your own higher level manager.
///     // We can use `universe.kmap` when we prefer to be sloppy.
///     universe.run(&mut print_inventory);
///     // Now we should see our data.
///     // But remember how those warehouses were on fire?
///     universe.kmap(|list: &mut warehouses::Ids, mut on_fire: warehouses::edit::on_fire| {
///         let mut dousing = true;
///         for wid in list.removing() {
///             if on_fire[wid] {
///                 if dousing {
///                     dousing = false;
///                     on_fire[wid] = false;
///                 } else {
///                     wid.remove();
///                 }
///             }
///         }
///     });
///     // v9 has ensured data consistency -- the cheese in the destroyed warehouses has been lost.
///     universe.run(&mut print_inventory);
///     universe.kmap(|cheeses: cheeses::Read| {
///         let mut n = 0;
///         for _ in cheeses.iter() {
///             n += 1;
///         }
///         assert_eq!(n, 20);
///     });
/// }
/// ```
///
/// # Details
///
/// There's several things to be aware of.
/// 1. Naming. The item name should be lowercase (it becomes a module), and plural. The names of
///    columns should be singular. (Unless it should be, like in `pub aliases: Vec<String>`.)
/// 2. The macro syntax kind of looks like a struct… but it very much is not.
/// 3. Type paths should be absolute, not relative.
/// 4. The struct's visiblity may be anything, but the field's visiblity must be `pub`.
///
/// # Meta-Attributes
/// There are certain meta-attributes that may be placed on the "struct". They must be provided in the order given here:
/// 1. `#[doc]`*. Places documentation on the module.
/// 2. `#[row::<meta>]`* Passes meta-attributes to the `Row`; eg `#[row(derive(serde::Serialize))]`.
///    `#[derive(Clone, Debug)]` is always provided.
/// 3. `#[raw_index(u32)]`. Defines the type used to index. The default is `u32`. Must be [`Raw`].
///    The last index is generally considered to be 'invalid'.
///
/// Any attributes on the columns will be passed as-is to the fields on `Row`.
///
/// [`Raw`]: id/trait.Raw.html
///
/// ## Example
///
/// ```
/// v9::table! {
///     /// Some of our many fine cheeses!
///     #[row::derive(Copy, PartialEq, Eq)]
///     #[row::derive(Hash)]
///     #[raw_index(u8)]
///     pub struct cheeses {
///         /// We have so much cheese!
///         pub quantity: u64,
///         /// P. U.!
///         pub stinky: bool,
///         /// If we were deriving serde, we could uncomment this meta-attribute.
///         //#[serde::skip]
///         pub on_fire: Option<bool>,
///     }
/// }
/// ```
// FIXME: Maybe I should go for a more v11-style syntax?
// FIXME: keep the stinky_cheeses example in sync or something...?
#[macro_export]
macro_rules! table {
    (@first $x:tt $($_xs:tt)*) => { $x };
    (
        $(#[doc = $doc:literal])*
        $(#[row::$row_meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$cmeta:meta])*
                pub $cn:ident: $cty:ty,
            )*
        }
    ) => {
        $crate::table! {
            $(#[doc = $doc])*
            $(#[row::$row_meta])*
            #[raw_index(u32)]
            $vis struct $name {
                $(
                    $(#[$cmeta])*
                    pub $cn: $cty,
                )*
            }
        }
    };
    (
        $(#[doc = $doc:literal])*
        $(#[row::$row_meta:meta])*
        #[raw_index($raw:ty)]
        $vis:vis struct $name:ident {
            $(
                $(#[$cmeta:meta])*
                pub $cn:ident: $cty:ty,
            )*
        }
    ) => {
        #[allow(non_camel_case_types, dead_code, non_upper_case_globals, non_snake_case)]
        $(#[doc = $doc])*
        $vis mod $name {
            // Annoyingly, we have to firewall out v9 types from the user's.
            // We could do `$crate::prelude_macro::Thing` instead but it's horrifically ugly, and
            // it gets *everywhere*.
            mod in_v9 {
                use $crate::prelude_macro::*;
                use super::in_user::{Read, Write, Edit, Row, RowRef};
                pub const NAME: &'static str = stringify!($name);
                /// A strongly typed index into the table. "Pre-checked" ids are available.
                pub type Id = IdV9<Marker>;
                /// The valid IDs. Kernels should take this by reference. Prefer using `List`.
                pub type Ids = IdList<Marker>;
                pub type CheckedId<'a> = CheckedIdV9<'a, Marker>;
                pub const FIRST: IdV9<Marker> = IdV9(0);
                pub const INVALID: IdV9<Marker> = IdV9(<$raw as Raw>::LAST);
                /// Holds static information about the table.
                #[derive(Default, Copy, Clone)]
                pub struct Marker;
                impl fmt::Debug for Marker {
                    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                        write!(f, stringify!($name))
                    }
                }
                impl fmt::Display for Marker {
                    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                        write!(f, stringify!($name))
                    }
                }

                pub mod names {
                    $(pub const $cn: &'static str = concat!(stringify!($table), ".", stringify!($cn));)*
                }

                impl<'a> Read<'a> {
                    pub fn check(&self, i: impl Check<'a, M=Marker>) -> CheckedId<'a> {
                        i.check(&self.__v9__iter)
                    }
                    pub fn clone_row(&self, i: impl Check<'a, M=Marker>) -> Row {
                        let i = self.check(i);
                        Row {
                            $($cn: self.$cn[i].clone(),)*
                        }
                    }
                    pub fn ref_row(&self, i: impl Check<'a, M=Marker>) -> RowRef {
                        let i = self.check(i);
                        RowRef {
                            $($cn: &self.$cn[i],)*
                        }
                    }
                    pub fn len(&self) -> usize {
                        $crate::table! {@first $({
                            self.$cn.col.data.len()
                        })*}
                    }
                    pub fn iter_all(&self) -> UncheckedIdRange<Marker> {
                        let end = self.len();
                        IdRange::to(Id::from_usize(end))
                    }
                    pub fn iter(&self) -> CheckedIter<Marker> {
                        self.__v9__iter.iter()
                    }
                }
                impl<'a> Edit<'a> {
                    pub fn len(&self) -> usize {
                        $crate::table! {@first $({
                            self.$cn.col.data.len()
                        })*}
                    }
                    pub fn iter_all(&self) -> IdRange<Id> {
                        let end = self.len();
                        IdRange::to(Id::from_usize(end))
                    }
                }
                impl<'a> Write<'a> {
                    pub fn len(&self) -> usize {
                        $crate::table! {@first $({
                            self.$cn.col.data.len()
                        })*}
                    }
                    pub fn reserve(&mut self, n: usize) {
                        unsafe {
                            $(self.$cn.col.get_mut().data.reserve(n);)*
                        }
                    }
                    pub fn push(&mut self, row: Row) -> Id {
                        unsafe {
                            match self.__v9__iter.recycle_id() {
                                Ok(id) => {
                                    let i = id.to_usize();
                                    $(
                                        *self.$cn.col.get_mut().data.get_unchecked_mut(i) = row.$cn;
                                    )*
                                    id
                                },
                                Err(id) => {
                                    $(self.$cn.col.get_mut().data.push(row.$cn);)*
                                    id
                                },
                            }
                        }
                    }
                    pub fn borrow(&self) -> Read {
                        Read {
                            $($cn: self.$cn.borrow(),)*
                            __v9__iter: self.__v9__iter, // FIXME: Dum name
                        }
                    }
                    pub fn remove(&mut self, i: impl Into<Id>) {
                        self.__v9__iter.deleting.get_mut().push(i.into());
                    }
                    pub fn iter_all(&self) -> IdRange<Id> {
                        let end = self.len();
                        IdRange::to(Id::from_usize(end))
                    }
                    pub fn iter(&self) -> CheckedIter<Marker> {
                        self.__v9__iter.iter()
                    }
                }
            }
            mod in_user {
                #[allow(unused_imports)]
                use super::super::*;

                impl $crate::prelude_macro::TableMarker for super::Marker {
                    const NAME: &'static str = super::in_v9::NAME;
                    type RawId = $raw;
                    fn header() -> $crate::prelude_macro::TableHeader {
                        $crate::prelude_macro::TableHeader {
                            name: Self::NAME,
                            marker: $crate::prelude_macro::TypeId::of::<super::Marker>(),
                            columns: vec![
                                $($crate::prelude_macro::TypeId::of::<$cty>()),*
                            ],
                        }
                    }
                }
                impl $crate::prelude_macro::Register for super::Marker {
                    fn register(universe: &mut $crate::prelude_macro::Universe) {
                        universe.add_mut(
                            $crate::prelude_macro::TypeId::of::<super::Marker>(),
                            <Self as $crate::prelude_macro::TableMarker>::header(),
                        );
                        universe.add_mut(
                            $crate::prelude_macro::TypeId::of::<$crate::prelude_macro::IdList<super::Marker>>(),
                            $crate::prelude_macro::IdList::<super::Marker>::default(),
                        );
                        // Interesting that we can't have duplicate types, hmm?
                        $(universe.add_mut(
                                $crate::prelude_macro::TypeId::of::<$crate::prelude_macro::Column<super::Marker, $cty>>(),
                                $crate::prelude_macro::Column::<super::Marker, $cty>::new(),
                        );)*
                        use $crate::prelude_macro::ForeignKey as _;
                        $({
                            type T = $cty;
                            T::__v9_link_foreign_key::<super::Marker>(universe);
                        })*
                    }
                }

                /// An owned copy of a row, AOS layout.
                #[derive(Debug, Clone)]
                $(#[$row_meta])*
                pub struct Row {
                    $(
                        $(#[$cmeta])*
                        pub $cn: $cty,
                    )*
                }
                /// A reference to every value in a row.
                #[derive(Debug, Clone)]
                pub struct RowRef<'a> {
                    $(pub $cn: &'a $cty,)*
                }

                pub mod own {
                    #[allow(unused_imports)]
                    use super::super::super::*;
                    $(pub type $cn = $crate::prelude_macro::Column<super::super::in_v9::Marker, $cty>;)*
                }
                /// Read an individual column.
                pub mod read {
                    #[allow(unused_imports)]
                    use super::super::super::*;
                    $(pub type $cn<'a> = $crate::prelude_macro::ReadColumn<'a, super::super::in_v9::Marker, $cty>;)*
                    pub type __V9__Iter<'a> = &'a $crate::prelude_macro::IdList<super::super::in_v9::Marker>;
                    /// Read-access to the rows in a table.
                    $crate::context! {
                        pub struct __Read {
                            $(pub $cn: $cn,)*
                            pub(in super::super::super) __v9__iter: __V9__Iter,
                        }
                    }
                }
                pub use self::read::__Read as Read;
                /// Edit an individual column.
                pub mod edit {
                    #[allow(unused_imports)]
                    use super::super::super::*;
                    $(pub type $cn<'a> = $crate::prelude_macro::EditColumn<'a, super::super::in_v9::Marker, $cty>;)*
                    #[doc(hidden)]
                    pub type __V9__Iter<'a> = &'a mut $crate::prelude_macro::IdList<super::super::in_v9::Marker>;
                    $crate::context! {
                        /// Write-access to the rows in a table.
                        pub struct __Edit {
                            $(pub $cn: $cn,)*
                            #[doc(hidden)]
                            pub(in super::super::super) __v9__iter: __V9__Iter,
                        }
                    }
                }
                pub use self::edit::__Edit as Edit;
                /// Write an individual column.
                // Uhm. Why would you want this!?
                pub mod write {
                    #[allow(unused_imports)]
                    use super::super::super::*;
                    $(pub type $cn<'a> = $crate::prelude_macro::WriteColumn<'a, super::super::in_v9::Marker, $cty>;)*
                    /// Lists valid IDs.
                    pub type __V9__Iter<'a> = &'a mut $crate::prelude_macro::IdList<super::super::in_v9::Marker>;
                    $crate::context! {
                        /// Structural access to the table.
                        pub struct __Write {
                            $(pub $cn: $cn,)*
                            #[doc(hidden)]
                            pub(in super::super::super) __v9__iter: __V9__Iter,
                        }
                    }
                }
                pub use self::write::__Write as Write;
                pub use self::write::__V9__Iter as List;
            }
            pub use self::in_v9::*;
            pub use self::in_user::*;
            // These might conflict, but then at least you'd deserve it.
        }
    };
}

#[cfg(test)]
mod test {
    pub use super::*;

    table! {
        pub struct bobs {
            pub name: Name,
            pub digestion_count: u64,
        }
    }

    #[test]
    fn register() {
        let mut universe = Universe::new();
        bobs::Marker::register(&mut universe);
    }
    #[test]
    #[should_panic]
    fn double_register() {
        let mut universe = Universe::new();
        bobs::Marker::register(&mut universe);
        bobs::Marker::register(&mut universe);
    }

    #[test]
    fn basics() {
        let universe = &mut Universe::new();
        bobs::Marker::register(universe);
        universe.kmap(|mut bobs: bobs::Write| {
            bobs.reserve(3);
            bobs.push(bobs::Row {
                name: "Bob",
                digestion_count: 237,
            });
            bobs.push(bobs::Row {
                name: "Bob",
                digestion_count: 42,
            });
            bobs.push(bobs::Row {
                name: "Bob",
                digestion_count: 69,
            });
        });
        universe.kmap(|bobs: bobs::Read| {
            println!("{:?}", bobs.clone_row(bobs::FIRST));
        });
    }

    #[test]
    fn separate_col_access() {
        let universe = &mut Universe::new();
        bobs::Marker::register(universe);
        universe.kmap(|_: bobs::read::name, _: bobs::edit::digestion_count| {});
    }
}
