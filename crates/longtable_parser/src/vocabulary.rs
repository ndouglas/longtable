//! Vocabulary registry for parser vocabulary definitions.
//!
//! Stores verbs, prepositions, directions, types, commands, and other
//! vocabulary definitions at runtime.

use std::collections::{HashMap, HashSet};

use longtable_foundation::KeywordId;

/// A registered verb with its canonical name and synonyms.
#[derive(Clone, Debug)]
pub struct Verb {
    /// Canonical verb name
    pub name: KeywordId,
    /// Synonym words that map to this verb
    pub synonyms: HashSet<KeywordId>,
}

/// A registered preposition.
#[derive(Clone, Debug)]
pub struct Preposition {
    /// Preposition keyword
    pub name: KeywordId,
    /// Semantic role this preposition implies (e.g., "instrument", "destination")
    pub implies: Option<KeywordId>,
}

/// A registered direction.
#[derive(Clone, Debug)]
pub struct Direction {
    /// Canonical direction name
    pub name: KeywordId,
    /// Synonym words
    pub synonyms: HashSet<KeywordId>,
    /// Opposite direction
    pub opposite: Option<KeywordId>,
}

/// A noun type constraint.
#[derive(Clone, Debug)]
pub struct NounType {
    /// Type name
    pub name: KeywordId,
    /// Types this extends
    pub extends: Vec<KeywordId>,
    /// Pattern that entities must match (stored as compiled form later)
    pub pattern_source: String,
}

/// A command syntax definition.
#[derive(Clone, Debug)]
pub struct CommandSyntax {
    /// Command name
    pub name: KeywordId,
    /// Associated action
    pub action: KeywordId,
    /// Priority for disambiguation
    pub priority: i32,
    /// Syntax pattern elements (stored as compiled form later)
    pub syntax_source: String,
}

/// A pronoun definition.
#[derive(Clone, Debug)]
pub struct Pronoun {
    /// Pronoun word
    pub name: KeywordId,
    /// Gender (masculine, feminine, neuter)
    pub gender: PronounGender,
    /// Number (singular, plural)
    pub number: PronounNumber,
}

/// Grammatical gender.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PronounGender {
    Masculine,
    Feminine,
    Neuter,
}

/// Grammatical number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PronounNumber {
    Singular,
    Plural,
}

/// Runtime storage for all vocabulary definitions.
#[derive(Clone, Debug, Default)]
pub struct VocabularyRegistry {
    /// Verbs by canonical name
    verbs: HashMap<KeywordId, Verb>,
    /// Verb synonym -> canonical name mapping
    verb_synonyms: HashMap<KeywordId, KeywordId>,
    /// Prepositions by name
    prepositions: HashMap<KeywordId, Preposition>,
    /// Directions by canonical name
    directions: HashMap<KeywordId, Direction>,
    /// Direction synonym -> canonical name mapping
    direction_synonyms: HashMap<KeywordId, KeywordId>,
    /// Noun types by name
    types: HashMap<KeywordId, NounType>,
    /// Command syntax definitions
    commands: Vec<CommandSyntax>,
    /// Pronouns by word
    pronouns: HashMap<KeywordId, Pronoun>,
    /// Adverbs (just a set of recognized words)
    adverbs: HashSet<KeywordId>,
}

impl VocabularyRegistry {
    /// Creates a new empty vocabulary registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Looks up a word string to find its KeywordId.
    ///
    /// This requires the word to have been previously registered.
    /// Returns None if the word is not in the vocabulary.
    #[must_use]
    pub fn vocabulary_lookup_word(&self, _word: &str) -> Option<KeywordId> {
        // TODO: This needs integration with the interner
        // For now, we don't have word->keyword mapping
        // This will be implemented when we connect to the World's interner
        None
    }

    /// Registers a verb with its synonyms.
    pub fn register_verb(&mut self, verb: Verb) {
        for syn in &verb.synonyms {
            self.verb_synonyms.insert(*syn, verb.name);
        }
        self.verbs.insert(verb.name, verb);
    }

    /// Looks up a verb by word (canonical or synonym).
    #[must_use]
    pub fn lookup_verb(&self, word: KeywordId) -> Option<&Verb> {
        if let Some(verb) = self.verbs.get(&word) {
            return Some(verb);
        }
        if let Some(canonical) = self.verb_synonyms.get(&word) {
            return self.verbs.get(canonical);
        }
        None
    }

    /// Registers a preposition.
    pub fn register_preposition(&mut self, prep: Preposition) {
        self.prepositions.insert(prep.name, prep);
    }

    /// Looks up a preposition by name.
    #[must_use]
    pub fn lookup_preposition(&self, word: KeywordId) -> Option<&Preposition> {
        self.prepositions.get(&word)
    }

    /// Registers a direction with its synonyms.
    pub fn register_direction(&mut self, dir: Direction) {
        for syn in &dir.synonyms {
            self.direction_synonyms.insert(*syn, dir.name);
        }
        self.directions.insert(dir.name, dir);
    }

    /// Looks up a direction by word (canonical or synonym).
    #[must_use]
    pub fn lookup_direction(&self, word: KeywordId) -> Option<&Direction> {
        if let Some(dir) = self.directions.get(&word) {
            return Some(dir);
        }
        if let Some(canonical) = self.direction_synonyms.get(&word) {
            return self.directions.get(canonical);
        }
        None
    }

    /// Registers a noun type.
    pub fn register_type(&mut self, noun_type: NounType) {
        self.types.insert(noun_type.name, noun_type);
    }

    /// Looks up a noun type by name.
    #[must_use]
    pub fn lookup_type(&self, name: KeywordId) -> Option<&NounType> {
        self.types.get(&name)
    }

    /// Registers a command syntax.
    pub fn register_command(&mut self, cmd: CommandSyntax) {
        self.commands.push(cmd);
    }

    /// Gets all command syntaxes that use a given verb.
    #[must_use]
    pub fn commands_for_verb(&self, _verb: KeywordId) -> Vec<&CommandSyntax> {
        // For now, returns all commands - proper filtering will come later
        // when we have compiled syntax patterns
        self.commands.iter().collect()
    }

    /// Registers a pronoun.
    pub fn register_pronoun(&mut self, pronoun: Pronoun) {
        self.pronouns.insert(pronoun.name, pronoun);
    }

    /// Looks up a pronoun by word.
    #[must_use]
    pub fn lookup_pronoun(&self, word: KeywordId) -> Option<&Pronoun> {
        self.pronouns.get(&word)
    }

    /// Registers an adverb.
    pub fn register_adverb(&mut self, adverb: KeywordId) {
        self.adverbs.insert(adverb);
    }

    /// Checks if a word is a registered adverb.
    #[must_use]
    pub fn is_adverb(&self, word: KeywordId) -> bool {
        self.adverbs.contains(&word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_registry_is_empty() {
        let registry = VocabularyRegistry::new();
        assert!(registry.verbs.is_empty());
        assert!(registry.prepositions.is_empty());
        assert!(registry.directions.is_empty());
    }
}
