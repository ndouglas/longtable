//! Main parser pipeline.
//!
//! Orchestrates the full parsing flow from raw input to command entity.

use std::collections::HashMap;

use longtable_foundation::EntityId;
use longtable_storage::World;

use crate::command::CommandEntity;
use crate::noun_phrase::{NounPhrase, NounResolution, NounResolver, Quantifier};
use crate::pronouns::PronounState;
use crate::scope::{CompiledScope, ScopeEvaluator};
use crate::syntax::{CompiledSyntax, SyntaxMatch, SyntaxMatcher};
use crate::tokenizer::{InputToken, InputTokenizer};
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
    /// Multiple commands (for "all" quantifier)
    Multiple(Vec<CommandEntity>),
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
    /// The original input
    pub input: String,
    /// The syntax match that needs disambiguation
    pub syntax_match: SyntaxMatch,
    /// Variable name that needs disambiguation
    pub var_name: String,
    /// Actor who issued the command
    pub actor: EntityId,
}

/// A parse error.
#[derive(Clone, Debug)]
pub enum ParseError {
    /// Unknown word
    UnknownWord(String),
    /// No matching syntax
    NoMatch,
    /// Empty input
    EmptyInput,
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
    syntaxes: Vec<CompiledSyntax>,
    scopes: Vec<CompiledScope>,
    scope_evaluator: Option<ScopeEvaluator>,
    noun_resolver: Option<NounResolver>,
    pronoun_state: PronounState,
}

impl NaturalLanguageParser {
    /// Creates a new parser with the given vocabulary.
    #[must_use]
    pub fn new(vocabulary: VocabularyRegistry) -> Self {
        Self {
            vocabulary,
            syntaxes: Vec::new(),
            scopes: Vec::new(),
            scope_evaluator: None,
            noun_resolver: None,
            pronoun_state: PronounState::new(),
        }
    }

    /// Configures the scope evaluator with the necessary keywords.
    pub fn with_scope_evaluator(mut self, evaluator: ScopeEvaluator) -> Self {
        self.scope_evaluator = Some(evaluator);
        self
    }

    /// Configures the noun resolver with the necessary keywords.
    pub fn with_noun_resolver(mut self, resolver: NounResolver) -> Self {
        self.noun_resolver = Some(resolver);
        self
    }

    /// Adds a compiled syntax pattern.
    pub fn add_syntax(&mut self, syntax: CompiledSyntax) {
        self.syntaxes.push(syntax);
    }

    /// Adds a scope definition.
    pub fn add_scope(&mut self, scope: CompiledScope) {
        self.scopes.push(scope);
    }

    /// Parses player input into a command.
    pub fn parse(&mut self, input: &str, actor: EntityId, world: &World) -> ParseResult {
        // 1. Tokenize
        let tokens = InputTokenizer::tokenize(input);

        if tokens.iter().all(|t| matches!(t, InputToken::End)) {
            return ParseResult::Error(ParseError::EmptyInput);
        }

        // 2. Try to match syntax patterns
        let matches =
            SyntaxMatcher::match_all(&tokens, &self.syntaxes, &self.vocabulary, world.interner());

        if matches.is_empty() {
            return ParseResult::Error(ParseError::NoMatch);
        }

        // 3. Use the best match (highest specificity/priority)
        let syntax_match = matches.into_iter().next().unwrap();

        // 4. Get visible entities for noun resolution
        let scope = self.get_scope(actor, world);

        // 5. Resolve all noun bindings
        self.resolve_nouns(input, syntax_match, actor, &scope, world)
    }

    /// Gets entities in scope for the actor.
    fn get_scope(&self, actor: EntityId, world: &World) -> Vec<EntityId> {
        if let Some(evaluator) = &self.scope_evaluator {
            evaluator.visible_entities(actor, world, &self.scopes)
        } else {
            // Fallback: return all entities
            world.entities().collect()
        }
    }

    /// Resolves noun bindings and creates command entity.
    fn resolve_nouns(
        &mut self,
        input: &str,
        syntax_match: SyntaxMatch,
        actor: EntityId,
        scope: &[EntityId],
        world: &World,
    ) -> ParseResult {
        let resolver = match &self.noun_resolver {
            Some(r) => r,
            None => {
                // No resolver configured - just create command without resolution
                return self.create_command_without_resolution(syntax_match, actor);
            }
        };

        let mut resolved_bindings: HashMap<String, EntityId> = HashMap::new();
        let mut multiple_commands = Vec::new();

        for (var_name, noun_phrase) in &syntax_match.noun_bindings {
            // Check for pronouns
            if self.is_pronoun(&noun_phrase.noun) {
                if let Some(entity) = self.resolve_pronoun(&noun_phrase.noun) {
                    resolved_bindings.insert(var_name.clone(), entity);
                    continue;
                } else {
                    return ParseResult::Error(ParseError::NoReferent(noun_phrase.noun.clone()));
                }
            }

            // Get type constraint from syntax match
            let type_constraint = syntax_match.type_constraints.get(var_name).copied();

            let resolution =
                resolver.resolve(noun_phrase, type_constraint, scope, world, &self.vocabulary);

            match resolution {
                NounResolution::Unique(entity) => {
                    resolved_bindings.insert(var_name.clone(), entity);
                    // Update pronoun state
                    self.pronoun_state.set_it(entity);
                }
                NounResolution::Ambiguous(entities) => {
                    // Need disambiguation
                    let options = entities
                        .iter()
                        .map(|&e| (resolver.describe(e, world), e))
                        .collect();

                    return ParseResult::Ambiguous(DisambiguationRequest {
                        question: format!("Which {} do you mean?", noun_phrase.noun),
                        options,
                        pending_parse: PendingParse {
                            input: input.to_string(),
                            syntax_match: syntax_match.clone(),
                            var_name: var_name.clone(),
                            actor,
                        },
                    });
                }
                NounResolution::NotFound => {
                    return ParseResult::Error(ParseError::NotFound(noun_phrase.noun.clone()));
                }
                NounResolution::WrongType { expected, .. } => {
                    return ParseResult::Error(ParseError::WrongType {
                        noun: noun_phrase.noun.clone(),
                        expected,
                    });
                }
                NounResolution::Multiple(entities) => {
                    // For "all" quantifier - create multiple commands
                    if matches!(
                        noun_phrase.quantifier,
                        Quantifier::All | Quantifier::AllExcept(_)
                    ) {
                        // Update "them" pronoun first (before moving entities)
                        self.pronoun_state.set_them(entities.clone());

                        for entity in entities {
                            let mut bindings = resolved_bindings.clone();
                            bindings.insert(var_name.clone(), entity);
                            multiple_commands.push(CommandEntity {
                                verb: syntax_match.command,
                                action: syntax_match.action,
                                actor,
                                noun_bindings: bindings,
                                direction: syntax_match.direction.clone(),
                                adverb: None,
                            });
                        }
                    }
                }
            }
        }

        // Return result
        if !multiple_commands.is_empty() {
            return ParseResult::Multiple(multiple_commands);
        }

        ParseResult::Success(CommandEntity {
            verb: syntax_match.command,
            action: syntax_match.action,
            actor,
            noun_bindings: resolved_bindings,
            direction: syntax_match.direction,
            adverb: None,
        })
    }

