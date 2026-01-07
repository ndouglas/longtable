//! Integration tests for the rule engine
//!
//! Tests rule compilation, activation finding, and firing.

use longtable_engine::pattern::{CompiledBinding, CompiledClause, CompiledPattern};
use longtable_engine::rule::{CompiledRule, ProductionRuleEngine};
use longtable_foundation::{LtMap, Value};
use longtable_storage::{ComponentSchema, World};

/// Helper to create a simple rule that matches entities with a component
fn simple_rule(
    name: longtable_foundation::KeywordId,
    component: longtable_foundation::KeywordId,
) -> CompiledRule {
    CompiledRule::new(
        name,
        CompiledPattern {
            clauses: vec![CompiledClause {
                entity_var: "?e".to_string(),
                component,
                binding: CompiledBinding::Wildcard,
            }],
            negations: vec![],
        },
    )
}

// =============================================================================
// Activation Finding
// =============================================================================

#[test]
fn find_activations_single_rule() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");
    let rule_kw = world.interner_mut().intern_keyword("rules/test");

    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();
    let (world, player) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(player, player_kw, Value::Bool(true)).unwrap();

    let rule = simple_rule(rule_kw, player_kw);
    let rules = vec![rule];

    let mut engine = ProductionRuleEngine::new();
    engine.begin_tick();
    let activations = engine.find_activations(&rules, &world);

    assert_eq!(activations.len(), 1);
    assert_eq!(activations[0].rule_name, rule_kw);
    assert_eq!(activations[0].bindings.get_entity("?e"), Some(player));
}

#[test]
fn find_activations_multiple_matches() {
    let mut world = World::new(42);
    let enemy_kw = world.interner_mut().intern_keyword("tag/enemy");
    let rule_kw = world.interner_mut().intern_keyword("rules/test");

    let world = world
        .register_component(ComponentSchema::tag(enemy_kw))
        .unwrap();
    let (world, e1) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(e1, enemy_kw, Value::Bool(true)).unwrap();
    let (world, e2) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(e2, enemy_kw, Value::Bool(true)).unwrap();

    let rule = simple_rule(rule_kw, enemy_kw);
    let rules = vec![rule];

    let mut engine = ProductionRuleEngine::new();
    engine.begin_tick();
    let activations = engine.find_activations(&rules, &world);

    assert_eq!(activations.len(), 2);
}

#[test]
fn find_activations_no_match() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");
    let enemy_kw = world.interner_mut().intern_keyword("tag/enemy");
    let rule_kw = world.interner_mut().intern_keyword("rules/test");

    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();
    let world = world
        .register_component(ComponentSchema::tag(enemy_kw))
        .unwrap();
    let (world, player) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(player, player_kw, Value::Bool(true)).unwrap();

    // Rule looks for enemies, but only players exist
    let rule = simple_rule(rule_kw, enemy_kw);
    let rules = vec![rule];

    let mut engine = ProductionRuleEngine::new();
    engine.begin_tick();
    let activations = engine.find_activations(&rules, &world);

    assert_eq!(activations.len(), 0);
}

// =============================================================================
// Salience (Priority)
// =============================================================================

#[test]
fn activations_sorted_by_salience() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");
    let low_kw = world.interner_mut().intern_keyword("rules/low");
    let high_kw = world.interner_mut().intern_keyword("rules/high");

    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();
    let (world, player) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(player, player_kw, Value::Bool(true)).unwrap();

    let low_rule = simple_rule(low_kw, player_kw).with_salience(0);
    let high_rule = simple_rule(high_kw, player_kw).with_salience(100);

    // Add low rule first
    let rules = vec![low_rule, high_rule];

    let mut engine = ProductionRuleEngine::new();
    engine.begin_tick();
    let activations = engine.find_activations(&rules, &world);

    assert_eq!(activations.len(), 2);
    // High salience should be first
    assert_eq!(activations[0].rule_name, high_kw);
    assert_eq!(activations[1].rule_name, low_kw);
}

// =============================================================================
// Once Rules
// =============================================================================

#[test]
fn once_rule_prevents_refire() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");
    let rule_kw = world.interner_mut().intern_keyword("rules/once");

    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();
    let (world, p1) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(p1, player_kw, Value::Bool(true)).unwrap();
    let (world, p2) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(p2, player_kw, Value::Bool(true)).unwrap();

    // Create a rule that only fires once per tick
    let rule = simple_rule(rule_kw, player_kw).with_once(true);
    let rules = vec![rule];

    let mut engine = ProductionRuleEngine::new();
    engine.begin_tick();

    // First call finds all matching entities
    let activations = engine.find_activations(&rules, &world);
    assert_eq!(activations.len(), 2);

    // Fire one activation - this marks the rule as "once_fired"
    let _ = engine.fire(&activations[0], world.clone(), &rules, |_, w| {
        Ok((vec![], w.clone()))
    });

    // After firing, once rule should not return any more activations
    let activations_after = engine.find_activations(&rules, &world);
    assert_eq!(activations_after.len(), 0);
}

// =============================================================================
// Engine State
// =============================================================================

#[test]
fn begin_tick_resets_state() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");
    let rule_kw = world.interner_mut().intern_keyword("rules/test");

    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();
    let (world, player) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(player, player_kw, Value::Bool(true)).unwrap();

    let rule = simple_rule(rule_kw, player_kw);
    let rules = vec![rule];

    let mut engine = ProductionRuleEngine::new();

    // First tick
    engine.begin_tick();
    let act1 = engine.find_activations(&rules, &world);
    assert_eq!(act1.len(), 1);

    // Second tick - should find activations again after reset
    engine.begin_tick();
    let act2 = engine.find_activations(&rules, &world);
    assert_eq!(act2.len(), 1);
}

// =============================================================================
// Max Activations (Kill Switch)
// =============================================================================

#[test]
fn max_activations_kill_switch() {
    use longtable_engine::pattern::Bindings;

    let mut world = World::new(42);
    let test_kw = world.interner_mut().intern_keyword("rules/test");

    // Engine with max 3 activations
    let mut engine = ProductionRuleEngine::new().with_max_activations(3);
    engine.begin_tick();

    let activation = longtable_engine::rule::Activation {
        rule_name: test_kw,
        bindings: Bindings::new(),
        salience: 0,
        specificity: 0,
    };

    // Fire 3 times successfully
    for _ in 0..3 {
        let result = engine.fire(&activation, world.clone(), &[], |_, w| {
            Ok((vec![], w.clone()))
        });
        assert!(result.is_ok());
    }

    // Fourth fire should fail (kill switch triggered)
    let result = engine.fire(&activation, world.clone(), &[], |_, w| {
        Ok((vec![], w.clone()))
    });
    assert!(result.is_err());
}
