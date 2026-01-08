//! Tick orchestration for Longtable.
//!
//! A tick is the fundamental unit of simulation time. Each tick:
//! 1. Injects external inputs
//! 2. Runs rules to quiescence
//! 3. Checks constraints
//! 4. Commits changes or rolls back on constraint violation

use longtable_foundation::{EntityId, KeywordId, Result, Value};
use longtable_storage::World;

use crate::constraint::{ConstraintChecker, ConstraintResult};
use crate::derived::DerivedEvaluator;
use crate::provenance::ProvenanceTracker;
use crate::rule::{CompiledRule, ProductionRuleEngine};

// =============================================================================
// Input Event
// =============================================================================

/// An external input to inject at the start of a tick.
#[derive(Clone, Debug)]
pub enum InputEvent {
    /// Set a component value on an entity
    Set {
        /// Target entity
        entity: EntityId,
        /// Component to set
        component: KeywordId,
        /// Value to set
        value: Value,
    },
    /// Spawn a new entity with components
    Spawn {
        /// Initial component values
        components: Vec<(KeywordId, Value)>,
    },
    /// Destroy an entity
    Destroy {
        /// Entity to destroy
        entity: EntityId,
    },
    /// Custom input (for game-specific events)
    Custom {
        /// Event name
        name: KeywordId,
        /// Event payload
        payload: Value,
    },
}

// =============================================================================
// Tick Result
// =============================================================================

/// Result of a tick execution.
#[derive(Clone, Debug)]
pub struct TickResult {
    /// New world state after tick
    pub world: World,
    /// Number of rule activations fired
    pub activations_fired: usize,
    /// Constraint check result
    pub constraint_result: ConstraintResult,
    /// Whether the tick was successful (no rollback)
    pub success: bool,
}

impl TickResult {
    /// Returns true if the tick succeeded without constraint violations.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.success
    }
}

// =============================================================================
// Tick Executor
// =============================================================================

/// Orchestrates the execution of a single tick.
#[derive(Clone, Debug)]
pub struct TickExecutor {
    /// Rule engine
    rule_engine: ProductionRuleEngine,
    /// Compiled rules
    rules: Vec<CompiledRule>,
    /// Constraint checker
    constraint_checker: ConstraintChecker,
    /// Derived component evaluator
    derived_evaluator: DerivedEvaluator,
    /// Provenance tracker
    provenance: ProvenanceTracker,
    /// Current tick number
    tick_number: u64,
}

