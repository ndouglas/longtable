//! Tokenizer tests.
//!
//! Tests for converting raw input to token streams.

use longtable_parser::tokenizer::{InputToken, InputTokenizer};

#[test]
fn tokenize_simple_command() {
    let tokens = InputTokenizer::tokenize("take sword");

    assert_eq!(tokens.len(), 3); // "take", "sword", End
    assert!(matches!(&tokens[0], InputToken::Word(w) if w == "take"));
    assert!(matches!(&tokens[1], InputToken::Word(w) if w == "sword"));
    assert!(matches!(&tokens[2], InputToken::End));
}

#[test]
fn tokenize_preserves_case_lowered() {
    let tokens = InputTokenizer::tokenize("TAKE SWORD");

    assert!(matches!(&tokens[0], InputToken::Word(w) if w == "take"));
    assert!(matches!(&tokens[1], InputToken::Word(w) if w == "sword"));
}

#[test]
fn tokenize_strips_punctuation() {
    let tokens = InputTokenizer::tokenize("take, the sword!");

    // Should get: "take", "the", "sword", End
    let words: Vec<_> = tokens
        .iter()
        .filter_map(|t| {
            if let InputToken::Word(w) = t {
                Some(w.as_str())
            } else {
                None
            }
        })
        .collect();

    assert!(words.contains(&"take"));
    assert!(words.contains(&"sword"));
}

#[test]
fn tokenize_quoted_string() {
    let tokens = InputTokenizer::tokenize("say \"hello world\"");

    assert!(matches!(&tokens[0], InputToken::Word(w) if w == "say"));
    assert!(matches!(&tokens[1], InputToken::QuotedString(s) if s == "hello world"));
}

#[test]
fn tokenize_empty_input() {
    let tokens = InputTokenizer::tokenize("");

    assert!(tokens.iter().all(|t| matches!(t, InputToken::End)));
}

#[test]
fn tokenize_whitespace_only() {
    let tokens = InputTokenizer::tokenize("   \t  ");

    assert!(tokens.iter().all(|t| matches!(t, InputToken::End)));
}

#[test]
fn tokenize_multiple_spaces() {
    let tokens = InputTokenizer::tokenize("take   the   brass   lamp");

    let words: Vec<_> = tokens
        .iter()
        .filter_map(|t| {
            if let InputToken::Word(w) = t {
                Some(w.as_str())
            } else {
                None
            }
        })
        .collect();

    assert_eq!(words, vec!["take", "the", "brass", "lamp"]);
}

#[test]
fn tokenize_preposition_command() {
    let tokens = InputTokenizer::tokenize("put sword in chest");

    let words: Vec<_> = tokens
        .iter()
        .filter_map(|t| {
            if let InputToken::Word(w) = t {
                Some(w.as_str())
            } else {
                None
            }
        })
        .collect();

    assert_eq!(words, vec!["put", "sword", "in", "chest"]);
}

#[test]
fn tokenize_direction_command() {
    let tokens = InputTokenizer::tokenize("go north");

    let words: Vec<_> = tokens
        .iter()
        .filter_map(|t| {
            if let InputToken::Word(w) = t {
                Some(w.as_str())
            } else {
                None
            }
        })
        .collect();

    assert_eq!(words, vec!["go", "north"]);
}

#[test]
fn tokenize_with_articles() {
    let tokens = InputTokenizer::tokenize("take the brass lamp");

    let words: Vec<_> = tokens
        .iter()
        .filter_map(|t| {
            if let InputToken::Word(w) = t {
                Some(w.as_str())
            } else {
                None
            }
        })
        .collect();

    assert_eq!(words, vec!["take", "the", "brass", "lamp"]);
}
