//! Logic operations and type predicates for the VM.

use super::is_truthy;
use longtable_foundation::{Result, Value};

// =============================================================================
// Logic Functions
// =============================================================================

/// Logic: and - returns first falsy value or last value
pub(crate) fn native_and(args: &[Value]) -> Result<Value> {
    for arg in args {
        if !is_truthy(arg) {
            return Ok(arg.clone());
        }
    }
    Ok(args.last().cloned().unwrap_or(Value::Bool(true)))
}

/// Logic: or - returns first truthy value or last value
pub(crate) fn native_or(args: &[Value]) -> Result<Value> {
    for arg in args {
        if is_truthy(arg) {
            return Ok(arg.clone());
        }
    }
    Ok(args.last().cloned().unwrap_or(Value::Bool(false)))
}

// =============================================================================
// Type Predicates
// =============================================================================

/// Predicate: nil?
pub(crate) fn native_nil_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Nil))))
}

/// Predicate: some?
pub(crate) fn native_some_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(!matches!(
        args.first(),
        Some(Value::Nil) | None
    )))
}

/// Predicate: int?
pub(crate) fn native_int_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Int(_)))))
}

/// Predicate: float?
pub(crate) fn native_float_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Float(_)))))
}

/// Predicate: string?
pub(crate) fn native_string_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::String(_)))))
}

/// Predicate: keyword?
pub(crate) fn native_keyword_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Keyword(_)))))
}

/// Predicate: symbol?
pub(crate) fn native_symbol_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Symbol(_)))))
}

/// Predicate: list? (vectors are lists in our model)
pub(crate) fn native_list_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Vec(_)))))
}

/// Predicate: vector?
pub(crate) fn native_vector_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Vec(_)))))
}

/// Predicate: map?
pub(crate) fn native_map_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Map(_)))))
}

/// Predicate: set?
pub(crate) fn native_set_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Set(_)))))
}

/// Predicate: bool?
pub(crate) fn native_bool_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Bool(_)))))
}

/// Predicate: number? - true for int or float
pub(crate) fn native_number_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(
        args.first(),
        Some(Value::Int(_) | Value::Float(_))
    )))
}

/// Predicate: coll? - true for vec, set, or map
pub(crate) fn native_coll_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(
        args.first(),
        Some(Value::Vec(_) | Value::Set(_) | Value::Map(_))
    )))
}

/// Predicate: fn?
pub(crate) fn native_fn_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Fn(_)))))
}

/// Predicate: entity?
pub(crate) fn native_entity_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(
        args.first(),
        Some(Value::EntityRef(_))
    )))
}

