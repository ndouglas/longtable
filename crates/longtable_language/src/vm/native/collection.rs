//! Collection manipulation functions for the VM.

use super::format_value;
use longtable_foundation::{Error, ErrorKind, LtMap, LtSet, LtVec, Result, Value};

// =============================================================================
// Basic Collection Operations
// =============================================================================

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

/// Collection: last - get last element of sequence
pub(crate) fn native_last(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Vec(v)) => Ok(v.last().cloned().unwrap_or(Value::Nil)),
        Some(Value::Nil) => Ok(Value::Nil),
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

// =============================================================================
// Stage S1: Range
// =============================================================================

/// Collection: range - generate a sequence of integers
/// (range end) - 0 to end-1
/// (range start end) - start to end-1
/// (range start end step) - start to end-1 by step
pub(crate) fn native_range(args: &[Value]) -> Result<Value> {
    let (start, end, step) = match args.len() {
        1 => {
            // (range end)
            let end = match args.first() {
                Some(Value::Int(n)) => *n,
                _ => {
                    return Err(Error::new(ErrorKind::TypeMismatch {
                        expected: longtable_foundation::Type::Int,
                        actual: args
                            .first()
                            .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
                    }));
                }
            };
            (0i64, end, 1i64)
        }
        2 => {
            // (range start end)
            let start = match args.first() {
                Some(Value::Int(n)) => *n,
                _ => {
                    return Err(Error::new(ErrorKind::TypeMismatch {
                        expected: longtable_foundation::Type::Int,
                        actual: args
                            .first()
                            .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
                    }));
                }
            };
            let end = match args.get(1) {
                Some(Value::Int(n)) => *n,
                _ => {
                    return Err(Error::new(ErrorKind::TypeMismatch {
                        expected: longtable_foundation::Type::Int,
                        actual: args
                            .get(1)
                            .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
                    }));
                }
            };
            (start, end, 1i64)
        }
        3 => {
            // (range start end step)
            let start = match args.first() {
                Some(Value::Int(n)) => *n,
                _ => {
                    return Err(Error::new(ErrorKind::TypeMismatch {
                        expected: longtable_foundation::Type::Int,
                        actual: args
                            .first()
                            .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
                    }));
                }
            };
            let end = match args.get(1) {
                Some(Value::Int(n)) => *n,
                _ => {
                    return Err(Error::new(ErrorKind::TypeMismatch {
                        expected: longtable_foundation::Type::Int,
                        actual: args
                            .get(1)
                            .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
                    }));
                }
            };
            let step = match args.get(2) {
                Some(Value::Int(n)) => *n,
                _ => {
                    return Err(Error::new(ErrorKind::TypeMismatch {
                        expected: longtable_foundation::Type::Int,
                        actual: args
                            .get(2)
                            .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
                    }));
                }
            };
            (start, end, step)
        }
        _ => {
            return Err(Error::new(ErrorKind::Internal(
                "range requires 1, 2, or 3 arguments".to_string(),
            )));
        }
    };

    // Validate step
    if step == 0 {
        return Err(Error::new(ErrorKind::Internal(
            "range step cannot be zero".to_string(),
        )));
    }

    // Generate the range
    let mut result = LtVec::new();
    if step > 0 {
        let mut i = start;
        while i < end {
            result = result.push_back(Value::Int(i));
            i += step;
        }
    } else {
        let mut i = start;
        while i > end {
            result = result.push_back(Value::Int(i));
            i += step;
        }
    }

    Ok(Value::Vec(result))
}

// =============================================================================
// Stage S3: Extended Collection Functions
// =============================================================================

