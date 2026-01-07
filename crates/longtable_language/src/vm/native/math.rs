//! Mathematical functions for the VM.

use longtable_foundation::{Error, ErrorKind, Result, Value};

// =============================================================================
// Basic Math Functions
// =============================================================================

/// Math: inc - increment by 1
pub(crate) fn native_inc(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(n + 1)),
        Some(Value::Float(n)) => Ok(Value::Float(n + 1.0)),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: dec - decrement by 1
pub(crate) fn native_dec(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(n - 1)),
        Some(Value::Float(n)) => Ok(Value::Float(n - 1.0)),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
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

// =============================================================================
// Stage S4: Extended Math Functions
// =============================================================================

/// Math: rem - remainder (modulo preserving sign of dividend)
pub(crate) fn native_rem(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(a)), Some(Value::Int(b))) => Ok(Value::Int(a % b)),
        (Some(Value::Float(a)), Some(Value::Float(b))) => Ok(Value::Float(a % b)),
        (Some(Value::Int(a)), Some(Value::Float(b))) => Ok(Value::Float(*a as f64 % b)),
        (Some(Value::Float(a)), Some(Value::Int(b))) => Ok(Value::Float(a % *b as f64)),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: clamp - clamp value between min and max
pub(crate) fn native_clamp(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1), args.get(2)) {
        (Some(Value::Int(x)), Some(Value::Int(lo)), Some(Value::Int(hi))) => {
            Ok(Value::Int((*x).max(*lo).min(*hi)))
        }
        (Some(Value::Float(x)), Some(Value::Float(lo)), Some(Value::Float(hi))) => {
            Ok(Value::Float(x.max(*lo).min(*hi)))
        }
        (Some(Value::Int(x)), Some(Value::Float(lo)), Some(Value::Float(hi))) => {
            Ok(Value::Float((*x as f64).max(*lo).min(*hi)))
        }
        (Some(Value::Float(x)), Some(Value::Int(lo)), Some(Value::Int(hi))) => {
            Ok(Value::Float(x.max(*lo as f64).min(*hi as f64)))
        }
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: trunc - truncate towards zero
pub(crate) fn native_trunc(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(*n)),
        Some(Value::Float(n)) => Ok(Value::Float(n.trunc())),
        Some(Value::Nil) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: pow - raise to power
pub(crate) fn native_pow(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(base)), Some(Value::Int(exp))) => {
            if *exp >= 0 {
                Ok(Value::Int(base.pow(*exp as u32)))
            } else {
                Ok(Value::Float((*base as f64).powf(*exp as f64)))
            }
        }
        (Some(Value::Float(base)), Some(Value::Float(exp))) => Ok(Value::Float(base.powf(*exp))),
        (Some(Value::Int(base)), Some(Value::Float(exp))) => {
            Ok(Value::Float((*base as f64).powf(*exp)))
        }
        (Some(Value::Float(base)), Some(Value::Int(exp))) => {
            Ok(Value::Float(base.powf(*exp as f64)))
        }
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: cbrt - cube root
pub(crate) fn native_cbrt(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).cbrt())),
        Some(Value::Float(n)) => Ok(Value::Float(n.cbrt())),
        Some(Value::Nil) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: exp - e^x
pub(crate) fn native_exp(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).exp())),
        Some(Value::Float(n)) => Ok(Value::Float(n.exp())),
        Some(Value::Nil) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: log - natural logarithm
pub(crate) fn native_log(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).ln())),
        Some(Value::Float(n)) => Ok(Value::Float(n.ln())),
        Some(Value::Nil) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: log10 - base-10 logarithm
pub(crate) fn native_log10(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).log10())),
        Some(Value::Float(n)) => Ok(Value::Float(n.log10())),
        Some(Value::Nil) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: log2 - base-2 logarithm
pub(crate) fn native_log2(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).log2())),
        Some(Value::Float(n)) => Ok(Value::Float(n.log2())),
        Some(Value::Nil) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

// =============================================================================
// Trigonometric Functions
// =============================================================================

/// Math: sin - sine
pub(crate) fn native_sin(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).sin())),
        Some(Value::Float(n)) => Ok(Value::Float(n.sin())),
        Some(Value::Nil) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: cos - cosine
pub(crate) fn native_cos(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).cos())),
        Some(Value::Float(n)) => Ok(Value::Float(n.cos())),
        Some(Value::Nil) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: tan - tangent
pub(crate) fn native_tan(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).tan())),
        Some(Value::Float(n)) => Ok(Value::Float(n.tan())),
        Some(Value::Nil) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: asin - arcsine
