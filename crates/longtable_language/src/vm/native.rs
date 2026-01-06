//! Native function implementations for the VM.
//!
//! This module contains all builtin functions and arithmetic/comparison helpers.

#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::match_same_arms)]

use longtable_foundation::{Error, ErrorKind, LtMap, LtVec, Result, Value};

// =============================================================================
// Arithmetic and Comparison Helpers
// =============================================================================

/// Checks if a value is truthy.
pub(crate) fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Nil => false,
        Value::Bool(b) => *b,
        _ => true,
    }
}

/// Adds two values.
pub(crate) fn add_values(a: Value, b: Value) -> Result<Value> {
    match (&a, &b) {
        (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x + y)),
        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x + y)),
        (Value::Int(x), Value::Float(y)) => Ok(Value::Float(*x as f64 + y)),
        (Value::Float(x), Value::Int(y)) => Ok(Value::Float(x + *y as f64)),
        (Value::String(x), Value::String(y)) => Ok(Value::String(format!("{x}{y}").into())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
            actual: a.value_type(),
        })),
    }
}

/// Subtracts two values.
pub(crate) fn sub_values(a: Value, b: Value) -> Result<Value> {
    match (&a, &b) {
        (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x - y)),
        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x - y)),
        (Value::Int(x), Value::Float(y)) => Ok(Value::Float(*x as f64 - y)),
        (Value::Float(x), Value::Int(y)) => Ok(Value::Float(x - *y as f64)),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
            actual: a.value_type(),
        })),
    }
}

/// Multiplies two values.
pub(crate) fn mul_values(a: Value, b: Value) -> Result<Value> {
    match (&a, &b) {
        (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x * y)),
        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x * y)),
        (Value::Int(x), Value::Float(y)) => Ok(Value::Float(*x as f64 * y)),
        (Value::Float(x), Value::Int(y)) => Ok(Value::Float(x * *y as f64)),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
            actual: a.value_type(),
        })),
    }
}

/// Divides two values.
pub(crate) fn div_values(a: Value, b: Value) -> Result<Value> {
    match (&a, &b) {
        (Value::Int(_) | Value::Float(_), Value::Int(0)) => {
            Err(Error::new(ErrorKind::DivisionByZero))
        }
        (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x / y)),
        (Value::Float(x), Value::Float(y)) => {
            if *y == 0.0 {
                Err(Error::new(ErrorKind::DivisionByZero))
            } else {
                Ok(Value::Float(x / y))
            }
        }
        (Value::Int(x), Value::Float(y)) => {
            if *y == 0.0 {
                Err(Error::new(ErrorKind::DivisionByZero))
            } else {
                Ok(Value::Float(*x as f64 / y))
            }
        }
        (Value::Float(x), Value::Int(y)) => Ok(Value::Float(x / *y as f64)),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
            actual: a.value_type(),
        })),
    }
}

/// Modulo of two values.
pub(crate) fn mod_values(a: Value, b: Value) -> Result<Value> {
    match (&a, &b) {
        (Value::Int(_), Value::Int(0)) => Err(Error::new(ErrorKind::DivisionByZero)),
        (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x % y)),
        (Value::Float(x), Value::Float(y)) => {
            if *y == 0.0 {
                Err(Error::new(ErrorKind::DivisionByZero))
            } else {
                Ok(Value::Float(x % y))
            }
        }
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
            actual: a.value_type(),
        })),
    }
}

/// Negates a value.
pub(crate) fn neg_value(a: Value) -> Result<Value> {
    match a {
        Value::Int(x) => Ok(Value::Int(-x)),
        Value::Float(x) => Ok(Value::Float(-x)),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
            actual: a.value_type(),
        })),
    }
}