/// Collection: take - take first n elements
/// (take n coll) -> vector of first n elements
pub(crate) fn native_take(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(n)), Some(Value::Vec(v))) => {
            let n = (*n).max(0) as usize;
            let result: LtVec<Value> = v.iter().take(n).cloned().collect();
            Ok(Value::Vec(result))
        }
        (Some(Value::Int(n)), Some(Value::Set(s))) => {
            let n = (*n).max(0) as usize;
            let result: LtVec<Value> = s.iter().take(n).cloned().collect();
            Ok(Value::Vec(result))
        }
        (Some(Value::Int(_)), Some(Value::Nil)) => Ok(Value::Vec(LtVec::new())),
        (Some(Value::Nil), _) | (_, Some(Value::Nil)) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Vec(Box::new(longtable_foundation::Type::Any)),
            actual: args
                .get(1)
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: drop - drop first n elements
/// (drop n coll) -> vector of remaining elements
pub(crate) fn native_drop(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(n)), Some(Value::Vec(v))) => {
            let n = (*n).max(0) as usize;
            let result: LtVec<Value> = v.iter().skip(n).cloned().collect();
            Ok(Value::Vec(result))
        }
        (Some(Value::Int(n)), Some(Value::Set(s))) => {
            let n = (*n).max(0) as usize;
            let result: LtVec<Value> = s.iter().skip(n).cloned().collect();
            Ok(Value::Vec(result))
        }
        (Some(Value::Int(_)), Some(Value::Nil)) => Ok(Value::Vec(LtVec::new())),
        (Some(Value::Nil), _) | (_, Some(Value::Nil)) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Vec(Box::new(longtable_foundation::Type::Any)),
            actual: args
                .get(1)
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: concat - concatenate multiple collections
/// (concat coll1 coll2 ...) -> single vector
pub(crate) fn native_concat(args: &[Value]) -> Result<Value> {
    let mut result = LtVec::new();
    for arg in args {
        match arg {
            Value::Vec(v) => {
                for item in v.iter() {
                    result = result.push_back(item.clone());
                }
            }
            Value::Set(s) => {
                for item in s.iter() {
                    result = result.push_back(item.clone());
                }
            }
            Value::Nil => {}
            _ => {
                return Err(Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Any,
                    )),
                    actual: arg.value_type(),
                }));
            }
        }
    }
    Ok(Value::Vec(result))
}

