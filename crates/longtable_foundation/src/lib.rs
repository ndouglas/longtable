//! Core types, values, and persistent collections for Longtable.
//!
//! This crate provides:
//! - [`Value`] - The core value type for all Longtable data
//! - [`EntityId`] - Generational entity identifiers
//! - [`Type`] - Type descriptors for schema validation
//! - [`Error`] - Rich error types with context
//! - Persistent collections ([`LtVec`], [`LtSet`], [`LtMap`])
//! - String interning ([`SymbolId`], [`KeywordId`], [`Interner`])

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]

pub mod collections;
pub mod entity;
pub mod error;
pub mod intern;
pub mod types;
pub mod value;

// Re-export primary types at crate root for convenience
pub use collections::{LtMap, LtSet, LtVec};
pub use entity::EntityId;
pub use error::{Error, ErrorContext, ErrorKind, SemanticLimit};
pub use intern::{Interner, KeywordId, SymbolId};
pub use types::{Arity, Type};
pub use value::{CompiledFn, LtFn, NativeFn, Value};

/// Result type alias using the crate's Error type.
pub type Result<T> = std::result::Result<T, Error>;
