//! Core value type for all Longtable data.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::collections::{LtMap, LtSet, LtVec};
use crate::entity::EntityId;
use crate::intern::{KeywordId, SymbolId};
use crate::types::Type;

/// Core value type for all Longtable data.
///
/// Values are immutable and cheaply cloneable (O(1) for most variants).
/// Large composite values use structural sharing via persistent data structures.
#[derive(Clone)]
pub enum Value {
    /// The nil value (represents absence).
    Nil,
    /// Boolean value.
    Bool(bool),
    /// 64-bit signed integer.
    Int(i64),
    /// 64-bit floating point.
    Float(f64),
    /// String value.
    String(Arc<str>),
    /// Interned symbol (identifier).
    Symbol(SymbolId),
    /// Interned keyword (`:name`).
    Keyword(KeywordId),
    /// Entity reference.
    EntityRef(EntityId),
    /// Persistent vector.
    Vec(LtVec<Value>),
    /// Persistent set.
    Set(LtSet<Value>),
    /// Persistent map.
    Map(LtMap<Value, Value>),
    /// Function reference.
    Fn(LtFn),
}

/// Function reference.
///
/// Functions can be either native (Rust) or compiled (bytecode).
#[derive(Clone)]
pub enum LtFn {
    /// Native function implemented in Rust.
    Native(NativeFn),
    /// Compiled function (bytecode index).
    Compiled(CompiledFn),
}

/// Native function callable from Longtable.
#[derive(Clone)]
pub struct NativeFn {
    /// Function name for debugging.
    pub name: &'static str,
    /// Function pointer.
    pub func: fn(&[Value]) -> crate::Result<Value>,
}

/// Compiled function reference.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CompiledFn {
    /// Index into the function table.
    pub index: u32,
    /// Captured environment (for closures).
    pub captures: Option<Arc<Vec<Value>>>,
}

impl Value {
    /// Returns the type of this value.
    #[must_use]
    pub fn value_type(&self) -> Type {
        match self {
            Self::Nil => Type::Nil,
            Self::Bool(_) => Type::Bool,
            Self::Int(_) => Type::Int,
            Self::Float(_) => Type::Float,
            Self::String(_) => Type::String,
            Self::Symbol(_) => Type::Symbol,
            Self::Keyword(_) => Type::Keyword,
            Self::EntityRef(_) => Type::EntityRef,
            Self::Vec(_) => Type::vec(Type::Any),
            Self::Set(_) => Type::set(Type::Any),
            Self::Map(_) => Type::map(Type::Any, Type::Any),
            Self::Fn(_) => Type::Fn(crate::types::Arity::Variadic(0)),
        }
    }

    /// Returns true if this value is nil.
    #[must_use]
    pub const fn is_nil(&self) -> bool {
        matches!(self, Self::Nil)
    }

    /// Returns true if this value is truthy.
    ///
    /// In Longtable, only `nil` and `false` are falsy.
    #[must_use]
    pub fn is_truthy(&self) -> bool {
        !matches!(self, Self::Nil | Self::Bool(false))
    }

    /// Attempts to extract a boolean value.
    #[must_use]
    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Attempts to extract an integer value.
    #[must_use]
    pub const fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(n) => Some(*n),
            _ => None,
        }
    }

    /// Attempts to extract a float value.
    #[must_use]
    pub const fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(n) => Some(*n),
            _ => None,
        }
    }

    /// Attempts to extract a number as f64 (converts int to float).
    ///
    /// Note: Converting large i64 values to f64 may lose precision.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Int(n) => Some(*n as f64),
            Self::Float(n) => Some(*n),
            _ => None,
        }
    }

    /// Attempts to extract a string reference.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Attempts to extract a symbol ID.
    #[must_use]
    pub const fn as_symbol(&self) -> Option<SymbolId> {
        match self {
            Self::Symbol(id) => Some(*id),
            _ => None,
        }
    }

    /// Attempts to extract a keyword ID.
    #[must_use]
    pub const fn as_keyword(&self) -> Option<KeywordId> {
        match self {
            Self::Keyword(id) => Some(*id),
            _ => None,
        }
    }

    /// Attempts to extract an entity ID.
    #[must_use]
    pub const fn as_entity(&self) -> Option<EntityId> {
        match self {
            Self::EntityRef(id) => Some(*id),
            _ => None,
        }
    }

    /// Attempts to extract a vector reference.
    #[must_use]
    pub const fn as_vec(&self) -> Option<&LtVec<Value>> {
        match self {
            Self::Vec(v) => Some(v),
            _ => None,
        }
    }

    /// Attempts to extract a set reference.
    #[must_use]
    pub const fn as_set(&self) -> Option<&LtSet<Value>> {
        match self {
            Self::Set(s) => Some(s),
            _ => None,
        }
    }

    /// Attempts to extract a map reference.
    #[must_use]
    pub const fn as_map(&self) -> Option<&LtMap<Value, Value>> {
        match self {
            Self::Map(m) => Some(m),
            _ => None,
        }
    }
}

