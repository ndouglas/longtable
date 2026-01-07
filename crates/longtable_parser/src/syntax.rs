//! Syntax pattern matching.
//!
//! Matches token streams against command syntax patterns.

use std::collections::HashMap;

use longtable_foundation::KeywordId;

use crate::noun_phrase::NounPhrase;
use crate::tokenizer::InputToken;
use crate::vocabulary::VocabularyRegistry;

/// A compiled syntax element.
#[derive(Clone, Debug)]
pub enum CompiledSyntaxElement {
    /// The verb position (always first)
    Verb,
    /// A literal word that must appear
    Literal(KeywordId),
    /// A noun slot with variable binding
    Noun {
        /// Variable name
        var: String,
        /// Type constraint
        type_constraint: Option<KeywordId>,
    },
    /// An optional noun slot
    OptionalNoun {
        /// Variable name
        var: String,
        /// Type constraint
        type_constraint: Option<KeywordId>,
    },
    /// A direction slot
    Direction {
        /// Variable name
        var: String,
    },
    /// A preposition that must appear
    Preposition(KeywordId),
}

/// A successful syntax match.
#[derive(Clone, Debug)]
pub struct SyntaxMatch {
    /// The matched command
    pub command: KeywordId,
    /// The action to invoke
    pub action: KeywordId,
    /// Noun bindings (variable name -> noun phrase)
    pub noun_bindings: HashMap<String, NounPhrase>,
    /// Prepositions that appeared
    pub prepositions: Vec<KeywordId>,
    /// Match specificity (higher = more specific)
    pub specificity: usize,
}

/// Matches token streams against syntax patterns.
pub struct SyntaxMatcher;

impl SyntaxMatcher {
    /// Attempts to match tokens against all registered syntax patterns.
    ///
    /// Returns all matching patterns, sorted by specificity (highest first).
    #[must_use]
    pub fn match_all(_tokens: &[InputToken], _vocab: &VocabularyRegistry) -> Vec<SyntaxMatch> {
        // TODO: Implement syntax matching
        Vec::new()
    }
}
