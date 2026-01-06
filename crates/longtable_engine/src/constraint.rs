//! Constraint system for Longtable.
//!
//! Constraints are invariants checked after rule execution:
//! - Pattern-based matching to find entities to check
//! - Expression evaluation for check conditions
//! - Rollback or warn on violation

use std::collections::HashSet;

use longtable_foundation::{Error, ErrorKind, Interner, KeywordId, Result, Value};
use longtable_language::declaration::{ConstraintDecl, ConstraintViolation};
use longtable_language::{Ast, Bytecode, compile_expression};
use longtable_storage::World;

use crate::pattern::{CompiledPattern, PatternCompiler, PatternMatcher};

// =============================================================================
// Compiled Constraint
// =============================================================================

/// A compiled constraint ready for checking.
#[derive(Clone, Debug)]
pub struct CompiledConstraint {
    /// Constraint name (interned keyword)
    pub name: KeywordId,
    /// Compiled pattern for finding entities to check
    pub pattern: CompiledPattern,
    /// Local bindings (name, AST)
    pub bindings: Vec<(String, Ast)>,
    /// Aggregation expressions (name, bytecode)
    pub aggregates: Vec<(String, Bytecode)>,
    /// Guard expressions (bytecode) - filter before checking
    pub guards: Vec<Bytecode>,
    /// Check expressions (bytecode) - all must be true
    pub checks: Vec<Bytecode>,
    /// Behavior on violation
    pub on_violation: ConstraintViolation,
}

// =============================================================================
// Constraint Violation Result
// =============================================================================

/// Details about a constraint violation.
#[derive(Clone, Debug)]
pub struct ViolationDetails {
    /// Which constraint was violated
    pub constraint: KeywordId,
    /// Variables bound during the violation
    pub bindings: Vec<(String, Value)>,
    /// Which check expression failed (index)
    pub failed_check_index: usize,
    /// Whether this should cause rollback or just warn
    pub behavior: ConstraintViolation,
}

/// Result of constraint checking.
#[derive(Clone, Debug)]
pub enum ConstraintResult {
    /// All constraints passed
    Ok,
    /// Some constraints failed with rollback
    Rollback(Vec<ViolationDetails>),
    /// Some constraints failed with warning
    Warn(Vec<ViolationDetails>),
}

impl ConstraintResult {
    /// Returns true if no rollback violations occurred.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        !matches!(self, Self::Rollback(_))
    }

    /// Returns violations that should cause rollback.
    #[must_use]
    pub fn rollback_violations(&self) -> &[ViolationDetails] {
        match self {
            Self::Rollback(v) => v,
            _ => &[],
        }
    }

    /// Returns violations that should only warn.
    #[must_use]
    pub fn warn_violations(&self) -> &[ViolationDetails] {
        match self {
            Self::Warn(v) => v,
            _ => &[],
        }
    }
}

// =============================================================================
// Constraint Compiler
// =============================================================================

/// Compiles constraint declarations.
pub struct ConstraintCompiler;

impl ConstraintCompiler {
    /// Compile a constraint declaration.
    ///
    /// # Errors
    /// Returns an error if compilation fails.
    pub fn compile(decl: &ConstraintDecl, interner: &mut Interner) -> Result<CompiledConstraint> {
        // Intern the constraint name
        let name = interner.intern_keyword(&decl.name);

        // Compile the pattern
        let pattern = PatternCompiler::compile(&decl.pattern, interner)?;

        // Collect all binding variables
        let binding_vars: Vec<String> = decl
            .pattern
            .bound_variables()
            .into_iter()
            .map(String::from)
            .collect();

        // Compile aggregation expressions
        let aggregates = decl
            .aggregates
            .iter()
            .map(|(agg_name, ast)| {
                let compiled = compile_expression(ast, &binding_vars)?;
                Ok((agg_name.clone(), compiled.code))
            })
            .collect::<Result<Vec<_>>>()?;

        // Compile guard expressions
        let guards = decl
            .guards
            .iter()
            .map(|ast| {
                let compiled = compile_expression(ast, &binding_vars)?;
                Ok(compiled.code)
            })
            .collect::<Result<Vec<_>>>()?;

        // Compile check expressions
        let checks = decl
            .checks
            .iter()
            .map(|ast| {
                let compiled = compile_expression(ast, &binding_vars)?;
                Ok(compiled.code)
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(CompiledConstraint {
            name,
            pattern,
            bindings: decl.bindings.clone(),
            aggregates,
            guards,
            checks,
            on_violation: decl.on_violation,
        })
    }

    /// Compile multiple constraints.
    ///
    /// # Errors
    /// Returns an error if any compilation fails.
    pub fn compile_all(
        decls: &[ConstraintDecl],
        interner: &mut Interner,
    ) -> Result<Vec<CompiledConstraint>> {
        decls.iter().map(|d| Self::compile(d, interner)).collect()
    }
}

// =============================================================================
// Constraint Checker
// =============================================================================

/// Checks constraints against a world state.
pub struct ConstraintChecker {
    /// Compiled constraints
    constraints: Vec<CompiledConstraint>,
}

impl Default for ConstraintChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl ConstraintChecker {
    /// Creates a new empty constraint checker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            constraints: Vec::new(),
        }
    }

