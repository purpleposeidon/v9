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
pub trait TableMarker: Default + Copy + 'static {
    const NAME: Name;
    type RawId: Raw;
    fn header() -> TableHeader;
    fn register(universe: &mut Universe);
}

/// Defines a table. This is the most important item in the crate!
///
/// # Usage
/// ```
/// // Declare a couple tables.
/// v9::table! {
///     #[raw_index(u64)]
///     pub struct cheeses {
///         pub quantity: f64,
///         pub warehouse: crate::warehouses::RowId,
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
///     use v9::prelude::TableMarker;
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
///         for wid in list.removing(&on_fire) {
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
/// 1. Naming. The item name should be lowercase (it becomes a module), and plural. The columns
///    should be singular. (Unless it should be, like in `pub aliases: Vec<String>`.)
/// 2. The macro syntax kind of looks like a struct… but it very much is not.
/// 3. Type paths should be absolute, not relative.
/// 4. The struct's visiblity may be anything, but the field's visiblity must be `pub`.
// FIXME: Maybe I should go for a more v11-style syntax?
// FIXME: keep the stinky_cheeses example in sync or something...?
#[macro_export]
macro_rules! table {
    (@first $x:tt $($_xs:tt)*) => { $x };
    (
        $vis:vis struct $name:ident {
            $(pub $cn:ident: $cty:ty,)*
        }
    ) => {
        $crate::table! {
            #[raw_index(u32)]
            $vis struct $name {
                $(pub $cn: $cty,)*
            }
        }
    };
    (
        #[raw_index($raw:ty)]
        $vis:vis struct $name:ident {
            $(pub $cn:ident: $cty:ty,)*
        }
    ) => {
        #[allow(non_camel_case_types, dead_code, non_upper_case_globals, non_snake_case)]
        $vis mod $name {
            #[allow(unused_imports)]
            use super::*;
            use $crate::prelude_macro::*;

            pub const NAME: Name = stringify!($name);
            pub type RowId = IdV9<Marker>;
            pub type Ids = IdList<Marker>;
            pub const FIRST: IdV9<Marker> = IdV9(0);
            pub const INVALID: IdV9<Marker> = IdV9(<$raw as Raw>::LAST);
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
            impl $crate::table::TableMarker for Marker {
                const NAME: Name = NAME;
                type RawId = $raw;
                fn header() -> TableHeader {
                    TableHeader {
                        name: NAME,
                        marker: TypeId::of::<Marker>(),
                        columns: vec![
                            $(TypeId::of::<$cty>()),*
                        ],
                    }
                }
                fn register(universe: &mut Universe) {
                    universe.add_mut(TypeId::of::<Marker>(), Self::header());
                    universe.add_mut(TypeId::of::<IdList<Marker>>(), IdList::<Marker>::default());
                    // Interesting that we can't have duplicate types, hmm?
                    $(universe.add_mut(TypeId::of::<Column<Marker, $cty>>(), Column::<Marker, $cty>::new());)*
                    $({
                        type T = $cty;
                        T::__v9_link_foreign_key::<Marker>(universe);
                    })*
                }
            }

            #[derive(Debug, Clone)]
            pub struct Row {
                $(pub $cn: $cty,)*
            }
            #[derive(Debug, Clone)]
            pub struct RowRef<'a> {
                $(pub $cn: &'a $cty,)*
            }
            pub mod names {
                $(pub const $cn: &'static str = concat!(stringify!($table), ".", stringify!($cn));)*
            }
            pub mod read {
                #[allow(unused_imports)]
                use super::super::*;
                $(pub type $cn<'a> = super::ReadColumn<'a, super::Marker, $cty>;)*
                pub type __V9__Iter<'a> = &'a super::IdList<super::Marker>;
                $crate::context! {
                    pub struct __Read {
                        $(pub $cn: $cn,)*
                        pub(in super::super) __v9__iter: __V9__Iter,
                    }
                }
            }
            pub use self::read::__Read as Read;
            pub mod edit {
                #[allow(unused_imports)]
                use super::super::*;
                $(pub type $cn<'a> = super::EditColumn<'a, super::Marker, $cty>;)*
                pub type __V9__Iter<'a> = &'a mut super::IdList<super::Marker>;
                $crate::context! {
                    pub struct __Edit {
                        $(pub $cn: $cn,)*
                        pub(in super::super) __v9__iter: __V9__Iter,
                    }
                }
            }
            pub use self::edit::__Edit as Edit;
            pub mod write {
                #[allow(unused_imports)]
                use super::super::*;
                $(pub type $cn<'a> = super::WriteColumn<'a, super::Marker, $cty>;)*
                pub type __V9__Iter<'a> = &'a mut super::IdList<super::Marker>;
                $crate::context! {
                    pub struct __Write {
                        $(pub $cn: $cn,)*
                        pub(in super::super) __v9__iter: __V9__Iter,
                    }
                }
            }
            pub use self::write::__Write as Write;
            pub use self::write::__V9__Iter as List;

            impl<'a> Read<'a> {
                pub fn clone_row(&self, i: impl Check<'a, M=Marker>) -> Row {
                    Row {
                        $($cn: self.$cn[i].clone(),)*
                    }
                }

                pub fn ref_row(&self, i: impl Check<'a, M=Marker>) -> RowRef {
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
                    IdRange::to(RowId::from_usize(end))
                }

                pub fn iter(&self) -> CheckedIter<Marker> {
                    unsafe {
                        self.__v9__iter.iter_by_len(self.len())
                    }
                }
            }
            impl<'a> Edit<'a> {
                pub fn len(&self) -> usize {
                    $crate::table! {@first $({
                        self.$cn.col.data.len()
                    })*}
                }

                pub fn iter_all(&self) -> IdRange<RowId> {
                    let end = self.len();
                    IdRange::to(RowId::from_usize(end))
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
                pub fn push(&mut self, row: Row) -> RowId {
                    let i = self.len();
                    unsafe {
                        $(self.$cn.col.get_mut().data.push(row.$cn);)*
                    }
                    RowId::from_usize(i)
                }
                pub fn borrow(&self) -> Read {
                    Read {
                        $($cn: self.$cn.borrow(),)*
                        __v9__iter: self.__v9__iter, // FIXME: Dum name
                    }
                }
                pub fn remove(&mut self, i: impl Into<RowId>) {
                    self.__v9__iter.deleting.get_mut().push(i.into());
                }
                pub fn iter_all(&self) -> IdRange<RowId> {
                    let end = self.len();
                    IdRange::to(RowId::from_usize(end))
                }
                pub fn iter(&self) -> CheckedIter<Marker> {
                    unsafe {
                        self.__v9__iter.iter_by_len(self.len())
                    }
                }
            }
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
