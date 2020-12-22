//! `v9` is a clean, easy to use, and flexible data engine.
//!
//! It provides a means to implement applications using [Data Oriented Design](http://dataorienteddesign.com/).
//!
//! ```
//! #[v9::table]
//! struct engines {
//!     pub cylinder_count: u8,
//!     pub lines_of_code: u64,
//! }
//!
//! use v9::prelude::Universe;
//! # fn f1() -> (Universe, engines::Id, engines::Id) {
//! let mut universe = Universe::new();
//!
//! use v9::prelude::Register;
//! engines::Marker::register(&mut universe);
//!
//! let (v9, v11) = universe.eval(|mut engines: engines::Write| {
//!     (
//!         engines.push(engines::Row {
//!             cylinder_count: 9,
//!             lines_of_code: 5000,
//!         }),
//!         engines.push(engines::Row {
//!             cylinder_count: 11,
//!             lines_of_code: std::u64::MAX,
//!         }),
//!     )
//! });
//! # (universe, v9, v11)
//! # }
//!
//! #[v9::table]
//! struct projects {
//!     pub name: &'static str,
//!     pub engine: crate::engines::Id,
//! }
//!
//! # fn f2((mut universe, v9, v11): (Universe, engines::Id, engines::Id)) {
//! # use v9::prelude::Register;
//! projects::Marker::register(&mut universe);
//! universe.eval(|mut projects: projects::Write| {
//!     projects.push(projects::Row {
//!         name: "TOP SECRET!",
//!         engine: v9,
//!     });
//!     projects.push(projects::Row {
//!         name: "Stinky Cheese Inc!",
//!         engine: v11,
//!     });
//! });
//!
//! universe.eval(|projects: projects::Read| {
//!     assert_eq!(projects.iter().count(), 2);
//! });
//!
//! universe.eval(|mut engines: engines::Write| {
//!     engines.remove(v11);
//! });
//!
//! universe.eval(|projects: projects::Read| {
//!     // No dangling pointers!
//!     assert_eq!(projects.iter().count(), 1);
//! });
//! # }
//! # fn main() { f2(f1()) }
//! ```
//!
//! ([Another example.](macro.decl_table.html#usage))
//!
//! # Design
//! A [`Universe`] works like a `HashMap<TypeId, Any>`.
//! A single instance of any type can be inserted into the universe.
// (...altho the TypeId key need not match the type_id of the Any...)
//! Changes can then be made by `run`ning a [`Kernel`].
//! A `Kernel` is any closure whose arguments all implement [`Extract`],
//! a trait that works like `fn extract(&Universe) -> Self`.
//!
//! [`Universe`]: object/struct.Universe.html
//! [`Extract`]: extract/trait.Extract.html
//! [`Kernel`]: kernel/struct.Kernel.html
//!
//! # Encapsulation
//! This crate makes an unreasonable amount of things public. It's very intentional!
//! It's hard to foresee all needs; hopefully you can do something useful with them,
//! and this is more honest than making things `pub` to satisfy my whims.
//!
//! A serious application should provide its own interfaces to hide `v9` behind.
//!
//! # Safety
//! ┐(ツ)┌
//!
//! My priorities are:
//! 1. A beautiful API.
//! 2. Gotta go fast:
//!     - Compile-times must be fast.
//!     - Bulk operations (via kernels) must be h*ckin' fast.
//! 3. Safety.
//!
//! Monkey-proofing is not a high priority. That said,
//! you'll probably only have trouble if you go looking for it.
//!
//! If you've tripped over something, that we'd maybe wish didn't compile, and it doesn't
//! blow up at runtime in an obvious way, then I'll be concerned.
// I interpret 'fast compiles' as:
// - minimizing the code output by macros & generics.
// - prefer dynamic dispatch to static dispatch.

#[allow(unused_imports)]
#[macro_use]
extern crate v9_attr;
#[doc(hidden)]
pub use v9_attr::*;

#[doc(hidden)]
pub extern crate paste;

// FIXME: Use UniquePtr, etc...?
// FIXME: Add universe.deny(TypeId) to allow constraints like "table is not sparse"

#[macro_use]
pub mod object;
pub mod extract;
pub mod kernel;
pub mod lock;
#[macro_use]
pub mod table;
pub mod column;
pub mod event;
pub mod id;
pub mod linkage;
pub mod property;
pub mod util;

/// A tasteful set of items.
pub mod prelude {
    pub use crate::object::{Universe, Register};
    pub use crate::table::TableMarker;
    pub use crate::id::Check as _;
}

/// Provides a single import statement for `decl_table!`.
pub mod prelude_macro {
    pub use crate::column::{Column, EditColumn, ReadColumn, WriteColumn};
    pub use crate::extract::*;
    pub use crate::id::{Check, CheckedIter, Id as IdV9, CheckedId as CheckedIdV9, IdList, IdRange, Raw, UncheckedIdRange};
    pub use crate::linkage::ForeignKey;
    pub use crate::object::{Universe, Register};
    pub use crate::property::*;
    pub use crate::table::{ColumnHeader, TableHeader, TableMarker};
    pub use std::any::TypeId;
    pub use std::fmt;
}

/// An indiscriminant selection of most things.
pub mod prelude_lib {
    pub use crate::extract::*;
    pub use crate::id::*;
    pub use crate::lock::*;
    pub use crate::object::*;
    pub use crate::prelude::*;
    pub use crate::property::*;
    pub use crate::table::{TableHeader, TableMarker};
    pub use crate::util::*;
    pub use crate::linkage::*;
    pub use std::any::{Any, TypeId, type_name};
    pub use std::cmp::Ordering;
    pub use std::marker::PhantomData;
    pub use std::ops::{Deref, DerefMut, Index, IndexMut, Range as StdRange};
    pub use std::{fmt, mem, panic};
    pub type Name = &'static str;
}
