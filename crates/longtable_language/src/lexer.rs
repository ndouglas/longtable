//! Lexer for the Longtable DSL.
//!
//! The lexer converts source text into a stream of tokens.

use crate::span::Span;
use crate::token::{Token, TokenKind};

/// Lexer for Longtable source code.
///
/// The lexer iterates through source text and produces tokens.
pub struct Lexer<'src> {
    /// Source text being tokenized.
    source: &'src str,
    /// Remaining source text (as bytes for efficient indexing).
    rest: &'src str,
    /// Current byte offset in source.
    position: usize,
    /// Current line number (1-based).
    line: u32,
    /// Current column number (1-based).
    column: u32,
}

impl<'src> Lexer<'src> {
    /// Creates a new lexer for the given source.
    #[must_use]
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            rest: source,
            position: 0,
            line: 1,
            column: 1,
        }
    }

    /// Returns the next token from the source.
    ///
    /// # Panics
    /// This function does not panic as it checks for EOF before accessing characters.
    #[allow(clippy::missing_panics_doc)]
    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        let start = self.position;
        let start_line = self.line;
        let start_column = self.column;

        if self.rest.is_empty() {
            return Token::new(
                TokenKind::Eof,
                Span::new(start, start, start_line, start_column),
            );
        }

        // SAFETY: We just checked that rest is not empty
        let c = self.peek_char().expect("rest is not empty");
        let kind = match c {
            '(' => {
                self.advance();
                TokenKind::LParen
            }
            ')' => {
                self.advance();
                TokenKind::RParen
            }
            '[' => {
                self.advance();
                TokenKind::LBracket
            }
            ']' => {
                self.advance();
                TokenKind::RBracket
            }
            '{' => {
                self.advance();
                TokenKind::LBrace
            }
            '}' => {
                self.advance();
                TokenKind::RBrace
            }
            '\'' => {
                self.advance();
                TokenKind::Quote
            }
            '`' => {
                self.advance();
                TokenKind::Backtick
            }
            '~' => {
                self.advance();
                if self.peek_char() == Some('@') {
                    self.advance();
                    TokenKind::UnquoteSplice
                } else {
                    TokenKind::Unquote
                }
            }
            ';' => self.scan_comment(),
            '#' => self.scan_hash(),
            ':' => self.scan_keyword(),
            '"' => self.scan_string(),
            c if c.is_ascii_digit() => self.scan_number(),
            '-' | '+' => {
                // Could be number or symbol
                if self.rest.len() > 1
                    && self.rest[1..]
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_digit())
                {
                    self.scan_number()
                } else {
                    self.scan_symbol()
                }
            }
            c if is_symbol_start(c) => self.scan_symbol(),
            c => {
                self.advance();
                TokenKind::Error(format!("unexpected character: {c}"))
            }
        };

        Token::new(
            kind,
            Span::new(start, self.position, start_line, start_column),
        )
    }

    /// Tokenizes all source and returns a vector of tokens.
    ///
    /// Comments are included in the output.
    #[must_use]
    pub fn tokenize_all(source: &str) -> Vec<Token> {
        let mut lexer = Lexer::new(source);
        let mut tokens = Vec::new();
        loop {
            let token = lexer.next_token();
            let is_eof = token.kind == TokenKind::Eof;
            tokens.push(token);
            if is_eof {
                break;
            }
        }
        tokens
    }

    /// Peeks at the next character without consuming it.
    fn peek_char(&self) -> Option<char> {
        self.rest.chars().next()
    }

    /// Peeks at the character after the next one.
    fn peek_char_n(&self, n: usize) -> Option<char> {
        self.rest.chars().nth(n)
    }

    /// Advances past the next character.
    fn advance(&mut self) {
        if let Some(c) = self.peek_char() {
            let len = c.len_utf8();
            self.rest = &self.rest[len..];
            self.position += len;
            if c == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }
    }

    /// Skips whitespace characters.
    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() {
                self.advance();
            } else if c == ',' {
                // Commas are whitespace in Clojure-style syntax
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Scans a comment starting with `;`.
    fn scan_comment(&mut self) -> TokenKind {
        let mut text = String::new();
        while let Some(c) = self.peek_char() {
            if c == '\n' {
                break;
            }
            text.push(c);
            self.advance();
        }
        TokenKind::Comment(text)
    }

    /// Scans tokens starting with `#`.
    fn scan_hash(&mut self) -> TokenKind {
        self.advance(); // consume '#'
        match self.peek_char() {
            Some('{') => {
                self.advance();
                TokenKind::HashBrace
            }
            Some('_') => {
                self.advance();
                TokenKind::Ignore
            }
            Some(c) if is_symbol_start(c) => {
                // Tagged literal like #entity[...] or #pos[...]
                let name = self.scan_symbol_text();
                TokenKind::Tag(name)
            }
            Some(c) => TokenKind::Error(format!("unexpected character after #: {c}")),
            None => TokenKind::Error("unexpected end of input after #".into()),
        }
    }

    /// Scans a keyword starting with `:`.
    fn scan_keyword(&mut self) -> TokenKind {
        self.advance(); // consume ':'
        let name = self.scan_symbol_text();
        if name.is_empty() {
            TokenKind::Error("expected keyword name after ':'".into())
        } else {
            TokenKind::Keyword(name)
        }
    }

    /// Scans a string literal.
    fn scan_string(&mut self) -> TokenKind {
        self.advance(); // consume opening '"'
        let mut text = String::new();
        loop {
            match self.peek_char() {
                Some('"') => {
                    self.advance();
                    break;
                }
                Some('\\') => {
                    self.advance();
                    match self.peek_char() {
                        Some('n') => {
                            self.advance();
                            text.push('\n');
                        }
                        Some('r') => {
                            self.advance();
                            text.push('\r');
                        }
                        Some('t') => {
                            self.advance();
                            text.push('\t');
                        }
                        Some('\\') => {
                            self.advance();
                            text.push('\\');
                        }
                        Some('"') => {
                            self.advance();
                            text.push('"');
                        }
                        Some(c) => {
                            return TokenKind::Error(format!("invalid escape sequence: \\{c}"));
                        }
                        None => {
                            return TokenKind::Error(
                                "unexpected end of input in string escape".into(),
                            );
                        }
                    }
                }
                Some(c) => {
                    self.advance();
                    text.push(c);
                }
                None => {
                    return TokenKind::Error("unterminated string literal".into());
                }
            }
        }
        TokenKind::String(text)
    }

    /// Scans a number (integer or float).
    fn scan_number(&mut self) -> TokenKind {
        let start = self.position;
        let mut has_dot = false;

        // Handle optional sign
        if self.peek_char() == Some('-') || self.peek_char() == Some('+') {
            self.advance();
        }

        // Scan digits
        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() {
                self.advance();
            } else if c == '.'
                && !has_dot
                && self.peek_char_n(1).is_some_and(|c| c.is_ascii_digit())
            {
                has_dot = true;
                self.advance();
            } else {
                break;
            }
        }

        let text = &self.source[start..self.position];

        if has_dot {
            match text.parse::<f64>() {
                Ok(n) => TokenKind::Float(n),
                Err(e) => TokenKind::Error(format!("invalid float: {e}")),
            }
        } else {
            match text.parse::<i64>() {
                Ok(n) => TokenKind::Int(n),
                Err(e) => TokenKind::Error(format!("invalid integer: {e}")),
            }
        }
    }

    /// Scans a symbol.
    fn scan_symbol(&mut self) -> TokenKind {
        let name = self.scan_symbol_text();

        // Check for reserved words
        match name.as_str() {
            "nil" => TokenKind::Nil,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            _ => TokenKind::Symbol(name),
        }
    }

    /// Scans symbol text (used for both symbols and keywords).
    fn scan_symbol_text(&mut self) -> String {
        let start = self.position;
        while let Some(c) = self.peek_char() {
            if is_symbol_char(c) {
                self.advance();
            } else {
                break;
            }
        }
        self.source[start..self.position].to_string()
    }
}

