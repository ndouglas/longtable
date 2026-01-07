//! Integration tests for constraints
//!
//! Tests constraint compilation and checking.
//!
//! Note: The constraint checker's check_all method is currently a placeholder
//! implementation that doesn't actually evaluate check expressions. These tests
//! verify the API structure and pattern matching portions of constraints.

use longtable_engine::constraint::{ConstraintChecker, ConstraintCompiler, ConstraintResult};
use longtable_foundation::{Interner, LtMap, Value};
use longtable_language::Span;
use longtable_language::declaration::{
    ConstraintDecl, ConstraintViolation, Pattern as DeclPattern, PatternClause as DeclClause,
    PatternValue,
};
use longtable_storage::{ComponentSchema, World};

/// Helper to create a constraint declaration that matches entities with a component
fn make_constraint_decl(
    name: &str,
    component: &str,
    violation: ConstraintViolation,
) -> ConstraintDecl {
    let mut decl = ConstraintDecl::new(name, Span::default());
    decl.pattern = DeclPattern {
        clauses: vec![DeclClause {
            entity_var: "e".to_string(),
            component: component.to_string(),
            value: PatternValue::Wildcard,
            span: Span::default(),
        }],
        negations: vec![],
    };
    decl.on_violation = violation;
    decl
}

// =============================================================================
// Constraint Checker API
// =============================================================================

#[test]
fn constraint_checker_new() {
    let checker = ConstraintChecker::new();
    assert!(checker.constraints().is_empty());
}

#[test]
fn constraint_checker_with_constraints() {
    let mut interner = Interner::new();

    let decl = make_constraint_decl("test/constraint", "health", ConstraintViolation::Rollback);
    let compiled = ConstraintCompiler::compile(&decl, &mut interner).unwrap();

    let checker = ConstraintChecker::new().with_constraints(vec![compiled]);
    assert_eq!(checker.constraints().len(), 1);
}

#[test]
fn constraint_checker_add_constraint() {
    let mut interner = Interner::new();

    let decl = make_constraint_decl("test/constraint", "health", ConstraintViolation::Rollback);
    let compiled = ConstraintCompiler::compile(&decl, &mut interner).unwrap();

    let mut checker = ConstraintChecker::new();
    checker.add_constraint(compiled);

    assert_eq!(checker.constraints().len(), 1);
}

#[test]
fn constraint_checker_monitored_components() {
    let mut interner = Interner::new();

    let decl1 = make_constraint_decl("c1", "health", ConstraintViolation::Rollback);
    let decl2 = make_constraint_decl("c2", "mana", ConstraintViolation::Warn);

    let c1 = ConstraintCompiler::compile(&decl1, &mut interner).unwrap();
    let c2 = ConstraintCompiler::compile(&decl2, &mut interner).unwrap();

    let checker = ConstraintChecker::new().with_constraints(vec![c1, c2]);
    let monitored = checker.monitored_components();

    // Should monitor both components
    assert_eq!(monitored.len(), 2);
}

// =============================================================================
// Constraint Compilation
// =============================================================================

#[test]
fn compile_simple_constraint() {
    let mut interner = Interner::new();

    let decl = make_constraint_decl("test/health-check", "health", ConstraintViolation::Rollback);
    let compiled = ConstraintCompiler::compile(&decl, &mut interner).unwrap();

    assert_eq!(compiled.on_violation, ConstraintViolation::Rollback);
    assert_eq!(compiled.pattern.clauses.len(), 1);
}

#[test]
fn compile_warn_constraint() {
    let mut interner = Interner::new();

    let decl = make_constraint_decl("test/soft-check", "health", ConstraintViolation::Warn);
    let compiled = ConstraintCompiler::compile(&decl, &mut interner).unwrap();

    assert_eq!(compiled.on_violation, ConstraintViolation::Warn);
}

#[test]
fn compile_constraint_with_multiple_clauses() {
    let mut interner = Interner::new();

    let mut decl = ConstraintDecl::new("test/multi-clause", Span::default());
    decl.pattern = DeclPattern {
        clauses: vec![
            DeclClause {
                entity_var: "e".to_string(),
                component: "health".to_string(),
                value: PatternValue::Variable("hp".to_string()),
                span: Span::default(),
            },
            DeclClause {
                entity_var: "e".to_string(),
                component: "max-health".to_string(),
                value: PatternValue::Variable("max_hp".to_string()),
                span: Span::default(),
            },
        ],
        negations: vec![],
    };
    decl.on_violation = ConstraintViolation::Rollback;

    let compiled = ConstraintCompiler::compile(&decl, &mut interner).unwrap();

    assert_eq!(compiled.pattern.clauses.len(), 2);
}

