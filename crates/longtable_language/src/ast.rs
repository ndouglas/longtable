//! Abstract Syntax Tree for the Longtable DSL.
//!
//! The AST represents the structure of parsed Longtable source code.

use crate::span::Span;

/// An AST node.
#[derive(Clone, Debug, PartialEq)]
pub enum Ast {
    /// `nil`
    Nil(Span),
    /// `true` or `false`
    Bool(bool, Span),
    /// Integer literal like `42`
    Int(i64, Span),
    /// Float literal like `3.14`
    Float(f64, Span),
    /// String literal like `"hello"`
    String(String, Span),
    /// Symbol like `foo` or `bar/baz`
    Symbol(String, Span),
    /// Keyword like `:foo` or `:bar/baz`
    Keyword(String, Span),

    /// List form like `(+ 1 2)`
    List(Vec<Ast>, Span),
    /// Vector form like `[1 2 3]`
    Vector(Vec<Ast>, Span),
    /// Set form like `#{1 2 3}`
    Set(Vec<Ast>, Span),
    /// Map form like `{:a 1 :b 2}`
    Map(Vec<(Ast, Ast)>, Span),

    /// Quoted form like `'x`
    Quote(Box<Ast>, Span),
    /// Unquoted form like `~x`
    Unquote(Box<Ast>, Span),
    /// Unquote-spliced form like `~@x`
    UnquoteSplice(Box<Ast>, Span),
    /// Syntax-quoted form like `` `x ``
    SyntaxQuote(Box<Ast>, Span),

    /// Tagged literal like `#entity[3.42]`
    Tagged(String, Box<Ast>, Span),
}

impl Ast {
    /// Returns the source span of this AST node.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Nil(s)
            | Self::Bool(_, s)
            | Self::Int(_, s)
            | Self::Float(_, s)
            | Self::String(_, s)
            | Self::Symbol(_, s)
            | Self::Keyword(_, s)
            | Self::List(_, s)
            | Self::Vector(_, s)
            | Self::Set(_, s)
            | Self::Map(_, s)
            | Self::Quote(_, s)
            | Self::Unquote(_, s)
            | Self::UnquoteSplice(_, s)
            | Self::SyntaxQuote(_, s)
            | Self::Tagged(_, _, s) => *s,
        }
    }

    /// Returns true if this is a nil literal.
    #[must_use]
    pub const fn is_nil(&self) -> bool {
        matches!(self, Self::Nil(_))
    }

    /// Returns true if this is a boolean literal.
    #[must_use]
    pub const fn is_bool(&self) -> bool {
        matches!(self, Self::Bool(_, _))
    }

    /// Returns true if this is an integer literal.
    #[must_use]
    pub const fn is_int(&self) -> bool {
        matches!(self, Self::Int(_, _))
    }

    /// Returns true if this is a float literal.
    #[must_use]
    pub const fn is_float(&self) -> bool {
        matches!(self, Self::Float(_, _))
    }

    /// Returns true if this is a string literal.
    #[must_use]
    pub const fn is_string(&self) -> bool {
        matches!(self, Self::String(_, _))
    }

    /// Returns true if this is a symbol.
    #[must_use]
    pub const fn is_symbol(&self) -> bool {
        matches!(self, Self::Symbol(_, _))
    }

    /// Returns true if this is a keyword.
    #[must_use]
    pub const fn is_keyword(&self) -> bool {
        matches!(self, Self::Keyword(_, _))
    }

    /// Returns true if this is a list.
    #[must_use]
    pub const fn is_list(&self) -> bool {
        matches!(self, Self::List(_, _))
    }

    /// Returns true if this is a vector.
    #[must_use]
    pub const fn is_vector(&self) -> bool {
        matches!(self, Self::Vector(_, _))
    }

    /// Returns true if this is a set.
    #[must_use]
    pub const fn is_set(&self) -> bool {
        matches!(self, Self::Set(_, _))
    }

    /// Returns true if this is a map.
    #[must_use]
    pub const fn is_map(&self) -> bool {
        matches!(self, Self::Map(_, _))
    }

    /// Returns the elements of a list, or None if not a list.
    #[must_use]
    pub fn as_list(&self) -> Option<&[Ast]> {
        match self {
            Self::List(elements, _) => Some(elements),
            _ => None,
        }
    }

    /// Returns the elements of a vector, or None if not a vector.
    #[must_use]
    pub fn as_vector(&self) -> Option<&[Ast]> {
        match self {
            Self::Vector(elements, _) => Some(elements),
            _ => None,
        }
    }

    /// Returns the symbol name, or None if not a symbol.
    #[must_use]
    pub fn as_symbol(&self) -> Option<&str> {
        match self {
            Self::Symbol(name, _) => Some(name),
            _ => None,
        }
    }

    /// Returns the keyword name, or None if not a keyword.
    #[must_use]
    pub fn as_keyword(&self) -> Option<&str> {
        match self {
            Self::Keyword(name, _) => Some(name),
            _ => None,
        }
    }

    /// Returns the integer value, or None if not an integer.
    #[must_use]
    pub const fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(n, _) => Some(*n),
            _ => None,
        }
    }

    /// Returns the string value, or None if not a string.
    #[must_use]
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(s, _) => Some(s),
            _ => None,
        }
    }

    /// A human-readable type name for this AST node.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Nil(_) => "nil",
            Self::Bool(_, _) => "bool",
            Self::Int(_, _) => "int",
            Self::Float(_, _) => "float",
            Self::String(_, _) => "string",
            Self::Symbol(_, _) => "symbol",
            Self::Keyword(_, _) => "keyword",
            Self::List(_, _) => "list",
            Self::Vector(_, _) => "vector",
            Self::Set(_, _) => "set",
            Self::Map(_, _) => "map",
            Self::Quote(_, _) => "quote",
            Self::Unquote(_, _) => "unquote",
            Self::UnquoteSplice(_, _) => "unquote-splice",
            Self::SyntaxQuote(_, _) => "syntax-quote",
            Self::Tagged(_, _, _) => "tagged",
        }
    }
}