impl Default for TickExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl TickExecutor {
    /// Creates a new tick executor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rule_engine: ProductionRuleEngine::new(),
            rules: Vec::new(),
            constraint_checker: ConstraintChecker::new(),
            derived_evaluator: DerivedEvaluator::new(),
            provenance: ProvenanceTracker::new(),
            tick_number: 0,
        }
    }

    /// Sets the rules for this executor.
    #[must_use]
    pub fn with_rules(mut self, rules: Vec<CompiledRule>) -> Self {
        self.rules = rules;
        self
    }

    /// Adds a single rule to this executor.
    pub fn add_rule(&mut self, rule: CompiledRule) {
        self.rules.push(rule);
    }

    /// Returns the number of registered rules.
    #[must_use]
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Sets the constraint checker.
    #[must_use]
    pub fn with_constraints(mut self, checker: ConstraintChecker) -> Self {
        self.constraint_checker = checker;
        self
    }

    /// Sets the derived evaluator.
    #[must_use]
    pub fn with_deriveds(mut self, evaluator: DerivedEvaluator) -> Self {
        self.derived_evaluator = evaluator;
        self
    }

    /// Returns the current tick number.
    #[must_use]
    pub fn tick_number(&self) -> u64 {
        self.tick_number
    }

    /// Returns the provenance tracker.
    #[must_use]
    pub fn provenance(&self) -> &ProvenanceTracker {
        &self.provenance
    }

    /// Returns mutable access to the provenance tracker.
    pub fn provenance_mut(&mut self) -> &mut ProvenanceTracker {
        &mut self.provenance
    }

    /// Execute a single tick.
    ///
    /// # Errors
    /// Returns an error if rule execution fails (e.g., kill switch triggered).
    pub fn tick(&mut self, world: World, inputs: &[InputEvent]) -> Result<TickResult> {
        // Increment tick number
        self.tick_number += 1;

        // Save the original world for potential rollback
        let original_world = world.clone();

        // Phase 1: Begin tick (reset engine state)
        self.rule_engine.begin_tick();
        self.derived_evaluator.begin_tick();
        self.provenance.begin_tick();

        // Phase 2: Inject inputs
        let mut world = self.inject_inputs(world, inputs)?;

        // Phase 3: Run rules to quiescence
        // Note: Using a simple no-op executor for now. Full rule body execution
        // requires integration with the VM which is outside Phase 4 scope.
        world = self
            .rule_engine
            .run_to_quiescence(&self.rules, world, |_activation, w| {
                // Placeholder: actual rule execution would evaluate the rule body
                // and return effects + modified world
                Ok((vec![], w.clone()))
            })?;

        let activations_fired = self.rule_engine.activation_count();

        // Phase 4: Check constraints
        let constraint_result = self.constraint_checker.check_all(&world);

        // Phase 5: Commit or rollback
        let (final_world, success) = if constraint_result.is_ok() {
            (world, true)
        } else {
            (original_world, false)
        };

        Ok(TickResult {
            world: final_world,
            activations_fired,
            constraint_result,
            success,
        })
    }

    /// Inject input events into the world.
    fn inject_inputs(&mut self, mut world: World, inputs: &[InputEvent]) -> Result<World> {
        for input in inputs {
            world = match input {
                InputEvent::Set {
                    entity,
                    component,
                    value,
                } => {
                    // Record provenance for input-driven writes
                    // Use a special "input" rule ID (we'll use the component as a proxy)
                    self.provenance
                        .record_write(*entity, *component, *component);
                    world.set(*entity, *component, value.clone())?
                }
                InputEvent::Spawn { components } => {
                    let (new_world, spawned_entity) =
                        world.spawn(&longtable_foundation::LtMap::new())?;
                    let mut w = new_world;
                    // Set initial components
                    for (comp, val) in components {
                        w = w.set(spawned_entity, *comp, val.clone())?;
                    }
                    w
                }
                InputEvent::Destroy { entity } => world.destroy(*entity)?,
                InputEvent::Custom { .. } => {
                    // Custom events are for game logic - not handled at this level
                    world
                }
            };
        }
        Ok(world)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use longtable_foundation::LtMap;
    use longtable_language::Span;
    use longtable_language::declaration::{
        Pattern as DeclPattern, PatternClause as DeclClause, PatternValue,
    };
    use longtable_storage::ComponentSchema;

    use crate::pattern::PatternCompiler;

    #[test]
    fn tick_with_no_rules() {
        let mut world = World::new(42);

        // Register a component
        let health = world.interner_mut().intern_keyword("health");
        world = world
            .register_component(ComponentSchema::tag(health))
            .unwrap();

        // Spawn an entity
        let (world, entity) = world.spawn(&LtMap::new()).unwrap();
        let world = world.set(entity, health, Value::Bool(true)).unwrap();

        // Execute tick
        let mut executor = TickExecutor::new();
        let result = executor.tick(world, &[]).unwrap();

        // Should succeed with 0 activations
        assert!(result.is_ok());
        assert_eq!(result.activations_fired, 0);
    }

    #[test]
    fn tick_with_input_injection() {
        let mut world = World::new(42);

        // Register a component
        let health = world.interner_mut().intern_keyword("health");
        world = world
            .register_component(ComponentSchema::tag(health))
            .unwrap();

        // Spawn an entity
        let (world, entity) = world.spawn(&LtMap::new()).unwrap();

        // Execute tick with an input that sets health
        let mut executor = TickExecutor::new();
        let inputs = vec![InputEvent::Set {
            entity,
            component: health,
            value: Value::Bool(true),
        }];
        let result = executor.tick(world, &inputs).unwrap();

        assert!(result.is_ok());
        // Entity should now have health
        assert!(result.world.has(entity, health));
    }

    #[test]
    fn tick_number_increments() {
        let world = World::new(42);
        let mut executor = TickExecutor::new();

        assert_eq!(executor.tick_number(), 0);

        executor.tick(world.clone(), &[]).unwrap();
        assert_eq!(executor.tick_number(), 1);

        executor.tick(world.clone(), &[]).unwrap();
        assert_eq!(executor.tick_number(), 2);
    }

    #[test]
    fn tick_runs_rules_to_quiescence() {
        let mut world = World::new(42);

        // Register components
        let health = world.interner_mut().intern_keyword("health");
        let processed = world.interner_mut().intern_keyword("processed");

        world = world
            .register_component(ComponentSchema::tag(health))
            .unwrap();
        world = world
            .register_component(ComponentSchema::tag(processed))
            .unwrap();

        // Spawn entities with health
        let (w, e1) = world.spawn(&LtMap::new()).unwrap();
        world = w;
        world = world.set(e1, health, Value::Bool(true)).unwrap();

        let (w, e2) = world.spawn(&LtMap::new()).unwrap();
        world = w;
        world = world.set(e2, health, Value::Bool(true)).unwrap();

        // Create a rule: [?e :health] (not [?e :processed]) -> mark processed
        let pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "health".to_string(),
                value: PatternValue::Wildcard,
                span: Span::default(),
            }],
            negations: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "processed".to_string(),
                value: PatternValue::Wildcard,
                span: Span::default(),
            }],
        };
        let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();
        let rule_name = world.interner_mut().intern_keyword("process-health");
        let rule = CompiledRule::new(rule_name, compiled);

        // Create executor with rules
        // Note: Rules fire but our execute_activation is a no-op, so entities
        // won't actually be processed. This is a placeholder test.
        let executor = TickExecutor::new().with_rules(vec![rule]);
        let mut executor = executor;

        let result = executor.tick(world, &[]).unwrap();

        assert!(result.is_ok());
        // With our no-op executor, rules fire but entities aren't modified
        // so we get 2 activations (one per entity)
        assert_eq!(result.activations_fired, 2);
    }

    #[test]
    fn tick_provenance_tracking() {
        let mut world = World::new(42);

        // Register a component
        let health = world.interner_mut().intern_keyword("health");
        world = world
            .register_component(ComponentSchema::tag(health))
            .unwrap();

        // Spawn an entity
        let (world, entity) = world.spawn(&LtMap::new()).unwrap();

        // Execute tick with an input
        let mut executor = TickExecutor::new();
        let inputs = vec![InputEvent::Set {
            entity,
            component: health,
            value: Value::Bool(true),
        }];
        executor.tick(world, &inputs).unwrap();

        // Check provenance was recorded
        let who_wrote = executor.provenance().why(entity, health);
        assert!(who_wrote.is_some());
    }
}
