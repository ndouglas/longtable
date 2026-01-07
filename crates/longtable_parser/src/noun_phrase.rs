//! Noun phrase representation and resolution.
//!
//! Handles parsing and resolving noun phrases like "the brass lamp" or "all swords".

use longtable_foundation::{EntityId, KeywordId};

/// A parsed noun phrase.
#[derive(Clone, Debug, PartialEq)]
pub struct NounPhrase {
    /// Adjectives modifying the noun
    pub adjectives: Vec<KeywordId>,
    /// The main noun
    pub noun: KeywordId,
    /// Quantifier (the, a, all, etc.)
    pub quantifier: Quantifier,
    /// Ordinal selector (first, second, etc.)
    pub ordinal: Option<usize>,
}

/// Quantifier for noun phrases.
#[derive(Clone, Debug, PartialEq)]
pub enum Quantifier {
    /// Specific item (the sword)
    Specific,
    /// Any matching item (a sword)
    Any,
    /// All matching items
    All,
    /// All except specified entities
    AllExcept(Vec<EntityId>),
}

/// Result of noun resolution.
#[derive(Clone, Debug)]
pub enum NounResolution {
    /// Uniquely resolved to one entity
    Unique(EntityId),
    /// Multiple matches - disambiguation needed
    Ambiguous(Vec<EntityId>),
    /// No matching entity found
    NotFound,
    /// Found entity but wrong type
    WrongType {
        /// The found entity
        found: EntityId,
        /// The expected type constraint
        expected: KeywordId,
    },
}

/// Resolves noun phrases to entities in scope.
pub struct NounResolver;

impl NounResolver {
    /// Resolves a noun phrase to entities in scope.
    ///
    /// Resolution order:
    /// 1. Exact name match
    /// 2. Alias match
    /// 3. Adjective + noun match
    /// 4. Tag/component match
    #[must_use]
    pub fn resolve(
        _phrase: &NounPhrase,
        _type_constraint: Option<KeywordId>,
        _scope: &[EntityId],
        // world: &World,
        // vocab: &VocabularyRegistry,
    ) -> NounResolution {
        // TODO: Implement resolution logic
        NounResolution::NotFound
    }
}