/// Misc: type (returns type as keyword string)
pub(crate) fn native_type(args: &[Value]) -> Result<Value> {
    let type_name = match args.first() {
        Some(Value::Nil) => "nil",
        Some(Value::Bool(_)) => "bool",
        Some(Value::Int(_)) => "int",
        Some(Value::Float(_)) => "float",
        Some(Value::String(_)) => "string",
        Some(Value::Symbol(_)) => "symbol",
        Some(Value::Keyword(_)) => "keyword",
        Some(Value::EntityRef(_)) => "entity",
        Some(Value::Vec(_)) => "vector",
        Some(Value::Set(_)) => "set",
        Some(Value::Map(_)) => "map",
        Some(Value::Fn(_)) => "fn",
        None => "nil",
    };
    Ok(Value::String(format!(":{type_name}").into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use longtable_foundation::{CompiledFn, EntityId, Interner, LtFn, LtMap, LtSet, LtVec};

    fn as_bool(v: &Value) -> bool {
        match v {
            Value::Bool(b) => *b,
            _ => panic!("Expected bool, got {v:?}"),
        }
    }

    fn empty_vec() -> LtVec<Value> {
        LtVec::new()
    }

    // ==================== Logic Functions ====================

    #[test]
    fn test_and_all_truthy() {
        let result = native_and(&[Value::Int(1), Value::Int(2), Value::Int(3)]).unwrap();
        // Returns last value
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn test_and_with_false() {
        let result = native_and(&[Value::Int(1), Value::Bool(false), Value::Int(3)]).unwrap();
        // Returns first falsy
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn test_and_with_nil() {
        let result = native_and(&[Value::Int(1), Value::Nil, Value::Int(3)]).unwrap();
        // Returns first falsy (nil)
        assert_eq!(result, Value::Nil);
    }

    #[test]
    fn test_and_empty() {
        let result = native_and(&[]).unwrap();
        // Empty returns true
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn test_or_first_truthy() {
        let result = native_or(&[Value::Nil, Value::Bool(false), Value::Int(42)]).unwrap();
        // Returns first truthy
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_or_all_falsy() {
        let result = native_or(&[Value::Nil, Value::Bool(false)]).unwrap();
        // Returns last value
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn test_or_empty() {
        let result = native_or(&[]).unwrap();
        // Empty returns false
        assert_eq!(result, Value::Bool(false));
    }

    // ==================== Type Predicates ====================

    #[test]
    fn test_nil_p_true() {
        assert!(as_bool(&native_nil_p(&[Value::Nil]).unwrap()));
    }

    #[test]
    fn test_nil_p_false() {
        assert!(!as_bool(&native_nil_p(&[Value::Int(0)]).unwrap()));
    }

    #[test]
    fn test_some_p_true() {
        assert!(as_bool(&native_some_p(&[Value::Int(0)]).unwrap()));
    }

    #[test]
    fn test_some_p_false_nil() {
        assert!(!as_bool(&native_some_p(&[Value::Nil]).unwrap()));
    }

    #[test]
    fn test_some_p_false_empty() {
        assert!(!as_bool(&native_some_p(&[]).unwrap()));
    }

    #[test]
    fn test_int_p_true() {
        assert!(as_bool(&native_int_p(&[Value::Int(42)]).unwrap()));
    }

    #[test]
    fn test_int_p_false() {
        assert!(!as_bool(&native_int_p(&[Value::Float(42.0)]).unwrap()));
    }

    #[test]
    fn test_float_p_true() {
        assert!(as_bool(&native_float_p(&[Value::Float(2.5)]).unwrap()));
    }

    #[test]
    fn test_float_p_false() {
        assert!(!as_bool(&native_float_p(&[Value::Int(3)]).unwrap()));
    }

    #[test]
    fn test_string_p_true() {
        assert!(as_bool(
            &native_string_p(&[Value::String("hello".into())]).unwrap()
        ));
    }

    #[test]
    fn test_string_p_false() {
        assert!(!as_bool(&native_string_p(&[Value::Int(42)]).unwrap()));
    }

    #[test]
    fn test_keyword_p_true() {
        let mut interner = Interner::new();
        let kw = interner.intern_keyword("test");
        assert!(as_bool(&native_keyword_p(&[Value::Keyword(kw)]).unwrap()));
    }

    #[test]
    fn test_keyword_p_false() {
        let mut interner = Interner::new();
        let sym = interner.intern_symbol("test");
        assert!(!as_bool(&native_keyword_p(&[Value::Symbol(sym)]).unwrap()));
    }

    #[test]
    fn test_symbol_p_true() {
        let mut interner = Interner::new();
        let sym = interner.intern_symbol("test");
        assert!(as_bool(&native_symbol_p(&[Value::Symbol(sym)]).unwrap()));
    }

    #[test]
    fn test_symbol_p_false() {
        let mut interner = Interner::new();
        let kw = interner.intern_keyword("test");
        assert!(!as_bool(&native_symbol_p(&[Value::Keyword(kw)]).unwrap()));
    }

    #[test]
    fn test_list_p_true() {
        assert!(as_bool(&native_list_p(&[Value::Vec(empty_vec())]).unwrap()));
    }

    #[test]
    fn test_list_p_false() {
        assert!(!as_bool(
            &native_list_p(&[Value::Set(LtSet::new())]).unwrap()
        ));
    }

    #[test]
    fn test_vector_p_true() {
        assert!(as_bool(
            &native_vector_p(&[Value::Vec(empty_vec())]).unwrap()
        ));
    }

    #[test]
    fn test_map_p_true() {
        assert!(as_bool(&native_map_p(&[Value::Map(LtMap::new())]).unwrap()));
    }

    #[test]
    fn test_map_p_false() {
        assert!(!as_bool(&native_map_p(&[Value::Vec(empty_vec())]).unwrap()));
    }

    #[test]
    fn test_set_p_true() {
        assert!(as_bool(&native_set_p(&[Value::Set(LtSet::new())]).unwrap()));
    }

    #[test]
    fn test_set_p_false() {
        assert!(!as_bool(&native_set_p(&[Value::Vec(empty_vec())]).unwrap()));
    }

    #[test]
    fn test_bool_p_true() {
        assert!(as_bool(&native_bool_p(&[Value::Bool(true)]).unwrap()));
    }

    #[test]
    fn test_bool_p_false() {
        assert!(!as_bool(&native_bool_p(&[Value::Int(1)]).unwrap()));
    }

    #[test]
    fn test_number_p_int() {
        assert!(as_bool(&native_number_p(&[Value::Int(42)]).unwrap()));
    }

    #[test]
    fn test_number_p_float() {
        assert!(as_bool(&native_number_p(&[Value::Float(2.5)]).unwrap()));
    }

    #[test]
    fn test_number_p_false() {
        assert!(!as_bool(
            &native_number_p(&[Value::String("42".into())]).unwrap()
        ));
    }

    #[test]
    fn test_coll_p_vec() {
        assert!(as_bool(&native_coll_p(&[Value::Vec(empty_vec())]).unwrap()));
    }

    #[test]
    fn test_coll_p_set() {
        assert!(as_bool(
            &native_coll_p(&[Value::Set(LtSet::new())]).unwrap()
        ));
    }

    #[test]
    fn test_coll_p_map() {
        assert!(as_bool(
            &native_coll_p(&[Value::Map(LtMap::new())]).unwrap()
        ));
    }

    #[test]
    fn test_coll_p_false() {
        assert!(!as_bool(&native_coll_p(&[Value::Int(42)]).unwrap()));
    }

    #[test]
    fn test_fn_p_true() {
        let f = LtFn::Compiled(CompiledFn {
            index: 0,
            captures: None,
        });
        assert!(as_bool(&native_fn_p(&[Value::Fn(f)]).unwrap()));
    }

    #[test]
    fn test_fn_p_false() {
        assert!(!as_bool(&native_fn_p(&[Value::Int(42)]).unwrap()));
    }

    #[test]
    fn test_entity_p_true() {
        assert!(as_bool(
            &native_entity_p(&[Value::EntityRef(EntityId::new(1, 1))]).unwrap()
        ));
    }

    #[test]
    fn test_entity_p_false() {
        assert!(!as_bool(&native_entity_p(&[Value::Int(42)]).unwrap()));
    }

    // ==================== Type Function ====================

    #[test]
    fn test_type_nil() {
        let result = native_type(&[Value::Nil]).unwrap();
        assert_eq!(result, Value::String(":nil".into()));
    }

    #[test]
    fn test_type_bool() {
        let result = native_type(&[Value::Bool(true)]).unwrap();
        assert_eq!(result, Value::String(":bool".into()));
    }

    #[test]
    fn test_type_int() {
        let result = native_type(&[Value::Int(42)]).unwrap();
        assert_eq!(result, Value::String(":int".into()));
    }

    #[test]
    fn test_type_float() {
        let result = native_type(&[Value::Float(2.5)]).unwrap();
        assert_eq!(result, Value::String(":float".into()));
    }

    #[test]
    fn test_type_string() {
        let result = native_type(&[Value::String("hello".into())]).unwrap();
        assert_eq!(result, Value::String(":string".into()));
    }

    #[test]
    fn test_type_vector() {
        let result = native_type(&[Value::Vec(empty_vec())]).unwrap();
        assert_eq!(result, Value::String(":vector".into()));
    }

    #[test]
    fn test_type_set() {
        let result = native_type(&[Value::Set(LtSet::new())]).unwrap();
        assert_eq!(result, Value::String(":set".into()));
    }

    #[test]
    fn test_type_map() {
        let result = native_type(&[Value::Map(LtMap::new())]).unwrap();
        assert_eq!(result, Value::String(":map".into()));
    }

    #[test]
    fn test_type_entity() {
        let result = native_type(&[Value::EntityRef(EntityId::new(1, 1))]).unwrap();
        assert_eq!(result, Value::String(":entity".into()));
    }

    #[test]
    fn test_type_empty() {
        let result = native_type(&[]).unwrap();
        assert_eq!(result, Value::String(":nil".into()));
    }
}