pub(crate) fn native_asin(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).asin())),
        Some(Value::Float(n)) => Ok(Value::Float(n.asin())),
        Some(Value::Nil) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: acos - arccosine
pub(crate) fn native_acos(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).acos())),
        Some(Value::Float(n)) => Ok(Value::Float(n.acos())),
        Some(Value::Nil) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: atan - arctangent
pub(crate) fn native_atan(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).atan())),
        Some(Value::Float(n)) => Ok(Value::Float(n.atan())),
        Some(Value::Nil) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: atan2 - two-argument arctangent
pub(crate) fn native_atan2(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(y)), Some(Value::Int(x))) => {
            Ok(Value::Float((*y as f64).atan2(*x as f64)))
        }
        (Some(Value::Float(y)), Some(Value::Float(x))) => Ok(Value::Float(y.atan2(*x))),
        (Some(Value::Int(y)), Some(Value::Float(x))) => Ok(Value::Float((*y as f64).atan2(*x))),
        (Some(Value::Float(y)), Some(Value::Int(x))) => Ok(Value::Float(y.atan2(*x as f64))),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

// =============================================================================
// Hyperbolic Trigonometry
// =============================================================================

/// Math: sinh - hyperbolic sine
pub(crate) fn native_sinh(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).sinh())),
        Some(Value::Float(n)) => Ok(Value::Float(n.sinh())),
        Some(Value::Nil) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: cosh - hyperbolic cosine
pub(crate) fn native_cosh(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).cosh())),
        Some(Value::Float(n)) => Ok(Value::Float(n.cosh())),
        Some(Value::Nil) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: tanh - hyperbolic tangent
pub(crate) fn native_tanh(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).tanh())),
        Some(Value::Float(n)) => Ok(Value::Float(n.tanh())),
        Some(Value::Nil) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

// =============================================================================
// Constants
// =============================================================================

/// Math constant: pi
pub(crate) fn native_pi(_args: &[Value]) -> Result<Value> {
    Ok(Value::Float(std::f64::consts::PI))
}

/// Math constant: e
pub(crate) fn native_e(_args: &[Value]) -> Result<Value> {
    Ok(Value::Float(std::f64::consts::E))
}

// =============================================================================
// Stage S6: Vector Math Functions
// =============================================================================

use longtable_foundation::LtVec;

/// Helper to extract numeric value as f64
fn to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Int(n) => Some(*n as f64),
        Value::Float(n) => Some(*n),
        _ => None,
    }
}

/// Helper to extract a numeric vector from a `Value::Vec`
fn extract_vec(v: &Value) -> Option<Vec<f64>> {
    match v {
        Value::Vec(vec) => {
            let mut result = Vec::with_capacity(vec.len());
            for item in vec.iter() {
                result.push(to_f64(item)?);
            }
            Some(result)
        }
        _ => None,
    }
}

/// Helper to create a `Value::Vec` from f64 values
fn make_vec(values: &[f64]) -> Value {
    let vec: LtVec<Value> = values.iter().map(|&x| Value::Float(x)).collect();
    Value::Vec(vec)
}

/// Vector: vec+ - element-wise vector addition
/// (vec+ [1 2] [3 4]) -> [4.0 6.0]
pub(crate) fn native_vec_add(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(a), Some(b)) => {
            let va = extract_vec(a).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Float,
                    )),
                    actual: a.value_type(),
                })
            })?;
            let vb = extract_vec(b).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Float,
                    )),
                    actual: b.value_type(),
                })
            })?;
            if va.len() != vb.len() {
                return Err(Error::new(ErrorKind::Internal(
                    "vec+ requires vectors of equal length".to_string(),
                )));
            }
            let result: Vec<f64> = va.iter().zip(vb.iter()).map(|(x, y)| x + y).collect();
            Ok(make_vec(&result))
        }
        _ => Err(Error::new(ErrorKind::Internal(
            "vec+ requires 2 arguments".to_string(),
        ))),
    }
}

/// Vector: vec- - element-wise vector subtraction
/// (vec- [3 4] [1 2]) -> [2.0 2.0]
pub(crate) fn native_vec_sub(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(a), Some(b)) => {
            let va = extract_vec(a).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Float,
                    )),
                    actual: a.value_type(),
                })
            })?;
            let vb = extract_vec(b).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Float,
                    )),
                    actual: b.value_type(),
                })
            })?;
            if va.len() != vb.len() {
                return Err(Error::new(ErrorKind::Internal(
                    "vec- requires vectors of equal length".to_string(),
                )));
            }
            let result: Vec<f64> = va.iter().zip(vb.iter()).map(|(x, y)| x - y).collect();
            Ok(make_vec(&result))
        }
        _ => Err(Error::new(ErrorKind::Internal(
            "vec- requires 2 arguments".to_string(),
        ))),
    }
}

