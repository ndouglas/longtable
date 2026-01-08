//! Production rule engine for Longtable.
//!
//! This module provides the rule engine that manages rule execution,
//! refraction, and the run-to-quiescence loop.

pub mod compiler;

pub use compiler::{CompiledRuleBody, FullCompiledRule, RuleCompiler};

use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use longtable_foundation::{Error, KeywordId, Result, SemanticLimit};
use longtable_language::VmEffect;
use longtable_storage::World;

use crate::pattern::{Bindings, CompiledPattern, PatternMatcher};

// =============================================================================
// Compiled Rule
// =============================================================================

/// A compiled rule ready for execution.
#[derive(Clone, Debug)]
pub struct CompiledRule {
    /// Rule name (interned keyword)
    pub name: KeywordId,
    /// Priority (higher fires first)
    pub salience: i32,
    /// Compiled pattern for matching
    pub pattern: CompiledPattern,
    /// Fire only once per tick
    pub once: bool,
    /// Whether rule is enabled
    pub enabled: bool,
}

impl CompiledRule {
    /// Creates a new compiled rule.
    #[must_use]
    pub fn new(name: KeywordId, pattern: CompiledPattern) -> Self {
        Self {
            name,
            salience: 0,
            pattern,
            once: false,
            enabled: true,
        }
    }

    /// Sets the salience (priority).
    #[must_use]
    pub fn with_salience(mut self, salience: i32) -> Self {
        self.salience = salience;
        self
    }

    /// Sets the once flag.
    #[must_use]
    pub fn with_once(mut self, once: bool) -> Self {
        self.once = once;
        self
    }
}

impl From<FullCompiledRule> for CompiledRule {
    fn from(full: FullCompiledRule) -> Self {
        Self {
            name: full.name,
            salience: full.salience,
            pattern: full.pattern,
            once: full.once,
            enabled: full.enabled,
        }
    }
}

// =============================================================================
// Activation
// =============================================================================

/// A rule activation ready to fire.
#[derive(Clone, Debug)]
pub struct Activation {
    /// Which rule
    pub rule_name: KeywordId,
    /// Variable bindings from pattern match
    pub bindings: Bindings,
    /// Rule salience
    pub salience: i32,
    /// Pattern specificity (number of clauses)
    pub specificity: usize,
}

impl Activation {
    /// Compute refraction key (rule + entity bindings).
    #[must_use]
    pub fn refraction_key(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.rule_name.hash(&mut hasher);
        self.bindings.refraction_key().hash(&mut hasher);
        hasher.finish()
    }
}

// =============================================================================
// Effect Record
// =============================================================================

/// Record of an effect for debugging/logging.
#[derive(Clone, Debug)]
pub struct EffectRecord {
    /// Which rule caused this
    pub rule: KeywordId,
    /// The effect
    pub effect: VmEffect,
}

// =============================================================================
// Rule Engine
// =============================================================================

/// Manages rule execution within a tick.
#[derive(Clone, Debug)]
pub struct ProductionRuleEngine {
    /// Refracted activations (already fired this tick)
    refracted: HashSet<u64>,
    /// Rules that fired with :once flag
    once_fired: HashSet<KeywordId>,
    /// Effect log for this tick
    effects: Vec<EffectRecord>,
    /// Number of activations this tick (for kill switch)
    activation_count: usize,
    /// Maximum activations per tick
    max_activations: usize,
}

impl Default for ProductionRuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ProductionRuleEngine {
    /// Creates a new rule engine.
    #[must_use]
    pub fn new() -> Self {
        Self {
            refracted: HashSet::new(),
            once_fired: HashSet::new(),
            effects: Vec::new(),
            activation_count: 0,
            max_activations: 10_000, // Kill switch
        }
    }

    /// Sets the maximum activations (kill switch threshold).
    #[must_use]
    pub fn with_max_activations(mut self, max: usize) -> Self {
        self.max_activations = max;
        self
    }

    /// Resets for a new tick.
    pub fn begin_tick(&mut self) {
        self.refracted.clear();
        self.once_fired.clear();
        self.effects.clear();
        self.activation_count = 0;
    }

