//! Native function implementations for the VM.
//!
//! This module contains all builtin functions organized by category:
//! - `arithmetic`: Value arithmetic and comparison helpers
//! - `predicates`: Type predicates and logic functions
//! - `collection`: Collection manipulation functions
//! - `string`: String manipulation functions
//! - `math`: Mathematical functions

mod arithmetic;
#[allow(clippy::unnecessary_wraps)]
#[allow(clippy::match_same_arms)]
mod collection;
#[allow(clippy::unnecessary_wraps)]
mod math;
#[allow(clippy::unnecessary_wraps)]
#[allow(clippy::match_same_arms)]
mod predicates;
#[allow(clippy::unnecessary_wraps)]
#[allow(clippy::redundant_closure_for_method_calls)]
mod string;

// Re-export everything for use by the VM
#[allow(clippy::wildcard_imports)]
pub(crate) use arithmetic::*;
#[allow(clippy::wildcard_imports)]
pub(crate) use collection::*;
#[allow(clippy::wildcard_imports)]
pub(crate) use math::*;
#[allow(clippy::wildcard_imports)]
pub(crate) use predicates::*;
#[allow(clippy::wildcard_imports)]
pub(crate) use string::*;

use longtable_foundation::Value;

/// Checks if a value is truthy.
///
/// In Longtable, `nil` and `false` are falsy, everything else is truthy.
pub(crate) fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Nil => false,
        Value::Bool(b) => *b,
        _ => true,
    }
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
