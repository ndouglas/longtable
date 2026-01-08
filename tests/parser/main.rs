//! Integration tests for the longtable_parser crate.
//!
//! Tests for natural language parsing pipeline:
//! - Tokenization
//! - Vocabulary lookup
//! - Syntax matching
//! - Noun resolution
//! - Full parser pipeline
//! - Action execution

mod action_tests;
mod noun_resolution_tests;
mod parser_integration_tests;
mod syntax_matching_tests;
mod tokenizer_tests;
mod vocabulary_tests;
