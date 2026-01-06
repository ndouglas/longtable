//! Entity-component storage, relationships, and world state for Longtable.
//!
//! This crate provides:
//! - [`EntityStore`] - Generational entity allocation with stale reference detection
//! - [`ComponentStore`] - Archetype-based component storage with schema validation
//! - [`RelationshipStore`] - Bidirectional relationship indices for O(1) traversal
//! - [`World`] - Immutable world state with structural sharing via persistent data structures
//!
//! All storage types are designed for immutable use - mutation methods return new instances
//! that share structure with the original via `Arc` and the `im` crate.

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
// Allow large error types - our Error has rich context
#![allow(clippy::result_large_err)]
// Allow missing error docs for now - will add comprehensive docs later
#![allow(clippy::missing_errors_doc)]

pub mod component;
pub mod entity;
pub mod relationship;
pub mod schema;
pub mod world;

// Re-export primary types at crate root
pub use component::{Archetype, ComponentStore};
pub use entity::EntityStore;
pub use relationship::RelationshipStore;
pub use schema::{
    Cardinality, ComponentSchema, FieldSchema, OnDelete, OnViolation, RelationshipSchema, Storage,
};
pub use world::World;