/// Collection: reverse - reverse a collection
/// (reverse coll) -> reversed vector
pub(crate) fn native_reverse(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Vec(v)) => {
            // Collect to Vec first since LtVec's iterator doesn't implement DoubleEndedIterator
            let mut items: Vec<Value> = v.iter().cloned().collect();
            items.reverse();
            let result: LtVec<Value> = items.into_iter().collect();
            Ok(Value::Vec(result))
        }
        Some(Value::String(s)) => {
            let result: String = s.chars().rev().collect();
            Ok(Value::String(result.into()))
        }
        Some(Value::Nil) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Vec(Box::new(longtable_foundation::Type::Any)),
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: vec - convert to vector
/// (vec coll) -> vector
pub(crate) fn native_vec(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Vec(v)) => Ok(Value::Vec(v.clone())),
        Some(Value::Set(s)) => {
            let result: LtVec<Value> = s.iter().cloned().collect();
            Ok(Value::Vec(result))
        }
        Some(Value::Map(m)) => {
            // Convert map to vector of [k v] pairs
            let result: LtVec<Value> = m
                .iter()
                .map(|(k, v)| {
                    let pair: LtVec<Value> = [k.clone(), v.clone()].into_iter().collect();
                    Value::Vec(pair)
                })
                .collect();
            Ok(Value::Vec(result))
        }
        Some(Value::String(s)) => {
            // Convert string to vector of single-char strings
            let result: LtVec<Value> = s
                .chars()
                .map(|c| Value::String(c.to_string().into()))
                .collect();
            Ok(Value::Vec(result))
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

/// Collection: set - convert to set
/// (set coll) -> set
pub(crate) fn native_set(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Vec(v)) => {
            let result: LtSet<Value> = v.iter().cloned().collect();
            Ok(Value::Set(result))
        }
        Some(Value::Set(s)) => Ok(Value::Set(s.clone())),
        Some(Value::Nil) => Ok(Value::Set(LtSet::new())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Vec(Box::new(longtable_foundation::Type::Any)),
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: into - pour elements from one collection into another
/// (into to from) -> to with elements from from added
pub(crate) fn native_into(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Vec(to)), Some(Value::Vec(from))) => {
            let mut result = to.clone();
            for item in from.iter() {
                result = result.push_back(item.clone());
            }
            Ok(Value::Vec(result))
        }
        (Some(Value::Vec(to)), Some(Value::Set(from))) => {
            let mut result = to.clone();
            for item in from.iter() {
                result = result.push_back(item.clone());
            }
            Ok(Value::Vec(result))
        }
        (Some(Value::Set(to)), Some(Value::Vec(from))) => {
            let mut result = to.clone();
            for item in from.iter() {
                result = result.insert(item.clone());
            }
            Ok(Value::Set(result))
        }
        (Some(Value::Set(to)), Some(Value::Set(from))) => {
            let mut result = to.clone();
            for item in from.iter() {
                result = result.insert(item.clone());
            }
            Ok(Value::Set(result))
        }
        (Some(Value::Map(to)), Some(Value::Vec(from))) => {
            // Vec should contain [k v] pairs
            let mut result = to.clone();
            for item in from.iter() {
                if let Value::Vec(pair) = item {
                    if let (Some(k), Some(v)) = (pair.get(0), pair.get(1)) {
                        result = result.insert(k.clone(), v.clone());
                    }
                }
            }
            Ok(Value::Map(result))
        }
        (Some(Value::Map(to)), Some(Value::Map(from))) => {
            let mut result = to.clone();
            for (k, v) in from.iter() {
                result = result.insert(k.clone(), v.clone());
            }
            Ok(Value::Map(result))
        }
        (Some(to), Some(Value::Nil)) => Ok(to.clone()),
        (Some(Value::Nil) | None, _) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Vec(Box::new(longtable_foundation::Type::Any)),
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: sort - sort a collection (numbers or strings)
/// (sort coll) -> sorted vector
pub(crate) fn native_sort(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Vec(v)) => {
            let mut items: Vec<Value> = v.iter().cloned().collect();
            items.sort_by(compare_for_sort);
            let result: LtVec<Value> = items.into_iter().collect();
            Ok(Value::Vec(result))
        }
        Some(Value::Set(s)) => {
            let mut items: Vec<Value> = s.iter().cloned().collect();
            items.sort_by(compare_for_sort);
            let result: LtVec<Value> = items.into_iter().collect();
            Ok(Value::Vec(result))
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

/// Compare two values for sorting
fn compare_for_sort(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x.cmp(y),
        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        (Value::Int(x), Value::Float(y)) => (*x as f64).partial_cmp(y).unwrap_or(Ordering::Equal),
        (Value::Float(x), Value::Int(y)) => x.partial_cmp(&(*y as f64)).unwrap_or(Ordering::Equal),
        (Value::String(x), Value::String(y)) => x.cmp(y),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        // For other types, compare by type name then format
        _ => format_value(a).cmp(&format_value(b)),
    }
}

/// Collection: merge - merge maps
/// (merge m1 m2 ...) -> merged map (later values override earlier)
pub(crate) fn native_merge(args: &[Value]) -> Result<Value> {
    let mut result = LtMap::new();
    for arg in args {
        match arg {
            Value::Map(m) => {
                for (k, v) in m.iter() {
                    result = result.insert(k.clone(), v.clone());
                }
            }
            Value::Nil => {}
            _ => {
                return Err(Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Map(
                        Box::new(longtable_foundation::Type::Any),
                        Box::new(longtable_foundation::Type::Any),
                    ),
                    actual: arg.value_type(),
                }));
            }
        }
    }
    Ok(Value::Map(result))
}

// =============================================================================
// Stage S5: Extended Collection Functions
// =============================================================================