    /// Creates a command without noun resolution (fallback).
    fn create_command_without_resolution(
        &self,
        syntax_match: SyntaxMatch,
        actor: EntityId,
    ) -> ParseResult {
        ParseResult::Success(CommandEntity {
            verb: syntax_match.command,
            action: syntax_match.action,
            actor,
            noun_bindings: HashMap::new(),
            direction: syntax_match.direction,
            adverb: None,
        })
    }

    /// Checks if a word is a pronoun.
    fn is_pronoun(&self, word: &str) -> bool {
        let lower = word.to_lowercase();
        matches!(lower.as_str(), "it" | "him" | "her" | "them")
    }

    /// Resolves a pronoun to its referent.
    fn resolve_pronoun(&self, word: &str) -> Option<EntityId> {
        let lower = word.to_lowercase();
        match lower.as_str() {
            "it" => self.pronoun_state.get_it(),
            "him" => self.pronoun_state.get_him(),
            "her" => self.pronoun_state.get_her(),
            "them" => self.pronoun_state.get_them().first().copied(),
            _ => None,
        }
    }

    /// Continues parsing after disambiguation.
    pub fn disambiguate(
        &mut self,
        choice: &str,
        pending: PendingParse,
        world: &World,
    ) -> ParseResult {
        let resolver = match &self.noun_resolver {
            Some(r) => r,
            None => return ParseResult::Error(ParseError::NoMatch),
        };

        // Try to parse the choice as a number (1, 2, etc.)
        if let Ok(idx) = choice.parse::<usize>() {
            // Find by index in the original options
            let scope = self.get_scope(pending.actor, world);
            let noun_phrase = pending.syntax_match.noun_bindings.get(&pending.var_name);

            if let Some(np) = noun_phrase {
                let resolution = resolver.resolve(np, None, &scope, world, &self.vocabulary);
                if let NounResolution::Ambiguous(entities) = resolution {
                    if idx > 0 && idx <= entities.len() {
                        let chosen = entities[idx - 1];

                        // Create the command with the chosen entity
                        let mut bindings: HashMap<String, EntityId> = HashMap::new();
                        bindings.insert(pending.var_name, chosen);

                        self.pronoun_state.set_it(chosen);

                        return ParseResult::Success(CommandEntity {
                            verb: pending.syntax_match.command,
                            action: pending.syntax_match.action,
                            actor: pending.actor,
                            noun_bindings: bindings,
                            direction: pending.syntax_match.direction.clone(),
                            adverb: None,
                        });
                    }
                }
            }
        }

        // Try to match the choice as a noun phrase
        let new_np = NounPhrase::new(choice);
        let scope = self.get_scope(pending.actor, world);
        let resolution = resolver.resolve(&new_np, None, &scope, world, &self.vocabulary);

        match resolution {
            NounResolution::Unique(entity) => {
                let mut bindings: HashMap<String, EntityId> = HashMap::new();
                bindings.insert(pending.var_name, entity);

                self.pronoun_state.set_it(entity);

                ParseResult::Success(CommandEntity {
                    verb: pending.syntax_match.command,
                    action: pending.syntax_match.action,
                    actor: pending.actor,
                    noun_bindings: bindings,
                    direction: pending.syntax_match.direction.clone(),
                    adverb: None,
                })
            }
            NounResolution::NotFound => {
                ParseResult::Error(ParseError::NotFound(choice.to_string()))
            }
            _ => ParseResult::Error(ParseError::NoMatch),
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_parser() {
        let vocab = VocabularyRegistry::new();
        let parser = NaturalLanguageParser::new(vocab);
        assert!(parser.syntaxes.is_empty());
        assert!(parser.scopes.is_empty());
    }

    #[test]
    fn test_parse_empty_input() {
        let vocab = VocabularyRegistry::new();
        let mut parser = NaturalLanguageParser::new(vocab);
        let world = World::new(42);
        let actor = EntityId::new(1, 0);

        let result = parser.parse("", actor, &world);
        assert!(matches!(result, ParseResult::Error(ParseError::EmptyInput)));
    }
}
