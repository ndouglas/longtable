//! Integration tests for the lexer
//!
//! Tests tokenization of Longtable DSL source code.

use longtable_language::{Lexer, TokenKind};

// =============================================================================
// Basic Tokens
// =============================================================================

#[test]
fn tokenize_nil() {
    let tokens = Lexer::tokenize_all("nil");
    assert_eq!(tokens.len(), 2); // nil + eof
    assert!(matches!(tokens[0].kind, TokenKind::Nil));
}

#[test]
fn tokenize_booleans() {
    let tokens = Lexer::tokenize_all("true false");
    assert!(matches!(tokens[0].kind, TokenKind::True));
    assert!(matches!(tokens[1].kind, TokenKind::False));
}

#[test]
fn tokenize_integers() {
    let tokens = Lexer::tokenize_all("0 42 -17 1000000");
    assert!(matches!(tokens[0].kind, TokenKind::Int(0)));
    assert!(matches!(tokens[1].kind, TokenKind::Int(42)));
    assert!(matches!(tokens[2].kind, TokenKind::Int(-17)));
    assert!(matches!(tokens[3].kind, TokenKind::Int(1000000)));
}

#[test]
fn tokenize_floats() {
    let tokens = Lexer::tokenize_all("3.14 -2.5 0.0 1e10");
    assert!(matches!(tokens[0].kind, TokenKind::Float(_)));
    assert!(matches!(tokens[1].kind, TokenKind::Float(_)));
}

#[test]
fn tokenize_string() {
    let tokens = Lexer::tokenize_all("\"hello world\"");
    if let TokenKind::String(s) = &tokens[0].kind {
        assert_eq!(s, "hello world");
    } else {
        panic!("Expected string token");
    }
}

