//! Parser for the Longtable DSL.
//!
//! The parser converts a stream of tokens into an abstract syntax tree.

use longtable_foundation::{Error, ErrorKind, Result};

use crate::ast::Ast;
use crate::lexer::Lexer;
use crate::span::Span;
use crate::token::{Token, TokenKind};

/// Parser for Longtable source code.
pub struct Parser<'src> {
    /// The lexer providing tokens.
    lexer: Lexer<'src>,
    /// Current token (lookahead).
    current: Token,
    /// Source text (for error messages).
    source: &'src str,
}

impl<'src> Parser<'src> {
    /// Creates a new parser for the given source.
    #[must_use]
    pub fn new(source: &'src str) -> Self {
        let mut lexer = Lexer::new(source);
        let current = lexer.next_token();
        Self {
            lexer,
            current,
            source,
        }
    }

    /// Parses a single expression from the source.
    ///
    /// # Errors
    /// Returns an error if the source cannot be parsed.
    pub fn parse(&mut self) -> Result<Ast> {
        self.skip_trivia();
        self.parse_form()
    }

    /// Parses all expressions from the source.
    ///
    /// # Errors
    /// Returns an error if the source cannot be parsed.
    pub fn parse_all(&mut self) -> Result<Vec<Ast>> {
        let mut forms = Vec::new();
        self.skip_trivia();

        while self.current.kind != TokenKind::Eof {
            forms.push(self.parse_form()?);
            self.skip_trivia();
        }

        Ok(forms)
    }

