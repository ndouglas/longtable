//! Semantic declarations extracted from parsed AST.
//!
//! This module transforms raw AST (lists, vectors, symbols) into typed
//! declaration structures (Rules, Patterns, Components, etc.).
//!
//! The flow is: Source → Parser → AST → `DeclarationAnalyzer` → Declaration → Compiler
//!
//! # Module Structure
//!
//! - `types` - All declaration type definitions
//! - `analyzer` - The `DeclarationAnalyzer` implementation

mod analyzer;
mod types;

#[cfg(test)]
mod tests;

// Re-export types
pub use types::{
    Cardinality, ComponentDecl, ConstraintDecl, ConstraintViolation, DerivedDecl, FieldDecl,
    LinkDecl, OnTargetDelete, OnViolation, OrderDirection, Pattern, PatternClause, PatternValue,
    QueryDecl, RelationshipDecl, RuleDecl, SpawnDecl, StorageKind,
};

// Re-export analyzer
pub use analyzer::DeclarationAnalyzer;

use crate::namespace::{LoadDecl, NamespaceDecl};

/// Any top-level declaration.
#[derive(Clone, Debug, PartialEq)]
pub enum Declaration {
    /// A component schema declaration.
    Component(ComponentDecl),
    /// A relationship declaration.
    Relationship(RelationshipDecl),
    /// A rule declaration.
    Rule(RuleDecl),
    /// A derived component declaration.
    Derived(DerivedDecl),
    /// A constraint declaration.
    Constraint(ConstraintDecl),
    /// A query expression.
    Query(QueryDecl),
    /// A namespace declaration.
    Namespace(NamespaceDecl),
    /// A load directive.
    Load(LoadDecl),
    /// A spawn declaration (create entity).
    Spawn(SpawnDecl),
    /// A link declaration (create relationship).
    Link(LinkDecl),
}
