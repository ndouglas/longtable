//! Constraint system for Longtable.
//!
//! Constraints are invariants checked after rule execution:
//! - Pattern-based matching to find entities to check
//! - Expression evaluation for check conditions
//! - Rollback or warn on violation

use std::collections::HashSet;

use longtable_foundation::{Error, ErrorKind, Interner, KeywordId, Result, Value};
use longtable_language::declaration::{ConstraintDecl, ConstraintViolation};
use longtable_language::{CompiledExpr, Vm, compile_expression_with_interner};
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
    /// Variable names in order (for binding lookup)
    pub binding_vars: Vec<String>,
    /// Local bindings (name, compiled expression)
    pub bindings: Vec<(String, CompiledExpr)>,
    /// Aggregation expressions (name, compiled expression)
    pub aggregates: Vec<(String, CompiledExpr)>,
    /// Guard expressions (compiled) - filter before checking
    pub guards: Vec<CompiledExpr>,
    /// Check expressions (compiled) - all must be true
    pub checks: Vec<CompiledExpr>,
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
#[derive(Clone, Debug, Default)]
pub struct ConstraintResult {
    /// Violations that require rollback
    rollback: Vec<ViolationDetails>,
    /// Violations that only warn
    warn: Vec<ViolationDetails>,
}

impl ConstraintResult {
    /// Creates an empty (ok) result.
    #[must_use]
    pub fn ok() -> Self {
        Self::default()
    }