// Implement PartialEq manually to handle float comparison
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Nil, Self::Nil) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a.to_bits() == b.to_bits(),
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Symbol(a), Self::Symbol(b)) => a == b,
            (Self::Keyword(a), Self::Keyword(b)) => a == b,
            (Self::EntityRef(a), Self::EntityRef(b)) => a == b,
            (Self::Vec(a), Self::Vec(b)) => a == b,
            (Self::Set(a), Self::Set(b)) => a == b,
            (Self::Map(a), Self::Map(b)) => a == b,
            (Self::Fn(a), Self::Fn(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Value {}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Nil => {}
            Self::Bool(b) => b.hash(state),
            Self::Int(n) => n.hash(state),
            Self::Float(n) => n.to_bits().hash(state),
            Self::String(s) => s.hash(state),
            Self::Symbol(id) => id.hash(state),
            Self::Keyword(id) => id.hash(state),
            Self::EntityRef(id) => id.hash(state),
            Self::Vec(v) => v.hash(state),
            Self::Set(s) => s.hash(state),
            Self::Map(m) => m.hash(state),
            Self::Fn(f) => f.hash(state),
        }
    }
}

impl PartialOrd for Value {
    #[allow(clippy::cast_precision_loss)]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::Nil, Self::Nil) => Some(Ordering::Equal),
            (Self::Bool(a), Self::Bool(b)) => a.partial_cmp(b),
            (Self::Int(a), Self::Int(b)) => a.partial_cmp(b),
            (Self::Float(a), Self::Float(b)) => a.partial_cmp(b),
            // Cross-type numeric comparison intentionally loses precision for large i64
            (Self::Int(a), Self::Float(b)) => (*a as f64).partial_cmp(b),
            (Self::Float(a), Self::Int(b)) => a.partial_cmp(&(*b as f64)),
            (Self::String(a), Self::String(b)) => a.partial_cmp(b),
            (Self::EntityRef(a), Self::EntityRef(b)) => match a.index.cmp(&b.index) {
                Ordering::Equal => Some(a.generation.cmp(&b.generation)),
                ord => Some(ord),
            },
            _ => None, // Different types or non-comparable
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nil => write!(f, "nil"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Int(n) => write!(f, "{n}"),
            Self::Float(n) => write!(f, "{n}"),
            Self::String(s) => write!(f, "{s:?}"),
            Self::Symbol(id) => write!(f, "Symbol({id:?})"),
            Self::Keyword(id) => write!(f, "Keyword({id:?})"),
            Self::EntityRef(id) => write!(f, "{id:?}"),
            Self::Vec(v) => write!(f, "{v:?}"),
            Self::Set(s) => write!(f, "#{s:?}"),
            Self::Map(m) => write!(f, "{m:?}"),
            Self::Fn(func) => write!(f, "{func:?}"),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nil => write!(f, "nil"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Int(n) => write!(f, "{n}"),
            Self::Float(n) => write!(f, "{n}"),
            Self::String(s) => write!(f, "{s}"),
            Self::Symbol(id) => write!(f, "Symbol({id:?})"),
            Self::Keyword(id) => write!(f, ":{id:?}"),
            Self::EntityRef(id) => write!(f, "{id}"),
            Self::Vec(v) => {
                write!(f, "[")?;
                for (i, item) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            Self::Set(s) => {
                write!(f, "#{{")?;
                for (i, item) in s.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "}}")
            }
            Self::Map(m) => {
                write!(f, "{{")?;
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k} {v}")?;
                }
                write!(f, "}}")
            }
            Self::Fn(func) => write!(f, "{func}"),
        }
    }
}