/// Compares two values with the given predicate.
pub(crate) fn compare_values<F>(a: Value, b: Value, pred: F) -> Result<Value>
where
    F: FnOnce(std::cmp::Ordering) -> bool,
{
    let ord = match (&a, &b) {
        (Value::Int(x), Value::Int(y)) => x.cmp(y),
        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
        (Value::Int(x), Value::Float(y)) => (*x as f64)
            .partial_cmp(y)
            .unwrap_or(std::cmp::Ordering::Equal),
        (Value::Float(x), Value::Int(y)) => x
            .partial_cmp(&(*y as f64))
            .unwrap_or(std::cmp::Ordering::Equal),
        (Value::String(x), Value::String(y)) => x.cmp(y),
        _ => {
            return Err(Error::new(ErrorKind::TypeMismatch {
                expected: longtable_foundation::Type::Int,
                actual: a.value_type(),
            }));
        }
    };
    Ok(Value::Bool(pred(ord)))
}

// =============================================================================
// Native Function Implementations
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

/// Collection: count
pub(crate) fn native_count(args: &[Value]) -> Result<Value> {
    let count = match args.first() {
        Some(Value::Vec(v)) => v.len() as i64,
        Some(Value::Set(s)) => s.len() as i64,
        Some(Value::Map(m)) => m.len() as i64,
        Some(Value::String(s)) => s.len() as i64,
        Some(Value::Nil) => 0,
        _ => {
            return Err(Error::new(ErrorKind::TypeMismatch {
                expected: longtable_foundation::Type::Vec(Box::new(
                    longtable_foundation::Type::Any,
                )),
                actual: args
                    .first()
                    .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
            }));
        }
    };
    Ok(Value::Int(count))
}

/// Collection: empty?
pub(crate) fn native_empty_p(args: &[Value]) -> Result<Value> {
    let empty = match args.first() {
        Some(Value::Vec(v)) => v.is_empty(),
        Some(Value::Set(s)) => s.is_empty(),
        Some(Value::Map(m)) => m.is_empty(),
        Some(Value::String(s)) => s.is_empty(),
        Some(Value::Nil) => true,
        _ => false,
    };
    Ok(Value::Bool(empty))
}

/// Collection: first
pub(crate) fn native_first(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Vec(v)) => Ok(v.first().cloned().unwrap_or(Value::Nil)),
        Some(Value::Nil) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Vec(Box::new(longtable_foundation::Type::Any)),
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: rest
pub(crate) fn native_rest(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Vec(v)) => {
            if v.is_empty() {
                Ok(Value::Vec(LtVec::new()))
            } else {
                // Skip the first element
                let rest: LtVec<Value> = v.iter().skip(1).cloned().collect();
                Ok(Value::Vec(rest))
            }
        }
        Some(Value::Nil) => Ok(Value::Vec(LtVec::new())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Vec(Box::new(longtable_foundation::Type::Any)),
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: nth
pub(crate) fn native_nth(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Vec(v)), Some(Value::Int(idx))) => {
            let idx = *idx as usize;
            Ok(v.get(idx).cloned().unwrap_or(Value::Nil))
        }
        (Some(Value::String(s)), Some(Value::Int(idx))) => {
            let idx = *idx as usize;
            Ok(s.chars()
                .nth(idx)
                .map_or(Value::Nil, |c| Value::String(c.to_string().into())))
        }
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Vec(Box::new(longtable_foundation::Type::Any)),
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: conj (add to collection)
pub(crate) fn native_conj(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Vec(v)) => {
            let mut result = v.clone();
            for arg in args.iter().skip(1) {
                result = result.push_back(arg.clone());
            }
            Ok(Value::Vec(result))
        }
        Some(Value::Set(s)) => {
            let mut result = s.clone();
            for arg in args.iter().skip(1) {
                result = result.insert(arg.clone());
            }
            Ok(Value::Set(result))
        }
        Some(Value::Nil) => {
            // conj on nil creates a vector
            let mut result = LtVec::new();
            for arg in args.iter().skip(1) {
                result = result.push_back(arg.clone());
            }
            Ok(Value::Vec(result))
        }
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Vec(Box::new(longtable_foundation::Type::Any)),
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: cons (prepend to collection)
pub(crate) fn native_cons(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(elem), Some(Value::Vec(v))) => {
            let mut result = LtVec::new();
            result = result.push_back(elem.clone());
            for item in v.iter() {
                result = result.push_back(item.clone());
            }
            Ok(Value::Vec(result))
        }
        (Some(elem), Some(Value::Nil)) => {
            let mut result = LtVec::new();
            result = result.push_back(elem.clone());
            Ok(Value::Vec(result))
        }
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Vec(Box::new(longtable_foundation::Type::Any)),
            actual: args
                .get(1)
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: get
pub(crate) fn native_get(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Map(m)), Some(key)) => Ok(m.get(key).cloned().unwrap_or(Value::Nil)),
        (Some(Value::Vec(v)), Some(Value::Int(idx))) => {
            let idx = *idx as usize;
            Ok(v.get(idx).cloned().unwrap_or(Value::Nil))
        }
        (Some(Value::Nil), _) => Ok(Value::Nil),
        _ => Ok(Value::Nil),
    }
}

