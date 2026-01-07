//! Integration tests for Error types
//!
//! Tests error construction, display, context, and error kinds.

use longtable_foundation::{EntityId, Error, ErrorKind, Type};

// =============================================================================
// Error Construction
// =============================================================================

#[test]
fn error_type_mismatch() {
    let err = Error::type_mismatch(Type::Int, Type::String);
    assert!(matches!(err.kind, ErrorKind::TypeMismatch { .. }));
    let msg = format!("{err}");
    assert!(msg.contains("int") || msg.contains("string"));
}

#[test]
fn error_undefined_symbol() {
    let err = Error::undefined_symbol("foo".to_string());
    assert!(matches!(err.kind, ErrorKind::UndefinedSymbol(_)));
    let msg = format!("{err}");
    assert!(msg.contains("foo"));
}

#[test]
fn error_entity_not_found() {
    let id = EntityId::new(42, 1);
    let err = Error::entity_not_found(id);
    assert!(matches!(err.kind, ErrorKind::EntityNotFound(_)));
    let msg = format!("{err}");
    assert!(msg.contains("42"));
}

#[test]
fn error_stale_entity() {
    let id = EntityId::new(5, 2);
    let err = Error::stale_entity(id);
    assert!(matches!(err.kind, ErrorKind::StaleEntity(_)));
    let msg = format!("{err}");
    assert!(msg.contains("5"));
}

#[test]
fn error_arity_mismatch() {
    let err = Error::arity_mismatch("2".to_string(), 3);
    assert!(matches!(err.kind, ErrorKind::ArityMismatch { .. }));
    let msg = format!("{err}");
    assert!(msg.contains("2"));
    assert!(msg.contains("3"));
}

// =============================================================================
// Error Display
// =============================================================================

#[test]
fn error_display_type_mismatch() {
    let err = Error::type_mismatch(Type::Bool, Type::Int);
    let display = format!("{err}");
    // Should contain type information
    assert!(!display.is_empty());
}

#[test]
fn error_display_undefined_symbol() {
    let err = Error::undefined_symbol("player".to_string());
    let display = format!("{err}");
    assert!(display.contains("player"));
}

// =============================================================================
// Error Kind Matching
// =============================================================================

#[test]
fn error_kind_type_mismatch() {
    let err = Error::type_mismatch(Type::Int, Type::String);
    if let ErrorKind::TypeMismatch { expected, actual } = &err.kind {
        assert_eq!(*expected, Type::Int);
        assert_eq!(*actual, Type::String);
    } else {
        panic!("Expected TypeMismatch");
    }
}

#[test]
fn error_kind_undefined_symbol() {
    let err = Error::undefined_symbol("x".to_string());
    if let ErrorKind::UndefinedSymbol(name) = &err.kind {
        assert_eq!(name, "x");
    } else {
        panic!("Expected UndefinedSymbol");
    }
}

#[test]
fn error_kind_entity_not_found() {
    let id = EntityId::new(99, 5);
    let err = Error::entity_not_found(id);
    if let ErrorKind::EntityNotFound(found_id) = &err.kind {
        assert_eq!(found_id.index, 99);
        assert_eq!(found_id.generation, 5);
    } else {
        panic!("Expected EntityNotFound");
    }
}

#[test]
fn error_kind_stale_entity() {
    let id = EntityId::new(10, 3);
    let err = Error::stale_entity(id);
    if let ErrorKind::StaleEntity(found_id) = &err.kind {
        assert_eq!(found_id.index, 10);
        assert_eq!(found_id.generation, 3);
    } else {
        panic!("Expected StaleEntity");
    }
}

#[test]
fn error_kind_arity_mismatch() {
    let err = Error::arity_mismatch("exactly 2".to_string(), 5);
    if let ErrorKind::ArityMismatch { expected, actual } = &err.kind {
        assert_eq!(expected, "exactly 2");
        assert_eq!(*actual, 5);
    } else {
        panic!("Expected ArityMismatch");
    }
}

// =============================================================================
// Error with Context
// =============================================================================

#[test]
fn error_with_context() {
    use longtable_foundation::ErrorContext;

    let err = Error::undefined_symbol("foo".to_string()).with_context(ErrorContext::default());

    assert!(err.context.is_some());
}

// =============================================================================
// Error Chaining
// =============================================================================

#[test]
#[allow(clippy::result_large_err)]
fn error_result_propagation() {
    fn inner() -> Result<(), Error> {
        Err(Error::new(ErrorKind::DivisionByZero))
    }

    fn outer() -> Result<(), Error> {
        inner()?;
        Ok(())
    }

    let result = outer();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err().kind,
        ErrorKind::DivisionByZero
    ));
}

// =============================================================================
// Type enum
// =============================================================================

#[test]
fn type_display() {
    assert_eq!(format!("{}", Type::Int), "int");
    assert_eq!(format!("{}", Type::Float), "float");
    assert_eq!(format!("{}", Type::String), "string");
    assert_eq!(format!("{}", Type::Bool), "bool");
    assert_eq!(format!("{}", Type::Nil), "nil");
}

#[test]
fn type_any_matches_all() {
    // Type::Any should be used for generic type requirements
    let any = Type::Any;
    // We can at least verify it exists and displays
    let display = format!("{any}");
    assert!(!display.is_empty());
}
