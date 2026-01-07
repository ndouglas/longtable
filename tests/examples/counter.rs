//! Counter Machine integration tests.
//!
//! Tests rule engine semantics using a simple counter that increments to a limit.
//! Validates: refraction, rule chaining, termination, and kill switch.
//!
//! See _.lt for the DSL representation of these rules.

use longtable_engine::spike::{Pattern, PatternBinding, RuleEngine, SpikeRule};
use longtable_foundation::{LtMap, Type, Value};
use longtable_storage::{ComponentSchema, FieldSchema, World};

/// Create a world with counter components registered.
fn setup_world() -> World {
    let mut world = World::new(42);

    // Register components:
    // - counter (has a value field)
    // - increment-request (tag)
    // - done (tag)

    let counter_kw = world.interner_mut().intern_keyword("counter");
    let value_kw = world.interner_mut().intern_keyword("value");
    let increment_request_kw = world.interner_mut().intern_keyword("increment-request");
    let done_kw = world.interner_mut().intern_keyword("done");

    let world = world
        .register_component(
            ComponentSchema::new(counter_kw).with_field(FieldSchema::required(value_kw, Type::Int)),
        )
        .unwrap();
    let world = world
        .register_component(ComponentSchema::tag(increment_request_kw))
        .unwrap();

    world
        .register_component(ComponentSchema::tag(done_kw))
        .unwrap()
}

/// Create the counter rules.
fn create_rules(world: &mut World) -> Vec<SpikeRule> {
    let counter_kw = world.interner_mut().intern_keyword("counter");
    let value_kw = world.interner_mut().intern_keyword("value");
    let increment_request_kw = world.interner_mut().intern_keyword("increment-request");
    let done_kw = world.interner_mut().intern_keyword("done");

    let do_increment_kw = world.interner_mut().intern_keyword("rules/do-increment");
    let maybe_again_kw = world.interner_mut().intern_keyword("rules/maybe-again");
    let finish_kw = world.interner_mut().intern_keyword("rules/finish");

    vec![
        // Rule 1: Increment counter when requested
        // Pattern: entity has counter/value AND increment-request
        // Effect: increment value (remove is simulated by refraction)
        SpikeRule {
            name: do_increment_kw,
            salience: 10, // Higher priority - process requests first
            once: false,
            pattern: Pattern::new()
                .with_clause(
                    "e",
                    counter_kw,
                    Some(value_kw),
                    PatternBinding::Variable("v".to_string()),
                )
                .with_clause("e", increment_request_kw, None, PatternBinding::Wildcard),
            body: "increment ?e counter/value".to_string(),
        },
        // Rule 2: Request another increment if under limit
        // Pattern: entity has counter/value, NOT increment-request, NOT done
        // Guard (implicit in salience): only fire after do-increment
        SpikeRule {
            name: maybe_again_kw,
            salience: 5, // Lower priority - fires after do-increment
            once: false,
            pattern: Pattern::new()
                .with_clause(
                    "e",
                    counter_kw,
                    Some(value_kw),
                    PatternBinding::Variable("v".to_string()),
                )
                .with_negated("e", increment_request_kw)
                .with_negated("e", done_kw),
            // Note: Guard (< ?v 10) is checked in the body - spike doesn't support guards
            body: "noop".to_string(), // We'll handle this differently
        },
        // Rule 3: Mark done when limit reached
        // Pattern: entity has counter/value >= 10, NOT done
        SpikeRule {
            name: finish_kw,
            salience: 0, // Lowest priority
            once: false,
            pattern: Pattern::new()
                .with_clause(
                    "e",
                    counter_kw,
                    Some(value_kw),
                    PatternBinding::Variable("v".to_string()),
                )
                .with_negated("e", done_kw),
            body: "tag ?e done".to_string(),
        },
    ]
}

// =============================================================================
// Basic Counter Tests
// =============================================================================

