//! A data engine for [Data Oriented Design](http://dataorienteddesign.com/).
//! `crate:v9` is a vastly simpler version of `crate:v11`.
//!
//! # Design
//! A `Universe` works like a `HashMap<TypeId, Any>`.
//! A single instance of any type can be inserted into the universe.
// (...altho the TypeId key need not match the type_id of the Any...)
//! Changes can then be made by `run`ning a `Kernel`.
//! A `Kernel` is any closure whose arguments all implement `Extract`.
//! (The `Extract` trait works like `fn extract(&Universe) -> Self`.)
//!
//! See [`decl_table!`](macro.table.html) for an example of usage.
//!
//! # Encapsulation
//! This crate makes an unreasonable amount of things public. This is intentional!
//! An application should encapsulate `v9` behind its own interfaces.
//!
//! It's hard to foresee all needs; hopefully you can do something useful with them,
//! and this is more honest than making things `pub` to satisfy my whims.
//!
//! # Safety
//! ┐(ツ)┌
//!
//! My priorities are:
//! 1. A clean API.
//! 2. Fast compiles.
//! 3. Gotta go fast.
//! 4. Safety.
//!
//! If you've tripped over something, that we'd maybe wish didn't compile, and it doesn't
//! blow up at runtime in an obvious way, then I'll be concerned. Monkey-proofing isn't the most
//! important thing.
// I interpret 'fast compiles' as:
// - minimizing the code output by macros & generics.
// - prefer dynamic dispatch to static dispatch.

#[macro_use]
extern crate mopa;

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
    pub use std::any::{Any as StdAny, TypeId};
    pub use std::cmp::Ordering;
    pub use std::marker::PhantomData;
    pub use std::ops::{Deref, DerefMut, Index, IndexMut, Range as StdRange};
    pub use std::{fmt, mem, panic};
    pub type Name = &'static str;
}
