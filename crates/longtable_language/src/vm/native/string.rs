//! String manipulation functions for the VM.

use super::format_value;
use longtable_foundation::{Error, ErrorKind, LtVec, Result, Value};

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

/// String: str/split - split string by delimiter
pub(crate) fn native_str_split(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::String(s)), Some(Value::String(delim))) => {
            let parts: LtVec<Value> = s
                .split(&**delim)
                .map(|p| Value::String(p.to_string().into()))
                .collect();
            Ok(Value::Vec(parts))
        }
        (Some(Value::Nil), _) | (_, Some(Value::Nil)) => Ok(Value::Vec(LtVec::new())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::String,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// String: str/join - join collection with delimiter
pub(crate) fn native_str_join(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::String(delim)), Some(Value::Vec(v))) => {
            let parts: Vec<String> = v.iter().map(format_value).collect();
            Ok(Value::String(parts.join(&**delim).into()))
        }
        (Some(Value::String(delim)), Some(Value::Nil)) => Ok(Value::String(delim.clone())),
        (Some(Value::String(_)), None) => Err(Error::new(ErrorKind::Internal(
            "str/join requires 2 arguments".to_string(),
        ))),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::String,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// String: str/trim - trim whitespace from both ends
pub(crate) fn native_str_trim(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::String(s)) => Ok(Value::String(s.trim().to_string().into())),
        Some(Value::Nil) => Ok(Value::String("".into())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::String,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// String: str/trim-left - trim whitespace from start
pub(crate) fn native_str_trim_left(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::String(s)) => Ok(Value::String(s.trim_start().to_string().into())),
        Some(Value::Nil) => Ok(Value::String("".into())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::String,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// String: str/trim-right - trim whitespace from end
pub(crate) fn native_str_trim_right(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::String(s)) => Ok(Value::String(s.trim_end().to_string().into())),
        Some(Value::Nil) => Ok(Value::String("".into())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::String,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// String: str/starts-with? - check prefix
pub(crate) fn native_str_starts_with(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::String(s)), Some(Value::String(prefix))) => {
            Ok(Value::Bool(s.starts_with(&**prefix)))
        }
        (Some(Value::Nil), _) | (_, Some(Value::Nil)) => Ok(Value::Bool(false)),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::String,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// String: str/ends-with? - check suffix
pub(crate) fn native_str_ends_with(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::String(s)), Some(Value::String(suffix))) => {
            Ok(Value::Bool(s.ends_with(&**suffix)))
        }
        (Some(Value::Nil), _) | (_, Some(Value::Nil)) => Ok(Value::Bool(false)),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::String,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// String: str/contains? - check substring
pub(crate) fn native_str_contains(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::String(s)), Some(Value::String(substr))) => {
            Ok(Value::Bool(s.contains(&**substr)))
        }
        (Some(Value::Nil), _) | (_, Some(Value::Nil)) => Ok(Value::Bool(false)),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::String,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// String: str/replace - replace first occurrence
pub(crate) fn native_str_replace(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1), args.get(2)) {
        (Some(Value::String(s)), Some(Value::String(from)), Some(Value::String(to))) => {
            Ok(Value::String(s.replacen(&**from, to, 1).into()))
        }
        (Some(Value::Nil), _, _) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::String,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// String: str/replace-all - replace all occurrences
pub(crate) fn native_str_replace_all(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1), args.get(2)) {
        (Some(Value::String(s)), Some(Value::String(from)), Some(Value::String(to))) => {
            Ok(Value::String(s.replace(&**from, to).into()))
        }
        (Some(Value::Nil), _, _) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::String,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// String: str/blank? - check if empty or whitespace only
pub(crate) fn native_str_blank(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::String(s)) => Ok(Value::Bool(s.trim().is_empty())),
        Some(Value::Nil) => Ok(Value::Bool(true)),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::String,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// String: str/substring - extract substring
/// (str/substring s start) or (str/substring s start end)
pub(crate) fn native_str_substring(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1), args.get(2)) {
        (Some(Value::String(s)), Some(Value::Int(start)), end) => {
            let start = *start as usize;
            let chars: Vec<char> = s.chars().collect();
            let end = match end {
                Some(Value::Int(e)) => (*e as usize).min(chars.len()),
                None => chars.len(),
                _ => {
                    return Err(Error::new(ErrorKind::TypeMismatch {
                        expected: longtable_foundation::Type::Int,
                        actual: end.unwrap().value_type(),
                    }));
                }
            };
            if start > chars.len() {
                return Ok(Value::String("".into()));
            }
            let result: String = chars[start..end.min(chars.len())].iter().collect();
            Ok(Value::String(result.into()))
        }
        (Some(Value::Nil), _, _) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::String,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Format: format - template formatting with {} placeholders
/// (format "Hello, {}!" "world") -> "Hello, world!"
pub(crate) fn native_format(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::String(template)) => {
            let mut result = template.to_string();
            for arg in args.iter().skip(1) {
                // Replace first {} with the argument
                if let Some(pos) = result.find("{}") {
                    let formatted = format_value(arg);
                    result = format!("{}{}{}", &result[..pos], formatted, &result[pos + 2..]);
                }
            }
            Ok(Value::String(result.into()))
        }
        Some(Value::Nil) => Ok(Value::String("".into())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::String,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}
