//! Type descriptors for schema validation.

use std::fmt;

/// Type descriptor for schema validation.
///
/// Used to declare component field types and validate values at runtime.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Type {
    /// The nil type (only value: nil).
    Nil,
    /// Boolean type.
    Bool,
    /// 64-bit signed integer.
    Int,
    /// 64-bit floating point.
    Float,
    /// String type.
    String,
    /// Symbol type (interned identifier).
    Symbol,
    /// Keyword type (interned, prefixed with `:`).
    Keyword,
    /// Entity reference type.
    EntityRef,
    /// Homogeneous vector type.
    Vec(Box<Type>),
    /// Homogeneous set type.
    Set(Box<Type>),
    /// Homogeneous map type.
    Map(Box<Type>, Box<Type>),
    /// Optional type (value or nil).
    Option(Box<Type>),
    /// Any type (accepts any value).
    Any,
    /// Function type (arity only, no parameter types).
    Fn(Arity),
}

/// Function arity specification.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Arity {
    /// Exactly N arguments.
    Exact(usize),
    /// Between min and max arguments (inclusive).
    Range(usize, usize),
    /// At least N arguments, then any number more.
    Variadic(usize),
}

impl Type {
    /// Creates a vector type with the given element type.
    #[must_use]
    pub fn vec(element: Type) -> Self {
        Self::Vec(Box::new(element))
    }

    /// Creates a set type with the given element type.
    #[must_use]
    pub fn set(element: Type) -> Self {
        Self::Set(Box::new(element))
    }

    /// Creates a map type with the given key and value types.
    #[must_use]
    pub fn map(key: Type, value: Type) -> Self {
        Self::Map(Box::new(key), Box::new(value))
    }

    /// Creates an optional type.
    #[must_use]
    pub fn option(inner: Type) -> Self {
        Self::Option(Box::new(inner))
    }

    /// Returns true if this type is `Any`.
    #[must_use]
    pub const fn is_any(&self) -> bool {
        matches!(self, Self::Any)
    }

    /// Returns true if this type can be nil.
    #[must_use]
    pub const fn is_nullable(&self) -> bool {
        matches!(self, Self::Nil | Self::Option(_) | Self::Any)
    }
}

impl fmt::Debug for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nil => write!(f, "nil"),
            Self::Bool => write!(f, "bool"),
            Self::Int => write!(f, "int"),
            Self::Float => write!(f, "float"),
            Self::String => write!(f, "string"),
            Self::Symbol => write!(f, "symbol"),
            Self::Keyword => write!(f, "keyword"),
            Self::EntityRef => write!(f, "entity-ref"),
            Self::Vec(t) => write!(f, "vec<{t:?}>"),
            Self::Set(t) => write!(f, "set<{t:?}>"),
            Self::Map(k, v) => write!(f, "map<{k:?}, {v:?}>"),
            Self::Option(t) => write!(f, "option<{t:?}>"),
            Self::Any => write!(f, "any"),
            Self::Fn(arity) => write!(f, "fn{arity:?}"),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_equality() {
        assert_eq!(Type::Int, Type::Int);
        assert_ne!(Type::Int, Type::Float);

        assert_eq!(Type::vec(Type::Int), Type::vec(Type::Int));
        assert_ne!(Type::vec(Type::Int), Type::vec(Type::Float));
    }

    #[test]
    fn type_display() {
        assert_eq!(format!("{}", Type::Int), "int");
        assert_eq!(format!("{}", Type::vec(Type::String)), "vec<string>");
        assert_eq!(
            format!("{}", Type::map(Type::Keyword, Type::Any)),
            "map<keyword, any>"
        );
    }

    #[test]
    fn nullable_types() {
        assert!(Type::Nil.is_nullable());
        assert!(Type::option(Type::Int).is_nullable());
        assert!(Type::Any.is_nullable());
        assert!(!Type::Int.is_nullable());
        assert!(!Type::String.is_nullable());
    }
}
