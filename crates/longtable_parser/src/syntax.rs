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
    Verb(KeywordId),
    /// A literal word that must appear
    Literal(String),
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

/// A compiled command syntax pattern.
#[derive(Clone, Debug)]
pub struct CompiledSyntax {
    /// Command name
    pub command: KeywordId,
    /// Action to invoke
    pub action: KeywordId,
    /// Syntax elements
    pub elements: Vec<CompiledSyntaxElement>,
    /// Priority (higher = matched first)
    pub priority: i32,
}

impl CompiledSyntax {
    /// Gets the verb this syntax starts with, if any.
    #[must_use]
    pub fn verb(&self) -> Option<KeywordId> {
        self.elements.first().and_then(|e| {
            if let CompiledSyntaxElement::Verb(v) = e {
                Some(*v)
            } else {
                None
            }
        })
    }

    /// Calculates specificity score (more elements = more specific).
    #[must_use]
    pub fn specificity(&self) -> usize {
        self.elements
            .iter()
            .filter(|e| !matches!(e, CompiledSyntaxElement::OptionalNoun { .. }))
            .count()
    }
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
    /// Direction binding, if any
    pub direction: Option<(String, KeywordId)>,
    /// Prepositions that appeared
    pub prepositions: Vec<KeywordId>,
    /// Match specificity (higher = more specific)
    pub specificity: usize,
    /// Priority from command definition
    pub priority: i32,
}

/// Matches token streams against syntax patterns.
pub struct SyntaxMatcher;

impl SyntaxMatcher {
    /// Attempts to match tokens against all registered syntax patterns.
    ///
    /// Returns all matching patterns, sorted by specificity (highest first).
    #[must_use]
    pub fn match_all(
        tokens: &[InputToken],
        syntaxes: &[CompiledSyntax],
        vocab: &VocabularyRegistry,
    ) -> Vec<SyntaxMatch> {
        let mut matches = Vec::new();

        // Get the first word (verb candidate)
        let verb_word = match tokens.first() {
            Some(InputToken::Word(w)) => w,
            _ => return matches,
        };

        // Look up verb in vocabulary
        let verb_kw = vocab.vocabulary_lookup_word(verb_word);

        for syntax in syntaxes {
            // Check if syntax verb matches
            if let Some(syntax_verb) = syntax.verb() {
                if let Some(kw) = verb_kw {
                    if let Some(verb) = vocab.lookup_verb(kw) {
                        if verb.name == syntax_verb || verb.synonyms.contains(&syntax_verb) {
                            // Try to match this syntax
                            if let Some(m) = Self::try_match(tokens, syntax, vocab) {
                                matches.push(m);
                            }
                        }
                    }
                }
            }
        }

        // Sort by specificity then priority (highest first)
        matches.sort_by(|a, b| {
            b.specificity
                .cmp(&a.specificity)
                .then_with(|| b.priority.cmp(&a.priority))
        });

        matches
    }