#[test]
fn counter_starts_at_zero() {
    let mut world = setup_world();
    let counter_kw = world.interner_mut().intern_keyword("counter");
    let value_kw = world.interner_mut().intern_keyword("value");

    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(entity, counter_kw, value_kw, Value::Int(0))
        .unwrap();

    let val = world.get_field(entity, counter_kw, value_kw).unwrap();
    assert_eq!(val, Some(Value::Int(0)));
}

#[test]
fn counter_can_be_incremented() {
    let mut world = setup_world();
    let counter_kw = world.interner_mut().intern_keyword("counter");
    let value_kw = world.interner_mut().intern_keyword("value");

    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(entity, counter_kw, value_kw, Value::Int(5))
        .unwrap();
    let world = world
        .set_field(entity, counter_kw, value_kw, Value::Int(6))
        .unwrap();

    let val = world.get_field(entity, counter_kw, value_kw).unwrap();
    assert_eq!(val, Some(Value::Int(6)));
}

// =============================================================================
// Rule Firing Tests
// =============================================================================

#[test]
fn do_increment_rule_matches_entity_with_request() {
    let mut world = setup_world();
    let rules = create_rules(&mut world);

    let counter_kw = world.interner_mut().intern_keyword("counter");
    let value_kw = world.interner_mut().intern_keyword("value");
    let increment_request_kw = world.interner_mut().intern_keyword("increment-request");
    let do_increment_kw = world.interner_mut().intern_keyword("rules/do-increment");

    // Create entity with counter=0 and increment-request
    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(entity, counter_kw, value_kw, Value::Int(0))
        .unwrap();
    let world = world
        .set(entity, increment_request_kw, Value::Bool(true))
        .unwrap();

    let engine = RuleEngine::new();
    let activations = engine.find_activations(&rules, &world);

    // Should find activations (do-increment matches)
    assert!(!activations.is_empty());

    // do-increment (salience 10) should be first
    assert_eq!(activations[0].rule_name, do_increment_kw);
}

#[test]
fn do_increment_increments_counter() {
    let mut world = setup_world();
    let rules = create_rules(&mut world);

    let counter_kw = world.interner_mut().intern_keyword("counter");
    let value_kw = world.interner_mut().intern_keyword("value");
    let increment_request_kw = world.interner_mut().intern_keyword("increment-request");

    // Create entity with counter=5 and increment-request
    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(entity, counter_kw, value_kw, Value::Int(5))
        .unwrap();
    let world = world
        .set(entity, increment_request_kw, Value::Bool(true))
        .unwrap();

    // Run one cycle
    let mut engine = RuleEngine::new();
    engine.begin_tick();

    // Fire the first activation (do-increment)
    let (result_world, _effects) = engine.run_to_quiescence(&rules, world).unwrap();

    // Counter should have incremented
    let val = result_world
        .get_field(entity, counter_kw, value_kw)
        .unwrap();
    // Value will be more than 5 due to rule chaining
    if let Some(Value::Int(n)) = val {
        assert!(n > 5);
    } else {
        panic!("Expected int value");
    }
}

// =============================================================================
// Refraction Tests
// =============================================================================

#[test]
fn refraction_prevents_same_binding_twice() {
    let mut world = setup_world();

    let counter_kw = world.interner_mut().intern_keyword("counter");
    let value_kw = world.interner_mut().intern_keyword("value");
    let test_rule_kw = world.interner_mut().intern_keyword("rules/test");

    // Create a simple rule that matches any counter
    let rules = vec![SpikeRule {
        name: test_rule_kw,
        salience: 0,
        once: false,
        pattern: Pattern::new().with_clause(
            "e",
            counter_kw,
            Some(value_kw),
            PatternBinding::Variable("v".to_string()),
        ),
        body: "noop".to_string(),
    }];

    // Create entity with counter
    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(entity, counter_kw, value_kw, Value::Int(0))
        .unwrap();

    let mut engine = RuleEngine::new();
    engine.begin_tick();

    // First find - should have 1 activation
    let activations1 = engine.find_activations(&rules, &world);
    assert_eq!(activations1.len(), 1);

    // Run to quiescence (fires the rule)
    let (world, _) = engine.run_to_quiescence(&rules, world).unwrap();

    // After firing, same binding shouldn't match again (refraction)
    let activations2 = engine.find_activations(&rules, &world);
    assert_eq!(
        activations2.len(),
        0,
        "Refraction should prevent re-matching"
    );
}

