//! Fuzz tests for lexer and parser crash resistance.
//!
//! These tests use property-based testing to verify that the lexer and parser
//! never panic on any input, even malformed or adversarial inputs.

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use crate::token::TokenKind;
    use crate::{Lexer, parse};

    /// Tokenize all input using the lexer (helper function).
    fn tokenize_all(input: &str) {
        let mut lexer = Lexer::new(input);
        loop {
            let token = lexer.next_token();
            if token.kind == TokenKind::Eof {
                break;
            }
        }
    }

    // ==========================================================================
    // Arbitrary String Generators
    // ==========================================================================

    /// Strategy for generating completely random strings (potential garbage).
    fn arbitrary_string() -> impl Strategy<Value = String> {
        prop::collection::vec(any::<char>(), 0..1000).prop_map(|chars| chars.into_iter().collect())
    }

    /// Strategy for generating strings with Longtable-like structure.
    fn lisp_like_string() -> impl Strategy<Value = String> {
        let atom = prop_oneof![
            "[0-9]+".prop_map(String::from),            // Numbers
            "[a-z][a-z0-9-]*".prop_map(String::from),   // Symbols
            ":[a-z][a-z0-9/-]*".prop_map(String::from), // Keywords
            r#""[^"\\]*""#.prop_map(String::from),      // Simple strings
            "(true|false|nil)".prop_map(String::from),  // Literals
        ];

        let delim = prop_oneof![
            Just("(".to_string()),
            Just(")".to_string()),
            Just("[".to_string()),
            Just("]".to_string()),
            Just("{".to_string()),
            Just("}".to_string()),
            Just(" ".to_string()),
            Just("\n".to_string()),
        ];

        prop::collection::vec(prop_oneof![atom, delim], 0..100).prop_map(|parts| parts.join(""))
    }

    /// Strategy for generating strings with unbalanced delimiters.
    fn unbalanced_delimiters() -> impl Strategy<Value = String> {
        let parts = prop::collection::vec(
            prop_oneof![
                Just("(".to_string()),
                Just(")".to_string()),
                Just("[".to_string()),
                Just("]".to_string()),
                Just("{".to_string()),
                Just("}".to_string()),
                Just("a".to_string()),
                Just(" ".to_string()),
            ],
            1..50,
        );
        parts.prop_map(|v| v.join(""))
    }

    /// Strategy for strings with escape sequences.
    fn strings_with_escapes() -> impl Strategy<Value = String> {
        let escape_chars = prop_oneof![
            Just(r"\n".to_string()),
            Just(r"\t".to_string()),
            Just(r"\r".to_string()),
            Just(r"\\".to_string()),
            Just(r#"\""#.to_string()),
            Just(r"\u0000".to_string()),
            Just(r"\xAB".to_string()),
            Just(r"\".to_string()), // Incomplete escape
        ];

        prop::collection::vec(
            prop_oneof![escape_chars, "[a-z ]".prop_map(String::from)],
            0..20,
        )
        .prop_map(|parts| format!("\"{}\"", parts.join("")))
    }

    /// Strategy for numeric edge cases.
    fn numeric_edge_cases() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("0".to_string()),
            Just("-0".to_string()),
            Just("9223372036854775807".to_string()), // i64::MAX
            Just("-9223372036854775808".to_string()), // i64::MIN
            Just("99999999999999999999999999999999".to_string()), // overflow
            Just("0.0".to_string()),
            Just("-0.0".to_string()),
            Just("1e308".to_string()),
            Just("1e-308".to_string()),
            Just("1e999".to_string()), // overflow
            Just("NaN".to_string()),
            Just("Infinity".to_string()),
            Just(".5".to_string()),
            Just("5.".to_string()),
            Just("5e".to_string()),
            Just("5e+".to_string()),
            Just("5e-".to_string()),
        ]
    }

    /// Strategy for deep nesting.
    fn deeply_nested() -> impl Strategy<Value = String> {
        (1..100usize).prop_map(|depth| {
            let open: String = std::iter::repeat_n('(', depth).collect();
            let close: String = std::iter::repeat_n(')', depth).collect();
            format!("{open}a{close}")
        })
    }

    /// Strategy for Unicode edge cases.
    fn unicode_edge_cases() -> impl Strategy<Value = String> {
        prop_oneof![
            Just(String::new()),
            Just("\u{0}".to_string()),      // Null
            Just("\u{FEFF}".to_string()),   // BOM
            Just("\u{FFFF}".to_string()),   // Non-character
            Just("\u{10FFFF}".to_string()), // Max codepoint
            Just("λ".to_string()),          // Greek lambda
            Just("🦀".to_string()),         // Emoji
            Just("中文".to_string()),       // CJK
            Just("مرحبا".to_string()),      // Arabic (RTL)
            Just("\u{0300}".to_string()),   // Combining diacritical
            Just("e\u{0301}".to_string()),  // e with combining accent
        ]
    }

    // ==========================================================================
    // Lexer Fuzz Tests
    // ==========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]

        /// Lexer never panics on arbitrary input.
        #[test]
        fn lexer_never_panics_on_arbitrary_input(input in arbitrary_string()) {
            tokenize_all(&input);
        }

        /// Lexer never panics on lisp-like input.
        #[test]
        fn lexer_never_panics_on_lisp_like_input(input in lisp_like_string()) {
            tokenize_all(&input);
        }

        /// Lexer never panics on unbalanced delimiters.
        #[test]
        fn lexer_never_panics_on_unbalanced(input in unbalanced_delimiters()) {
            tokenize_all(&input);
        }

        /// Lexer handles strings with escapes.
        #[test]
        fn lexer_handles_escape_sequences(input in strings_with_escapes()) {
            tokenize_all(&input);
        }

        /// Lexer handles numeric edge cases.
        #[test]
        fn lexer_handles_numeric_edge_cases(input in numeric_edge_cases()) {
            tokenize_all(&input);
        }

        /// Lexer handles Unicode edge cases.
        #[test]
        fn lexer_handles_unicode(input in unicode_edge_cases()) {
            tokenize_all(&input);
        }
    }

    // ==========================================================================
    // Parser Fuzz Tests
    // ==========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]

        /// Parser never panics on arbitrary input.
        #[test]
        fn parser_never_panics_on_arbitrary_input(input in arbitrary_string()) {
            let _ = parse(&input);
        }

        /// Parser never panics on lisp-like input.
        #[test]
        fn parser_never_panics_on_lisp_like_input(input in lisp_like_string()) {
            let _ = parse(&input);
        }

        /// Parser never panics on unbalanced delimiters.
        #[test]
        fn parser_never_panics_on_unbalanced(input in unbalanced_delimiters()) {
            let _ = parse(&input);
        }

        /// Parser handles deeply nested structures.
        #[test]
        fn parser_handles_deep_nesting(input in deeply_nested()) {
            let _ = parse(&input);
        }

        /// Parser handles numeric edge cases.
        #[test]
        fn parser_handles_numeric_edge_cases(input in numeric_edge_cases()) {
            let _ = parse(&input);
        }

        /// Parser handles Unicode edge cases.
        #[test]
        fn parser_handles_unicode(input in unicode_edge_cases()) {
            let _ = parse(&input);
        }
    }

    // ==========================================================================
    // Round-Trip Tests
    // ==========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Valid expressions should parse successfully and not panic on re-lex.
        #[test]
        fn valid_int_roundtrip(n in any::<i64>()) {
            let input = format!("{n}");
            let result = parse(&input);
            prop_assert!(result.is_ok(), "Failed to parse: {}", input);
        }

        /// Valid lists should parse successfully.
        #[test]
        fn valid_list_roundtrip(depth in 1..10usize, symbol in "[a-z]+") {
            let open: String = std::iter::repeat_n('(', depth).collect();
            let close: String = std::iter::repeat_n(')', depth).collect();
            let input = format!("{open}{symbol}{close}");
            let result = parse(&input);
            prop_assert!(result.is_ok(), "Failed to parse: {}", input);
        }
    }

    // ==========================================================================
    // Specific Edge Cases
    // ==========================================================================

    #[test]
    fn lexer_handles_empty_input() {
        let mut lexer = Lexer::new("");
        let token = lexer.next_token();
        assert_eq!(token.kind, TokenKind::Eof);
    }

    #[test]
    fn parser_handles_empty_input() {
        let result = parse("");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn lexer_handles_only_whitespace() {
        tokenize_all("   \n\t   ");
    }

    #[test]
    fn parser_handles_only_whitespace() {
        let result = parse("   \n\t   ");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn lexer_handles_only_comments() {
        tokenize_all("; this is a comment\n; another comment");
    }

    #[test]
    fn parser_handles_only_comments() {
        let result = parse("; this is a comment\n; another comment");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn lexer_handles_very_long_symbol() {
        let long_symbol: String = "x".repeat(10000);
        tokenize_all(&long_symbol);
    }

    #[test]
    fn parser_handles_very_long_symbol() {
        let long_symbol: String = "x".repeat(10000);
        let _ = parse(&long_symbol);
    }

    #[test]
    fn lexer_handles_very_long_string() {
        let content: String = "a".repeat(10000);
        let long_string = format!("\"{content}\"");
        tokenize_all(&long_string);
    }

    #[test]
    fn parser_handles_very_long_string() {
        let content: String = "a".repeat(10000);
        let long_string = format!("\"{content}\"");
        let _ = parse(&long_string);
    }

    #[test]
    fn parser_handles_many_siblings() {
        let many_items: String = (0..1000).map(|i| format!("{i} ")).collect();
        let input = format!("({many_items})");
        let _ = parse(&input);
    }

    #[test]
    fn lexer_handles_alternating_delimiters() {
        let input = "([{([{([{([{";
        tokenize_all(input);
    }

    #[test]
    fn parser_handles_alternating_delimiters() {
        let input = "([{([{([{([{";
        let _ = parse(input);
    }

    #[test]
    fn lexer_handles_mismatched_delimiters() {
        let input = "(])([}({)]";
        tokenize_all(input);
    }

    #[test]
    fn parser_handles_mismatched_delimiters() {
        let input = "(])([}({)]";
        let result = parse(input);
        // Should error but not panic
        assert!(result.is_err());
    }

    #[test]
    fn lexer_handles_quote_variations() {
        let inputs = ["'", "''", "'(", "('", "'''x", "'x'", "''x''"];
        for input in inputs {
            tokenize_all(input);
        }
    }

    #[test]
    fn parser_handles_quote_variations() {
        let inputs = ["'", "''", "'(", "('", "'''x", "'x'", "''x''"];
        for input in inputs {
            let _ = parse(input);
        }
    }
}
