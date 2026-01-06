//! Error types for the Longtable system.
//!
//! Uses `thiserror` for ergonomic error definition with rich context.

use std::fmt;

use thiserror::Error;

use crate::entity::EntityId;
use crate::types::Type;

/// The main error type for Longtable operations.
#[derive(Debug, Error)]
#[error("{kind}")]
pub struct Error {
    /// The kind of error that occurred.
    pub kind: ErrorKind,
    /// Optional context about where the error occurred.
    pub context: Option<ErrorContext>,
}

impl Error {
    /// Creates a new error with the given kind.
    #[must_use]
    pub fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            context: None,
        }
    }

    /// Adds context to this error.
    #[must_use]
    pub fn with_context(mut self, context: ErrorContext) -> Self {
        self.context = Some(context);
        self
    }

    /// Creates a type mismatch error.
    #[must_use]
    pub fn type_mismatch(expected: Type, actual: Type) -> Self {
        Self::new(ErrorKind::TypeMismatch { expected, actual })
    }

    /// Creates an entity not found error.
    #[must_use]
    pub fn entity_not_found(id: EntityId) -> Self {
        Self::new(ErrorKind::EntityNotFound(id))
    }

    /// Creates a stale entity reference error.
    #[must_use]
    pub fn stale_entity(id: EntityId) -> Self {
        Self::new(ErrorKind::StaleEntity(id))
    }

    /// Creates an undefined symbol error.
    #[must_use]
    pub fn undefined_symbol(name: String) -> Self {
        Self::new(ErrorKind::UndefinedSymbol(name))
    }

    /// Creates an arity mismatch error.
    #[must_use]
    pub fn arity_mismatch(expected: String, actual: usize) -> Self {
        Self::new(ErrorKind::ArityMismatch { expected, actual })
    }

    /// Creates a semantic limit exceeded error.
    #[must_use]
    pub fn limit_exceeded(limit: SemanticLimit) -> Self {
        Self::new(ErrorKind::LimitExceeded(limit))
    }
}

/// Categorized error kinds for pattern matching.
#[derive(Debug, Error)]
pub enum ErrorKind {
    /// Type mismatch during runtime type checking.
    #[error("type mismatch: expected {expected}, got {actual}")]
    TypeMismatch {
        /// The expected type.
        expected: Type,
        /// The actual type encountered.
        actual: Type,
    },

    /// Entity was not found in storage.
    #[error("entity not found: {0:?}")]
    EntityNotFound(EntityId),

    /// Entity reference is stale (generation mismatch).
    #[error("stale entity reference: {0:?}")]
    StaleEntity(EntityId),

    /// Symbol was not defined.
    #[error("undefined symbol: {0}")]
    UndefinedSymbol(String),

    /// Wrong number of arguments to function.
    #[error("arity mismatch: expected {expected}, got {actual}")]
    ArityMismatch {
        /// Description of expected arity.
        expected: String,
        /// Actual number of arguments.
        actual: usize,
    },

    /// Component not found on entity.
    #[error("component not found: {component} on entity {entity:?}")]
    ComponentNotFound {
        /// The entity that was queried.
        entity: EntityId,
        /// The component name that was not found.
        component: String,
    },

    /// Attribute not found on component.
    #[error("attribute not found: {attribute} on component {component}")]
    AttributeNotFound {
        /// The component that was queried.
        component: String,
        /// The attribute name that was not found.
        attribute: String,
    },

    /// Division by zero.
    #[error("division by zero")]
    DivisionByZero,

    /// Index out of bounds.
    #[error("index out of bounds: {index} (length {length})")]
    IndexOutOfBounds {
        /// The index that was accessed.
        index: usize,
        /// The actual length of the collection.
        length: usize,
    },

    /// Parse error in DSL.
    #[error("parse error at {line}:{column}: {message}")]
    ParseError {
        /// Description of the parse error.
        message: String,
        /// Line number (1-indexed).
        line: u32,
        /// Column number (1-indexed).
        column: u32,
        /// The source line where the error occurred.
        context: String,
    },

    /// Semantic limit exceeded (kill switch triggered).
    #[error("limit exceeded: {0}")]
    LimitExceeded(SemanticLimit),

    /// Internal error (should not happen).
    #[error("internal error: {0}")]
    Internal(String),
}