/// Vector: vec* - element-wise vector multiplication
/// (vec* [2 3] [4 5]) -> [8.0 15.0]
pub(crate) fn native_vec_mul(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(a), Some(b)) => {
            let va = extract_vec(a).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Float,
                    )),
                    actual: a.value_type(),
                })
            })?;
            let vb = extract_vec(b).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Float,
                    )),
                    actual: b.value_type(),
                })
            })?;
            if va.len() != vb.len() {
                return Err(Error::new(ErrorKind::Internal(
                    "vec* requires vectors of equal length".to_string(),
                )));
            }
            let result: Vec<f64> = va.iter().zip(vb.iter()).map(|(x, y)| x * y).collect();
            Ok(make_vec(&result))
        }
        _ => Err(Error::new(ErrorKind::Internal(
            "vec* requires 2 arguments".to_string(),
        ))),
    }
}

/// Vector: vec-scale - scale vector by scalar
/// (vec-scale [1 2 3] 2) -> [2.0 4.0 6.0]
pub(crate) fn native_vec_scale(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(v), Some(s)) => {
            let vec = extract_vec(v).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Float,
                    )),
                    actual: v.value_type(),
                })
            })?;
            let scalar = to_f64(s).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Float,
                    actual: s.value_type(),
                })
            })?;
            let result: Vec<f64> = vec.iter().map(|x| x * scalar).collect();
            Ok(make_vec(&result))
        }
        _ => Err(Error::new(ErrorKind::Internal(
            "vec-scale requires 2 arguments".to_string(),
        ))),
    }
}

/// Vector: vec-dot - dot product of two vectors
/// (vec-dot [1 2] [3 4]) -> 11.0
pub(crate) fn native_vec_dot(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(a), Some(b)) => {
            let va = extract_vec(a).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Float,
                    )),
                    actual: a.value_type(),
                })
            })?;
            let vb = extract_vec(b).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Float,
                    )),
                    actual: b.value_type(),
                })
            })?;
            if va.len() != vb.len() {
                return Err(Error::new(ErrorKind::Internal(
                    "vec-dot requires vectors of equal length".to_string(),
                )));
            }
            let result: f64 = va.iter().zip(vb.iter()).map(|(x, y)| x * y).sum();
            Ok(Value::Float(result))
        }
        _ => Err(Error::new(ErrorKind::Internal(
            "vec-dot requires 2 arguments".to_string(),
        ))),
    }
}

/// Vector: vec-cross - cross product of two 3D vectors
/// (vec-cross [1 0 0] [0 1 0]) -> [0.0 0.0 1.0]
pub(crate) fn native_vec_cross(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(a), Some(b)) => {
            let va = extract_vec(a).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Float,
                    )),
                    actual: a.value_type(),
                })
            })?;
            let vb = extract_vec(b).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Float,
                    )),
                    actual: b.value_type(),
                })
            })?;
            if va.len() != 3 || vb.len() != 3 {
                return Err(Error::new(ErrorKind::Internal(
                    "vec-cross requires 3D vectors".to_string(),
                )));
            }
            let result = [
                va[1] * vb[2] - va[2] * vb[1],
                va[2] * vb[0] - va[0] * vb[2],
                va[0] * vb[1] - va[1] * vb[0],
            ];
            Ok(make_vec(&result))
        }
        _ => Err(Error::new(ErrorKind::Internal(
            "vec-cross requires 2 arguments".to_string(),
        ))),
    }
}

/// Vector: vec-length - magnitude of a vector
/// (vec-length [3 4]) -> 5.0
pub(crate) fn native_vec_length(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(v) => {
            let vec = extract_vec(v).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Float,
                    )),
                    actual: v.value_type(),
                })
            })?;
            let len_sq: f64 = vec.iter().map(|x| x * x).sum();
            Ok(Value::Float(len_sq.sqrt()))
        }
        _ => Err(Error::new(ErrorKind::Internal(
            "vec-length requires 1 argument".to_string(),
        ))),
    }
}

/// Vector: vec-length-sq - squared magnitude of a vector (faster, no sqrt)
/// (vec-length-sq [3 4]) -> 25.0
pub(crate) fn native_vec_length_sq(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(v) => {
            let vec = extract_vec(v).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Float,
                    )),
                    actual: v.value_type(),
                })
            })?;
            let len_sq: f64 = vec.iter().map(|x| x * x).sum();
            Ok(Value::Float(len_sq))
        }
        _ => Err(Error::new(ErrorKind::Internal(
            "vec-length-sq requires 1 argument".to_string(),
        ))),
    }
}

