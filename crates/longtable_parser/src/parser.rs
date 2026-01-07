//! Main parser pipeline.
//!
//! Orchestrates the full parsing flow from raw input to command entity.

use longtable_foundation::EntityId;

use crate::command::CommandEntity;
use crate::pronouns::PronounState;
use crate::scope::CompiledScope;
use crate::vocabulary::VocabularyRegistry;

/// Result of parsing player input.
#[derive(Clone, Debug)]
pub enum ParseResult {
    /// Successfully parsed into a command
    Success(CommandEntity),
    /// Ambiguous - disambiguation needed
    Ambiguous(DisambiguationRequest),
    /// Parse error
    Error(ParseError),
}

/// A disambiguation request when multiple entities match.
#[derive(Clone, Debug)]
pub struct DisambiguationRequest {
    /// Question to ask the player
    pub question: String,
    /// Available options (description, entity)
    pub options: Vec<(String, EntityId)>,
    /// State needed to continue parsing after disambiguation
    pub pending_parse: PendingParse,
}

/// State saved when disambiguation is needed.
#[derive(Clone, Debug)]
pub struct PendingParse {
    // Internal state for continuing after disambiguation
    // Will be filled in during implementation
}

/// A parse error.
#[derive(Clone, Debug)]
pub enum ParseError {
    /// Unknown word
    UnknownWord(String),
    /// No matching syntax
    NoMatch,
    /// Entity not found
    NotFound(String),
    /// Wrong type for slot
    WrongType {
        /// The noun that was found
        noun: String,
        /// The expected type
        expected: String,
    },
    /// Pronoun has no referent
    NoReferent(String),
}

/// The main natural language parser.
#[derive(Debug)]
pub struct NaturalLanguageParser {
    vocabulary: VocabularyRegistry,
    scopes: Vec<CompiledScope>,
    pronoun_state: PronounState,
}

impl NaturalLanguageParser {
    /// Creates a new parser with the given vocabulary.
    #[must_use]
    pub fn new(vocabulary: VocabularyRegistry) -> Self {
        Self {
            vocabulary,
            scopes: Vec::new(),
            pronoun_state: PronounState::new(),
        }
    }

    /// Parses player input into a command.
    pub fn parse(
        &mut self,
        _input: &str,
        _actor: EntityId,
        // world: &World,
    ) -> ParseResult {
        // TODO: Implement full parsing pipeline
        // 1. Tokenize
        // 2. Vocabulary lookup
        // 3. Syntax matching
        // 4. Scope evaluation
        // 5. Noun resolution
        // 6. Build command entity
        ParseResult::Error(ParseError::NoMatch)
    }

    /// Continues parsing after disambiguation.
    pub fn disambiguate(&mut self, _choice: &str, _pending: PendingParse) -> ParseResult {
        // TODO: Implement disambiguation handling
        ParseResult::Error(ParseError::NoMatch)
    }

    /// Gets a reference to the vocabulary registry.
    #[must_use]
    pub fn vocabulary(&self) -> &VocabularyRegistry {
        &self.vocabulary
    }

    /// Gets a mutable reference to the vocabulary registry.
    pub fn vocabulary_mut(&mut self) -> &mut VocabularyRegistry {
        &mut self.vocabulary
    }

    /// Gets a reference to the pronoun state.
    #[must_use]
    pub fn pronoun_state(&self) -> &PronounState {
        &self.pronoun_state
    }

    /// Gets a mutable reference to the pronoun state.
    pub fn pronoun_state_mut(&mut self) -> &mut PronounState {
        &mut self.pronoun_state
    }

    /// Adds a scope definition.
    pub fn add_scope(&mut self, scope: CompiledScope) {
        self.scopes.push(scope);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_parser() {
        let vocab = VocabularyRegistry::new();
        let parser = NaturalLanguageParser::new(vocab);
        assert!(parser.scopes.is_empty());
    }
}