/// Semantic limits (kill switches) that can be exceeded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticLimit {
    /// Maximum rule activations per tick exceeded.
    MaxActivations {
        /// The configured limit.
        limit: u32,
        /// Additional context about which rule(s) caused the issue.
        context: Option<String>,
    },
    /// Maximum effects per tick exceeded.
    MaxEffects {
        /// The configured limit.
        limit: u32,
    },
    /// Maximum derived component refire depth exceeded.
    MaxRefireDepth {
        /// The configured limit.
        limit: u32,
        /// The component that exceeded the limit.
        component: Option<String>,
    },
    /// Maximum query results exceeded.
    MaxQueryResults {
        /// The configured limit.
        limit: usize,
    },
}

impl fmt::Display for SemanticLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MaxActivations { limit, context } => {
                write!(f, "max activations ({limit}) exceeded")?;
                if let Some(ctx) = context {
                    write!(f, ": {ctx}")?;
                }
                Ok(())
            }
            Self::MaxEffects { limit } => {
                write!(f, "max effects ({limit}) exceeded")
            }
            Self::MaxRefireDepth { limit, component } => {
                write!(f, "max refire depth ({limit}) exceeded")?;
                if let Some(comp) = component {
                    write!(f, " for component {comp}")?;
                }
                Ok(())
            }
            Self::MaxQueryResults { limit } => {
                write!(f, "max query results ({limit}) exceeded")
            }
        }
    }
}

/// Context about where an error occurred.
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// Source file or rule name.
    pub source: Option<String>,
    /// Line number in source.
    pub line: Option<usize>,
    /// Column number in source.
    pub column: Option<usize>,
    /// Stack trace of rule/function calls.
    pub stack: Vec<String>,
}

impl ErrorContext {
    /// Creates a new empty context.
    #[must_use]
    pub fn new() -> Self {
        Self {
            source: None,
            line: None,
            column: None,
            stack: Vec::new(),
        }
    }

    /// Sets the source location.
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Sets the line and column.
    #[must_use]
    pub fn with_position(mut self, line: usize, column: usize) -> Self {
        self.line = Some(line);
        self.column = Some(column);
        self
    }

    /// Adds a stack frame.
    #[must_use]
    pub fn with_frame(mut self, frame: impl Into<String>) -> Self {
        self.stack.push(frame.into());
        self
    }
}

impl Default for ErrorContext {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(source) = &self.source {
            write!(f, "at {source}")?;
            if let (Some(line), Some(col)) = (self.line, self.column) {
                write!(f, ":{line}:{col}")?;
            }
        }
        if !self.stack.is_empty() {
            writeln!(f)?;
            for frame in &self.stack {
                writeln!(f, "  in {frame}")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_type_mismatch() {
        let err = Error::type_mismatch(Type::Int, Type::String);
        assert!(matches!(err.kind, ErrorKind::TypeMismatch { .. }));
        let msg = format!("{err}");
        assert!(msg.contains("int"));
        assert!(msg.contains("string"));
    }

    #[test]
    fn error_with_context() {
        let err = Error::undefined_symbol("foo".to_string()).with_context(
            ErrorContext::new()
                .with_source("test.lt")
                .with_position(10, 5),
        );

        assert!(err.context.is_some());
        let ctx = err.context.unwrap();
        assert_eq!(ctx.source, Some("test.lt".to_string()));
        assert_eq!(ctx.line, Some(10));
        assert_eq!(ctx.column, Some(5));
    }

    #[test]
    fn semantic_limit_display() {
        let limit = SemanticLimit::MaxActivations {
            limit: 1000,
            context: Some("in rule combat-damage".to_string()),
        };
        let msg = format!("{limit}");
        assert!(msg.contains("1000"));
        assert!(msg.contains("combat-damage"));
    }

    #[test]
    fn error_entity_not_found() {
        let id = EntityId::new(42, 1);
        let err = Error::entity_not_found(id);
        assert!(matches!(err.kind, ErrorKind::EntityNotFound(_)));
    }

    #[test]
    fn error_stale_entity() {
        let id = EntityId::new(42, 1);
        let err = Error::stale_entity(id);
        assert!(matches!(err.kind, ErrorKind::StaleEntity(_)));
    }
}