// LtFn implementations

impl PartialEq for LtFn {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Native(a), Self::Native(b)) => std::ptr::fn_addr_eq(a.func, b.func),
            (Self::Compiled(a), Self::Compiled(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for LtFn {}

impl Hash for LtFn {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Native(f) => {
                (f.func as usize).hash(state);
            }
            Self::Compiled(f) => f.hash(state),
        }
    }
}

impl fmt::Debug for LtFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native(func) => write!(f, "<native fn {}>", func.name),
            Self::Compiled(func) => write!(f, "<fn #{}>", func.index),
        }
    }
}

impl fmt::Display for LtFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl fmt::Debug for NativeFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NativeFn({})", self.name)
    }
}

// Convenience From implementations

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Self::Int(n)
    }
}

impl From<i32> for Value {
    fn from(n: i32) -> Self {
        Self::Int(i64::from(n))
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Self::Float(n)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::String(s.into())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Self::String(s.into())
    }
}

impl From<Arc<str>> for Value {
    fn from(s: Arc<str>) -> Self {
        Self::String(s)
    }
}

impl From<EntityId> for Value {
    fn from(id: EntityId) -> Self {
        Self::EntityRef(id)
    }
}

impl From<SymbolId> for Value {
    fn from(id: SymbolId) -> Self {
        Self::Symbol(id)
    }
}