    /// Find all current activations, respecting refraction.
    #[must_use]
    pub fn find_activations(&self, rules: &[CompiledRule], world: &World) -> Vec<Activation> {
        let mut activations = Vec::new();

        for rule in rules {
            // Skip disabled rules
            if !rule.enabled {
                continue;
            }

            // Skip :once rules that already fired
            if rule.once && self.once_fired.contains(&rule.name) {
                continue;
            }

            // Find pattern matches
            let matches = PatternMatcher::match_pattern(&rule.pattern, world);

            for bindings in matches {
                let activation = Activation {
                    rule_name: rule.name,
                    bindings,
                    salience: rule.salience,
                    specificity: rule.pattern.clauses.len(),
                };

                // Skip if refracted
                if self.refracted.contains(&activation.refraction_key()) {
                    continue;
                }

                activations.push(activation);
            }
        }

        // Sort by salience (descending), then specificity (descending)
        activations.sort_by(|a, b| {
            b.salience
                .cmp(&a.salience)
                .then_with(|| b.specificity.cmp(&a.specificity))
        });

        activations
    }

    /// Fire an activation.
    ///
    /// The `execute` callback is called with the activation and should return
    /// effects and a new world state.
    ///
    /// # Errors
    /// Returns an error if max activations is exceeded.
    #[allow(clippy::needless_pass_by_value)]
    pub fn fire<F>(
        &mut self,
        activation: &Activation,
        world: World,
        rules: &[CompiledRule],
        mut execute: F,
    ) -> Result<World>
    where
        F: FnMut(&Activation, &World) -> Result<(Vec<VmEffect>, World)>,
    {
        // Check kill switch
        self.activation_count += 1;
        if self.activation_count > self.max_activations {
            #[allow(clippy::cast_possible_truncation)]
            return Err(Error::limit_exceeded(SemanticLimit::MaxActivations {
                limit: self.max_activations as u32,
                context: None,
            }));
        }

        // Record refraction
        self.refracted.insert(activation.refraction_key());

        // Track :once rules
        if let Some(rule) = rules.iter().find(|r| r.name == activation.rule_name) {
            if rule.once {
                self.once_fired.insert(activation.rule_name);
            }
        }

        // Execute and collect effects
        let (effects, new_world) = execute(activation, &world)?;

        // Record effects
        for effect in effects {
            self.effects.push(EffectRecord {
                rule: activation.rule_name,
                effect,
            });
        }

        Ok(new_world)
    }

    /// Run rules to quiescence.
    ///
    /// Repeatedly finds the highest-priority activation and fires it until
    /// no more activations exist or the kill switch triggers.
    ///
    /// # Errors
    /// Returns an error if max activations is exceeded or if rule execution fails.
    pub fn run_to_quiescence<F>(
        &mut self,
        rules: &[CompiledRule],
        mut world: World,
        mut execute: F,
    ) -> Result<World>
    where
        F: FnMut(&Activation, &World) -> Result<(Vec<VmEffect>, World)>,
    {
        loop {
            let activations = self.find_activations(rules, &world);

            if activations.is_empty() {
                break;
            }

            // Fire the first (highest priority) activation
            let activation = &activations[0];
            world = self.fire(activation, world, rules, &mut execute)?;
        }

        Ok(world)
    }

    /// Returns the effects recorded this tick.
    #[must_use]
    pub fn effects(&self) -> &[EffectRecord] {
        &self.effects
    }

