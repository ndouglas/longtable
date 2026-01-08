//! Type descriptors for schema validation.

use std::fmt;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Type descriptor for schema validation.
///
/// Used to declare component field types and validate values at runtime.
#[derive(Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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

    /// Checks if a value type is accepted by this type.
    ///
    /// This performs structural type checking:
    /// - `Any` accepts all types
    /// - `Option(T)` accepts `Nil` and any type that `T` accepts
    /// - Primitive types must match exactly
    /// - Collection types check element types recursively
    #[must_use]
    pub fn accepts(&self, value_type: &Type) -> bool {
        // Any accepts everything
        if matches!(self, Self::Any) {
            return true;
        }

        // Option accepts nil or the inner type
        if let Self::Option(inner) = self {
            return matches!(value_type, Self::Nil) || inner.accepts(value_type);
        }

        // Check for exact type matches and special cases
        match (self, value_type) {
            // Exact type matches (including numeric promotion: Float accepts Int)
            (Self::Nil, Self::Nil)
            | (Self::Bool, Self::Bool)
            | (Self::Int | Self::Float, Self::Int)
            | (Self::Float, Self::Float)
            | (Self::String, Self::String)
            | (Self::Symbol, Self::Symbol)
            | (Self::Keyword, Self::Keyword)
            | (Self::EntityRef, Self::EntityRef) => true,

            // Collection types - Vec(Any), Set(Any), Map(Any,Any) indicate runtime values
            // where element types are not known statically. Accept these when expecting
            // any collection of that kind, since we can't inspect element types efficiently.
            (Self::Vec(_), Self::Vec(actual_elem)) => {
                actual_elem.is_any() || self.accepts_vec_element(actual_elem)
            }
            (Self::Set(_), Self::Set(actual_elem)) => {
                actual_elem.is_any() || self.accepts_set_element(actual_elem)
            }
            (Self::Map(_, _), Self::Map(actual_k, actual_v)) => {
                (actual_k.is_any() && actual_v.is_any())
                    || self.accepts_map_elements(actual_k, actual_v)
            }

            // Function types - just check arity
            (Self::Fn(expected_arity), Self::Fn(actual_arity)) => {
                Self::arity_compatible(expected_arity, actual_arity)
            }

            // No match
            _ => false,
        }
    }

    /// Helper for Vec element type checking.
    fn accepts_vec_element(&self, actual_elem: &Type) -> bool {
        if let Self::Vec(expected_elem) = self {
            expected_elem.accepts(actual_elem)
        } else {
            false
        }
    }

    /// Helper for Set element type checking.
    fn accepts_set_element(&self, actual_elem: &Type) -> bool {
        if let Self::Set(expected_elem) = self {
            expected_elem.accepts(actual_elem)
        } else {
            false
        }
    }

    /// Helper for Map element type checking.
    fn accepts_map_elements(&self, actual_k: &Type, actual_v: &Type) -> bool {
        if let Self::Map(expected_k, expected_v) = self {
            expected_k.accepts(actual_k) && expected_v.accepts(actual_v)
        } else {
            false
        }
    }

    /// Helper for function arity compatibility.
    fn arity_compatible(expected: &Arity, actual: &Arity) -> bool {
        match (expected, actual) {
            (Arity::Exact(e), Arity::Exact(a)) => e == a,
            (Arity::Range(e_min, e_max), Arity::Exact(a)) => *a >= *e_min && *a <= *e_max,
            (Arity::Variadic(e_min), Arity::Exact(a)) => *a >= *e_min,
            (Arity::Variadic(e_min), Arity::Variadic(a_min)) => *a_min >= *e_min,
            // Conservative: accept if either has variadic
            (Arity::Variadic(_), _) | (_, Arity::Variadic(_)) => true,
            _ => false,
        }
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

    #[test]
    fn accepts_any() {
        assert!(Type::Any.accepts(&Type::Int));
        assert!(Type::Any.accepts(&Type::String));
        assert!(Type::Any.accepts(&Type::Nil));
        assert!(Type::Any.accepts(&Type::vec(Type::Int)));
    }

    #[test]
    fn accepts_primitives() {
        assert!(Type::Int.accepts(&Type::Int));
        assert!(Type::Bool.accepts(&Type::Bool));
        assert!(Type::String.accepts(&Type::String));
        assert!(Type::Keyword.accepts(&Type::Keyword));
        assert!(Type::EntityRef.accepts(&Type::EntityRef));

        assert!(!Type::Int.accepts(&Type::String));
        assert!(!Type::Bool.accepts(&Type::Int));
    }

    #[test]
    fn accepts_numeric_promotion() {
        // Float should accept Int (numeric promotion)
        assert!(Type::Float.accepts(&Type::Int));
        // But Int should not accept Float
        assert!(!Type::Int.accepts(&Type::Float));
    }

    #[test]
    fn accepts_option() {
        let opt_int = Type::option(Type::Int);
        assert!(opt_int.accepts(&Type::Nil));
        assert!(opt_int.accepts(&Type::Int));
        assert!(!opt_int.accepts(&Type::String));
    }

    #[test]
    fn accepts_collections() {
        let vec_int = Type::vec(Type::Int);
        let vec_string = Type::vec(Type::String);
        let vec_any = Type::vec(Type::Any);

        assert!(vec_int.accepts(&Type::vec(Type::Int)));
        assert!(!vec_int.accepts(&vec_string));
        assert!(vec_any.accepts(&vec_int)); // Any element accepts Int element

        let map_kw_int = Type::map(Type::Keyword, Type::Int);
        let map_kw_string = Type::map(Type::Keyword, Type::String);
        assert!(map_kw_int.accepts(&Type::map(Type::Keyword, Type::Int)));
        assert!(!map_kw_int.accepts(&map_kw_string));
    }
}