    /// Tries to match tokens against a single syntax pattern.
    fn try_match(
        tokens: &[InputToken],
        syntax: &CompiledSyntax,
        vocab: &VocabularyRegistry,
    ) -> Option<SyntaxMatch> {
        let mut token_idx = 0;
        let mut noun_bindings = HashMap::new();
        let mut direction = None;
        let mut prepositions = Vec::new();

        for element in &syntax.elements {
            match element {
                CompiledSyntaxElement::Verb(_) => {
                    // Verb already matched at call site, consume it
                    token_idx += 1;
                }
                CompiledSyntaxElement::Literal(word) => {
                    // Must match literal word
                    match tokens.get(token_idx) {
                        Some(InputToken::Word(w)) if w.to_lowercase() == word.to_lowercase() => {
                            token_idx += 1;
                        }
                        _ => return None,
                    }
                }
                CompiledSyntaxElement::Preposition(prep_kw) => {
                    // Must match preposition
                    match tokens.get(token_idx) {
                        Some(InputToken::Word(w)) => {
                            let w_kw = vocab.vocabulary_lookup_word(w)?;
                            if let Some(prep) = vocab.lookup_preposition(w_kw) {
                                if prep.name == *prep_kw {
                                    prepositions.push(*prep_kw);
                                    token_idx += 1;
                                } else {
                                    return None;
                                }
                            } else {
                                return None;
                            }
                        }
                        _ => return None,
                    }
                }
                CompiledSyntaxElement::Noun {
                    var,
                    type_constraint: _,
                } => {
                    // Collect noun phrase
                    let (np, consumed) = Self::collect_noun_phrase(tokens, token_idx, vocab)?;
                    noun_bindings.insert(var.clone(), np);
                    token_idx += consumed;
                }
                CompiledSyntaxElement::OptionalNoun {
                    var,
                    type_constraint: _,
                } => {
                    // Try to collect noun phrase, but don't fail if absent
                    if let Some((np, consumed)) =
                        Self::collect_noun_phrase(tokens, token_idx, vocab)
                    {
                        noun_bindings.insert(var.clone(), np);
                        token_idx += consumed;
                    }
                }
                CompiledSyntaxElement::Direction { var } => {
                    // Must match direction
                    match tokens.get(token_idx) {
                        Some(InputToken::Word(w)) => {
                            let w_kw = vocab.vocabulary_lookup_word(w)?;
                            if let Some(dir) = vocab.lookup_direction(w_kw) {
                                direction = Some((var.clone(), dir.name));
                                token_idx += 1;
                            } else {
                                return None;
                            }
                        }
                        _ => return None,
                    }
                }
            }
        }

        // Check that we consumed all tokens (except End)
        let remaining: Vec<_> = tokens[token_idx..]
            .iter()
            .filter(|t| !matches!(t, InputToken::End))
            .collect();

        if !remaining.is_empty() {
            // Extra tokens - might not match
            // For now, allow trailing tokens (could be adverbs, etc.)
        }

        Some(SyntaxMatch {
            command: syntax.command,
            action: syntax.action,
            noun_bindings,
            direction,
            prepositions,
            specificity: syntax.specificity(),
            priority: syntax.priority,
        })
    }

    /// Collects a noun phrase from the token stream.
    ///
    /// Returns the noun phrase and number of tokens consumed.
    fn collect_noun_phrase(
        tokens: &[InputToken],
        start: usize,
        vocab: &VocabularyRegistry,
    ) -> Option<(NounPhrase, usize)> {
        let mut idx = start;
        let mut adjectives = Vec::new();
        let mut noun = None;

        // Skip articles
        if let Some(InputToken::Word(w)) = tokens.get(idx) {
            let lower = w.to_lowercase();
            if lower == "the" || lower == "a" || lower == "an" {
                idx += 1;
            }
        }

        // Collect words until we hit a preposition, end, or direction
        while let Some(token) = tokens.get(idx) {
            match token {
                InputToken::Word(w) => {
                    let w_kw = vocab.vocabulary_lookup_word(w);

                    // Stop if we hit a preposition
                    if let Some(kw) = w_kw {
                        if vocab.lookup_preposition(kw).is_some() {
                            break;
                        }
                        // Stop if we hit a direction
                        if vocab.lookup_direction(kw).is_some() {
                            break;
                        }
                    }

                    // This is either an adjective or the noun
                    // For now, assume last word is noun, others are adjectives
                    if let Some(prev_noun) = noun.take() {
                        adjectives.push(prev_noun);
                    }
                    noun = Some(w.clone());
                    idx += 1;
                }
                InputToken::QuotedString(s) => {
                    // Quoted string is the entire noun
                    noun = Some(s.clone());
                    idx += 1;
                    break;
                }
                InputToken::End => break,
            }
        }

        let noun = noun?;
        let consumed = idx - start;

        if consumed == 0 {
            return None;
        }

        Some((
            NounPhrase {
                adjectives,
                noun,
                quantifier: crate::noun_phrase::Quantifier::Specific,
                ordinal: None,
            },
            consumed,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compiled_syntax_specificity() {
        // Use reserved keywords for testing (these are always valid)
        let syntax = CompiledSyntax {
            command: KeywordId::REL_TYPE,
            action: KeywordId::REL_SOURCE,
            elements: vec![
                CompiledSyntaxElement::Verb(KeywordId::REL_TARGET),
                CompiledSyntaxElement::Noun {
                    var: "obj".to_string(),
                    type_constraint: None,
                },
            ],
            priority: 0,
        };

        assert_eq!(syntax.specificity(), 2);
    }

    #[test]
    fn test_compiled_syntax_verb() {
        let verb_kw = KeywordId::VALUE;
        let syntax = CompiledSyntax {
            command: KeywordId::REL_TYPE,
            action: KeywordId::REL_SOURCE,
            elements: vec![CompiledSyntaxElement::Verb(verb_kw)],
            priority: 0,
        };

        assert_eq!(syntax.verb(), Some(verb_kw));
    }
}