/// Helper constructors for AST nodes (for testing).
impl Ast {
    /// Creates a nil node with default span.
    #[cfg(test)]
    pub fn nil() -> Self {
        Self::Nil(Span::default())
    }

    /// Creates a bool node with default span.
    #[cfg(test)]
    pub fn bool_lit(b: bool) -> Self {
        Self::Bool(b, Span::default())
    }

    /// Creates an int node with default span.
    #[cfg(test)]
    pub fn int(n: i64) -> Self {
        Self::Int(n, Span::default())
    }

    /// Creates a float node with default span.
    #[cfg(test)]
    pub fn float(n: f64) -> Self {
        Self::Float(n, Span::default())
    }

    /// Creates a string node with default span.
    #[cfg(test)]
    pub fn string(s: impl Into<String>) -> Self {
        Self::String(s.into(), Span::default())
    }

    /// Creates a symbol node with default span.
    #[cfg(test)]
    pub fn symbol(s: impl Into<String>) -> Self {
        Self::Symbol(s.into(), Span::default())
    }

    /// Creates a keyword node with default span.
    #[cfg(test)]
    pub fn keyword(s: impl Into<String>) -> Self {
        Self::Keyword(s.into(), Span::default())
    }

    /// Creates a list node with default span.
    #[cfg(test)]
    pub fn list(elements: Vec<Ast>) -> Self {
        Self::List(elements, Span::default())
    }

    /// Creates a vector node with default span.
    #[cfg(test)]
    pub fn vector(elements: Vec<Ast>) -> Self {
        Self::Vector(elements, Span::default())
    }

    /// Creates a set node with default span.
    #[cfg(test)]
    pub fn set(elements: Vec<Ast>) -> Self {
        Self::Set(elements, Span::default())
    }

    /// Creates a map node with default span.
    #[cfg(test)]
    pub fn map(entries: Vec<(Ast, Ast)>) -> Self {
        Self::Map(entries, Span::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ast_type_predicates() {
        assert!(Ast::nil().is_nil());
        assert!(Ast::bool_lit(true).is_bool());
        assert!(Ast::int(42).is_int());
        assert!(Ast::float(3.14).is_float());
        assert!(Ast::string("hi").is_string());
        assert!(Ast::symbol("foo").is_symbol());
        assert!(Ast::keyword("bar").is_keyword());
        assert!(Ast::list(vec![]).is_list());
        assert!(Ast::vector(vec![]).is_vector());
        assert!(Ast::set(vec![]).is_set());
        assert!(Ast::map(vec![]).is_map());
    }

    #[test]
    fn ast_accessors() {
        assert_eq!(Ast::int(42).as_int(), Some(42));
        assert_eq!(Ast::int(42).as_symbol(), None);

        assert_eq!(Ast::symbol("foo").as_symbol(), Some("foo"));
        assert_eq!(Ast::keyword("bar").as_keyword(), Some("bar"));
        assert_eq!(Ast::string("hi").as_string(), Some("hi"));

        let list = Ast::list(vec![Ast::int(1), Ast::int(2)]);
        assert_eq!(list.as_list().map(|l| l.len()), Some(2));

        let vec = Ast::vector(vec![Ast::int(1)]);
        assert_eq!(vec.as_vector().map(|v| v.len()), Some(1));
    }

    #[test]
    fn ast_type_name() {
        assert_eq!(Ast::nil().type_name(), "nil");
        assert_eq!(Ast::int(42).type_name(), "int");
        assert_eq!(Ast::list(vec![]).type_name(), "list");
    }

    #[test]
    fn ast_span() {
        let span = Span::new(5, 10, 2, 3);
        let ast = Ast::Int(42, span);
        assert_eq!(ast.span(), span);
    }
}
