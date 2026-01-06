//! Arithmetic and comparison helpers for the VM.

use longtable_foundation::{Error, ErrorKind, Result, Value};

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