    /// Creates a checker with the given constraints.
    #[must_use]
    pub fn with_constraints(mut self, constraints: Vec<CompiledConstraint>) -> Self {
        self.constraints = constraints;
        self
    }

    /// Adds a constraint.
    pub fn add_constraint(&mut self, constraint: CompiledConstraint) {
        self.constraints.push(constraint);
    }

    /// Returns the constraints.
    #[must_use]
    pub fn constraints(&self) -> &[CompiledConstraint] {
        &self.constraints
    }

    /// Check all constraints against the world.
    ///
    /// Returns `ConstraintResult::Ok` if all constraints pass,
    /// or the appropriate violation result otherwise.
    #[must_use]
    pub fn check_all(&self, world: &World) -> ConstraintResult {
        let rollback_violations = Vec::new();
        let warn_violations = Vec::new();

        for constraint in &self.constraints {
            // Find all matches for this constraint's pattern
            let matches = PatternMatcher::match_pattern(&constraint.pattern, world);

            for bindings in matches {
                // TODO: Evaluate guard expressions to filter
                // TODO: Actually evaluate check bytecode
                // For now, assume all checks pass (placeholder implementation)
                // When check evaluation is implemented, failing checks would record:
                //     ViolationDetails { constraint, bindings, failed_check_index, behavior }
                let _ = bindings; // Suppress unused warning until checks are implemented
            }
        }

        if !rollback_violations.is_empty() {
            ConstraintResult::Rollback(rollback_violations)
        } else if !warn_violations.is_empty() {
            ConstraintResult::Warn(warn_violations)
        } else {
            ConstraintResult::Ok
        }
    }

    /// Validate a constraint check result.
    ///
    /// # Errors
    /// Returns an error if any rollback violations occurred.
    pub fn validate(&self, result: ConstraintResult) -> Result<Vec<ViolationDetails>> {
        match result {
            ConstraintResult::Ok => Ok(vec![]),
            ConstraintResult::Warn(violations) => Ok(violations),
            ConstraintResult::Rollback(violations) => {
                let names: Vec<_> = violations
                    .iter()
                    .map(|v| format!("{:?}", v.constraint))
                    .collect();
                Err(Error::new(ErrorKind::Internal(format!(
                    "constraint violations require rollback: {}",
                    names.join(", ")
                ))))
            }
        }
    }

