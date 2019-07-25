use crate::prelude_lib::*;

/// Generic information about a table.
// Doesn't include len tho. :(
#[derive(Debug)]
pub struct TableHeader {
    pub name: Name,
    pub marker: TypeId,
    pub columns: Vec<ColumnHeader>,
}
impl Obj for TableHeader {}
pub trait TableMarker: 'static + Default + Copy + Send + Sync + Register + fmt::Debug {
    const NAME: Name;
    type RawId: Raw;
    fn header() -> TableHeader;
}

#[derive(Debug, Clone)]
pub struct ColumnHeader {
    pub column_type: TypeId,
    pub element_type: TypeId,
    pub name: Name,
    pub foreign_table: Option<Name>,
}

/// Defines a table. This is the most important item in the crate!
///
/// # Usage
/// This macro is called `decl_table!`, but it's nicer to use it as `#[v9::table]`. This spares you
/// some indentation, and lets IDEs do "jump to definition".
/// ```
/// // Declare a couple tables.
/// // Note the snake_casing. The table macro actually epxands this into a module.
/// #[v9::table]
/// pub struct cheeses {
///     pub quantity: f64,
///     // NOTE: You should generally use absolute paths. You may get weird errors otherwise. :(
///     pub warehouse: crate::warehouses::Id,
///     pub stinky: bool,
/// }
///
/// #[v9::table]
/// pub struct warehouses {
///     pub coordinates: (i32, i32),
///     pub on_fire: bool,
/// }
///
/// fn main() {
///     // We create a new Universe. The Universe holds everything!
///     use v9::prelude::Universe;
///     let mut universe = Universe::new();
///
///     // It doesn't know about the tables until we register them.
///     use v9::prelude::Register;
///     cheeses::Marker::register(&mut universe);
///     warehouses::Marker::register(&mut universe);
///
///     // Let's write a closure to print out the inventory.
///     let print_inventory = |cheeses: cheeses::Read, warehouses: warehouses::Read| {
///         println!("  Warehouses:");
///         for id in warehouses.iter() {
///             println!("    {:?}", warehouses.ref_row(id));
///         }
///         println!("  Cheeses:");
///         for id in cheeses.iter() {
///             println!("    {:?}", cheeses.ref_row(id));
///         }
///     };
///     // The kernel holds our closure, and keeps track of all of the arguments it requires.
///     use v9::kernel::Kernel;
///     let mut print_inventory = Kernel::new(print_inventory);
///     println!("An empty inventory:");
///     universe.run(&mut print_inventory);
///
///     // If we don't care allocation, you can use kmap. It reduces noise.
///     // Let's use it add some things:
///     universe.kmap(|mut warehouses: warehouses::Write, mut cheeses: cheeses::Write| {
///         let w0 = warehouses.push(warehouses::Row {
///             coordinates: (1, 2),
///             on_fire: true,
///         });
///         let w1 = warehouses.push(warehouses::Row {
///             coordinates: (2, 4),
///             on_fire: false,
///         });
///         let w2 = warehouses.push(warehouses::Row {
///             coordinates: (4, 2),
///             on_fire: true,
///         });
///         cheeses.reserve(30);
///         for wid in &[w0, w1, w2] {
///             for _ in 0..10 {
///                 cheeses.push(cheeses::Row {
///                     quantity: 237.0,
///                     warehouse: *wid,
///                     stinky: true,
///                 });
///             }
///         }
///     });
///     println!("A non-empty inventory:");
///     universe.run(&mut print_inventory);
///     // But what about those warehouses that are on fire?
///     universe.kmap(|list: &mut warehouses::Ids, mut on_fire: warehouses::edit::on_fire| {
///         let mut have_extinguisher = true;
///         for wid in list.removing() {
///             if on_fire[wid] {
///                 if have_extinguisher {
///                     have_extinguisher = false;
///                     on_fire[wid] = false;
///                 } else {
///                     wid.remove();
///                 }
///             }
///         }
///     });
///     // v9 has ensured data consistency.
///     // The burnt cheese has been destroyed.
///     println!("A diminished inventory:");
///     universe.run(&mut print_inventory);
///     universe.kmap(|cheeses: cheeses::Read| {
///         assert_eq!(
///             20,
///             cheeses.iter().count(),
///         );
///     });
/// }
/// ```
///
/// # Output
/// [**OUTPUT EXAMPLE**](table/example/cheeses/index.html)
///
/// # Details
///
/// There's several things to be aware of.
/// 1. Naming. The item name should be snake case (it becomes a module), and plural. The names of
///    columns should be singular, because they will be used like `students.mailing_address[student_id]`.
///    (Unless the element itself is plural, eg if `students.known_aliases[student_id]` is a `Vec<String>`.)
/// 2. The macro syntax kind of looks like a struct… but it very much is not.
/// 3. Type paths should be absolute, not relative.
/// 4. The "struct"'s visiblity may be anything, but the fields are always `pub`.
/// 5. Each column must have a unique element type. A table with columns `age: u64, income: u64`
///    *will not work*. You can wrap the structs in a newtype. (I have created the [crate
///    `new_units`](https://crates.io/crates/new_units) to help cope with this.) Or if you don't
///    care about memory access patterns you can combine the columns into a single Array Of Structs column.
///
/// # Meta-Attributes
/// There are certain meta-attributes that may be placed on the "struct". Due to `macro_rules`
/// silliness, **they must be given in the order listed here**:
/// 1. Documentation. It is placed on the generated module.
/// 2. `#[row::<meta>]`* Passes meta-attributes to the generated `struct Row`; eg `#[row::derive(serde::Serialize))]`.
///    `#[row::derive(Clone, Debug)]` is always provided. (If your type is inconvenient to clone,
///    consider wrapping it in an `Arc`, or something that panics.)
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
/// v9::decl_table! {
///     /// Some of our many fine cheeses!
///     #[row::derive(serde::Serialize)]
///     #[row::doc = "Why does our cheese keep catching on fire!??"]
///     #[raw_index(u8)]
///     pub struct cheeses {
///         pub quantity: u64,
///         /// P. U.!
///         pub stinky: bool,
///         #[serde(skip)]
///         pub on_fire: Option<bool>,
///     }
/// }
/// ```
// FIXME: Maybe I should go for a more v11-style syntax?
// FIXME: keep the stinky_cheeses example in sync or something...?
#[macro_export]
macro_rules! decl_table {
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
        $crate::decl_table! {
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
        // FIXME: `in mod $in_mod:tt`
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
                /// Table's name.
                pub const NAME: &'static str = stringify!($name);
                /// A strongly typed index into the table.
                pub type Id = IdV9<Marker>;
                /// A contiguous range of `Id`s on this table.
                pub type Range = IdRange<'static, Id>;
                /// The valid IDs. Kernels should take this by reference. Prefer using `List`.
                pub type Ids = IdList<Marker>;
                /// A 'pre-checked' index into the table. This index is known to be within the
                /// array bounds.
                pub type CheckedId<'a> = CheckedIdV9<'a, Marker>;
                /// Id 0.
                pub const FIRST: IdV9<Marker> = IdV9(0);
                /// The last possible Id.
                // FIXME: Assert that we panic if this is reached?
                pub const INVALID: IdV9<Marker> = IdV9(<$raw as Raw>::LAST);
                /// Holds static information about the table.
                #[derive(Default, Copy, Clone)]
                pub struct Marker;
                impl fmt::Debug for Marker {
                    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                        write!(f, "{}", NAME)
                    }
                }
                impl fmt::Display for Marker {
                    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                        write!(f, "{}", NAME)
                    }
                }

                /// Column names.
                pub mod names {
                    $(pub const $cn: &'static str = concat!(stringify!($table), ".", stringify!($cn));)*
                }

                impl<'a> Read<'a> {
                    pub fn len(&self) -> usize {
                        self.__v9__iter.len()
                    }
                    pub fn ids(&self) -> &Ids {
                        self.__v9__iter
                    }
                    pub fn clone_row(&self, i: impl 'a + Check<M=Marker>) -> Row {
                        self.ref_row(i).to_owned()
                    }
                    pub fn ref_row(&self, i: impl 'a + Check<M=Marker>) -> RowRef {
                        let i = self.ids().check(i);
                        RowRef {
                            $($cn: &self.$cn[i],)*
                        }
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
                    pub fn clone_row(&self, i: impl 'a + Check<M=Marker>) -> Row {
                        self.ref_row(i).to_owned()
                    }
                    pub fn ref_row(&self, i: impl 'a + Check<M=Marker>) -> RowRef {
                        // We can't actually check.
                        RowRef {
                            $($cn: &self.$cn[i],)*
                        }
                    }
                }
                impl<'a> Write<'a> {
                    pub fn len(&self) -> usize {
                        self.__v9__iter.len()
                    }
                    pub fn ids(&self) -> &Ids {
                        self.__v9__iter
                    }
                    pub fn ids_mut(&mut self) -> &mut Ids {
                        self.__v9__iter
                    }
                    pub fn reserve(&mut self, n: usize) {
                        // FIXME: Consider the free list.
                        unsafe {
                            $(self.$cn.col.get_mut().data_mut().reserve(n);)*
                        }
                    }
                    pub fn push(&mut self, row: Row) -> Id {
                        unsafe {
                            match self.__v9__iter.recycle_id() {
                                Ok(id) => {
                                    self.set_immediate(id.to_usize(), row);
                                    id
                                },
                                Err(id) => {
                                    self.push_immediate(row);
                                    id
                                },
                            }
                        }
                    }
                    pub unsafe fn push_immediate(&mut self, row: Row) {
                        $(self.$cn.col.get_mut().data_mut().push(row.$cn);)*
                    }
                    pub unsafe fn set_immediate(&mut self, i: usize, row: Row) {
                        $(
                            *self.$cn.col.get_mut().data_mut().get_unchecked_mut(i) = row.$cn;
                        )*
                    }
                    pub fn push_contiguous(&mut self, rows: impl IntoIterator<Item=Row>) -> Range {
                        use $crate::util::die::bad_iter_len;
                        let rows = rows.into_iter();
                        let n = {
                            let (min, max) = rows.size_hint();
                            if Some(min) != max {
                                bad_iter_len();
                            }
                            min
                        };
                        unsafe {
                            match self.ids_mut().recycle_id_contiguous(n) {
                                Ok(range) => {
                                    let mut id_iter = range.iter();
                                    for row in rows {
                                        let id = id_iter.next().expect($crate::util::die::BAD_ITER_LEN);
                                        self.set_immediate(id.to_usize(), row);
                                    }
                                    if id_iter.next().is_some() {
                                        bad_iter_len();
                                    }
                                    range
                                },
                                Err(range) => {
                                    for row in rows {
                                        self.push_immediate(row);
                                    }
                                    range
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
                        // FIXME: This probably needs more testing.
                        self.__v9__iter.deleting.get_mut().push(i.into());
                    }
                    pub fn iter_all(&self) -> IdRange<Id> {
                        let end = self.len();
                        IdRange::to(Id::from_usize(end))
                    }
                    pub fn iter(&self) -> CheckedIter<Marker> {
                        self.__v9__iter.iter()
                    }
                    pub fn clear(&mut self) {
                        // FIXME: Crap impl
                        let to_delete = self.iter().map(|i| i.uncheck()).collect::<Vec<_>>();
                        for id in to_delete {
                            self.remove(id);
                        }
                    }
                }
            }
            #[allow(unused_imports)]
            mod in_user {
                use super::super::*;
                // Again, we have to firewall v9 from user types.
                // Macro hygiene doesn't extend so far as `$_:ty`.
                // The compiler won't know the types unless they're in scope.

                use $crate::prelude_macro::ForeignKey as _;
                impl $crate::prelude_macro::TableMarker for super::Marker {
                    const NAME: &'static str = super::in_v9::NAME;
                    type RawId = $raw;
                    fn header() -> $crate::prelude_macro::TableHeader {
                        $crate::prelude_macro::TableHeader {
                            name: Self::NAME,
                            marker: $crate::prelude_macro::TypeId::of::<super::Marker>(),
                            columns: vec![$($crate::prelude_macro::ColumnHeader {
                                column_type: $crate::prelude_macro::TypeId::of::<self::types::$cn>(),
                                element_type: $crate::prelude_macro::TypeId::of::<self::own::$cn>(),
                                name: super::names::$cn,
                                foreign_table: {
                                    type T = $cty;
                                    T::__v9_link_foreign_table_name()
                                },
                            }),*],
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
                        $({
                            type T = $cty;
                            T::__v9_link_foreign_key::<super::Marker>(universe);
                        })*
                    }
                }

                // FIXME: Maybe we shouldn't have these by default...
                #[derive(Debug, Clone)]
                $(#[$row_meta])*
                // Doc goes *after* attributes because the user might provide their own, better,
                // documentation. No way to get rid of this
                ///
                /// An AOS row.
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
                impl<'a> RowRef<'a> {
                    #[inline]
                    pub fn to_owned(&self) -> Row {
                        Row {
                            $($cn: self.$cn.clone(),)*
                        }
                    }
                }

                /// The type of the element of a column.
                pub mod types {
                    #[allow(unused_imports)]
                    use super::super::super::*;
                    $(pub type $cn = $cty;)*
                }
                /// The type of the columns that are actually stored in the universe.
                /// You'll usually want `read::MyColumn` or `edit::MyColumn`.
                pub mod own {
                    $(pub type $cn = $crate::prelude_macro::Column<super::super::in_v9::Marker, super::types::$cn>;)*
                }
                /// Read an individual column.
                pub mod read {
                    $(pub type $cn<'a> = $crate::prelude_macro::ReadColumn<'a, super::super::in_v9::Marker, super::types::$cn>;)*
                    pub type __V9__Iter<'a> = &'a $crate::prelude_macro::IdList<super::super::in_v9::Marker>;
                    $crate::decl_context! {
                        /// Read-access to the rows in a table.
                        pub struct __Read {
                            $(pub $cn: $cn,)*
                            pub(in super::super::super) __v9__iter: __V9__Iter,
                        }
                    }
                }
                pub use self::read::__Read as Read;
                /// Edit an individual column.
                pub mod edit {
                    $(pub type $cn<'a> = $crate::prelude_macro::EditColumn<'a, super::super::in_v9::Marker, super::types::$cn>;)*
                    #[doc(hidden)]
                    $crate::decl_context! {
                        /// Modification-access to the elements of a table. This does **not** allow adding or
                        /// removing rows. Changes will be logged if necessary.
                        /// The id list can't be stored in here, so you must ask for it separately,
                        /// like `my_table_ids: &my_table::Ids`. If you are only editing one
                        /// column, you might consider `_: my_table::edit::specific_column`.
                        pub struct __Edit {
                            $(pub $cn: $cn,)*
                        }
                    }
                }
                pub use self::edit::__Edit as Edit;
                /// Write an individual column.
                pub mod write {
                    // FIXME: Why would you want this!? You could make the columns uneven!
                    // Maybe we should only make public the context?
                    // A possible use is that you might be deserializing from a SOA.
                    // However that's probably the only usage.
                    $(pub type $cn<'a> = $crate::prelude_macro::WriteColumn<'a, super::super::in_v9::Marker, super::types::$cn>;)*
                    /// Lists valid IDs.
                    pub type __V9__Iter<'a> = &'a mut $crate::prelude_macro::IdList<super::super::in_v9::Marker>;
                    $crate::decl_context! {
                        /// Structural access to the table. You can push or delete rows. However,
                        /// existing elements can not be modified.
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

    decl_table! {
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
    #[should_panic]
    fn duplicate_column_types() {
        decl_table! {
            pub struct dupes {
                pub speed: f32,
                pub scale: f32,
            }
        }
        dupes::Marker::register(&mut Universe::new());
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

// FIXME: It'd be nice to have `cfg(doc)`.
#[cfg(not(release))]
pub mod example {
    decl_table! {
        /// Our many fine cheeses!
        pub struct cheeses {
            pub quantity: f64,
            // NOTE: You should generally use absolute paths. You may get weird errors otherwise. :(
            pub stinky: bool,
        }
    }
}
