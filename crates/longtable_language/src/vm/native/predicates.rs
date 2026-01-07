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