/// Collection: flatten - flatten one level of nesting
/// (flatten [[1 2] [3 4]]) -> [1 2 3 4]
pub(crate) fn native_flatten(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Vec(v)) => {
            let mut result = LtVec::new();
            for item in v.iter() {
                match item {
                    Value::Vec(inner) => {
                        for x in inner.iter() {
                            result = result.push_back(x.clone());
                        }
                    }
                    Value::Set(inner) => {
                        for x in inner.iter() {
                            result = result.push_back(x.clone());
                        }
                    }
                    other => {
                        result = result.push_back(other.clone());
                    }
                }
            }
            Ok(Value::Vec(result))
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

/// Collection: distinct - remove duplicate values
/// (distinct [1 2 1 3 2]) -> [1 2 3]
pub(crate) fn native_distinct(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Vec(v)) => {
            let mut seen = LtSet::new();
            let mut result = LtVec::new();
            for item in v.iter() {
                if !seen.contains(item) {
                    seen = seen.insert(item.clone());
                    result = result.push_back(item.clone());
                }
            }
            Ok(Value::Vec(result))
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

/// Collection: dedupe - remove consecutive duplicates
/// (dedupe [1 1 2 2 1 1]) -> [1 2 1]
pub(crate) fn native_dedupe(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Vec(v)) => {
            let mut result = LtVec::new();
            let mut last: Option<&Value> = None;
            for item in v.iter() {
                if last != Some(item) {
                    result = result.push_back(item.clone());
                    last = Some(item);
                }
            }
            Ok(Value::Vec(result))
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

/// Collection: partition - partition into groups of n
/// (partition 2 [1 2 3 4 5]) -> [[1 2] [3 4]]
pub(crate) fn native_partition(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(n)), Some(Value::Vec(v))) => {
            if *n <= 0 {
                return Err(Error::new(ErrorKind::Internal(
                    "partition size must be positive".to_string(),
                )));
            }
            let n = *n as usize;
            let items: Vec<Value> = v.iter().cloned().collect();
            let mut result = LtVec::new();
            for chunk in items.chunks(n) {
                if chunk.len() == n {
                    let group: LtVec<Value> = chunk.iter().cloned().collect();
                    result = result.push_back(Value::Vec(group));
                }
            }
            Ok(Value::Vec(result))
        }
        (Some(Value::Int(_)), Some(Value::Nil)) => Ok(Value::Vec(LtVec::new())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: partition-all - partition into groups of n, including incomplete final group
/// (partition-all 2 [1 2 3 4 5]) -> [[1 2] [3 4] [5]]
pub(crate) fn native_partition_all(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(n)), Some(Value::Vec(v))) => {
            if *n <= 0 {
                return Err(Error::new(ErrorKind::Internal(
                    "partition size must be positive".to_string(),
                )));
            }
            let n = *n as usize;
            let items: Vec<Value> = v.iter().cloned().collect();
            let mut result = LtVec::new();
            for chunk in items.chunks(n) {
                let group: LtVec<Value> = chunk.iter().cloned().collect();
                result = result.push_back(Value::Vec(group));
            }
            Ok(Value::Vec(result))
        }
        (Some(Value::Int(_)), Some(Value::Nil)) => Ok(Value::Vec(LtVec::new())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

// =============================================================================
// Stage S7: Combining Functions
// =============================================================================

/// Collection: interleave - interleave elements from multiple collections
/// (interleave [1 2] [a b] [x y]) -> [1 a x 2 b y]
pub(crate) fn native_interleave(args: &[Value]) -> Result<Value> {
    // Collect all vectors
    let vecs: Vec<Vec<Value>> = args
        .iter()
        .map(|arg| match arg {
            Value::Vec(v) => Ok(v.iter().cloned().collect()),
            Value::Nil => Ok(vec![]),
            _ => Err(Error::new(ErrorKind::TypeMismatch {
                expected: longtable_foundation::Type::Vec(Box::new(
                    longtable_foundation::Type::Any,
                )),
                actual: arg.value_type(),
            })),
        })
        .collect::<Result<Vec<_>>>()?;

    if vecs.is_empty() {
        return Ok(Value::Vec(LtVec::new()));
    }

    // Find minimum length (interleave stops at shortest)
    let min_len = vecs.iter().map(Vec::len).min().unwrap_or(0);

    let mut result = LtVec::new();
    for i in 0..min_len {
        for vec in &vecs {
            result = result.push_back(vec[i].clone());
        }
    }
    Ok(Value::Vec(result))
}

/// Collection: interpose - interpose separator between elements
/// (interpose :sep [1 2 3]) -> [1 :sep 2 :sep 3]
pub(crate) fn native_interpose(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(sep), Some(Value::Vec(v))) => {
            let mut result = LtVec::new();
            let mut first = true;
            for item in v.iter() {
                if !first {
                    result = result.push_back(sep.clone());
                }
                first = false;
                result = result.push_back(item.clone());
            }
            Ok(Value::Vec(result))
        }
        (Some(_), Some(Value::Nil)) => Ok(Value::Vec(LtVec::new())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Vec(Box::new(longtable_foundation::Type::Any)),
            actual: args
                .get(1)
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: zip - zip collections into vectors of tuples
/// (zip [1 2] [a b]) -> [[1 a] [2 b]]
pub(crate) fn native_zip(args: &[Value]) -> Result<Value> {
    // Collect all vectors
    let vecs: Vec<Vec<Value>> = args
        .iter()
        .map(|arg| match arg {
            Value::Vec(v) => Ok(v.iter().cloned().collect()),
            Value::Nil => Ok(vec![]),
            _ => Err(Error::new(ErrorKind::TypeMismatch {
                expected: longtable_foundation::Type::Vec(Box::new(
                    longtable_foundation::Type::Any,
                )),
                actual: arg.value_type(),
            })),
        })
        .collect::<Result<Vec<_>>>()?;

    if vecs.is_empty() {
        return Ok(Value::Vec(LtVec::new()));
    }

    // Find minimum length
    let min_len = vecs.iter().map(Vec::len).min().unwrap_or(0);

    let mut result = LtVec::new();
    for i in 0..min_len {
        let mut tuple = LtVec::new();
        for vec in &vecs {
            tuple = tuple.push_back(vec[i].clone());
        }
        result = result.push_back(Value::Vec(tuple));
    }
    Ok(Value::Vec(result))
}

/// Collection: repeat - repeat a value n times
/// (repeat 3 :x) -> [:x :x :x]
pub(crate) fn native_repeat(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(n)), Some(value)) => {
            if *n < 0 {
                return Err(Error::new(ErrorKind::Internal(
                    "repeat count must be non-negative".to_string(),
                )));
            }
            let mut result = LtVec::new();
            for _ in 0..(*n as usize) {
                result = result.push_back(value.clone());
            }
            Ok(Value::Vec(result))
        }
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a vector from values
    fn vec_of(values: &[Value]) -> Value {
        Value::Vec(values.iter().cloned().collect())
    }

    // ==========================================================================
    // Stage S5: Extended Collection Tests
    // ==========================================================================

    #[test]
    fn test_flatten_nested_vectors() {
        let inner1 = vec_of(&[Value::Int(1), Value::Int(2)]);
        let inner2 = vec_of(&[Value::Int(3), Value::Int(4)]);
        let outer = vec_of(&[inner1, inner2]);

        let result = native_flatten(&[outer]).unwrap();
        let expected = vec_of(&[Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4)]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_flatten_mixed() {
        let inner = vec_of(&[Value::Int(2), Value::Int(3)]);
        let outer = vec_of(&[Value::Int(1), inner, Value::Int(4)]);

        let result = native_flatten(&[outer]).unwrap();
        let expected = vec_of(&[Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4)]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_flatten_empty() {
        let empty = vec_of(&[]);
        let result = native_flatten(&[empty]).unwrap();
        assert_eq!(result, vec_of(&[]));
    }

    #[test]
    fn test_flatten_nil() {
        let result = native_flatten(&[Value::Nil]).unwrap();
        assert_eq!(result, vec_of(&[]));
    }

    #[test]
    fn test_distinct_removes_duplicates() {
        let input = vec_of(&[
            Value::Int(1),
            Value::Int(2),
            Value::Int(1),
            Value::Int(3),
            Value::Int(2),
        ]);
        let result = native_distinct(&[input]).unwrap();
        let expected = vec_of(&[Value::Int(1), Value::Int(2), Value::Int(3)]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_distinct_preserves_order() {
        let input = vec_of(&[Value::Int(3), Value::Int(1), Value::Int(2), Value::Int(1)]);
        let result = native_distinct(&[input]).unwrap();
        let expected = vec_of(&[Value::Int(3), Value::Int(1), Value::Int(2)]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_distinct_empty() {
        let result = native_distinct(&[vec_of(&[])]).unwrap();
        assert_eq!(result, vec_of(&[]));
    }

    #[test]
    fn test_dedupe_consecutive() {
        let input = vec_of(&[
            Value::Int(1),
            Value::Int(1),
            Value::Int(2),
            Value::Int(2),
            Value::Int(1),
        ]);
        let result = native_dedupe(&[input]).unwrap();
        let expected = vec_of(&[Value::Int(1), Value::Int(2), Value::Int(1)]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_dedupe_no_consecutive() {
        let input = vec_of(&[Value::Int(1), Value::Int(2), Value::Int(3)]);
        let result = native_dedupe(&[input.clone()]).unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_dedupe_all_same() {
        let input = vec_of(&[Value::Int(5), Value::Int(5), Value::Int(5)]);
        let result = native_dedupe(&[input]).unwrap();
        assert_eq!(result, vec_of(&[Value::Int(5)]));
    }

    #[test]
    fn test_partition_exact() {
        let input = vec_of(&[Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4)]);
        let result = native_partition(&[Value::Int(2), input]).unwrap();

        let g1 = vec_of(&[Value::Int(1), Value::Int(2)]);
        let g2 = vec_of(&[Value::Int(3), Value::Int(4)]);
        let expected = vec_of(&[g1, g2]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_partition_drops_incomplete() {
        let input = vec_of(&[
            Value::Int(1),
            Value::Int(2),
            Value::Int(3),
            Value::Int(4),
            Value::Int(5),
        ]);
        let result = native_partition(&[Value::Int(2), input]).unwrap();

        let g1 = vec_of(&[Value::Int(1), Value::Int(2)]);
        let g2 = vec_of(&[Value::Int(3), Value::Int(4)]);
        let expected = vec_of(&[g1, g2]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_partition_all_includes_incomplete() {
        let input = vec_of(&[
            Value::Int(1),
            Value::Int(2),
            Value::Int(3),
            Value::Int(4),
            Value::Int(5),
        ]);
        let result = native_partition_all(&[Value::Int(2), input]).unwrap();

        let g1 = vec_of(&[Value::Int(1), Value::Int(2)]);
        let g2 = vec_of(&[Value::Int(3), Value::Int(4)]);
        let g3 = vec_of(&[Value::Int(5)]);
        let expected = vec_of(&[g1, g2, g3]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_partition_size_larger_than_input() {
        let input = vec_of(&[Value::Int(1), Value::Int(2)]);
        let result = native_partition(&[Value::Int(5), input]).unwrap();
        assert_eq!(result, vec_of(&[]));
    }

    #[test]
    fn test_partition_all_size_larger_than_input() {
        let input = vec_of(&[Value::Int(1), Value::Int(2)]);
        let result = native_partition_all(&[Value::Int(5), input]).unwrap();
        let expected = vec_of(&[vec_of(&[Value::Int(1), Value::Int(2)])]);
        assert_eq!(result, expected);
    }

    // ==========================================================================
    // Stage S7: Combining Function Tests
    // ==========================================================================

    #[test]
    fn test_interleave_two_vectors() {
        let v1 = vec_of(&[Value::Int(1), Value::Int(2), Value::Int(3)]);
        let v2 = vec_of(&[Value::Int(10), Value::Int(20), Value::Int(30)]);
        let result = native_interleave(&[v1, v2]).unwrap();

        let expected = vec_of(&[
            Value::Int(1),
            Value::Int(10),
            Value::Int(2),
            Value::Int(20),
            Value::Int(3),
            Value::Int(30),
        ]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_interleave_three_vectors() {
        let v1 = vec_of(&[Value::Int(1), Value::Int(2)]);
        let v2 = vec_of(&[Value::Int(10), Value::Int(20)]);
        let v3 = vec_of(&[Value::Int(100), Value::Int(200)]);
        let result = native_interleave(&[v1, v2, v3]).unwrap();

        let expected = vec_of(&[
            Value::Int(1),
            Value::Int(10),
            Value::Int(100),
            Value::Int(2),
            Value::Int(20),
            Value::Int(200),
        ]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_interleave_unequal_lengths() {
        let v1 = vec_of(&[Value::Int(1), Value::Int(2), Value::Int(3)]);
        let v2 = vec_of(&[Value::Int(10)]);
        let result = native_interleave(&[v1, v2]).unwrap();

        // Stops at shortest
        let expected = vec_of(&[Value::Int(1), Value::Int(10)]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_interleave_empty() {
        let result = native_interleave(&[]).unwrap();
        assert_eq!(result, vec_of(&[]));
    }

    #[test]
    fn test_interpose_basic() {
        let input = vec_of(&[Value::Int(1), Value::Int(2), Value::Int(3)]);
        let result = native_interpose(&[Value::Int(0), input]).unwrap();

        let expected = vec_of(&[
            Value::Int(1),
            Value::Int(0),
            Value::Int(2),
            Value::Int(0),
            Value::Int(3),
        ]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_interpose_single_element() {
        let input = vec_of(&[Value::Int(1)]);
        let result = native_interpose(&[Value::Int(0), input]).unwrap();
        assert_eq!(result, vec_of(&[Value::Int(1)]));
    }

    #[test]
    fn test_interpose_empty() {
        let input = vec_of(&[]);
        let result = native_interpose(&[Value::Int(0), input]).unwrap();
        assert_eq!(result, vec_of(&[]));
    }

    #[test]
    fn test_zip_two_vectors() {
        let v1 = vec_of(&[Value::Int(1), Value::Int(2), Value::Int(3)]);
        let v2 = vec_of(&[Value::Int(10), Value::Int(20), Value::Int(30)]);
        let result = native_zip(&[v1, v2]).unwrap();

        let t1 = vec_of(&[Value::Int(1), Value::Int(10)]);
        let t2 = vec_of(&[Value::Int(2), Value::Int(20)]);
        let t3 = vec_of(&[Value::Int(3), Value::Int(30)]);
        let expected = vec_of(&[t1, t2, t3]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_zip_three_vectors() {
        let v1 = vec_of(&[Value::Int(1), Value::Int(2)]);
        let v2 = vec_of(&[Value::Int(10), Value::Int(20)]);
        let v3 = vec_of(&[Value::Int(100), Value::Int(200)]);
        let result = native_zip(&[v1, v2, v3]).unwrap();

        let t1 = vec_of(&[Value::Int(1), Value::Int(10), Value::Int(100)]);
        let t2 = vec_of(&[Value::Int(2), Value::Int(20), Value::Int(200)]);
        let expected = vec_of(&[t1, t2]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_zip_unequal_lengths() {
        let v1 = vec_of(&[Value::Int(1), Value::Int(2), Value::Int(3)]);
        let v2 = vec_of(&[Value::Int(10)]);
        let result = native_zip(&[v1, v2]).unwrap();

        let expected = vec_of(&[vec_of(&[Value::Int(1), Value::Int(10)])]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_zip_empty() {
        let result = native_zip(&[]).unwrap();
        assert_eq!(result, vec_of(&[]));
    }

    #[test]
    fn test_repeat_basic() {
        let result = native_repeat(&[Value::Int(3), Value::Int(42)]).unwrap();
        let expected = vec_of(&[Value::Int(42), Value::Int(42), Value::Int(42)]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_repeat_zero() {
        let result = native_repeat(&[Value::Int(0), Value::Int(42)]).unwrap();
        assert_eq!(result, vec_of(&[]));
    }

    #[test]
    fn test_repeat_with_string() {
        let result = native_repeat(&[Value::Int(2), Value::String("hi".into())]).unwrap();
        let expected = vec_of(&[Value::String("hi".into()), Value::String("hi".into())]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_repeat_negative_fails() {
        let result = native_repeat(&[Value::Int(-1), Value::Int(42)]);
        assert!(result.is_err());
    }
}
