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
    ActionDecl, AdverbDecl, Cardinality, CommandDecl, ComponentDecl, ConstraintDecl,
    ConstraintViolation, DerivedDecl, DirectionDecl, FieldDecl, LinkDecl, NounTypeDecl,
    OnTargetDelete, OnViolation, OrderDirection, Pattern, PatternClause, PatternValue,
    Precondition, PrepositionDecl, PronounDecl, PronounGender, PronounNumber, QueryDecl,
    RelationshipDecl, RuleDecl, ScopeDecl, SpawnDecl, StorageKind, SyntaxElement, VerbDecl,
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
    /// A verb declaration (parser vocabulary).
    Verb(VerbDecl),
    /// A preposition declaration (parser vocabulary).
    Preposition(PrepositionDecl),
    /// A direction declaration (parser vocabulary).
    Direction(DirectionDecl),
    /// A noun type declaration (parser vocabulary).
    NounType(NounTypeDecl),
    /// A command declaration (parser syntax).
    Command(CommandDecl),
    /// An action declaration (parser behavior).
    Action(ActionDecl),
    /// A pronoun declaration (parser vocabulary).
    Pronoun(PronounDecl),
    /// A scope declaration (parser visibility).
    Scope(ScopeDecl),
    /// An adverb declaration (parser vocabulary).
    Adverb(AdverbDecl),
}