    /// Parses a form (expression).
    fn parse_form(&mut self) -> Result<Ast> {
        self.skip_trivia();

        match &self.current.kind {
            TokenKind::Nil => {
                let span = self.current.span;
                self.advance();
                Ok(Ast::Nil(span))
            }
            TokenKind::True => {
                let span = self.current.span;
                self.advance();
                Ok(Ast::Bool(true, span))
            }
            TokenKind::False => {
                let span = self.current.span;
                self.advance();
                Ok(Ast::Bool(false, span))
            }
            TokenKind::Int(n) => {
                let n = *n;
                let span = self.current.span;
                self.advance();
                Ok(Ast::Int(n, span))
            }
            TokenKind::Float(n) => {
                let n = *n;
                let span = self.current.span;
                self.advance();
                Ok(Ast::Float(n, span))
            }
            TokenKind::String(s) => {
                let s = s.clone();
                let span = self.current.span;
                self.advance();
                Ok(Ast::String(s, span))
            }
            TokenKind::Symbol(name) => {
                let name = name.clone();
                let span = self.current.span;
                self.advance();
                Ok(Ast::Symbol(name, span))
            }
            TokenKind::Keyword(name) => {
                let name = name.clone();
                let span = self.current.span;
                self.advance();
                Ok(Ast::Keyword(name, span))
            }
            TokenKind::LParen => self.parse_list(),
            TokenKind::LBracket => self.parse_vector(),
            TokenKind::LBrace => self.parse_map(),
            TokenKind::HashBrace => self.parse_set(),
            TokenKind::Quote => self.parse_quote(),
            TokenKind::Backtick => self.parse_syntax_quote(),
            TokenKind::Unquote => self.parse_unquote(),
            TokenKind::UnquoteSplice => self.parse_unquote_splice(),
            TokenKind::Tag(name) => {
                let name = name.clone();
                self.parse_tagged(name)
            }
            TokenKind::Ignore => self.parse_ignore(),
            TokenKind::Eof => Err(self.error("unexpected end of input")),
            TokenKind::Error(msg) => Err(self.error(msg)),
            TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                Err(self.error(&format!("unexpected {}", self.current.kind.name())))
            }
            TokenKind::Comment(_) => {
                // Should be skipped by skip_trivia, but handle defensively
                self.advance();
                self.parse_form()
            }
        }
    }

    /// Parses a list: `(...)`.
    fn parse_list(&mut self) -> Result<Ast> {
        let start_span = self.current.span;
        self.expect(&TokenKind::LParen)?;

        let mut elements = Vec::new();
        self.skip_trivia();

        while self.current.kind != TokenKind::RParen {
            if self.current.kind == TokenKind::Eof {
                return Err(self.error_at(start_span, "unterminated list"));
            }
            elements.push(self.parse_form()?);
            self.skip_trivia();
        }

        let end_span = self.current.span;
        self.expect(&TokenKind::RParen)?;

        Ok(Ast::List(elements, start_span.to(end_span)))
    }

    /// Parses a vector: `[...]`.
    fn parse_vector(&mut self) -> Result<Ast> {
        let start_span = self.current.span;
        self.expect(&TokenKind::LBracket)?;

        let mut elements = Vec::new();
        self.skip_trivia();

        while self.current.kind != TokenKind::RBracket {
            if self.current.kind == TokenKind::Eof {
                return Err(self.error_at(start_span, "unterminated vector"));
            }
            elements.push(self.parse_form()?);
            self.skip_trivia();
        }

        let end_span = self.current.span;
        self.expect(&TokenKind::RBracket)?;

        Ok(Ast::Vector(elements, start_span.to(end_span)))
    }

    /// Parses a set: `#{...}`.
    fn parse_set(&mut self) -> Result<Ast> {
        let start_span = self.current.span;
        self.expect(&TokenKind::HashBrace)?;

        let mut elements = Vec::new();
        self.skip_trivia();

        while self.current.kind != TokenKind::RBrace {
            if self.current.kind == TokenKind::Eof {
                return Err(self.error_at(start_span, "unterminated set"));
            }
            elements.push(self.parse_form()?);
            self.skip_trivia();
        }

        let end_span = self.current.span;
        self.expect(&TokenKind::RBrace)?;

        Ok(Ast::Set(elements, start_span.to(end_span)))
    }

    /// Parses a map: `{...}`.
    fn parse_map(&mut self) -> Result<Ast> {
        let start_span = self.current.span;
        self.expect(&TokenKind::LBrace)?;

        let mut entries = Vec::new();
        self.skip_trivia();

        while self.current.kind != TokenKind::RBrace {
            if self.current.kind == TokenKind::Eof {
                return Err(self.error_at(start_span, "unterminated map"));
            }
            let key = self.parse_form()?;
            self.skip_trivia();

            if self.current.kind == TokenKind::RBrace || self.current.kind == TokenKind::Eof {
                return Err(self.error("map must have even number of elements"));
            }
            let value = self.parse_form()?;
            self.skip_trivia();

            entries.push((key, value));
        }

        let end_span = self.current.span;
        self.expect(&TokenKind::RBrace)?;

        Ok(Ast::Map(entries, start_span.to(end_span)))
    }

    /// Parses a quoted form: `'x`.
    fn parse_quote(&mut self) -> Result<Ast> {
        let start_span = self.current.span;
        self.expect(&TokenKind::Quote)?;

        self.skip_trivia();
        if self.current.kind == TokenKind::Eof {
            return Err(self.error_at(start_span, "expected form after quote"));
        }

        let inner = self.parse_form()?;
        let end_span = inner.span();

        Ok(Ast::Quote(Box::new(inner), start_span.to(end_span)))
    }

    /// Parses a syntax-quoted form: `` `x ``.
    fn parse_syntax_quote(&mut self) -> Result<Ast> {
        let start_span = self.current.span;
        self.expect(&TokenKind::Backtick)?;

        self.skip_trivia();
        if self.current.kind == TokenKind::Eof {
            return Err(self.error_at(start_span, "expected form after backtick"));
        }

        let inner = self.parse_form()?;
        let end_span = inner.span();

        Ok(Ast::SyntaxQuote(Box::new(inner), start_span.to(end_span)))
    }

    /// Parses an unquoted form: `~x`.
    fn parse_unquote(&mut self) -> Result<Ast> {
        let start_span = self.current.span;
        self.expect(&TokenKind::Unquote)?;

        self.skip_trivia();
        if self.current.kind == TokenKind::Eof {
            return Err(self.error_at(start_span, "expected form after unquote"));
        }

        let inner = self.parse_form()?;
        let end_span = inner.span();

        Ok(Ast::Unquote(Box::new(inner), start_span.to(end_span)))
    }

    /// Parses an unquote-spliced form: `~@x`.
    fn parse_unquote_splice(&mut self) -> Result<Ast> {
        let start_span = self.current.span;
        self.expect(&TokenKind::UnquoteSplice)?;

        self.skip_trivia();
        if self.current.kind == TokenKind::Eof {
            return Err(self.error_at(start_span, "expected form after unquote-splice"));
        }

        let inner = self.parse_form()?;
        let end_span = inner.span();

        Ok(Ast::UnquoteSplice(Box::new(inner), start_span.to(end_span)))
    }

    /// Parses a tagged literal: `#name ...`.
    fn parse_tagged(&mut self, tag_name: String) -> Result<Ast> {
        let start_span = self.current.span;
        self.advance(); // consume tag

        self.skip_trivia();
        if self.current.kind == TokenKind::Eof {
            return Err(self.error_at(start_span, "expected form after tag"));
        }

        let inner = self.parse_form()?;
        let end_span = inner.span();

        Ok(Ast::Tagged(
            tag_name,
            Box::new(inner),
            start_span.to(end_span),
        ))
    }

    /// Parses an ignore directive: `#_ form`.
    fn parse_ignore(&mut self) -> Result<Ast> {
        self.advance(); // consume #_
        self.skip_trivia();

        if self.current.kind == TokenKind::Eof {
            return Err(self.error("expected form after #_"));
        }

        // Parse and discard the ignored form
        let _ignored = self.parse_form()?;

        // Parse the next actual form
        self.skip_trivia();
        if self.current.kind == TokenKind::Eof {
            return Err(self.error("expected form after ignored form"));
        }

        self.parse_form()
    }

    /// Skips comment tokens.
    fn skip_trivia(&mut self) {
        while self.current.kind.is_trivia() {
            self.advance();
        }
    }

    /// Advances to the next token.
    fn advance(&mut self) {
        self.current = self.lexer.next_token();
    }

    /// Expects the current token to be of a specific kind, then advances.
    fn expect(&mut self, expected: &TokenKind) -> Result<()> {
        // Use discriminant comparison for token kinds that carry data
        let matches =
            std::mem::discriminant(&self.current.kind) == std::mem::discriminant(expected);

        if matches {
            self.advance();
            Ok(())
        } else {
            let expected_name = expected.name();
            Err(self.error(&format!(
                "expected {expected_name}, found {}",
                self.current.kind.name()
            )))
        }
    }

    /// Creates a parse error at the current position.
    fn error(&self, message: &str) -> Error {
        self.error_at(self.current.span, message)
    }

    /// Creates a parse error at a specific span.
    fn error_at(&self, span: Span, message: &str) -> Error {
        Error::new(ErrorKind::ParseError {
            message: message.to_string(),
            line: span.line,
            column: span.column,
            context: self.context_at(span),
        })
    }

    /// Gets context around a span for error messages.
    fn context_at(&self, span: Span) -> String {
        // Find the line containing this span
        let line_start = self.source[..span.start].rfind('\n').map_or(0, |i| i + 1);
        let line_end = self.source[span.start..]
            .find('\n')
            .map_or(self.source.len(), |i| span.start + i);

        self.source[line_start..line_end].to_string()
    }
}

