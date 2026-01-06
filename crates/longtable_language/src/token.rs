//! Token types for the Longtable DSL.
//!
//! Tokens are the output of the lexer and input to the parser.

use crate::span::Span;

/// A token from lexical analysis.
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    /// The type and value of this token.
    pub kind: TokenKind,
    /// Source location of this token.
    pub span: Span,
}

impl Token {
    /// Creates a new token.
    #[must_use]
    pub const fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// Returns the text this token covers in the given source.
    #[must_use]
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        self.span.text(source)
    }

    /// Returns true if this token is a delimiter (opening paren, bracket, brace).
    #[must_use]
    pub const fn is_open_delimiter(&self) -> bool {
        matches!(
            self.kind,
            TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace | TokenKind::HashBrace
        )
    }

    /// Returns true if this token is a closing delimiter.
    #[must_use]
    pub const fn is_close_delimiter(&self) -> bool {
        matches!(
            self.kind,
            TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace
        )
    }
}

/// Token types for the Longtable DSL.
#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    // Delimiters
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `#{` for sets
    HashBrace,

    // Literals
    /// `nil`
    Nil,
    /// `true`
    True,
    /// `false`
    False,
    /// Integer literal like `42` or `-17`
    Int(i64),
    /// Float literal like `3.14` or `-0.5`
    Float(f64),
    /// String literal like `"hello"`
    String(String),
    /// Symbol like `foo` or `bar/baz`
    Symbol(String),
    /// Keyword like `:foo` or `:bar/baz`
    Keyword(String),

    // Special forms
    /// `'` for quote
    Quote,
    /// `` ` `` for syntax-quote
    Backtick,
    /// `~` for unquote
    Unquote,
    /// `~@` for unquote-splicing
    UnquoteSplice,
    /// `#name` for tagged literals (without the following form)
    Tag(String),

    // Meta
    /// Comment text (including `;`)
    Comment(String),
    /// `#_` for ignoring next form
    Ignore,
    /// End of input
    Eof,
    /// Lexer error
    Error(String),
}

impl TokenKind {
    /// Returns true if this token kind should be ignored during parsing.
    #[must_use]
    pub const fn is_trivia(&self) -> bool {
        matches!(self, Self::Comment(_))
    }

    /// Returns a human-readable name for this token kind.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::LParen => "'('",
            Self::RParen => "')'",
            Self::LBracket => "'['",
            Self::RBracket => "']'",
            Self::LBrace => "'{'",
            Self::RBrace => "'}'",
            Self::HashBrace => "'#{'",
            Self::Nil => "nil",
            Self::True => "true",
            Self::False => "false",
            Self::Int(_) => "integer",
            Self::Float(_) => "float",
            Self::String(_) => "string",
            Self::Symbol(_) => "symbol",
            Self::Keyword(_) => "keyword",
            Self::Quote => "quote",
            Self::Backtick => "backtick",
            Self::Unquote => "unquote",
            Self::UnquoteSplice => "unquote-splice",
            Self::Tag(_) => "tag",
            Self::Comment(_) => "comment",
            Self::Ignore => "ignore",
            Self::Eof => "end of input",
            Self::Error(_) => "error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_new() {
        let token = Token::new(TokenKind::Int(42), Span::new(0, 2, 1, 1));
        assert_eq!(token.kind, TokenKind::Int(42));
        assert_eq!(token.span.start, 0);
    }

    #[test]
    fn token_text() {
        let source = "42 hello";
        let token = Token::new(TokenKind::Int(42), Span::new(0, 2, 1, 1));
        assert_eq!(token.text(source), "42");
    }

    #[test]
    fn token_is_open_delimiter() {
        assert!(Token::new(TokenKind::LParen, Span::at_start()).is_open_delimiter());
        assert!(Token::new(TokenKind::LBracket, Span::at_start()).is_open_delimiter());
        assert!(Token::new(TokenKind::LBrace, Span::at_start()).is_open_delimiter());
        assert!(Token::new(TokenKind::HashBrace, Span::at_start()).is_open_delimiter());
        assert!(!Token::new(TokenKind::RParen, Span::at_start()).is_open_delimiter());
    }

    #[test]
    fn token_kind_name() {
        assert_eq!(TokenKind::LParen.name(), "'('");
        assert_eq!(TokenKind::Int(42).name(), "integer");
        assert_eq!(TokenKind::String("hi".into()).name(), "string");
    }

    #[test]
    fn token_kind_is_trivia() {
        assert!(TokenKind::Comment(";; test".into()).is_trivia());
        assert!(!TokenKind::Symbol("test".into()).is_trivia());
    }
}