#[test]
fn tokenize_string_with_escapes() {
    let tokens = Lexer::tokenize_all(r#""line1\nline2""#);
    if let TokenKind::String(s) = &tokens[0].kind {
        assert!(s.contains("\\n") || s.contains('\n'));
    } else {
        panic!("Expected string token");
    }
}

#[test]
fn tokenize_symbol() {
    let tokens = Lexer::tokenize_all("foo bar-baz _underscore");
    assert!(matches!(&tokens[0].kind, TokenKind::Symbol(s) if s == "foo"));
    assert!(matches!(&tokens[1].kind, TokenKind::Symbol(s) if s == "bar-baz"));
    assert!(matches!(&tokens[2].kind, TokenKind::Symbol(s) if s == "_underscore"));
}

#[test]
fn tokenize_keyword() {
    let tokens = Lexer::tokenize_all(":foo :bar-baz :namespaced/keyword");
    assert!(matches!(&tokens[0].kind, TokenKind::Keyword(s) if s == "foo"));
    assert!(matches!(&tokens[1].kind, TokenKind::Keyword(s) if s == "bar-baz"));
    assert!(matches!(&tokens[2].kind, TokenKind::Keyword(s) if s == "namespaced/keyword"));
}

// =============================================================================
// Delimiters
// =============================================================================

#[test]
fn tokenize_parentheses() {
    let tokens = Lexer::tokenize_all("()");
    assert!(matches!(tokens[0].kind, TokenKind::LParen));
    assert!(matches!(tokens[1].kind, TokenKind::RParen));
}

#[test]
fn tokenize_brackets() {
    let tokens = Lexer::tokenize_all("[]");
    assert!(matches!(tokens[0].kind, TokenKind::LBracket));
    assert!(matches!(tokens[1].kind, TokenKind::RBracket));
}

#[test]
fn tokenize_braces() {
    let tokens = Lexer::tokenize_all("{}");
    assert!(matches!(tokens[0].kind, TokenKind::LBrace));
    assert!(matches!(tokens[1].kind, TokenKind::RBrace));
}

// =============================================================================
// Special Tokens
// =============================================================================

#[test]
fn tokenize_quote() {
    let tokens = Lexer::tokenize_all("'foo");
    assert!(matches!(tokens[0].kind, TokenKind::Quote));
}

#[test]
fn tokenize_backtick() {
    let tokens = Lexer::tokenize_all("`foo");
    assert!(matches!(tokens[0].kind, TokenKind::Backtick));
}

#[test]
fn tokenize_unquote() {
    let tokens = Lexer::tokenize_all("~foo ~@bar");
    assert!(matches!(tokens[0].kind, TokenKind::Unquote));
    assert!(matches!(tokens[2].kind, TokenKind::UnquoteSplice));
}

#[test]
fn tokenize_comment() {
    let tokens = Lexer::tokenize_all("; this is a comment\n42");
    // Comment may be skipped or included depending on lexer behavior
    let has_int = tokens.iter().any(|t| matches!(t.kind, TokenKind::Int(42)));
    assert!(has_int);
}

// =============================================================================
// Complex Expressions
// =============================================================================

#[test]
fn tokenize_list_expression() {
    let tokens = Lexer::tokenize_all("(+ 1 2)");
    assert!(matches!(tokens[0].kind, TokenKind::LParen));
    assert!(matches!(&tokens[1].kind, TokenKind::Symbol(s) if s == "+"));
    assert!(matches!(tokens[2].kind, TokenKind::Int(1)));
    assert!(matches!(tokens[3].kind, TokenKind::Int(2)));
    assert!(matches!(tokens[4].kind, TokenKind::RParen));
}

#[test]
fn tokenize_vector() {
    let tokens = Lexer::tokenize_all("[1 2 3]");
    assert!(matches!(tokens[0].kind, TokenKind::LBracket));
    assert!(matches!(tokens[4].kind, TokenKind::RBracket));
}

#[test]
fn tokenize_map() {
    let tokens = Lexer::tokenize_all("{:a 1 :b 2}");
    assert!(matches!(tokens[0].kind, TokenKind::LBrace));
    assert!(matches!(&tokens[1].kind, TokenKind::Keyword(k) if k == "a"));
}

#[test]
fn tokenize_nested() {
    let tokens = Lexer::tokenize_all("(if (> x 0) (+ x 1) (- x 1))");
    // Count parentheses
    let lparen = tokens
        .iter()
        .filter(|t| matches!(t.kind, TokenKind::LParen))
        .count();
    let rparen = tokens
        .iter()
        .filter(|t| matches!(t.kind, TokenKind::RParen))
        .count();
    assert_eq!(lparen, rparen);
    assert_eq!(lparen, 4);
}

// =============================================================================
// Whitespace Handling
// =============================================================================

#[test]
fn whitespace_ignored() {
    let tokens1 = Lexer::tokenize_all("(+ 1 2)");
    let tokens2 = Lexer::tokenize_all("(+   1   2)");
    let tokens3 = Lexer::tokenize_all("( + 1 2 )");

    // Same tokens, different spacing
    assert_eq!(tokens1.len(), tokens2.len());
    assert_eq!(tokens2.len(), tokens3.len());
}

#[test]
fn newlines_as_whitespace() {
    let tokens = Lexer::tokenize_all("(+\n1\n2)");
    let has_plus = tokens
        .iter()
        .any(|t| matches!(&t.kind, TokenKind::Symbol(s) if s == "+"));
    assert!(has_plus);
}

// =============================================================================
// Span Tracking
// =============================================================================

#[test]
fn tokens_have_spans() {
    let tokens = Lexer::tokenize_all("foo bar");
    // First token starts at position 0
    assert_eq!(tokens[0].span.start, 0);
    // Second token starts after "foo "
    assert!(tokens[1].span.start > 0);
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn empty_input() {
    let tokens = Lexer::tokenize_all("");
    assert_eq!(tokens.len(), 1); // just EOF
    assert!(matches!(tokens[0].kind, TokenKind::Eof));
}

#[test]
fn whitespace_only() {
    let tokens = Lexer::tokenize_all("   \n\t  ");
    assert_eq!(tokens.len(), 1); // just EOF
}

#[test]
fn question_mark_symbol() {
    // Pattern variables like ?entity
    let tokens = Lexer::tokenize_all("?entity");
    assert!(matches!(&tokens[0].kind, TokenKind::Symbol(s) if s.starts_with('?')));
}