/// Collection: assoc
pub(crate) fn native_assoc(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Map(m)) => {
            let mut result = m.clone();
            let mut i = 1;
            while i + 1 < args.len() {
                result = result.insert(args[i].clone(), args[i + 1].clone());
                i += 2;
            }
            Ok(Value::Map(result))
        }
        Some(Value::Vec(v)) => {
            let mut result = v.clone();
            let mut i = 1;
            while i + 1 < args.len() {
                if let Value::Int(idx) = &args[i] {
                    let idx = *idx as usize;
                    if idx < result.len() {
                        result = result.update(idx, args[i + 1].clone()).unwrap_or(result);
                    }
                }
                i += 2;
            }
            Ok(Value::Vec(result))
        }
        Some(Value::Nil) => {
            // assoc on nil creates a map
            let mut result = LtMap::new();
            let mut i = 1;
            while i + 1 < args.len() {
                result = result.insert(args[i].clone(), args[i + 1].clone());
                i += 2;
            }
            Ok(Value::Map(result))
        }
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Map(
                Box::new(longtable_foundation::Type::Any),
                Box::new(longtable_foundation::Type::Any),
            ),
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: dissoc
pub(crate) fn native_dissoc(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Map(m)) => {
            let mut result = m.clone();
            for key in args.iter().skip(1) {
                result = result.remove(key);
            }
            Ok(Value::Map(result))
        }
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Map(
                Box::new(longtable_foundation::Type::Any),
                Box::new(longtable_foundation::Type::Any),
            ),
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: contains?
pub(crate) fn native_contains_p(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Map(m)), Some(key)) => Ok(Value::Bool(m.contains_key(key))),
        (Some(Value::Set(s)), Some(elem)) => Ok(Value::Bool(s.contains(elem))),
        (Some(Value::Vec(v)), Some(Value::Int(idx))) => {
            let idx = *idx as usize;
            Ok(Value::Bool(idx < v.len()))
        }
        _ => Ok(Value::Bool(false)),
    }
}

