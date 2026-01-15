//! Syntax pattern matching.
//!
//! Matches token streams against command syntax patterns.

use std::collections::HashMap;

use longtable_foundation::{Interner, KeywordId};

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

    /// Checks if this syntax starts with a direction slot (rather than a verb).
    #[must_use]
    pub fn starts_with_direction(&self) -> bool {
        matches!(
            self.elements.first(),
            Some(CompiledSyntaxElement::Direction { .. })
        )
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
    /// Type constraints for noun bindings (variable name -> type)
    pub type_constraints: HashMap<String, KeywordId>,
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
        interner: &Interner,
    ) -> Vec<SyntaxMatch> {
        let mut matches = Vec::new();

        // Get the first word (verb candidate)
        let verb_word = match tokens.first() {
            Some(InputToken::Word(w)) => w,
            _ => return matches,
        };

        // Look up verb in vocabulary
        let verb_kw = vocab.lookup_word(verb_word, interner);

        for syntax in syntaxes {
            // Check if syntax starts with a verb
            if let Some(syntax_verb) = syntax.verb() {
                if let Some(kw) = verb_kw {
                    if let Some(verb) = vocab.lookup_verb(kw) {
                        if verb.name == syntax_verb || verb.synonyms.contains(&syntax_verb) {
                            // Try to match this syntax
                            if let Some(m) = Self::try_match(tokens, syntax, vocab, interner) {
                                matches.push(m);
                            }
                        }
                    }
                }
            } else if syntax.starts_with_direction() {
                // Syntax starts with a direction slot (e.g., [:direction ?dir])
                // Check if first word is a recognized direction
                if let Some(kw) = verb_kw {
                    if vocab.lookup_direction(kw).is_some() {
                        // Try to match this direction-first syntax
                        if let Some(m) =
                            Self::try_match_direction_first(tokens, syntax, vocab, interner)
                        {
                            matches.push(m);
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
        interner: &Interner,
    ) -> Option<SyntaxMatch> {
        let mut token_idx = 0;
        let mut noun_bindings = HashMap::new();
        let mut type_constraints = HashMap::new();
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
                            let w_kw = vocab.lookup_word(w, interner)?;
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
                    type_constraint,
                } => {
                    // Collect noun phrase
                    let (np, consumed) =
                        Self::collect_noun_phrase(tokens, token_idx, vocab, interner)?;
                    noun_bindings.insert(var.clone(), np);
                    if let Some(tc) = type_constraint {
                        type_constraints.insert(var.clone(), *tc);
                    }
                    token_idx += consumed;
                }
                CompiledSyntaxElement::OptionalNoun {
                    var,
                    type_constraint,
                } => {
                    // Try to collect noun phrase, but don't fail if absent
                    if let Some((np, consumed)) =
                        Self::collect_noun_phrase(tokens, token_idx, vocab, interner)
                    {
                        noun_bindings.insert(var.clone(), np);
                        if let Some(tc) = type_constraint {
                            type_constraints.insert(var.clone(), *tc);
                        }
                        token_idx += consumed;
                    }
                }
                CompiledSyntaxElement::Direction { var } => {
                    // Must match direction
                    match tokens.get(token_idx) {
                        Some(InputToken::Word(w)) => {
                            let w_kw = vocab.lookup_word(w, interner)?;
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
            type_constraints,
            direction,
            prepositions,
            specificity: syntax.specificity(),
            priority: syntax.priority,
        })
    }

    /// Tries to match tokens against a direction-first syntax pattern.
    ///
    /// Direction-first patterns like `[:direction ?dir]` don't start with a verb.
    /// The first token is expected to be a direction.
    fn try_match_direction_first(
        tokens: &[InputToken],
        syntax: &CompiledSyntax,
        vocab: &VocabularyRegistry,
        interner: &Interner,
    ) -> Option<SyntaxMatch> {
        let mut token_idx = 0;
        let mut direction = None;

        for element in &syntax.elements {
            match element {
                CompiledSyntaxElement::Direction { var } => {
                    // Must match direction
                    match tokens.get(token_idx) {
                        Some(InputToken::Word(w)) => {
                            let w_kw = vocab.lookup_word(w, interner)?;
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
                // Direction-first patterns shouldn't have verbs, but handle anyway
                CompiledSyntaxElement::Verb(_) => {
                    return None;
                }
                _ => {
                    // Other elements not expected in direction-first patterns
                }
            }
        }

        // Check that we consumed all tokens (except End)
        let remaining: Vec<_> = tokens[token_idx..]
            .iter()
            .filter(|t| !matches!(t, InputToken::End))
            .collect();

        if !remaining.is_empty() {
            // Extra tokens - don't match
            return None;
        }

        Some(SyntaxMatch {
            command: syntax.command,
            action: syntax.action,
            noun_bindings: HashMap::new(),
            type_constraints: HashMap::new(),
            direction,
            prepositions: Vec::new(),
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
        interner: &Interner,
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
                    let w_kw = vocab.lookup_word(w, interner);

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

/// Compiles a syntax specification from a Value (vector) into a CompiledSyntax.
///
/// The syntax format supports:
/// - `:verb/go` - Requires verb "go" (or its synonyms)
/// - `:prep/in` - Requires preposition "in"
/// - `:direction` - Followed by a direction variable like `?dir`
/// - `?var` - Noun variable
/// - `?var:type` - Noun variable with type constraint
pub struct SyntaxCompiler;

impl SyntaxCompiler {
    /// Compiles a syntax Value into a CompiledSyntax.
    ///
    /// # Arguments
    /// * `syntax_value` - The `:syntax` field value (a vector)
    /// * `command_name` - The command name keyword
    /// * `action_name` - The action to invoke keyword
    /// * `priority` - Command priority for disambiguation
    /// * `interner` - The keyword interner
    pub fn compile(
        syntax_value: &longtable_foundation::Value,
        command_name: longtable_foundation::KeywordId,
        action_name: longtable_foundation::KeywordId,
        priority: i32,
        interner: &longtable_foundation::Interner,
    ) -> Option<CompiledSyntax> {
        let vec = syntax_value.as_vec()?;
        let mut elements = Vec::new();
        let mut expect_direction_var = false;

        for item in vec.iter() {
            match item {
                longtable_foundation::Value::Keyword(kw) => {
                    let name = interner.get_keyword(*kw)?;

                    if name == "direction" {
                        // Next element should be a direction variable
                        expect_direction_var = true;
                    } else if let Some(verb_name) = name.strip_prefix("verb/") {
                        // Verb element like :verb/go
                        let verb_kw = interner.lookup_keyword(verb_name)?;
                        elements.push(CompiledSyntaxElement::Verb(verb_kw));
                    } else if let Some(prep_name) = name.strip_prefix("prep/") {
                        // Preposition element like :prep/in
                        let prep_kw = interner.lookup_keyword(prep_name)?;
                        elements.push(CompiledSyntaxElement::Preposition(prep_kw));
                    } else if name == "all" {
                        // Special "all" literal
                        elements.push(CompiledSyntaxElement::Literal("all".to_string()));
                    }
                    // Other keywords are ignored for now
                }
                longtable_foundation::Value::Symbol(sym_id) => {
                    let sym = interner.get_symbol(*sym_id)?;
                    if let Some(var_spec) = sym.strip_prefix('?') {
                        // Variable like ?dir or ?target:thing
                        let (var_name, type_constraint) =
                            if let Some((name, type_name)) = var_spec.split_once(':') {
                                let type_kw = interner.lookup_keyword(type_name);
                                (name.to_string(), type_kw)
                            } else {
                                (var_spec.to_string(), None)
                            };

                        if expect_direction_var {
                            // This is a direction variable following :direction
                            elements.push(CompiledSyntaxElement::Direction { var: var_name });
                            expect_direction_var = false;
                        } else if type_constraint.and_then(|tc| interner.get_keyword(tc))
                            == Some("direction")
                        {
                            // Variable with :direction type constraint
                            elements.push(CompiledSyntaxElement::Direction { var: var_name });
                        } else {
                            // Regular noun variable
                            elements.push(CompiledSyntaxElement::Noun {
                                var: var_name,
                                type_constraint,
                            });
                        }
                    }
                }
                _ => {
                    // Skip other value types
                }
            }
        }

        Some(CompiledSyntax {
            command: command_name,
            action: action_name,
            elements,
            priority,
        })
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