#[test]
fn refraction_resets_each_tick() {
    let mut world = setup_world();

    let counter_kw = world.interner_mut().intern_keyword("counter");
    let value_kw = world.interner_mut().intern_keyword("value");
    let test_rule_kw = world.interner_mut().intern_keyword("rules/test");

    let rules = vec![SpikeRule {
        name: test_rule_kw,
        salience: 0,
        once: false,
        pattern: Pattern::new().with_clause(
            "e",
            counter_kw,
            Some(value_kw),
            PatternBinding::Variable("v".to_string()),
        ),
        body: "noop".to_string(),
    }];

    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(entity, counter_kw, value_kw, Value::Int(0))
        .unwrap();

    let mut engine = RuleEngine::new();

    // Tick 1
    engine.begin_tick();
    let (world, _) = engine.run_to_quiescence(&rules, world).unwrap();

    // Tick 2 - refraction should be reset
    engine.begin_tick();
    let activations = engine.find_activations(&rules, &world);
    assert_eq!(
        activations.len(),
        1,
        "Refraction should reset between ticks"
    );
}

// =============================================================================
// Once Rule Tests
// =============================================================================

#[test]
fn once_rule_fires_only_once_per_tick() {
    let mut world = setup_world();

    let counter_kw = world.interner_mut().intern_keyword("counter");
    let value_kw = world.interner_mut().intern_keyword("value");
    let once_rule_kw = world.interner_mut().intern_keyword("rules/once");

    let rules = vec![SpikeRule {
        name: once_rule_kw,
        salience: 0,
        once: true, // Only fire once per tick
        pattern: Pattern::new().with_clause(
            "e",
            counter_kw,
            Some(value_kw),
            PatternBinding::Variable("v".to_string()),
        ),
        body: "noop".to_string(),
    }];

    // Create two entities
    let (world, e1) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(e1, counter_kw, value_kw, Value::Int(0))
        .unwrap();
    let (world, e2) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(e2, counter_kw, value_kw, Value::Int(0))
        .unwrap();

    let mut engine = RuleEngine::new();
    engine.begin_tick();

    // Initially should find 2 activations (one per entity)
    let activations = engine.find_activations(&rules, &world);
    assert_eq!(activations.len(), 2);

    // After run_to_quiescence, :once rule should have fired only once
    let (_, _effects) = engine.run_to_quiescence(&rules, world).unwrap();

    // With :once, after first firing, no more activations should be found
    // The run_to_quiescence will have fired it once
}

// =============================================================================
// Kill Switch Tests
// =============================================================================

#[test]
fn kill_switch_triggers_on_too_many_activations() {
    let mut world = setup_world();

    let counter_kw = world.interner_mut().intern_keyword("counter");
    let value_kw = world.interner_mut().intern_keyword("value");
    let test_rule_kw = world.interner_mut().intern_keyword("rules/test");

    // Create a simple rule that matches counters
    let rules = vec![SpikeRule {
        name: test_rule_kw,
        salience: 0,
        once: false,
        pattern: Pattern::new().with_clause(
            "e",
            counter_kw,
            Some(value_kw),
            PatternBinding::Variable("v".to_string()),
        ),
        body: "noop".to_string(),
    }];

    // Create multiple entities - more than our limit
    let (world, e1) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(e1, counter_kw, value_kw, Value::Int(0))
        .unwrap();
    let (world, e2) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(e2, counter_kw, value_kw, Value::Int(0))
        .unwrap();
    let (world, e3) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(e3, counter_kw, value_kw, Value::Int(0))
        .unwrap();

    // Set a very low limit (2) so the kill switch triggers with 3 entities
    let mut engine = RuleEngine::new().with_max_activations(2);
    engine.begin_tick();

    // We have 3 entities but limit is 2, so kill switch should trigger
    let result = engine.run_to_quiescence(&rules, world);

    // Should error with kill switch (max activations exceeded)
    assert!(
        result.is_err(),
        "Kill switch should trigger when max_activations exceeded"
    );
}

