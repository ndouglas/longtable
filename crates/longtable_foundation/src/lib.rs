//! Core types, values, and persistent collections for Longtable.
//!
//! This crate provides:
//! - [`Value`] - The core value type for all Longtable data
//! - [`EntityId`] - Generational entity identifiers
//! - [`Type`] - Type descriptors for schema validation
//! - [`Error`] - Rich error types with context
//! - Persistent collections ([`LtVec`], [`LtSet`], [`LtMap`])

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
