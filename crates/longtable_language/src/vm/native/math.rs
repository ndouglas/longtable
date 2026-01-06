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