impl From<KeywordId> for Value {
    fn from(id: KeywordId) -> Self {
        Self::Keyword(id)
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Self {
        Self::Vec(v.into_iter().map(Into::into).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_nil() {
        let v = Value::Nil;
        assert!(v.is_nil());
        assert!(!v.is_truthy());
    }

    #[test]
    fn value_bool() {
        assert!(Value::Bool(true).is_truthy());
        assert!(!Value::Bool(false).is_truthy());
        assert_eq!(Value::Bool(true).as_bool(), Some(true));
    }

    #[test]
    fn value_int() {
        let v = Value::Int(42);
        assert_eq!(v.as_int(), Some(42));
        assert_eq!(v.as_number(), Some(42.0));
    }

    #[test]
    fn value_float() {
        let v = Value::Float(2.718);
        assert_eq!(v.as_float(), Some(2.718));
        assert_eq!(v.as_number(), Some(2.718));
    }

    #[test]
    fn value_string() {
        let v = Value::from("hello");
        assert_eq!(v.as_str(), Some("hello"));
    }

    #[test]
    fn value_equality() {
        assert_eq!(Value::Int(1), Value::Int(1));
        assert_ne!(Value::Int(1), Value::Int(2));
        assert_ne!(Value::Int(1), Value::Float(1.0));

        // NaN handling - we use bit equality for Hash consistency,
        // so NaN equals itself (unlike IEEE 754 semantics).
        // This is required for Eq reflexivity.
        let nan = Value::Float(f64::NAN);
        assert_eq!(nan, nan); // NaN == NaN for Eq consistency
    }

    #[test]
    fn value_ordering() {
        assert!(Value::Int(1) < Value::Int(2));
        assert!(Value::Float(1.0) < Value::Float(2.0));
        assert!(Value::from("a") < Value::from("b"));

        // Cross-type numeric comparison
        assert!(Value::Int(1) < Value::Float(2.0));
        assert!(Value::Float(1.0) < Value::Int(2));
    }

    #[test]
    fn value_type() {
        assert_eq!(Value::Nil.value_type(), Type::Nil);
        assert_eq!(Value::Bool(true).value_type(), Type::Bool);
        assert_eq!(Value::Int(42).value_type(), Type::Int);
        assert_eq!(Value::Float(2.718).value_type(), Type::Float);
    }

    #[test]
    fn value_from_vec() {
        let v: Value = vec![1i32, 2, 3].into();
        let vec = v.as_vec().unwrap();
        assert_eq!(vec.len(), 3);
        assert_eq!(vec.get(0), Some(&Value::Int(1)));
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::hash_map::DefaultHasher;

    fn hash_value(v: &Value) -> u64 {
        let mut hasher = DefaultHasher::new();
        v.hash(&mut hasher);
        hasher.finish()
    }

    /// Strategy to generate scalar Value variants (no recursion).
    fn scalar_value() -> impl Strategy<Value = Value> {
        prop_oneof![
            Just(Value::Nil),
            any::<bool>().prop_map(Value::Bool),
            any::<i64>().prop_map(Value::Int),
            any::<f64>().prop_map(Value::Float),
            "[a-zA-Z0-9]{0,20}".prop_map(|s| Value::from(s.as_str())),
        ]
    }

    proptest! {
        #[test]
        fn eq_reflexivity(v in scalar_value()) {
            // Every value must be equal to itself (Eq reflexivity).
            prop_assert_eq!(&v, &v);
        }

        #[test]
        fn eq_hash_consistency(v in scalar_value()) {
            // If two values are equal, they must have the same hash.
            // Test by hashing the same value twice.
            let h1 = hash_value(&v);
            let h2 = hash_value(&v);
            prop_assert_eq!(h1, h2, "Same value must hash consistently");
        }

        #[test]
        fn nil_equality(_unused in Just(())) {
            let a = Value::Nil;
            let b = Value::Nil;
            prop_assert_eq!(&a, &b);
            prop_assert_eq!(hash_value(&a), hash_value(&b));
        }

        #[test]
        fn bool_eq_hash(b1 in any::<bool>(), b2 in any::<bool>()) {
            let v1 = Value::Bool(b1);
            let v2 = Value::Bool(b2);
            if b1 == b2 {
                prop_assert_eq!(&v1, &v2);
                prop_assert_eq!(hash_value(&v1), hash_value(&v2));
            } else {
                prop_assert_ne!(&v1, &v2);
            }
        }

        #[test]
        fn int_eq_hash(n1 in any::<i64>(), n2 in any::<i64>()) {
            let v1 = Value::Int(n1);
            let v2 = Value::Int(n2);
            if n1 == n2 {
                prop_assert_eq!(&v1, &v2);
                prop_assert_eq!(hash_value(&v1), hash_value(&v2));
            } else {
                prop_assert_ne!(&v1, &v2);
            }
        }

        #[test]
        fn float_eq_hash(f1 in any::<f64>(), f2 in any::<f64>()) {
            let v1 = Value::Float(f1);
            let v2 = Value::Float(f2);
            // We use bit equality, so NaN == NaN
            if f1.to_bits() == f2.to_bits() {
                prop_assert_eq!(&v1, &v2);
                prop_assert_eq!(hash_value(&v1), hash_value(&v2));
            } else {
                prop_assert_ne!(&v1, &v2);
            }
        }

        #[test]
        fn string_eq_hash(s1 in "[a-zA-Z0-9]{0,20}", s2 in "[a-zA-Z0-9]{0,20}") {
            let v1 = Value::from(s1.as_str());
            let v2 = Value::from(s2.as_str());
            if s1 == s2 {
                prop_assert_eq!(&v1, &v2);
                prop_assert_eq!(hash_value(&v1), hash_value(&v2));
            } else {
                prop_assert_ne!(&v1, &v2);
            }
        }

        #[test]
        fn different_types_not_equal(
            b in any::<bool>(),
            n in any::<i64>(),
            f in any::<f64>(),
            s in "[a-zA-Z0-9]{0,10}"
        ) {
            // Values of different types are never equal
            let bool_val = Value::Bool(b);
            let int_val = Value::Int(n);
            let float_val = Value::Float(f);
            let str_val = Value::from(s.as_str());
            let nil_val = Value::Nil;

            prop_assert_ne!(&nil_val, &bool_val);
            prop_assert_ne!(&nil_val, &int_val);
            prop_assert_ne!(&nil_val, &float_val);
            prop_assert_ne!(&nil_val, &str_val);
            prop_assert_ne!(&bool_val, &int_val);
            prop_assert_ne!(&bool_val, &float_val);
            prop_assert_ne!(&bool_val, &str_val);
            prop_assert_ne!(&int_val, &float_val);
            prop_assert_ne!(&int_val, &str_val);
            prop_assert_ne!(&float_val, &str_val);
        }
    }
}
