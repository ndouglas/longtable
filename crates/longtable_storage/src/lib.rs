//! Entity-component storage, relationships, and world state for Longtable.
//!
//! This crate will provide:
//! - `EntityStore` - Generational entity allocation
//! - `ComponentStore` - Archetype-based component storage
//! - `RelationshipStore` - Bidirectional relationship indices
//! - `World` - Immutable world state with structural sharing

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
