//! Macro definition types.
//!
//! This module provides types for representing macro definitions.
//!
//! # Example
//!
//! ```clojure
//! (defmacro when [condition & body]
//!   `(if ~condition
//!      (do ~@body)
//!      nil))
//! ```

use crate::ast::Ast;
use crate::span::Span;

// =============================================================================
// MacroParam
// =============================================================================

/// A parameter in a macro definition.
#[derive(Clone, Debug, PartialEq)]
pub enum MacroParam {
    /// A normal positional parameter.
    Normal(String),
    /// A rest parameter (& rest) that collects remaining arguments.
    Rest(String),
}

impl MacroParam {
    /// Returns the parameter name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Normal(name) | Self::Rest(name) => name,
        }
    }

    /// Returns true if this is a rest parameter.
    #[must_use]
    pub fn is_rest(&self) -> bool {
        matches!(self, Self::Rest(_))
    }
}

// =============================================================================
// MacroDef
// =============================================================================

/// A macro definition.
///
/// Represents a `defmacro` form:
/// ```clojure
/// (defmacro name [params...]
///   body...)
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct MacroDef {
    /// The macro name (unqualified).
    pub name: String,
    /// The namespace this macro is defined in.
    pub namespace: String,
    /// Macro parameters.
    pub params: Vec<MacroParam>,
    /// Whether this macro accepts variadic arguments.
    pub variadic: bool,
    /// Macro body expressions (to be substituted).
    pub body: Vec<Ast>,
    /// Source span for error reporting.
    pub span: Span,
}

impl MacroDef {
    /// Creates a new macro definition.
    #[must_use]
    pub fn new(
        name: String,
        namespace: String,
        params: Vec<MacroParam>,
        body: Vec<Ast>,
        span: Span,
    ) -> Self {
        let variadic = params.iter().any(MacroParam::is_rest);
        Self {
            name,
            namespace,
            params,
            variadic,
            body,
            span,
        }
    }

    /// Returns the fully qualified name of the macro.
    #[must_use]
    pub fn qualified_name(&self) -> String {
        format!("{}/{}", self.namespace, self.name)
    }

    /// Returns the minimum required argument count.
    #[must_use]
    pub fn min_arity(&self) -> usize {
        self.params.iter().filter(|p| !p.is_rest()).count()
    }

    /// Returns the maximum argument count (None if variadic).
    #[must_use]
    pub fn max_arity(&self) -> Option<usize> {
        if self.variadic {
            None
        } else {
            Some(self.params.len())
        }
    }

    /// Checks if the given argument count is valid for this macro.
    #[must_use]
    pub fn accepts_arity(&self, count: usize) -> bool {
        let min = self.min_arity();
        match self.max_arity() {
            Some(max) => count >= min && count <= max,
            None => count >= min,
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macro_param_normal() {
        let param = MacroParam::Normal("x".to_string());
        assert_eq!(param.name(), "x");
        assert!(!param.is_rest());
    }

    #[test]
    fn macro_param_rest() {
        let param = MacroParam::Rest("body".to_string());
        assert_eq!(param.name(), "body");
        assert!(param.is_rest());
    }

    #[test]
    fn macro_def_non_variadic() {
        let def = MacroDef::new(
            "my-macro".to_string(),
            "user".to_string(),
            vec![
                MacroParam::Normal("a".to_string()),
                MacroParam::Normal("b".to_string()),
            ],
            vec![],
            Span::default(),
        );

        assert_eq!(def.name, "my-macro");
        assert!(!def.variadic);
        assert_eq!(def.min_arity(), 2);
        assert_eq!(def.max_arity(), Some(2));
        assert!(def.accepts_arity(2));
        assert!(!def.accepts_arity(1));
        assert!(!def.accepts_arity(3));
    }

    #[test]
    fn macro_def_variadic() {
        let def = MacroDef::new(
            "when".to_string(),
            "core".to_string(),
            vec![
                MacroParam::Normal("condition".to_string()),
                MacroParam::Rest("body".to_string()),
            ],
            vec![],
            Span::default(),
        );

        assert_eq!(def.name, "when");
        assert!(def.variadic);
        assert_eq!(def.min_arity(), 1);
        assert_eq!(def.max_arity(), None);
        assert!(!def.accepts_arity(0));
        assert!(def.accepts_arity(1));
        assert!(def.accepts_arity(5));
        assert!(def.accepts_arity(100));
    }

    #[test]
    fn macro_def_qualified_name() {
        let def = MacroDef::new(
            "when".to_string(),
            "clojure.core".to_string(),
            vec![],
            vec![],
            Span::default(),
        );

        assert_eq!(def.qualified_name(), "clojure.core/when");
    }
}