/// Parses source code into AST.
///
/// # Errors
/// Returns an error if the source cannot be parsed.
pub fn parse(source: &str) -> Result<Vec<Ast>> {
    Parser::new(source).parse_all()
}

/// Parses a single expression from source.
///
/// # Errors
/// Returns an error if the source cannot be parsed.
pub fn parse_one(source: &str) -> Result<Ast> {
    Parser::new(source).parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_test(source: &str) -> Vec<Ast> {
        parse(source).expect("parse failed")
    }

    fn parse_one_test(source: &str) -> Ast {
        parse_one(source).expect("parse failed")
    }

    #[test]
    fn parse_nil() {
        let ast = parse_one_test("nil");
        assert!(ast.is_nil());
    }

    #[test]
    fn parse_bool() {
        assert!(matches!(parse_one_test("true"), Ast::Bool(true, _)));
        assert!(matches!(parse_one_test("false"), Ast::Bool(false, _)));
    }

    #[test]
    fn parse_int() {
        assert_eq!(parse_one_test("42").as_int(), Some(42));
        assert_eq!(parse_one_test("-17").as_int(), Some(-17));
    }

    #[test]
    fn parse_float() {
        assert!(matches!(parse_one_test("2.5"), Ast::Float(n, _) if (n - 2.5).abs() < 0.001));
    }

    #[test]
    fn parse_string() {
        assert_eq!(parse_one_test(r#""hello""#).as_string(), Some("hello"));
    }

    #[test]
    fn parse_symbol() {
        assert_eq!(parse_one_test("foo").as_symbol(), Some("foo"));
        assert_eq!(parse_one_test("bar/baz").as_symbol(), Some("bar/baz"));
    }

    #[test]
    fn parse_keyword() {
        assert_eq!(parse_one_test(":foo").as_keyword(), Some("foo"));
        assert_eq!(parse_one_test(":bar/baz").as_keyword(), Some("bar/baz"));
    }

    #[test]
    fn parse_empty_list() {
        let ast = parse_one_test("()");
        assert!(matches!(ast, Ast::List(elems, _) if elems.is_empty()));
    }

    #[test]
    fn parse_list() {
        let ast = parse_one_test("(+ 1 2)");
        let elems = ast.as_list().unwrap();
        assert_eq!(elems.len(), 3);
        assert_eq!(elems[0].as_symbol(), Some("+"));
        assert_eq!(elems[1].as_int(), Some(1));
        assert_eq!(elems[2].as_int(), Some(2));
    }

    #[test]
    fn parse_nested_list() {
        let ast = parse_one_test("(* (+ 1 2) 3)");
        let elems = ast.as_list().unwrap();
        assert_eq!(elems.len(), 3);
        assert!(elems[1].is_list());
    }

    #[test]
    fn parse_vector() {
        let ast = parse_one_test("[1 2 3]");
        let elems = ast.as_vector().unwrap();
        assert_eq!(elems.len(), 3);
    }

    #[test]
    fn parse_set() {
        let ast = parse_one_test("#{1 2 3}");
        assert!(matches!(ast, Ast::Set(elems, _) if elems.len() == 3));
    }

    #[test]
    fn parse_map() {
        let ast = parse_one_test("{:a 1 :b 2}");
        assert!(matches!(ast, Ast::Map(entries, _) if entries.len() == 2));
    }

    #[test]
    fn parse_quote() {
        let ast = parse_one_test("'x");
        assert!(matches!(ast, Ast::Quote(inner, _) if inner.as_symbol() == Some("x")));
    }

    #[test]
    fn parse_syntax_quote() {
        let ast = parse_one_test("`x");
        assert!(matches!(ast, Ast::SyntaxQuote(inner, _) if inner.as_symbol() == Some("x")));
    }

    #[test]
    fn parse_unquote() {
        let ast = parse_one_test("~x");
        assert!(matches!(ast, Ast::Unquote(inner, _) if inner.as_symbol() == Some("x")));
    }

    #[test]
    fn parse_unquote_splice() {
        let ast = parse_one_test("~@x");
        assert!(matches!(ast, Ast::UnquoteSplice(inner, _) if inner.as_symbol() == Some("x")));
    }

    #[test]
    fn parse_tagged() {
        let ast = parse_one_test("#entity[3 42]");
        assert!(matches!(ast, Ast::Tagged(tag, _, _) if tag == "entity"));
    }

    #[test]
    fn parse_ignore() {
        // #_ ignores the next form, then parses the following form
        let ast = parse_one_test("#_ ignored-form actual-form");
        assert_eq!(ast.as_symbol(), Some("actual-form"));
    }

    #[test]
    fn parse_multiple_forms() {
        let forms = parse_test("1 2 3");
        assert_eq!(forms.len(), 3);
        assert_eq!(forms[0].as_int(), Some(1));
        assert_eq!(forms[1].as_int(), Some(2));
        assert_eq!(forms[2].as_int(), Some(3));
    }

    #[test]
    fn parse_with_comments() {
        let ast = parse_one_test("; comment\n42");
        assert_eq!(ast.as_int(), Some(42));
    }

    #[test]
    fn parse_complex_expression() {
        let source = r"
            (def x 42)
        ";
        let forms = parse_test(source);
        assert_eq!(forms.len(), 1);
        let elems = forms[0].as_list().unwrap();
        assert_eq!(elems[0].as_symbol(), Some("def"));
        assert_eq!(elems[1].as_symbol(), Some("x"));
        assert_eq!(elems[2].as_int(), Some(42));
    }

    #[test]
    fn parse_rule_declaration() {
        let source = r"
            (rule: apply-damage
              :salience 50
              :where [[?e :health ?hp]
                      [?e :damage ?dmg]]
              :then [(set! ?e :health (- ?hp ?dmg))])
        ";
        let forms = parse_test(source);
        assert_eq!(forms.len(), 1);
        assert!(forms[0].is_list());
    }

    #[test]
    fn parse_error_unterminated_list() {
        let result = parse("(1 2 3");
        assert!(result.is_err());
    }

    #[test]
    fn parse_error_unterminated_string() {
        let result = parse(r#""hello"#);
        assert!(result.is_err());
    }

    #[test]
    fn parse_error_unbalanced_delimiters() {
        let result = parse("(1 2])");
        assert!(result.is_err());
    }

    #[test]
    fn parse_error_odd_map() {
        let result = parse("{:a 1 :b}");
        assert!(result.is_err());
    }

    #[test]
    fn parse_span_tracking() {
        let source = "foo bar";
        let forms = parse_test(source);
        assert_eq!(forms[0].span().start, 0);
        assert_eq!(forms[0].span().end, 3);
        assert_eq!(forms[1].span().start, 4);
        assert_eq!(forms[1].span().end, 7);
    }

    #[test]
    fn parse_commas_as_whitespace() {
        let ast = parse_one_test("[1, 2, 3]");
        let elems = ast.as_vector().unwrap();
        assert_eq!(elems.len(), 3);
    }
}
