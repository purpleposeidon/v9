//! A data engine for Data Oriented Design.
//! `crate::v9` is a vastly simpler version of `crate::v11`.
//!
//! # Design
//! A `Universe` has the same shape as a `HashMap<TypeId, Any>`.
//! A single instance of any type can be inserted into the universe.
//! Changes can then be made by `run`ning a `Kernel`.
//! A `Kernel` is any closure whose arguments all implement `Extract`.
//! `Extract` indicates to the `Universe` what resources it needs,
//! and does whatever is necessary to provide itself as an argument to the kernel.
//!
//! This crate intentionally shares more fields that you might expect.
//! It's hard to foresee all needs; hopefully you can do something useful with them.

#[macro_use]
extern crate mopa;
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
pub mod util;

pub mod prelude {
    pub use crate::object::{Universe, Register};
    pub use crate::table::TableMarker;
}

pub mod prelude_macro {
    pub use crate::column::{Column, EditColumn, ReadColumn, WriteColumn};
    pub use crate::extract::*;
    pub use crate::id::{Check, CheckedIter, Id as IdV9, IdList, IdRange, Raw, UncheckedIdRange};
    pub use crate::linkage::ForeignKey;
    pub use crate::object::{Universe, Register};
    pub use crate::table::{TableHeader, TableMarker};
    pub use std::any::TypeId;
    pub use std::fmt;
}

pub mod prelude_lib {
    pub use crate::extract::*;
    pub use crate::id::*;
    pub use crate::lock::*;
    pub use crate::object::*;
    pub use crate::prelude::*;
    pub use crate::table::{TableHeader, TableMarker};
    pub use crate::util::*;
    pub use std::any::{Any as StdAny, TypeId};
    pub use std::cmp::Ordering;
    pub use std::marker::PhantomData;
    pub use std::ops::{Deref, DerefMut, Index, IndexMut, Range as StdRange};
    pub use std::{fmt, mem, panic};
    pub type Name = &'static str;
}