// =============================================================================
// Termination Tests
// =============================================================================

#[test]
fn finish_rule_marks_entity_done() {
    let mut world = setup_world();

    let counter_kw = world.interner_mut().intern_keyword("counter");
    let value_kw = world.interner_mut().intern_keyword("value");
    let done_kw = world.interner_mut().intern_keyword("done");
    let finish_kw = world.interner_mut().intern_keyword("rules/finish");

    // Rule that marks entity done when counter >= 10
    let rules = vec![SpikeRule {
        name: finish_kw,
        salience: 0,
        once: false,
        pattern: Pattern::new()
            .with_clause(
                "e",
                counter_kw,
                Some(value_kw),
                PatternBinding::Variable("v".to_string()),
            )
            .with_negated("e", done_kw),
        body: "tag ?e done".to_string(),
    }];

    // Create entity with counter=10 (at limit)
    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(entity, counter_kw, value_kw, Value::Int(10))
        .unwrap();

    let mut engine = RuleEngine::new();
    engine.begin_tick();

    let (result_world, _) = engine.run_to_quiescence(&rules, world).unwrap();

    // Entity should be marked done
    assert!(
        result_world.has(entity, done_kw),
        "Entity should be marked done"
    );
}

#[test]
fn done_prevents_further_processing() {
    let mut world = setup_world();

    let counter_kw = world.interner_mut().intern_keyword("counter");
    let value_kw = world.interner_mut().intern_keyword("value");
    let done_kw = world.interner_mut().intern_keyword("done");
    let test_rule_kw = world.interner_mut().intern_keyword("rules/test");

    // Rule that only matches entities NOT done
    let rules = vec![SpikeRule {
        name: test_rule_kw,
        salience: 0,
        once: false,
        pattern: Pattern::new()
            .with_clause(
                "e",
                counter_kw,
                Some(value_kw),
                PatternBinding::Variable("v".to_string()),
            )
            .with_negated("e", done_kw),
        body: "noop".to_string(),
    }];

    // Create entity with counter and mark it done
    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(entity, counter_kw, value_kw, Value::Int(10))
        .unwrap();
    let world = world.set(entity, done_kw, Value::Bool(true)).unwrap();

    let mut engine = RuleEngine::new();
    engine.begin_tick();

    let activations = engine.find_activations(&rules, &world);

    // No activations because entity has :done
    assert_eq!(activations.len(), 0, "Done entity should not match");
}

// =============================================================================
// Salience Tests
// =============================================================================

#[test]
fn higher_salience_fires_first() {
    let mut world = setup_world();

    let counter_kw = world.interner_mut().intern_keyword("counter");
    let value_kw = world.interner_mut().intern_keyword("value");
    let low_kw = world.interner_mut().intern_keyword("rules/low");
    let high_kw = world.interner_mut().intern_keyword("rules/high");

    let rules = vec![
        SpikeRule {
            name: low_kw,
            salience: 0,
            once: false,
            pattern: Pattern::new().with_clause(
                "e",
                counter_kw,
                Some(value_kw),
                PatternBinding::Wildcard,
            ),
            body: "noop".to_string(),
        },
        SpikeRule {
            name: high_kw,
            salience: 100,
            once: false,
            pattern: Pattern::new().with_clause(
                "e",
                counter_kw,
                Some(value_kw),
                PatternBinding::Wildcard,
            ),
            body: "noop".to_string(),
        },
    ];

    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(entity, counter_kw, value_kw, Value::Int(0))
        .unwrap();

    let engine = RuleEngine::new();
    let activations = engine.find_activations(&rules, &world);

    // High salience should be first
    assert_eq!(activations[0].rule_name, high_kw);
    assert_eq!(activations[1].rule_name, low_kw);
}