/// Vector: vec-normalize - unit vector in same direction
/// (vec-normalize [3 4]) -> [0.6 0.8]
pub(crate) fn native_vec_normalize(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(v) => {
            let vec = extract_vec(v).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Float,
                    )),
                    actual: v.value_type(),
                })
            })?;
            let len_sq: f64 = vec.iter().map(|x| x * x).sum();
            if len_sq == 0.0 {
                // Return zero vector for zero-length input
                return Ok(make_vec(&vec![0.0; vec.len()]));
            }
            let len = len_sq.sqrt();
            let result: Vec<f64> = vec.iter().map(|x| x / len).collect();
            Ok(make_vec(&result))
        }
        _ => Err(Error::new(ErrorKind::Internal(
            "vec-normalize requires 1 argument".to_string(),
        ))),
    }
}

/// Vector: vec-distance - distance between two points
/// (vec-distance [0 0] [3 4]) -> 5.0
pub(crate) fn native_vec_distance(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(a), Some(b)) => {
            let va = extract_vec(a).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Float,
                    )),
                    actual: a.value_type(),
                })
            })?;
            let vb = extract_vec(b).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Float,
                    )),
                    actual: b.value_type(),
                })
            })?;
            if va.len() != vb.len() {
                return Err(Error::new(ErrorKind::Internal(
                    "vec-distance requires vectors of equal length".to_string(),
                )));
            }
            let dist_sq: f64 = va
                .iter()
                .zip(vb.iter())
                .map(|(x, y)| (x - y) * (x - y))
                .sum();
            Ok(Value::Float(dist_sq.sqrt()))
        }
        _ => Err(Error::new(ErrorKind::Internal(
            "vec-distance requires 2 arguments".to_string(),
        ))),
    }
}

/// Vector: vec-lerp - linear interpolation between two vectors
/// (vec-lerp [0 0] [10 10] 0.5) -> [5.0 5.0]
pub(crate) fn native_vec_lerp(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1), args.get(2)) {
        (Some(a), Some(b), Some(t)) => {
            let va = extract_vec(a).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Float,
                    )),
                    actual: a.value_type(),
                })
            })?;
            let vb = extract_vec(b).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Float,
                    )),
                    actual: b.value_type(),
                })
            })?;
            let t_val = to_f64(t).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Float,
                    actual: t.value_type(),
                })
            })?;
            if va.len() != vb.len() {
                return Err(Error::new(ErrorKind::Internal(
                    "vec-lerp requires vectors of equal length".to_string(),
                )));
            }
            let result: Vec<f64> = va
                .iter()
                .zip(vb.iter())
                .map(|(x, y)| x + (y - x) * t_val)
                .collect();
            Ok(make_vec(&result))
        }
        _ => Err(Error::new(ErrorKind::Internal(
            "vec-lerp requires 3 arguments".to_string(),
        ))),
    }
}

