//! Natural language parser for text adventure games.
//!
//! This crate transforms player input like "take sword" or "put sword in chest"
//! into command entities that the rule engine can process.
//!
//! # Architecture
//!
//! ```text
//! "kill goblin with sword"
//!          │
//!          ▼
//! ┌─────────────────┐
//! │   TOKENIZER     │  → ["kill", "goblin", "with", "sword"]
//! └─────────────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │ VOCABULARY      │  → [Verb(:attack), Unknown, Prep(:with), Unknown]
//! │ LOOKUP          │
//! └─────────────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │ SYNTAX          │  → Matches: (command: attack [:verb ?target :with ?weapon])
//! │ MATCHING        │
//! └─────────────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │ NOUN            │  → goblin-entity, sword-entity (or AMBIGUOUS)
//! │ RESOLUTION      │
//! └─────────────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │ COMMAND         │  → Entity { :command/verb :attack, :command/target goblin, ... }
//! │ ENTITY          │
//! └─────────────────┘
//! ```
//!
//! # Modules
//!
//! - [`vocabulary`] - Runtime storage for vocabulary definitions (verbs, prepositions, etc.)
//! - [`tokenizer`] - Convert raw input to token stream
//! - [`noun_phrase`] - Noun phrase representation and resolution
//! - [`scope`] - Entity visibility for noun resolution
//! - [`syntax`] - Syntax pattern matching
//! - [`parser`] - Main parser pipeline orchestration
//! - [`pronouns`] - Pronoun tracking state
//! - [`command`] - Command entity creation
//! - [`action`] - Action registry and execution
//! - [`stdlib`] - Standard vocabulary for adventure games

pub mod action;
pub mod command;
pub mod noun_phrase;
pub mod parser;
pub mod pronouns;
pub mod scope;
pub mod stdlib;
pub mod syntax;
pub mod tokenizer;
pub mod vocabulary;

// Re-export main types for convenience
pub use action::{ActionRegistry, CompiledAction};
pub use noun_phrase::NounResolver;
pub use parser::{NaturalLanguageParser, ParseResult};
pub use syntax::{CompiledSyntax, CompiledSyntaxElement, SyntaxCompiler};
pub use vocabulary::VocabularyRegistry;
