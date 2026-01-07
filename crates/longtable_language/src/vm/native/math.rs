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