/// Returns true if `c` can start a symbol.
fn is_symbol_start(c: char) -> bool {
    c.is_alphabetic()
        || matches!(
            c,
            '_' | '+' | '-' | '*' | '/' | '!' | '?' | '<' | '>' | '=' | '&' | '%' | '$' | '^'
        )
}

/// Returns true if `c` can appear in a symbol (not at start).
fn is_symbol_char(c: char) -> bool {
    is_symbol_start(c) || c.is_ascii_digit() || c == '.' || c == ':'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(source: &str) -> Vec<TokenKind> {
        Lexer::tokenize_all(source)
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn lex_empty() {
        assert_eq!(lex(""), vec![TokenKind::Eof]);
    }

    #[test]
    fn lex_whitespace() {
        assert_eq!(lex("   "), vec![TokenKind::Eof]);
        assert_eq!(lex("\n\t\r"), vec![TokenKind::Eof]);
        assert_eq!(lex(",,,"), vec![TokenKind::Eof]); // Commas are whitespace
    }

    #[test]
    fn lex_delimiters() {
        assert_eq!(
            lex("()[]{}"),
            vec![
                TokenKind::LParen,
                TokenKind::RParen,
                TokenKind::LBracket,
                TokenKind::RBracket,
                TokenKind::LBrace,
                TokenKind::RBrace,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_hash_brace() {
        assert_eq!(
            lex("#{}"),
            vec![TokenKind::HashBrace, TokenKind::RBrace, TokenKind::Eof]
        );
    }

    #[test]
    fn lex_nil_true_false() {
        assert_eq!(lex("nil"), vec![TokenKind::Nil, TokenKind::Eof]);
        assert_eq!(lex("true"), vec![TokenKind::True, TokenKind::Eof]);
        assert_eq!(lex("false"), vec![TokenKind::False, TokenKind::Eof]);
    }

    #[test]
    fn lex_integers() {
        assert_eq!(lex("42"), vec![TokenKind::Int(42), TokenKind::Eof]);
        assert_eq!(lex("-17"), vec![TokenKind::Int(-17), TokenKind::Eof]);
        assert_eq!(lex("+5"), vec![TokenKind::Int(5), TokenKind::Eof]);
        assert_eq!(lex("0"), vec![TokenKind::Int(0), TokenKind::Eof]);
    }

    #[test]
    fn lex_floats() {
        assert_eq!(lex("3.14"), vec![TokenKind::Float(3.14), TokenKind::Eof]);
        assert_eq!(lex("-0.5"), vec![TokenKind::Float(-0.5), TokenKind::Eof]);
        assert_eq!(lex("1.0"), vec![TokenKind::Float(1.0), TokenKind::Eof]);
    }

    #[test]
    fn lex_strings() {
        assert_eq!(
            lex(r#""hello""#),
            vec![TokenKind::String("hello".into()), TokenKind::Eof]
        );
        assert_eq!(
            lex(r#""hello\nworld""#),
            vec![TokenKind::String("hello\nworld".into()), TokenKind::Eof]
        );
        assert_eq!(
            lex(r#""say \"hi\"""#),
            vec![TokenKind::String("say \"hi\"".into()), TokenKind::Eof]
        );
    }

    #[test]
    fn lex_symbols() {
        assert_eq!(
            lex("foo"),
            vec![TokenKind::Symbol("foo".into()), TokenKind::Eof]
        );
        assert_eq!(
            lex("bar/baz"),
            vec![TokenKind::Symbol("bar/baz".into()), TokenKind::Eof]
        );
        assert_eq!(
            lex("inc"),
            vec![TokenKind::Symbol("inc".into()), TokenKind::Eof]
        );
        assert_eq!(
            lex("+"),
            vec![TokenKind::Symbol("+".into()), TokenKind::Eof]
        );
        assert_eq!(
            lex("some?"),
            vec![TokenKind::Symbol("some?".into()), TokenKind::Eof]
        );
    }

    #[test]
    fn lex_keywords() {
        assert_eq!(
            lex(":foo"),
            vec![TokenKind::Keyword("foo".into()), TokenKind::Eof]
        );
        assert_eq!(
            lex(":bar/baz"),
            vec![TokenKind::Keyword("bar/baz".into()), TokenKind::Eof]
        );
    }

    #[test]
    fn lex_quote_unquote() {
        assert_eq!(lex("'"), vec![TokenKind::Quote, TokenKind::Eof]);
        assert_eq!(lex("`"), vec![TokenKind::Backtick, TokenKind::Eof]);
        assert_eq!(lex("~"), vec![TokenKind::Unquote, TokenKind::Eof]);
        assert_eq!(lex("~@"), vec![TokenKind::UnquoteSplice, TokenKind::Eof]);
    }

    #[test]
    fn lex_comments() {
        let tokens = lex("; comment\n42");
        assert_eq!(tokens.len(), 3);
        assert!(matches!(tokens[0], TokenKind::Comment(_)));
        assert_eq!(tokens[1], TokenKind::Int(42));
    }

    #[test]
    fn lex_ignore() {
        assert_eq!(
            lex("#_ 42"),
            vec![TokenKind::Ignore, TokenKind::Int(42), TokenKind::Eof]
        );
    }

    #[test]
    fn lex_tag() {
        assert_eq!(
            lex("#entity"),
            vec![TokenKind::Tag("entity".into()), TokenKind::Eof]
        );
        assert_eq!(
            lex("#pos[1 2]"),
            vec![
                TokenKind::Tag("pos".into()),
                TokenKind::LBracket,
                TokenKind::Int(1),
                TokenKind::Int(2),
                TokenKind::RBracket,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_expression() {
        let tokens = lex("(+ 1 2)");
        assert_eq!(
            tokens,
            vec![
                TokenKind::LParen,
                TokenKind::Symbol("+".into()),
                TokenKind::Int(1),
                TokenKind::Int(2),
                TokenKind::RParen,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_complex_expression() {
        let source = r#"
            (def x 42)
            (fn: add [a b]
              (+ a b))
        "#;
        let tokens = lex(source);
        // Just verify it lexes without error
        assert!(tokens.iter().all(|t| !matches!(t, TokenKind::Error(_))));
        assert!(matches!(tokens.last(), Some(TokenKind::Eof)));
    }

    #[test]
    fn lex_unterminated_string() {
        let tokens = lex(r#""hello"#);
        assert!(matches!(tokens[0], TokenKind::Error(_)));
    }

    #[test]
    fn lex_span_tracking() {
        let source = "foo bar";
        let mut lexer = Lexer::new(source);

        let t1 = lexer.next_token();
        assert_eq!(t1.span.start, 0);
        assert_eq!(t1.span.end, 3);
        assert_eq!(t1.span.line, 1);
        assert_eq!(t1.span.column, 1);

        let t2 = lexer.next_token();
        assert_eq!(t2.span.start, 4);
        assert_eq!(t2.span.end, 7);
        assert_eq!(t2.span.line, 1);
        assert_eq!(t2.span.column, 5);
    }

    #[test]
    fn lex_multiline_span_tracking() {
        let source = "foo\nbar";
        let mut lexer = Lexer::new(source);

        let t1 = lexer.next_token();
        assert_eq!(t1.span.line, 1);

        let t2 = lexer.next_token();
        assert_eq!(t2.span.line, 2);
        assert_eq!(t2.span.column, 1);
    }
}