#[test]
fn compile_multiple_constraints() {
    let mut interner = Interner::new();

    let decls = vec![
        make_constraint_decl("c1", "health", ConstraintViolation::Rollback),
        make_constraint_decl("c2", "mana", ConstraintViolation::Warn),
        make_constraint_decl("c3", "stamina", ConstraintViolation::Rollback),
    ];

    let compiled = ConstraintCompiler::compile_all(&decls, &mut interner).unwrap();

    assert_eq!(compiled.len(), 3);
}

// =============================================================================
// Constraint Checking (Basic - placeholder implementation)
// =============================================================================

#[test]
fn check_all_empty_world() {
    let world = World::new(42);
    let checker = ConstraintChecker::new();

    let result = checker.check_all(&world);
    assert!(result.is_ok());
}

#[test]
fn check_all_no_constraints() {
    let mut world = World::new(42);
    let health_kw = world.interner_mut().intern_keyword("health");

    let world = world
        .register_component(ComponentSchema::tag(health_kw))
        .unwrap();
    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let _world = world.set(entity, health_kw, Value::Bool(true)).unwrap();

    let checker = ConstraintChecker::new();
    let result = checker.check_all(&_world);

    // No constraints = always passes
    assert!(result.is_ok());
}

#[test]
fn check_all_with_constraints_no_matches() {
    let mut world = World::new(42);
    let health_kw = world.interner_mut().intern_keyword("health");

    let world = world
        .register_component(ComponentSchema::tag(health_kw))
        .unwrap();
    // Don't add any entities with health

    let mut interner = Interner::new();
    let decl = make_constraint_decl("test/constraint", "health", ConstraintViolation::Rollback);
    let compiled = ConstraintCompiler::compile(&decl, &mut interner).unwrap();

    let checker = ConstraintChecker::new().with_constraints(vec![compiled]);
    let result = checker.check_all(&world);

    // No matches = passes (constraint only applies to matching entities)
    assert!(result.is_ok());
}

// =============================================================================
// Constraint Result API
// =============================================================================

#[test]
fn constraint_result_ok_methods() {
    let result = ConstraintResult::Ok;

    assert!(result.is_ok());
    assert!(result.rollback_violations().is_empty());
    assert!(result.warn_violations().is_empty());
}

// =============================================================================
// Constraint Order Preservation
// =============================================================================

#[test]
fn constraints_preserve_declaration_order() {
    let mut interner = Interner::new();

    let c1 = ConstraintCompiler::compile(
        &make_constraint_decl("first", "a", ConstraintViolation::Rollback),
        &mut interner,
    )
    .unwrap();
    let c2 = ConstraintCompiler::compile(
        &make_constraint_decl("second", "b", ConstraintViolation::Warn),
        &mut interner,
    )
    .unwrap();
    let c3 = ConstraintCompiler::compile(
        &make_constraint_decl("third", "c", ConstraintViolation::Rollback),
        &mut interner,
    )
    .unwrap();

    let checker =
        ConstraintChecker::new().with_constraints(vec![c1.clone(), c2.clone(), c3.clone()]);

    let constraints = checker.constraints();
    assert_eq!(constraints.len(), 3);
    assert_eq!(constraints[0].name, c1.name);
    assert_eq!(constraints[1].name, c2.name);
    assert_eq!(constraints[2].name, c3.name);
}

#[test]
fn add_constraint_preserves_order() {
    let mut interner = Interner::new();

    let c1 = ConstraintCompiler::compile(
        &make_constraint_decl("first", "a", ConstraintViolation::Rollback),
        &mut interner,
    )
    .unwrap();
    let c2 = ConstraintCompiler::compile(
        &make_constraint_decl("second", "b", ConstraintViolation::Warn),
        &mut interner,
    )
    .unwrap();

    let mut checker = ConstraintChecker::new();
    checker.add_constraint(c1.clone());
    checker.add_constraint(c2.clone());

    let constraints = checker.constraints();
    assert_eq!(constraints[0].name, c1.name);
    assert_eq!(constraints[1].name, c2.name);
}

// =============================================================================
// Validate Helper
// =============================================================================

#[test]
fn validate_ok_result() {
    let checker = ConstraintChecker::new();
    let result = ConstraintResult::Ok;

    let validation = checker.validate(result);
    assert!(validation.is_ok());
    assert!(validation.unwrap().is_empty());
}