/// Vector: vec-angle - angle between two vectors in radians
/// (vec-angle [1 0] [0 1]) -> 1.5707... (pi/2)
pub(crate) fn native_vec_angle(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(a), Some(b)) => {
            let va = extract_vec(a).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Float,
                    )),
                    actual: a.value_type(),
                })
            })?;
            let vb = extract_vec(b).ok_or_else(|| {
                Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Vec(Box::new(
                        longtable_foundation::Type::Float,
                    )),
                    actual: b.value_type(),
                })
            })?;
            if va.len() != vb.len() {
                return Err(Error::new(ErrorKind::Internal(
                    "vec-angle requires vectors of equal length".to_string(),
                )));
            }
            let dot: f64 = va.iter().zip(vb.iter()).map(|(x, y)| x * y).sum();
            let len_a: f64 = va.iter().map(|x| x * x).sum::<f64>().sqrt();
            let len_b: f64 = vb.iter().map(|x| x * x).sum::<f64>().sqrt();
            if len_a == 0.0 || len_b == 0.0 {
                return Ok(Value::Float(0.0));
            }
            // Clamp to handle floating point errors
            let cos_angle = (dot / (len_a * len_b)).clamp(-1.0, 1.0);
            Ok(Value::Float(cos_angle.acos()))
        }
        _ => Err(Error::new(ErrorKind::Internal(
            "vec-angle requires 2 arguments".to_string(),
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    // Helper to create a float vector
    fn float_vec(values: &[f64]) -> Value {
        Value::Vec(values.iter().map(|&v| Value::Float(v)).collect())
    }

    // Helper to extract float from result
    fn as_float(v: &Value) -> f64 {
        match v {
            Value::Float(f) => *f,
            Value::Int(i) => *i as f64,
            _ => panic!("Expected float, got {v:?}"),
        }
    }

    // Helper to extract vec of floats from result
    fn as_float_vec(v: &Value) -> Vec<f64> {
        match v {
            Value::Vec(vec) => vec.iter().map(|v| as_float(v)).collect(),
            _ => panic!("Expected vec, got {v:?}"),
        }
    }

    // ==================== Stage S4: Basic Math Tests ====================

    #[test]
    fn test_pow_basic() {
        let result = native_pow(&[Value::Float(2.0), Value::Float(3.0)]).unwrap();
        assert!((as_float(&result) - 8.0).abs() < 1e-10);
    }

    #[test]
    fn test_pow_with_ints() {
        let result = native_pow(&[Value::Int(2), Value::Int(10)]).unwrap();
        assert!((as_float(&result) - 1024.0).abs() < 1e-10);
    }

    #[test]
    fn test_pow_fractional() {
        let result = native_pow(&[Value::Float(4.0), Value::Float(0.5)]).unwrap();
        assert!((as_float(&result) - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_exp_basic() {
        let result = native_exp(&[Value::Float(0.0)]).unwrap();
        assert!((as_float(&result) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_exp_one() {
        let result = native_exp(&[Value::Float(1.0)]).unwrap();
        assert!((as_float(&result) - std::f64::consts::E).abs() < 1e-10);
    }

    #[test]
    fn test_log_basic() {
        let result = native_log(&[Value::Float(std::f64::consts::E)]).unwrap();
        assert!((as_float(&result) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_log_one() {
        let result = native_log(&[Value::Float(1.0)]).unwrap();
        assert!(as_float(&result).abs() < 1e-10);
    }

    #[test]
    fn test_log10_basic() {
        let result = native_log10(&[Value::Float(100.0)]).unwrap();
        assert!((as_float(&result) - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_log2_basic() {
        let result = native_log2(&[Value::Float(8.0)]).unwrap();
        assert!((as_float(&result) - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_cbrt_basic() {
        let result = native_cbrt(&[Value::Float(27.0)]).unwrap();
        assert!((as_float(&result) - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_cbrt_negative() {
        let result = native_cbrt(&[Value::Float(-8.0)]).unwrap();
        assert!((as_float(&result) - (-2.0)).abs() < 1e-10);
    }

    // ==================== Trigonometric Functions ====================

    #[test]
    fn test_sin_zero() {
        let result = native_sin(&[Value::Float(0.0)]).unwrap();
        assert!(as_float(&result).abs() < 1e-10);
    }

    #[test]
    fn test_sin_pi_half() {
        let result = native_sin(&[Value::Float(PI / 2.0)]).unwrap();
        assert!((as_float(&result) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_cos_zero() {
        let result = native_cos(&[Value::Float(0.0)]).unwrap();
        assert!((as_float(&result) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_cos_pi() {
        let result = native_cos(&[Value::Float(PI)]).unwrap();
        assert!((as_float(&result) - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn test_tan_zero() {
        let result = native_tan(&[Value::Float(0.0)]).unwrap();
        assert!(as_float(&result).abs() < 1e-10);
    }

    #[test]
    fn test_tan_pi_quarter() {
        let result = native_tan(&[Value::Float(PI / 4.0)]).unwrap();
        assert!((as_float(&result) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_asin_zero() {
        let result = native_asin(&[Value::Float(0.0)]).unwrap();
        assert!(as_float(&result).abs() < 1e-10);
    }

    #[test]
    fn test_asin_one() {
        let result = native_asin(&[Value::Float(1.0)]).unwrap();
        assert!((as_float(&result) - PI / 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_acos_one() {
        let result = native_acos(&[Value::Float(1.0)]).unwrap();
        assert!(as_float(&result).abs() < 1e-10);
    }

    #[test]
    fn test_acos_zero() {
        let result = native_acos(&[Value::Float(0.0)]).unwrap();
        assert!((as_float(&result) - PI / 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_atan_zero() {
        let result = native_atan(&[Value::Float(0.0)]).unwrap();
        assert!(as_float(&result).abs() < 1e-10);
    }

    #[test]
    fn test_atan_one() {
        let result = native_atan(&[Value::Float(1.0)]).unwrap();
        assert!((as_float(&result) - PI / 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_atan2_basic() {
        let result = native_atan2(&[Value::Float(1.0), Value::Float(1.0)]).unwrap();
        assert!((as_float(&result) - PI / 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_atan2_y_axis() {
        let result = native_atan2(&[Value::Float(1.0), Value::Float(0.0)]).unwrap();
        assert!((as_float(&result) - PI / 2.0).abs() < 1e-10);
    }

    // ==================== Stage S7: Hyperbolic Functions ====================

    #[test]
    fn test_sinh_zero() {
        let result = native_sinh(&[Value::Float(0.0)]).unwrap();
        assert!(as_float(&result).abs() < 1e-10);
    }

    #[test]
    fn test_sinh_one() {
        let result = native_sinh(&[Value::Float(1.0)]).unwrap();
        // sinh(1) ≈ 1.1752
        assert!((as_float(&result) - 1.0_f64.sinh()).abs() < 1e-10);
    }

    #[test]
    fn test_sinh_negative() {
        let result = native_sinh(&[Value::Float(-1.0)]).unwrap();
        // sinh(-x) = -sinh(x)
        let positive = native_sinh(&[Value::Float(1.0)]).unwrap();
        assert!((as_float(&result) + as_float(&positive)).abs() < 1e-10);
    }

    #[test]
    fn test_cosh_zero() {
        let result = native_cosh(&[Value::Float(0.0)]).unwrap();
        assert!((as_float(&result) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_cosh_one() {
        let result = native_cosh(&[Value::Float(1.0)]).unwrap();
        // cosh(1) ≈ 1.5431
        assert!((as_float(&result) - 1.0_f64.cosh()).abs() < 1e-10);
    }

    #[test]
    fn test_cosh_symmetric() {
        // cosh(-x) = cosh(x)
        let positive = native_cosh(&[Value::Float(2.0)]).unwrap();
        let negative = native_cosh(&[Value::Float(-2.0)]).unwrap();
        assert!((as_float(&positive) - as_float(&negative)).abs() < 1e-10);
    }

    #[test]
    fn test_tanh_zero() {
        let result = native_tanh(&[Value::Float(0.0)]).unwrap();
        assert!(as_float(&result).abs() < 1e-10);
    }

    #[test]
    fn test_tanh_one() {
        let result = native_tanh(&[Value::Float(1.0)]).unwrap();
        // tanh(1) ≈ 0.7616
        assert!((as_float(&result) - 1.0_f64.tanh()).abs() < 1e-10);
    }

    #[test]
    fn test_tanh_large_approaches_one() {
        let result = native_tanh(&[Value::Float(10.0)]).unwrap();
        // tanh approaches 1 for large positive values
        assert!((as_float(&result) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_tanh_large_negative_approaches_negative_one() {
        let result = native_tanh(&[Value::Float(-10.0)]).unwrap();
        // tanh approaches -1 for large negative values
        assert!((as_float(&result) + 1.0).abs() < 1e-5);
    }

    // ==================== Stage S6: Vector Math Tests ====================

    #[test]
    fn test_vec_add_basic() {
        let a = float_vec(&[1.0, 2.0, 3.0]);
        let b = float_vec(&[4.0, 5.0, 6.0]);
        let result = native_vec_add(&[a, b]).unwrap();
        let vals = as_float_vec(&result);
        assert_eq!(vals.len(), 3);
        assert!((vals[0] - 5.0).abs() < 1e-10);
        assert!((vals[1] - 7.0).abs() < 1e-10);
        assert!((vals[2] - 9.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_add_2d() {
        let a = float_vec(&[10.0, 20.0]);
        let b = float_vec(&[-3.0, 7.0]);
        let result = native_vec_add(&[a, b]).unwrap();
        let vals = as_float_vec(&result);
        assert!((vals[0] - 7.0).abs() < 1e-10);
        assert!((vals[1] - 27.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_add_unequal_lengths_fails() {
        let a = float_vec(&[1.0, 2.0]);
        let b = float_vec(&[1.0, 2.0, 3.0]);
        assert!(native_vec_add(&[a, b]).is_err());
    }

    #[test]
    fn test_vec_sub_basic() {
        let a = float_vec(&[5.0, 7.0, 9.0]);
        let b = float_vec(&[1.0, 2.0, 3.0]);
        let result = native_vec_sub(&[a, b]).unwrap();
        let vals = as_float_vec(&result);
        assert!((vals[0] - 4.0).abs() < 1e-10);
        assert!((vals[1] - 5.0).abs() < 1e-10);
        assert!((vals[2] - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_mul_basic() {
        let a = float_vec(&[2.0, 3.0, 4.0]);
        let b = float_vec(&[3.0, 4.0, 5.0]);
        let result = native_vec_mul(&[a, b]).unwrap();
        let vals = as_float_vec(&result);
        assert!((vals[0] - 6.0).abs() < 1e-10);
        assert!((vals[1] - 12.0).abs() < 1e-10);
        assert!((vals[2] - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_scale_basic() {
        let v = float_vec(&[2.0, 4.0, 6.0]);
        let result = native_vec_scale(&[v, Value::Float(0.5)]).unwrap();
        let vals = as_float_vec(&result);
        assert!((vals[0] - 1.0).abs() < 1e-10);
        assert!((vals[1] - 2.0).abs() < 1e-10);
        assert!((vals[2] - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_scale_negative() {
        let v = float_vec(&[1.0, 2.0]);
        let result = native_vec_scale(&[v, Value::Float(-2.0)]).unwrap();
        let vals = as_float_vec(&result);
        assert!((vals[0] + 2.0).abs() < 1e-10);
        assert!((vals[1] + 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_dot_basic() {
        let a = float_vec(&[1.0, 2.0, 3.0]);
        let b = float_vec(&[4.0, 5.0, 6.0]);
        let result = native_vec_dot(&[a, b]).unwrap();
        // 1*4 + 2*5 + 3*6 = 4 + 10 + 18 = 32
        assert!((as_float(&result) - 32.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_dot_perpendicular() {
        let a = float_vec(&[1.0, 0.0]);
        let b = float_vec(&[0.0, 1.0]);
        let result = native_vec_dot(&[a, b]).unwrap();
        assert!(as_float(&result).abs() < 1e-10);
    }

    #[test]
    fn test_vec_cross_basic() {
        let a = float_vec(&[1.0, 0.0, 0.0]);
        let b = float_vec(&[0.0, 1.0, 0.0]);
        let result = native_vec_cross(&[a, b]).unwrap();
        let vals = as_float_vec(&result);
        // i × j = k
        assert!(vals[0].abs() < 1e-10);
        assert!(vals[1].abs() < 1e-10);
        assert!((vals[2] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_cross_anti_commutative() {
        let a = float_vec(&[1.0, 2.0, 3.0]);
        let b = float_vec(&[4.0, 5.0, 6.0]);
        let ab = native_vec_cross(&[a.clone(), b.clone()]).unwrap();
        let ba = native_vec_cross(&[b, a]).unwrap();
        let ab_vals = as_float_vec(&ab);
        let ba_vals = as_float_vec(&ba);
        // a × b = -(b × a)
        for i in 0..3 {
            assert!((ab_vals[i] + ba_vals[i]).abs() < 1e-10);
        }
    }

    #[test]
    fn test_vec_cross_non_3d_fails() {
        let a = float_vec(&[1.0, 2.0]);
        let b = float_vec(&[3.0, 4.0]);
        assert!(native_vec_cross(&[a, b]).is_err());
    }

    #[test]
    fn test_vec_length_basic() {
        let v = float_vec(&[3.0, 4.0]);
        let result = native_vec_length(&[v]).unwrap();
        assert!((as_float(&result) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_length_3d() {
        let v = float_vec(&[1.0, 2.0, 2.0]);
        let result = native_vec_length(&[v]).unwrap();
        // sqrt(1 + 4 + 4) = 3
        assert!((as_float(&result) - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_length_zero() {
        let v = float_vec(&[0.0, 0.0, 0.0]);
        let result = native_vec_length(&[v]).unwrap();
        assert!(as_float(&result).abs() < 1e-10);
    }

    #[test]
    fn test_vec_length_sq_basic() {
        let v = float_vec(&[3.0, 4.0]);
        let result = native_vec_length_sq(&[v]).unwrap();
        // 9 + 16 = 25
        assert!((as_float(&result) - 25.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_normalize_basic() {
        let v = float_vec(&[3.0, 4.0]);
        let result = native_vec_normalize(&[v]).unwrap();
        let vals = as_float_vec(&result);
        assert!((vals[0] - 0.6).abs() < 1e-10);
        assert!((vals[1] - 0.8).abs() < 1e-10);
    }

    #[test]
    fn test_vec_normalize_unit() {
        let v = float_vec(&[1.0, 0.0, 0.0]);
        let result = native_vec_normalize(&[v]).unwrap();
        let vals = as_float_vec(&result);
        assert!((vals[0] - 1.0).abs() < 1e-10);
        assert!(vals[1].abs() < 1e-10);
        assert!(vals[2].abs() < 1e-10);
    }

    #[test]
    fn test_vec_normalize_zero_returns_zero() {
        let v = float_vec(&[0.0, 0.0]);
        let result = native_vec_normalize(&[v]).unwrap();
        let vals = as_float_vec(&result);
        assert!(vals[0].abs() < 1e-10);
        assert!(vals[1].abs() < 1e-10);
    }

    #[test]
    fn test_vec_distance_basic() {
        let a = float_vec(&[0.0, 0.0]);
        let b = float_vec(&[3.0, 4.0]);
        let result = native_vec_distance(&[a, b]).unwrap();
        assert!((as_float(&result) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_distance_same_point() {
        let a = float_vec(&[5.0, 5.0, 5.0]);
        let b = float_vec(&[5.0, 5.0, 5.0]);
        let result = native_vec_distance(&[a, b]).unwrap();
        assert!(as_float(&result).abs() < 1e-10);
    }

    #[test]
    fn test_vec_lerp_t0() {
        let a = float_vec(&[0.0, 0.0]);
        let b = float_vec(&[10.0, 20.0]);
        let result = native_vec_lerp(&[a, b, Value::Float(0.0)]).unwrap();
        let vals = as_float_vec(&result);
        assert!(vals[0].abs() < 1e-10);
        assert!(vals[1].abs() < 1e-10);
    }

    #[test]
    fn test_vec_lerp_t1() {
        let a = float_vec(&[0.0, 0.0]);
        let b = float_vec(&[10.0, 20.0]);
        let result = native_vec_lerp(&[a, b, Value::Float(1.0)]).unwrap();
        let vals = as_float_vec(&result);
        assert!((vals[0] - 10.0).abs() < 1e-10);
        assert!((vals[1] - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_lerp_t_half() {
        let a = float_vec(&[0.0, 0.0]);
        let b = float_vec(&[10.0, 20.0]);
        let result = native_vec_lerp(&[a, b, Value::Float(0.5)]).unwrap();
        let vals = as_float_vec(&result);
        assert!((vals[0] - 5.0).abs() < 1e-10);
        assert!((vals[1] - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_angle_perpendicular() {
        let a = float_vec(&[1.0, 0.0]);
        let b = float_vec(&[0.0, 1.0]);
        let result = native_vec_angle(&[a, b]).unwrap();
        assert!((as_float(&result) - PI / 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_angle_same_direction() {
        let a = float_vec(&[1.0, 0.0]);
        let b = float_vec(&[2.0, 0.0]);
        let result = native_vec_angle(&[a, b]).unwrap();
        assert!(as_float(&result).abs() < 1e-10);
    }

    #[test]
    fn test_vec_angle_opposite() {
        let a = float_vec(&[1.0, 0.0]);
        let b = float_vec(&[-1.0, 0.0]);
        let result = native_vec_angle(&[a, b]).unwrap();
        assert!((as_float(&result) - PI).abs() < 1e-10);
    }

    #[test]
    fn test_vec_angle_zero_vector() {
        let a = float_vec(&[0.0, 0.0]);
        let b = float_vec(&[1.0, 0.0]);
        let result = native_vec_angle(&[a, b]).unwrap();
        // Zero vector returns 0 angle
        assert!(as_float(&result).abs() < 1e-10);
    }

    // ==================== Math Constants ====================

    #[test]
    fn test_pi_constant() {
        let result = native_pi(&[]).unwrap();
        assert!((as_float(&result) - PI).abs() < 1e-10);
    }

    #[test]
    fn test_e_constant() {
        let result = native_e(&[]).unwrap();
        assert!((as_float(&result) - std::f64::consts::E).abs() < 1e-10);
    }

    // ==================== Utility Math Functions ====================

    #[test]
    fn test_clamp_within_range() {
        let result =
            native_clamp(&[Value::Float(5.0), Value::Float(0.0), Value::Float(10.0)]).unwrap();
        assert!((as_float(&result) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_clamp_below_min() {
        let result =
            native_clamp(&[Value::Float(-5.0), Value::Float(0.0), Value::Float(10.0)]).unwrap();
        assert!(as_float(&result).abs() < 1e-10);
    }

    #[test]
    fn test_clamp_above_max() {
        let result =
            native_clamp(&[Value::Float(15.0), Value::Float(0.0), Value::Float(10.0)]).unwrap();
        assert!((as_float(&result) - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_trunc_positive() {
        let result = native_trunc(&[Value::Float(3.7)]).unwrap();
        assert!((as_float(&result) - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_trunc_negative() {
        let result = native_trunc(&[Value::Float(-3.7)]).unwrap();
        assert!((as_float(&result) + 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_rem_positive() {
        let result = native_rem(&[Value::Int(10), Value::Int(3)]).unwrap();
        match result {
            Value::Int(v) => assert_eq!(v, 1),
            _ => panic!("Expected int"),
        }
    }

    #[test]
    fn test_rem_float() {
        let result = native_rem(&[Value::Float(10.5), Value::Float(3.0)]).unwrap();
        assert!((as_float(&result) - 1.5).abs() < 1e-10);
    }
}