    /// Returns the number of activations fired this tick.
    #[must_use]
    pub fn activation_count(&self) -> usize {
        self.activation_count
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::PatternCompiler;
    use longtable_foundation::{LtMap, Value};
    use longtable_language::Span;
    use longtable_language::declaration::{Pattern as DeclPattern, PatternClause, PatternValue};
    use longtable_storage::ComponentSchema;

    fn setup_world_with_entities() -> (World, KeywordId, KeywordId) {
        let mut world = World::new(42);

        // Intern keywords
        let health = world.interner_mut().intern_keyword("health");
        let processed = world.interner_mut().intern_keyword("processed");

        // Register schemas
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

        (world, health, processed)
    }

    #[test]
    fn find_activations_respects_refraction() {
        let (mut world, _health, _processed) = setup_world_with_entities();

        // Create a pattern [?e :health]
        let decl_pattern = DeclPattern {
            clauses: vec![PatternClause {
                entity_var: "e".to_string(),
                component: "health".to_string(),
                value: PatternValue::Wildcard,
                span: Span::default(),
            }],
            negations: vec![],
        };
        let compiled = PatternCompiler::compile(&decl_pattern, world.interner_mut()).unwrap();

        let rule_name = world.interner_mut().intern_keyword("test-rule");
        let rule = CompiledRule::new(rule_name, compiled);
        let rules = vec![rule];

        let mut engine = ProductionRuleEngine::new();
        engine.begin_tick();

        // First find should return 2 activations
        let activations = engine.find_activations(&rules, &world);
        assert_eq!(activations.len(), 2);

        // Mark one as refracted
        engine.refracted.insert(activations[0].refraction_key());

        // Now should return only 1
        let activations = engine.find_activations(&rules, &world);
        assert_eq!(activations.len(), 1);
    }

    #[test]
    fn once_flag_prevents_refire() {
        let (mut world, _health, _processed) = setup_world_with_entities();

        // Create pattern
        let decl_pattern = DeclPattern {
            clauses: vec![PatternClause {
                entity_var: "e".to_string(),
                component: "health".to_string(),
                value: PatternValue::Wildcard,
                span: Span::default(),
            }],
            negations: vec![],
        };
        let compiled = PatternCompiler::compile(&decl_pattern, world.interner_mut()).unwrap();

        let rule_name = world.interner_mut().intern_keyword("once-rule");
        let rule = CompiledRule::new(rule_name, compiled).with_once(true);
        let rules = vec![rule];

        let mut engine = ProductionRuleEngine::new();
        engine.begin_tick();

        // Mark rule as once-fired
        engine.once_fired.insert(rule_name);

        // Should find no activations
        let activations = engine.find_activations(&rules, &world);
        assert!(activations.is_empty());
    }

    #[test]
    fn activation_sorting_by_salience() {
        let (mut world, _health, _processed) = setup_world_with_entities();

        // Create two rules with different salience
        let decl_pattern = DeclPattern {
            clauses: vec![PatternClause {
                entity_var: "e".to_string(),
                component: "health".to_string(),
                value: PatternValue::Wildcard,
                span: Span::default(),
            }],
            negations: vec![],
        };

        let compiled1 = PatternCompiler::compile(&decl_pattern, world.interner_mut()).unwrap();
        let compiled2 = PatternCompiler::compile(&decl_pattern, world.interner_mut()).unwrap();

        let rule1_name = world.interner_mut().intern_keyword("low-priority");
        let rule2_name = world.interner_mut().intern_keyword("high-priority");

        let rule1 = CompiledRule::new(rule1_name, compiled1).with_salience(10);
        let rule2 = CompiledRule::new(rule2_name, compiled2).with_salience(100);

        let rule_list = vec![rule1, rule2];

        let mut engine = ProductionRuleEngine::new();
        engine.begin_tick();

        let activations = engine.find_activations(&rule_list, &world);

        // Higher salience should be first
        assert!(!activations.is_empty());
        assert_eq!(activations[0].salience, 100);
    }

    #[test]
    fn kill_switch_triggers() {
        let (mut world, _health, _processed) = setup_world_with_entities();

        let mut engine = ProductionRuleEngine::new().with_max_activations(3);
        engine.begin_tick();

        // Manually exceed activation count
        engine.activation_count = 3;

        // Create dummy activation
        let activation = Activation {
            rule_name: world.interner_mut().intern_keyword("test"),
            bindings: Bindings::new(),
            salience: 0,
            specificity: 0,
        };

        // Fire should fail due to kill switch
        let result = engine.fire(&activation, world, &[], |_, w| Ok((vec![], w.clone())));

        assert!(result.is_err());
    }

    /// Test that `run_to_quiescence` terminates properly when no more rules can fire
    #[test]
    fn quiescence_termination() {
        let (mut world, health, processed) = setup_world_with_entities();

        // Create a rule that matches [?e :health] (not [?e :processed])
        // When it fires, it should add :processed to the entity
        let decl_pattern = DeclPattern {
            clauses: vec![PatternClause {
                entity_var: "e".to_string(),
                component: "health".to_string(),
                value: PatternValue::Wildcard,
                span: Span::default(),
            }],
            negations: vec![PatternClause {
                entity_var: "e".to_string(),
                component: "processed".to_string(),
                value: PatternValue::Wildcard,
                span: Span::default(),
            }],
        };
        let compiled = PatternCompiler::compile(&decl_pattern, world.interner_mut()).unwrap();

        let rule_name = world.interner_mut().intern_keyword("process-health");
        let rule = CompiledRule::new(rule_name, compiled);
        let rules = vec![rule];

        let mut engine = ProductionRuleEngine::new();
        engine.begin_tick();

        // Initially should have 2 activations (2 entities with health, none processed)
        let initial_activations = engine.find_activations(&rules, &world);
        assert_eq!(initial_activations.len(), 2);

        // Run to quiescence with an executor that marks entities as processed
        let result = engine.run_to_quiescence(&rules, world, |activation, w| {
            // Mark the entity as processed
            if let Some(entity) = activation.bindings.get_entity("e") {
                let new_world = w.set(entity, processed, Value::Bool(true)).unwrap();
                return Ok((vec![], new_world));
            }
            Ok((vec![], w.clone()))
        });

        assert!(result.is_ok());
        let final_world = result.unwrap();

        // Verify all entities are now processed
        for entity in final_world.with_component(health) {
            assert!(
                final_world.has(entity, processed),
                "Entity should be processed"
            );
        }

        // Verify 2 activations fired
        assert_eq!(engine.activation_count(), 2);
    }

    /// Test deterministic ordering of activations by salience then specificity
    #[test]
    fn deterministic_ordering() {
        let (mut world, health, _processed) = setup_world_with_entities();

        // Add more components for specificity testing
        let tag_a = world.interner_mut().intern_keyword("tag-a");
        let tag_b = world.interner_mut().intern_keyword("tag-b");

        world = world
            .register_component(ComponentSchema::tag(tag_a))
            .unwrap();
        world = world
            .register_component(ComponentSchema::tag(tag_b))
            .unwrap();

        // Get an entity and add tags to it
        let entities: Vec<_> = world.with_component(health).collect();
        let e1 = entities[0];
        world = world.set(e1, tag_a, Value::Bool(true)).unwrap();
        world = world.set(e1, tag_b, Value::Bool(true)).unwrap();

        // Create rules with different saliences and specificities:
        // Rule 1: salience 10, 1 clause (low priority, low specificity)
        // Rule 2: salience 10, 2 clauses (low priority, high specificity)
        // Rule 3: salience 20, 1 clause (high priority, low specificity)

        // Rule 1: [?e :health] - salience 10, specificity 1
        let pattern1 = DeclPattern {
            clauses: vec![PatternClause {
                entity_var: "e".to_string(),
                component: "health".to_string(),
                value: PatternValue::Wildcard,
                span: Span::default(),
            }],
            negations: vec![],
        };
        let compiled1 = PatternCompiler::compile(&pattern1, world.interner_mut()).unwrap();
        let rule1_name = world.interner_mut().intern_keyword("rule1");
        let rule1 = CompiledRule::new(rule1_name, compiled1).with_salience(10);

        // Rule 2: [?e :tag-a] [?e :tag-b] - salience 10, specificity 2
        let pattern2 = DeclPattern {
            clauses: vec![
                PatternClause {
                    entity_var: "e".to_string(),
                    component: "tag-a".to_string(),
                    value: PatternValue::Wildcard,
                    span: Span::default(),
                },
                PatternClause {
                    entity_var: "e".to_string(),
                    component: "tag-b".to_string(),
                    value: PatternValue::Wildcard,
                    span: Span::default(),
                },
            ],
            negations: vec![],
        };
        let compiled2 = PatternCompiler::compile(&pattern2, world.interner_mut()).unwrap();
        let rule2_name = world.interner_mut().intern_keyword("rule2");
        let rule2 = CompiledRule::new(rule2_name, compiled2).with_salience(10);

        // Rule 3: [?e :tag-a] - salience 20, specificity 1
        let pattern3 = DeclPattern {
            clauses: vec![PatternClause {
                entity_var: "e".to_string(),
                component: "tag-a".to_string(),
                value: PatternValue::Wildcard,
                span: Span::default(),
            }],
            negations: vec![],
        };
        let compiled3 = PatternCompiler::compile(&pattern3, world.interner_mut()).unwrap();
        let rule3_name = world.interner_mut().intern_keyword("rule3");
        let rule3 = CompiledRule::new(rule3_name, compiled3).with_salience(20);

        let all_rules = vec![rule1, rule2, rule3];

        let mut engine = ProductionRuleEngine::new();
        engine.begin_tick();

        let activations = engine.find_activations(&all_rules, &world);

        // Activations for e1 should be ordered:
        // 1. rule3 (salience 20) - highest salience wins
        // 2. rule2 (salience 10, specificity 2) - same salience, higher specificity
        // 3. rule1 (salience 10, specificity 1) - same salience, lower specificity

        // Find activations for e1
        let e1_activations: Vec<_> = activations
            .iter()
            .filter(|a| a.bindings.get_entity("e") == Some(e1))
            .collect();

        // Should have at least 3 activations for e1
        assert!(e1_activations.len() >= 3);

        // First activation should be rule3 (highest salience)
        assert_eq!(e1_activations[0].salience, 20);
        assert_eq!(e1_activations[0].rule_name, rule3_name);

        // Among salience 10 activations, higher specificity should come first
        let salience_10: Vec<_> = e1_activations.iter().filter(|a| a.salience == 10).collect();
        if salience_10.len() >= 2 {
            assert!(
                salience_10[0].specificity >= salience_10[1].specificity,
                "Higher specificity should come first"
            );
        }
    }
}