/// Collection: keys
pub(crate) fn native_keys(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Map(m)) => {
            let keys: LtVec<Value> = m.keys().cloned().collect();
            Ok(Value::Vec(keys))
        }
        Some(Value::Nil) => Ok(Value::Vec(LtVec::new())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Map(
                Box::new(longtable_foundation::Type::Any),
                Box::new(longtable_foundation::Type::Any),
            ),
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: vals
pub(crate) fn native_vals(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Map(m)) => {
            let vals: LtVec<Value> = m.values().cloned().collect();
            Ok(Value::Vec(vals))
        }
        Some(Value::Nil) => Ok(Value::Vec(LtVec::new())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Map(
                Box::new(longtable_foundation::Type::Any),
                Box::new(longtable_foundation::Type::Any),
            ),
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// String: str (concatenate to string)
pub(crate) fn native_str(args: &[Value]) -> Result<Value> {
    let result: String = args.iter().map(format_value).collect();
    Ok(Value::String(result.into()))
}

/// String: str/len
pub(crate) fn native_str_len(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::String(s)) => Ok(Value::Int(s.len() as i64)),
        Some(Value::Nil) => Ok(Value::Int(0)),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::String,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// String: str/upper
pub(crate) fn native_str_upper(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::String(s)) => Ok(Value::String(s.to_uppercase().into())),
        Some(Value::Nil) => Ok(Value::String("".into())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::String,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// String: str/lower
pub(crate) fn native_str_lower(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::String(s)) => Ok(Value::String(s.to_lowercase().into())),
        Some(Value::Nil) => Ok(Value::String("".into())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::String,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: abs
pub(crate) fn native_abs(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(n.abs())),
        Some(Value::Float(n)) => Ok(Value::Float(n.abs())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: min
pub(crate) fn native_min(args: &[Value]) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::new(ErrorKind::Internal(
            "min requires at least one argument".to_string(),
        )));
    }
    let mut result = args[0].clone();
    for arg in args.iter().skip(1) {
        result = match (&result, arg) {
            (Value::Int(a), Value::Int(b)) => Value::Int(*a.min(b)),
            (Value::Float(a), Value::Float(b)) => Value::Float(a.min(*b)),
            (Value::Int(a), Value::Float(b)) => Value::Float((*a as f64).min(*b)),
            (Value::Float(a), Value::Int(b)) => Value::Float(a.min(*b as f64)),
            _ => {
                return Err(Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Int,
                    actual: arg.value_type(),
                }));
            }
        };
    }
    Ok(result)
}

/// Math: max
pub(crate) fn native_max(args: &[Value]) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::new(ErrorKind::Internal(
            "max requires at least one argument".to_string(),
        )));
    }
    let mut result = args[0].clone();
    for arg in args.iter().skip(1) {
        result = match (&result, arg) {
            (Value::Int(a), Value::Int(b)) => Value::Int(*a.max(b)),
            (Value::Float(a), Value::Float(b)) => Value::Float(a.max(*b)),
            (Value::Int(a), Value::Float(b)) => Value::Float((*a as f64).max(*b)),
            (Value::Float(a), Value::Int(b)) => Value::Float(a.max(*b as f64)),
            _ => {
                return Err(Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Int,
                    actual: arg.value_type(),
                }));
            }
        };
    }
    Ok(result)
}

/// Math: floor
pub(crate) fn native_floor(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(*n)),
        Some(Value::Float(n)) => Ok(Value::Float(n.floor())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: ceil
pub(crate) fn native_ceil(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(*n)),
        Some(Value::Float(n)) => Ok(Value::Float(n.ceil())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: round
pub(crate) fn native_round(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(*n)),
        Some(Value::Float(n)) => Ok(Value::Float(n.round())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: sqrt
pub(crate) fn native_sqrt(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).sqrt())),
        Some(Value::Float(n)) => Ok(Value::Float(n.sqrt())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
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

/// Formats a value for display.
pub(crate) fn format_value(value: &Value) -> String {
    match value {
        Value::Nil => "nil".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Float(n) => {
            if n.fract() == 0.0 {
                format!("{n}.0")
            } else {
                n.to_string()
            }
        }
        Value::String(s) => s.to_string(),
        Value::Symbol(id) => format!("Symbol({})", id.index()),
        Value::Keyword(id) => format!("Keyword({})", id.index()),
        Value::EntityRef(id) => format!("Entity({}, {})", id.index, id.generation),
        Value::Vec(v) => {
            let items: Vec<_> = v.iter().map(format_value).collect();
            format!("[{}]", items.join(" "))
        }
        Value::Set(s) => {
            let items: Vec<_> = s.iter().map(format_value).collect();
            format!("#{{{}}}", items.join(" "))
        }
        Value::Map(m) => {
            let pairs: Vec<_> = m
                .iter()
                .map(|(k, v)| format!("{} {}", format_value(k), format_value(v)))
                .collect();
            format!("{{{}}}", pairs.join(" "))
        }
        Value::Fn(_) => "<fn>".to_string(),
    }
}