    /// Returns which components are monitored by constraints.
    #[must_use]
    pub fn monitored_components(&self) -> HashSet<KeywordId> {
        self.constraints
            .iter()
            .flat_map(|c| c.pattern.clauses.iter().map(|cl| cl.component))
            .collect()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use longtable_language::Span;
    use longtable_language::declaration::{
        Pattern as DeclPattern, PatternClause as DeclClause, PatternValue,
    };
    use longtable_storage::ComponentSchema;

    #[test]
    fn compile_simple_constraint() {
        let mut interner = Interner::new();

        // Create a simple constraint
        let mut decl = ConstraintDecl::new("health-bounds", Span::default());
        decl.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "health".to_string(),
                value: PatternValue::Variable("hp".to_string()),
                span: Span::default(),
            }],
            negations: vec![],
        };
        decl.on_violation = ConstraintViolation::Rollback;

        let compiled = ConstraintCompiler::compile(&decl, &mut interner).unwrap();

        assert_eq!(compiled.on_violation, ConstraintViolation::Rollback);
        assert_eq!(compiled.pattern.clauses.len(), 1);
    }

    #[test]
    fn compile_constraint_with_checks() {
        let mut interner = Interner::new();

        // Create a constraint with check expressions
        let mut decl = ConstraintDecl::new("positive-health", Span::default());
        decl.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "health".to_string(),
                value: PatternValue::Variable("hp".to_string()),
                span: Span::default(),
            }],
            negations: vec![],
        };
        // Add a check expression: (>= ?hp 0)
        decl.checks.push(Ast::List(
            vec![
                Ast::Symbol(">=".to_string(), Span::default()),
                Ast::Symbol("?hp".to_string(), Span::default()),
                Ast::Int(0, Span::default()),
            ],
            Span::default(),
        ));
        decl.on_violation = ConstraintViolation::Warn;

        let compiled = ConstraintCompiler::compile(&decl, &mut interner).unwrap();

        assert_eq!(compiled.checks.len(), 1);
        assert_eq!(compiled.on_violation, ConstraintViolation::Warn);
    }

    #[test]
    fn check_passes_with_no_constraints() {
        let world = World::new(42);
        let checker = ConstraintChecker::new();

        let result = checker.check_all(&world);
        assert!(result.is_ok());
    }

    #[test]
    fn check_with_constraints_no_matches() {
        let mut world = World::new(42);

        // Register a component but don't add it to any entity
        let health = world.interner_mut().intern_keyword("health");
        world = world
            .register_component(ComponentSchema::tag(health))
            .unwrap();

        // Create a constraint that requires health
        let mut interner = Interner::new();
        let mut decl = ConstraintDecl::new("health-required", Span::default());
        decl.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "health".to_string(),
                value: PatternValue::Wildcard,
                span: Span::default(),
            }],
            negations: vec![],
        };
        let compiled = ConstraintCompiler::compile(&decl, &mut interner).unwrap();

        let checker = ConstraintChecker::new().with_constraints(vec![compiled]);

        // No entities match, so constraint trivially passes
        let result = checker.check_all(&world);
        assert!(result.is_ok());
    }

    #[test]
    fn monitored_components() {
        let mut interner = Interner::new();

        // Create constraints that monitor different components
        let mut decl1 = ConstraintDecl::new("c1", Span::default());
        decl1.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "health".to_string(),
                value: PatternValue::Wildcard,
                span: Span::default(),
            }],
            negations: vec![],
        };

        let mut decl2 = ConstraintDecl::new("c2", Span::default());
        decl2.pattern = DeclPattern {
            clauses: vec![
                DeclClause {
                    entity_var: "e".to_string(),
                    component: "health".to_string(),
                    value: PatternValue::Wildcard,
                    span: Span::default(),
                },
                DeclClause {
                    entity_var: "e".to_string(),
                    component: "mana".to_string(),
                    value: PatternValue::Wildcard,
                    span: Span::default(),
                },
            ],
            negations: vec![],
        };

        let c1 = ConstraintCompiler::compile(&decl1, &mut interner).unwrap();
        let c2 = ConstraintCompiler::compile(&decl2, &mut interner).unwrap();

        let checker = ConstraintChecker::new().with_constraints(vec![c1, c2]);
        let monitored = checker.monitored_components();

        // Should monitor health and mana
        assert_eq!(monitored.len(), 2);
    }

    #[test]
    fn constraints_preserve_declaration_order() {
        // Per SPECIFICATION.md: constraints are checked in declaration order
        // This test verifies the ConstraintChecker maintains order
        let mut interner = Interner::new();

        // Create three constraints with different names
        let mut decl1 = ConstraintDecl::new("first-constraint", Span::default());
        decl1.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "health".to_string(),
                value: PatternValue::Wildcard,
                span: Span::default(),
            }],
            negations: vec![],
        };

        let mut decl2 = ConstraintDecl::new("second-constraint", Span::default());
        decl2.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "mana".to_string(),
                value: PatternValue::Wildcard,
                span: Span::default(),
            }],
            negations: vec![],
        };

        let mut decl3 = ConstraintDecl::new("third-constraint", Span::default());
        decl3.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "stamina".to_string(),
                value: PatternValue::Wildcard,
                span: Span::default(),
            }],
            negations: vec![],
        };

        let c1 = ConstraintCompiler::compile(&decl1, &mut interner).unwrap();
        let c2 = ConstraintCompiler::compile(&decl2, &mut interner).unwrap();
        let c3 = ConstraintCompiler::compile(&decl3, &mut interner).unwrap();

        // Add in specific order
        let checker =
            ConstraintChecker::new().with_constraints(vec![c1.clone(), c2.clone(), c3.clone()]);

        // Verify constraints() returns them in the same order
        let constraints = checker.constraints();
        assert_eq!(constraints.len(), 3);
        assert_eq!(constraints[0].name, c1.name);
        assert_eq!(constraints[1].name, c2.name);
        assert_eq!(constraints[2].name, c3.name);

        // Also test adding constraints one at a time
        let mut checker2 = ConstraintChecker::new();
        checker2.add_constraint(c1.clone());
        checker2.add_constraint(c2.clone());
        checker2.add_constraint(c3.clone());

        let constraints2 = checker2.constraints();
        assert_eq!(constraints2.len(), 3);
        assert_eq!(constraints2[0].name, c1.name);
        assert_eq!(constraints2[1].name, c2.name);
        assert_eq!(constraints2[2].name, c3.name);
    }
}