    /// Returns true if no rollback violations occurred.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.rollback.is_empty()
    }

    /// Returns true if there are no violations of any kind.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.rollback.is_empty() && self.warn.is_empty()
    }

    /// Returns violations that should cause rollback.
    #[must_use]
    pub fn rollback_violations(&self) -> &[ViolationDetails] {
        &self.rollback
    }

    /// Returns violations that should only warn.
    #[must_use]
    pub fn warn_violations(&self) -> &[ViolationDetails] {
        &self.warn
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

        // Collect all variable names from pattern for binding lookup
        let mut binding_vars = Vec::new();
        for clause in &pattern.clauses {
            if !binding_vars.contains(&clause.entity_var) {
                binding_vars.push(clause.entity_var.clone());
            }
            if let crate::pattern::CompiledBinding::Variable(v) = &clause.binding {
                if !binding_vars.contains(v) {
                    binding_vars.push(v.clone());
                }
            }
        }

        // Compile let bindings and add their variable names
        // Clone the interner for each compilation so keywords are properly resolved
        let mut compiled_bindings = Vec::new();
        for (bind_name, ast) in &decl.bindings {
            let compiled = compile_expression_with_interner(ast, &binding_vars, interner.clone())?;
            compiled_bindings.push((bind_name.clone(), compiled));
            if !binding_vars.contains(bind_name) {
                binding_vars.push(bind_name.clone());
            }
        }

        // Compile aggregation expressions
        let aggregates = decl
            .aggregates
            .iter()
            .map(|(agg_name, ast)| {
                let compiled =
                    compile_expression_with_interner(ast, &binding_vars, interner.clone())?;
                Ok((agg_name.clone(), compiled))
            })
            .collect::<Result<Vec<_>>>()?;

        // Compile guard expressions
        let guards = decl
            .guards
            .iter()
            .map(|ast| compile_expression_with_interner(ast, &binding_vars, interner.clone()))
            .collect::<Result<Vec<_>>>()?;

        // Compile check expressions
        let checks = decl
            .checks
            .iter()
            .map(|ast| compile_expression_with_interner(ast, &binding_vars, interner.clone()))
            .collect::<Result<Vec<_>>>()?;

        Ok(CompiledConstraint {
            name,
            pattern,
            binding_vars,
            bindings: compiled_bindings,
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
#[derive(Clone, Debug)]
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
        let mut rollback_violations = Vec::new();
        let mut warn_violations = Vec::new();

        for constraint in &self.constraints {
            // Find all matches for this constraint's pattern
            let matches = PatternMatcher::match_pattern(&constraint.pattern, world);

            'binding_loop: for bindings in matches {
                // Convert bindings to value vector for VM
                let mut values = Self::bindings_to_vec(&bindings, &constraint.binding_vars);

                // Apply let bindings
                for (name, expr) in &constraint.bindings {
                    let mut vm = Vm::new();
                    vm.set_bindings(values.clone());
                    let Ok(result) = vm.execute_bytecode(&expr.code, &expr.constants) else {
                        continue 'binding_loop; // Skip on evaluation error
                    };

                    // Add to values
                    let idx = constraint
                        .binding_vars
                        .iter()
                        .position(|v| v == name)
                        .unwrap_or(values.len());
                    if idx < values.len() {
                        values[idx] = result;
                    } else {
                        values.push(result);
                    }
                }

                // Evaluate guard expressions - if any returns false, skip this binding
                let mut passes_guards = true;
                for guard in &constraint.guards {
                    let mut vm = Vm::new();
                    vm.set_bindings(values.clone());
                    let Ok(result) = vm.execute_bytecode(&guard.code, &guard.constants) else {
                        passes_guards = false;
                        break;
                    };
                    if result != Value::Bool(true) {
                        passes_guards = false;
                        break;
                    }
                }

                if !passes_guards {
                    continue 'binding_loop;
                }

                // Evaluate check expressions - all must be true
                for (check_index, check) in constraint.checks.iter().enumerate() {
                    let mut vm = Vm::new();
                    vm.set_bindings(values.clone());
                    let result = match vm.execute_bytecode(&check.code, &check.constants) {
                        Ok(v) => v,
                        Err(_) => Value::Bool(false), // Treat evaluation error as check failure
                    };

                    if result != Value::Bool(true) {
                        // Check failed - record violation
                        let violation = ViolationDetails {
                            constraint: constraint.name,
                            bindings: constraint
                                .binding_vars
                                .iter()
                                .zip(values.iter())
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect(),
                            failed_check_index: check_index,
                            behavior: constraint.on_violation,
                        };

                        match constraint.on_violation {
                            ConstraintViolation::Rollback => rollback_violations.push(violation),
                            ConstraintViolation::Warn => warn_violations.push(violation),
                        }

                        // Stop checking further checks for this binding
                        break;
                    }
                }
            }
        }

        ConstraintResult {
            rollback: rollback_violations,
            warn: warn_violations,
        }
    }

    /// Converts pattern bindings to a value vector for VM execution.
    fn bindings_to_vec(bindings: &crate::pattern::Bindings, vars: &[String]) -> Vec<Value> {
        vars.iter()
            .map(|v| bindings.get(v).cloned().unwrap_or(Value::Nil))
            .collect()
    }

    /// Validate a constraint check result.
    ///
    /// # Errors
    /// Returns an error if any rollback violations occurred.
    pub fn validate(&self, result: ConstraintResult) -> Result<Vec<ViolationDetails>> {
        if result.is_ok() {
            // Return warn violations (may be empty)
            Ok(result.warn)
        } else {
            let names: Vec<_> = result
                .rollback
                .iter()
                .map(|v| format!("{:?}", v.constraint))
                .collect();
            Err(Error::new(ErrorKind::Internal(format!(
                "constraint violations require rollback: {}",
                names.join(", ")
            ))))
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
    use longtable_language::declaration::{
        Pattern as DeclPattern, PatternClause as DeclClause, PatternValue,
    };
    use longtable_language::{Ast, Span};
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

    #[test]
    fn check_passes_when_condition_met() {
        use longtable_foundation::LtMap;

        let mut world = World::new(42);

        // Register a simple "score" tag component that stores an integer directly
        let score = world.interner_mut().intern_keyword("score");
        world = world
            .register_component(ComponentSchema::tag(score))
            .unwrap();

        // Spawn an entity with score = true (tag present)
        let (world, entity) = world.spawn(&LtMap::new()).unwrap();
        let mut world = world.set(entity, score, Value::Bool(true)).unwrap();

        // Create a constraint: score must be true (entity must have the tag)
        // Use world's interner so keyword IDs match
        let mut decl = ConstraintDecl::new("has-score", Span::default());
        decl.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "score".to_string(),
                value: PatternValue::Variable("s".to_string()),
                span: Span::default(),
            }],
            negations: vec![],
        };
        // Check: (= ?s true) - the bound value must be true
        decl.checks.push(Ast::List(
            vec![
                Ast::Symbol("=".to_string(), Span::default()),
                Ast::Symbol("?s".to_string(), Span::default()),
                Ast::Bool(true, Span::default()),
            ],
            Span::default(),
        ));
        decl.on_violation = ConstraintViolation::Rollback;

        let compiled = ConstraintCompiler::compile(&decl, world.interner_mut()).unwrap();
        let checker = ConstraintChecker::new().with_constraints(vec![compiled]);

        // Score is true, so constraint passes
        let result = checker.check_all(&world);
        assert!(result.is_ok());
    }

    #[test]
    fn check_fails_with_rollback_violation() {
        use longtable_foundation::LtMap;
        use longtable_storage::FieldSchema;

        let mut world = World::new(42);

        // Register a component with a boolean "active" field
        let status = world.interner_mut().intern_keyword("status");
        let active_field = world.interner_mut().intern_keyword("active");
        let schema = ComponentSchema::new(status).with_field(FieldSchema::required(
            active_field,
            longtable_foundation::Type::Bool,
        ));
        world = world.register_component(schema).unwrap();

        // Spawn an entity with status.active = false
        let (world, entity) = world.spawn(&LtMap::new()).unwrap();
        let mut comp = LtMap::new();
        comp = comp.insert(Value::Keyword(active_field), Value::Bool(false));
        let mut world = world.set(entity, status, Value::Map(comp)).unwrap();

        // Create a constraint: (get ?s :active) must be true
        let mut decl = ConstraintDecl::new("must-be-active", Span::default());
        decl.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "status".to_string(),
                value: PatternValue::Variable("s".to_string()),
                span: Span::default(),
            }],
            negations: vec![],
        };
        // Check: (= (get ?s :active) true)
        decl.checks.push(Ast::List(
            vec![
                Ast::Symbol("=".to_string(), Span::default()),
                Ast::List(
                    vec![
                        Ast::Symbol("get".to_string(), Span::default()),
                        Ast::Symbol("?s".to_string(), Span::default()),
                        Ast::Keyword("active".to_string(), Span::default()),
                    ],
                    Span::default(),
                ),
                Ast::Bool(true, Span::default()),
            ],
            Span::default(),
        ));
        decl.on_violation = ConstraintViolation::Rollback;

        let compiled = ConstraintCompiler::compile(&decl, world.interner_mut()).unwrap();
        let checker = ConstraintChecker::new().with_constraints(vec![compiled]);

        // Active is false, but constraint requires true, so it fails with rollback
        let result = checker.check_all(&world);
        assert!(!result.is_ok());
        assert_eq!(result.rollback_violations().len(), 1);
        assert_eq!(result.rollback_violations()[0].failed_check_index, 0);
    }

    #[test]
    fn check_fails_with_warn_violation() {
        use longtable_foundation::LtMap;
        use longtable_storage::FieldSchema;

        let mut world = World::new(42);

        // Register a component with a boolean "valid" field
        let status = world.interner_mut().intern_keyword("status");
        let valid_field = world.interner_mut().intern_keyword("valid");
        let schema = ComponentSchema::new(status).with_field(FieldSchema::required(
            valid_field,
            longtable_foundation::Type::Bool,
        ));
        world = world.register_component(schema).unwrap();

        // Spawn an entity with status.valid = false
        let (world, entity) = world.spawn(&LtMap::new()).unwrap();
        let mut comp = LtMap::new();
        comp = comp.insert(Value::Keyword(valid_field), Value::Bool(false));
        let mut world = world.set(entity, status, Value::Map(comp)).unwrap();

        // Create a constraint with Warn behavior
        let mut decl = ConstraintDecl::new("should-be-valid", Span::default());
        decl.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "status".to_string(),
                value: PatternValue::Variable("s".to_string()),
                span: Span::default(),
            }],
            negations: vec![],
        };
        // Check: (= (get ?s :valid) true)
        decl.checks.push(Ast::List(
            vec![
                Ast::Symbol("=".to_string(), Span::default()),
                Ast::List(
                    vec![
                        Ast::Symbol("get".to_string(), Span::default()),
                        Ast::Symbol("?s".to_string(), Span::default()),
                        Ast::Keyword("valid".to_string(), Span::default()),
                    ],
                    Span::default(),
                ),
                Ast::Bool(true, Span::default()),
            ],
            Span::default(),
        ));
        decl.on_violation = ConstraintViolation::Warn;

        let compiled = ConstraintCompiler::compile(&decl, world.interner_mut()).unwrap();
        let checker = ConstraintChecker::new().with_constraints(vec![compiled]);

        // Constraint fails but is_ok returns true (only rollback makes is_ok false)
        let result = checker.check_all(&world);
        assert!(result.is_ok()); // Warn doesn't block
        assert_eq!(result.warn_violations().len(), 1);
    }

    #[test]
    fn guard_filters_entities_before_check() {
        use longtable_foundation::LtMap;
        use longtable_storage::FieldSchema;

        let mut world = World::new(42);

        // Register components with boolean fields
        let status = world.interner_mut().intern_keyword("status");
        let active_field = world.interner_mut().intern_keyword("active");
        let is_player_field = world.interner_mut().intern_keyword("is-player");
        let schema = ComponentSchema::new(status)
            .with_field(FieldSchema::required(
                active_field,
                longtable_foundation::Type::Bool,
            ))
            .with_field(FieldSchema::required(
                is_player_field,
                longtable_foundation::Type::Bool,
            ));
        world = world.register_component(schema).unwrap();

        // Spawn an NPC with active = false, is_player = false
        let (world, _npc) = world.spawn(&LtMap::new()).unwrap();
        let mut comp = LtMap::new();
        comp = comp.insert(Value::Keyword(active_field), Value::Bool(false));
        comp = comp.insert(Value::Keyword(is_player_field), Value::Bool(false));
        let world = world.set(_npc, status, Value::Map(comp)).unwrap();

        // Spawn a player with active = true, is_player = true
        let (world, player_entity) = world.spawn(&LtMap::new()).unwrap();
        let mut comp = LtMap::new();
        comp = comp.insert(Value::Keyword(active_field), Value::Bool(true));
        comp = comp.insert(Value::Keyword(is_player_field), Value::Bool(true));
        let mut world = world.set(player_entity, status, Value::Map(comp)).unwrap();

        // Create constraint: only players must be active
        // Guard: (= (get ?s :is-player) true) - only check players
        // Check: (= (get ?s :active) true) - active must be true
        let mut decl = ConstraintDecl::new("player-must-be-active", Span::default());
        decl.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "status".to_string(),
                value: PatternValue::Variable("s".to_string()),
                span: Span::default(),
            }],
            negations: vec![],
        };
        // Guard: only check if is_player is true
        decl.guards.push(Ast::List(
            vec![
                Ast::Symbol("=".to_string(), Span::default()),
                Ast::List(
                    vec![
                        Ast::Symbol("get".to_string(), Span::default()),
                        Ast::Symbol("?s".to_string(), Span::default()),
                        Ast::Keyword("is-player".to_string(), Span::default()),
                    ],
                    Span::default(),
                ),
                Ast::Bool(true, Span::default()),
            ],
            Span::default(),
        ));
        // Check: active must be true
        decl.checks.push(Ast::List(
            vec![
                Ast::Symbol("=".to_string(), Span::default()),
                Ast::List(
                    vec![
                        Ast::Symbol("get".to_string(), Span::default()),
                        Ast::Symbol("?s".to_string(), Span::default()),
                        Ast::Keyword("active".to_string(), Span::default()),
                    ],
                    Span::default(),
                ),
                Ast::Bool(true, Span::default()),
            ],
            Span::default(),
        ));
        decl.on_violation = ConstraintViolation::Rollback;

        let compiled = ConstraintCompiler::compile(&decl, world.interner_mut()).unwrap();
        let checker = ConstraintChecker::new().with_constraints(vec![compiled]);

        // NPC is filtered out by guard (is_player = false),
        // player is active, so constraint passes
        let result = checker.check_all(&world);
        assert!(result.is_ok());
    }

    #[test]
    fn multiple_checks_reports_first_failure() {
        use longtable_foundation::LtMap;
        use longtable_storage::FieldSchema;

        let mut world = World::new(42);

        // Register component with two boolean fields
        let checks = world.interner_mut().intern_keyword("checks");
        let field_a = world.interner_mut().intern_keyword("a");
        let field_b = world.interner_mut().intern_keyword("b");
        let schema = ComponentSchema::new(checks)
            .with_field(FieldSchema::required(
                field_a,
                longtable_foundation::Type::Bool,
            ))
            .with_field(FieldSchema::required(
                field_b,
                longtable_foundation::Type::Bool,
            ));
        world = world.register_component(schema).unwrap();

        // Spawn an entity with a = true, b = false
        // First check passes, second check fails
        let (world, entity) = world.spawn(&LtMap::new()).unwrap();
        let mut comp = LtMap::new();
        comp = comp.insert(Value::Keyword(field_a), Value::Bool(true));
        comp = comp.insert(Value::Keyword(field_b), Value::Bool(false));
        let mut world = world.set(entity, checks, Value::Map(comp)).unwrap();

        // Create constraint with two checks: a must be true AND b must be true
        let mut decl = ConstraintDecl::new("both-checks", Span::default());
        decl.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "checks".to_string(),
                value: PatternValue::Variable("c".to_string()),
                span: Span::default(),
            }],
            negations: vec![],
        };
        // Check 1: (= (get ?c :a) true) - will pass
        decl.checks.push(Ast::List(
            vec![
                Ast::Symbol("=".to_string(), Span::default()),
                Ast::List(
                    vec![
                        Ast::Symbol("get".to_string(), Span::default()),
                        Ast::Symbol("?c".to_string(), Span::default()),
                        Ast::Keyword("a".to_string(), Span::default()),
                    ],
                    Span::default(),
                ),
                Ast::Bool(true, Span::default()),
            ],
            Span::default(),
        ));
        // Check 2: (= (get ?c :b) true) - will fail
        decl.checks.push(Ast::List(
            vec![
                Ast::Symbol("=".to_string(), Span::default()),
                Ast::List(
                    vec![
                        Ast::Symbol("get".to_string(), Span::default()),
                        Ast::Symbol("?c".to_string(), Span::default()),
                        Ast::Keyword("b".to_string(), Span::default()),
                    ],
                    Span::default(),
                ),
                Ast::Bool(true, Span::default()),
            ],
            Span::default(),
        ));
        decl.on_violation = ConstraintViolation::Rollback;

        let compiled = ConstraintCompiler::compile(&decl, world.interner_mut()).unwrap();
        let checker = ConstraintChecker::new().with_constraints(vec![compiled]);

        // First check passes (a=true), second check fails (b=false)
        let result = checker.check_all(&world);
        assert!(!result.is_ok());
        assert_eq!(result.rollback_violations().len(), 1);
        assert_eq!(result.rollback_violations()[0].failed_check_index, 1); // Second check failed
    }

    #[test]
    fn constraint_with_let_binding() {
        use longtable_foundation::LtMap;
        use longtable_storage::FieldSchema;

        let mut world = World::new(42);

        // Register a component with x and y fields
        let point = world.interner_mut().intern_keyword("point");
        let x_field = world.interner_mut().intern_keyword("x");
        let y_field = world.interner_mut().intern_keyword("y");
        let schema = ComponentSchema::new(point)
            .with_field(FieldSchema::required(
                x_field,
                longtable_foundation::Type::Int,
            ))
            .with_field(FieldSchema::required(
                y_field,
                longtable_foundation::Type::Int,
            ));
        world = world.register_component(schema).unwrap();

        // Spawn an entity with x=3, y=4 (sum = 7)
        let (world, entity) = world.spawn(&LtMap::new()).unwrap();
        let mut comp = LtMap::new();
        comp = comp.insert(Value::Keyword(x_field), Value::Int(3));
        comp = comp.insert(Value::Keyword(y_field), Value::Int(4));
        let mut world = world.set(entity, point, Value::Map(comp)).unwrap();

        // Create constraint with let binding: sum = x + y, check sum < 10
        let mut decl = ConstraintDecl::new("sum-check", Span::default());
        decl.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "point".to_string(),
                value: PatternValue::Variable("p".to_string()),
                span: Span::default(),
            }],
            negations: vec![],
        };
        // Let: sum = (+ (get ?p :x) (get ?p :y))
        decl.bindings.push((
            "sum".to_string(),
            Ast::List(
                vec![
                    Ast::Symbol("+".to_string(), Span::default()),
                    Ast::List(
                        vec![
                            Ast::Symbol("get".to_string(), Span::default()),
                            Ast::Symbol("?p".to_string(), Span::default()),
                            Ast::Keyword("x".to_string(), Span::default()),
                        ],
                        Span::default(),
                    ),
                    Ast::List(
                        vec![
                            Ast::Symbol("get".to_string(), Span::default()),
                            Ast::Symbol("?p".to_string(), Span::default()),
                            Ast::Keyword("y".to_string(), Span::default()),
                        ],
                        Span::default(),
                    ),
                ],
                Span::default(),
            ),
        ));
        // Check: (< ?sum 10)
        decl.checks.push(Ast::List(
            vec![
                Ast::Symbol("<".to_string(), Span::default()),
                Ast::Symbol("?sum".to_string(), Span::default()),
                Ast::Int(10, Span::default()),
            ],
            Span::default(),
        ));
        decl.on_violation = ConstraintViolation::Rollback;

        let compiled = ConstraintCompiler::compile(&decl, world.interner_mut()).unwrap();
        let checker = ConstraintChecker::new().with_constraints(vec![compiled]);

        // sum = 3 + 4 = 7 < 10, so constraint passes
        let result = checker.check_all(&world);
        assert!(result.is_ok());
    }

    #[test]
    fn multiple_constraints_reports_both_violations() {
        use longtable_foundation::LtMap;

        let mut world = World::new(42);

        // Register two separate tag components
        let valid_tag = world.interner_mut().intern_keyword("valid-tag");
        let active_tag = world.interner_mut().intern_keyword("active-tag");
        world = world
            .register_component(ComponentSchema::tag(valid_tag))
            .unwrap();
        world = world
            .register_component(ComponentSchema::tag(active_tag))
            .unwrap();

        // Spawn entity with both tags = true (but we'll check for them being "not present"
        // by using a check that they equal false, which will fail)
        let (world, entity) = world.spawn(&LtMap::new()).unwrap();
        let world = world.set(entity, valid_tag, Value::Bool(true)).unwrap();
        let mut world = world.set(entity, active_tag, Value::Bool(true)).unwrap();

        // Constraint 1: valid-tag must equal false (Rollback) - will fail since it's true
        let mut decl1 = ConstraintDecl::new("must-be-invalid", Span::default());
        decl1.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "valid-tag".to_string(),
                value: PatternValue::Variable("v".to_string()),
                span: Span::default(),
            }],
            negations: vec![],
        };
        decl1.checks.push(Ast::List(
            vec![
                Ast::Symbol("=".to_string(), Span::default()),
                Ast::Symbol("?v".to_string(), Span::default()),
                Ast::Bool(false, Span::default()), // Expect false, but it's true
            ],
            Span::default(),
        ));
        decl1.on_violation = ConstraintViolation::Rollback;

        // Constraint 2: active-tag must equal false (Warn) - will fail since it's true
        let mut decl2 = ConstraintDecl::new("should-be-inactive", Span::default());
        decl2.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "active-tag".to_string(),
                value: PatternValue::Variable("a".to_string()),
                span: Span::default(),
            }],
            negations: vec![],
        };
        decl2.checks.push(Ast::List(
            vec![
                Ast::Symbol("=".to_string(), Span::default()),
                Ast::Symbol("?a".to_string(), Span::default()),
                Ast::Bool(false, Span::default()), // Expect false, but it's true
            ],
            Span::default(),
        ));
        decl2.on_violation = ConstraintViolation::Warn;

        let c1 = ConstraintCompiler::compile(&decl1, world.interner_mut()).unwrap();
        let c2 = ConstraintCompiler::compile(&decl2, world.interner_mut()).unwrap();
        let checker = ConstraintChecker::new().with_constraints(vec![c1, c2]);

        // Both constraints should fail
        let result = checker.check_all(&world);
        assert!(!result.is_ok()); // Rollback makes is_ok false
        assert_eq!(result.rollback_violations().len(), 1);
        assert_eq!(result.warn_violations().len(), 1);
    }

    #[test]
    fn constraint_with_no_checks_always_passes() {
        use longtable_foundation::LtMap;

        let mut world = World::new(42);

        // Register a tag component
        let marker = world.interner_mut().intern_keyword("marker");
        world = world
            .register_component(ComponentSchema::tag(marker))
            .unwrap();

        // Spawn entity with marker
        let (world, entity) = world.spawn(&LtMap::new()).unwrap();
        let mut world = world.set(entity, marker, Value::Bool(true)).unwrap();

        // Constraint with pattern but no checks
        let mut decl = ConstraintDecl::new("marker-exists", Span::default());
        decl.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "marker".to_string(),
                value: PatternValue::Variable("m".to_string()),
                span: Span::default(),
            }],
            negations: vec![],
        };
        // No checks added
        decl.on_violation = ConstraintViolation::Rollback;

        let compiled = ConstraintCompiler::compile(&decl, world.interner_mut()).unwrap();
        let checker = ConstraintChecker::new().with_constraints(vec![compiled]);

        // No checks means always passes
        let result = checker.check_all(&world);
        assert!(result.is_ok());
    }

    #[test]
    fn multiple_entities_all_checked() {
        use longtable_foundation::LtMap;
        use longtable_storage::FieldSchema;

        let mut world = World::new(42);

        // Register component
        let status = world.interner_mut().intern_keyword("status");
        let valid_field = world.interner_mut().intern_keyword("valid");
        let schema = ComponentSchema::new(status).with_field(FieldSchema::required(
            valid_field,
            longtable_foundation::Type::Bool,
        ));
        world = world.register_component(schema).unwrap();

        // Spawn 3 entities: 2 valid, 1 invalid
        let (world, e1) = world.spawn(&LtMap::new()).unwrap();
        let mut comp = LtMap::new();
        comp = comp.insert(Value::Keyword(valid_field), Value::Bool(true));
        let world = world.set(e1, status, Value::Map(comp)).unwrap();

        let (world, e2) = world.spawn(&LtMap::new()).unwrap();
        let mut comp = LtMap::new();
        comp = comp.insert(Value::Keyword(valid_field), Value::Bool(false)); // Invalid!
        let world = world.set(e2, status, Value::Map(comp)).unwrap();

        let (world, e3) = world.spawn(&LtMap::new()).unwrap();
        let mut comp = LtMap::new();
        comp = comp.insert(Value::Keyword(valid_field), Value::Bool(true));
        let mut world = world.set(e3, status, Value::Map(comp)).unwrap();

        // Constraint: valid must be true
        let mut decl = ConstraintDecl::new("must-be-valid", Span::default());
        decl.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "status".to_string(),
                value: PatternValue::Variable("s".to_string()),
                span: Span::default(),
            }],
            negations: vec![],
        };
        decl.checks.push(Ast::List(
            vec![
                Ast::Symbol("=".to_string(), Span::default()),
                Ast::List(
                    vec![
                        Ast::Symbol("get".to_string(), Span::default()),
                        Ast::Symbol("?s".to_string(), Span::default()),
                        Ast::Keyword("valid".to_string(), Span::default()),
                    ],
                    Span::default(),
                ),
                Ast::Bool(true, Span::default()),
            ],
            Span::default(),
        ));
        decl.on_violation = ConstraintViolation::Rollback;

        let compiled = ConstraintCompiler::compile(&decl, world.interner_mut()).unwrap();
        let checker = ConstraintChecker::new().with_constraints(vec![compiled]);

        // Only e2 violates, so we should have exactly 1 violation
        let result = checker.check_all(&world);
        assert!(!result.is_ok());
        assert_eq!(result.rollback_violations().len(), 1);
    }
}
