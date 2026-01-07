//! Tick cycle integration tests
//!
//! Tests the complete tick execution flow across engine and storage layers.

use longtable_engine::rule::CompiledRule;
use longtable_engine::tick::{InputEvent, TickExecutor};
use longtable_foundation::{LtMap, Value};
use longtable_storage::{ComponentSchema, World};

// =============================================================================
// Basic Tick Cycle
// =============================================================================

#[test]
fn tick_cycle_empty_world() {
    let world = World::new(42);
    let mut executor = TickExecutor::new();

    let result = executor.tick(world, &[]).unwrap();

    assert!(result.is_ok());
    assert_eq!(result.activations_fired, 0);
    assert_eq!(executor.tick_number(), 1);
}

#[test]
fn tick_cycle_with_entities() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");

    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();
    let (world, player) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(player, player_kw, Value::Bool(true)).unwrap();

    let mut executor = TickExecutor::new();
    let result = executor.tick(world, &[]).unwrap();

    assert!(result.is_ok());
    // Player should still exist after tick
    assert!(result.world.has(player, player_kw));
}

// =============================================================================
// Input Injection
// =============================================================================

#[test]
fn tick_injects_set_input() {
    let mut world = World::new(42);
    let health_kw = world.interner_mut().intern_keyword("health");

    let world = world
        .register_component(ComponentSchema::tag(health_kw))
        .unwrap();
    let (world, entity) = world.spawn(&LtMap::new()).unwrap();

    // Entity doesn't have health initially
    assert!(!world.has(entity, health_kw));

    let inputs = vec![InputEvent::Set {
        entity,
        component: health_kw,
        value: Value::Bool(true),
    }];

    let mut executor = TickExecutor::new();
    let result = executor.tick(world, &inputs).unwrap();

    // After tick, entity should have health
    assert!(result.world.has(entity, health_kw));
}

#[test]
fn tick_injects_spawn_input() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");

    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();

    // No entities initially
    assert_eq!(world.with_component(player_kw).count(), 0);

    let inputs = vec![InputEvent::Spawn {
        components: vec![(player_kw, Value::Bool(true))],
    }];

    let mut executor = TickExecutor::new();
    let result = executor.tick(world, &inputs).unwrap();

    // After tick, should have one player
    assert_eq!(result.world.with_component(player_kw).count(), 1);
}

#[test]
fn tick_injects_destroy_input() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");

    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();
    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(entity, player_kw, Value::Bool(true)).unwrap();

    // Entity exists initially
    assert!(world.exists(entity));

    let inputs = vec![InputEvent::Destroy { entity }];

    let mut executor = TickExecutor::new();
    let result = executor.tick(world, &inputs).unwrap();

    // After tick, entity should be destroyed
    assert!(!result.world.exists(entity));
}

// =============================================================================
// Multiple Ticks
// =============================================================================

#[test]
fn tick_number_increments() {
    let world = World::new(42);
    let mut executor = TickExecutor::new();

    assert_eq!(executor.tick_number(), 0);

    executor.tick(world.clone(), &[]).unwrap();
    assert_eq!(executor.tick_number(), 1);

    executor.tick(world.clone(), &[]).unwrap();
    assert_eq!(executor.tick_number(), 2);

    executor.tick(world, &[]).unwrap();
    assert_eq!(executor.tick_number(), 3);
}

#[test]
fn multiple_ticks_accumulate_state() {
    let mut world = World::new(42);
    let counter_kw = world.interner_mut().intern_keyword("tag/counter");

    let world = world
        .register_component(ComponentSchema::tag(counter_kw))
        .unwrap();

    let mut executor = TickExecutor::new();
    let mut current_world = world;

    // Spawn an entity each tick
    for _ in 0..3 {
        let inputs = vec![InputEvent::Spawn {
            components: vec![(counter_kw, Value::Bool(true))],
        }];
        let result = executor.tick(current_world, &inputs).unwrap();
        current_world = result.world;
    }

    // Should have 3 entities
    assert_eq!(current_world.with_component(counter_kw).count(), 3);
}

// =============================================================================
// Rules in Tick Cycle
// =============================================================================

#[test]
fn tick_with_rules_fires_activations() {
    use longtable_engine::pattern::{CompiledBinding, CompiledClause, CompiledPattern};

    let mut world = World::new(42);
    let target_kw = world.interner_mut().intern_keyword("tag/target");
    let rule_kw = world.interner_mut().intern_keyword("rules/test");

    let world = world
        .register_component(ComponentSchema::tag(target_kw))
        .unwrap();
    let (world, e1) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(e1, target_kw, Value::Bool(true)).unwrap();
    let (world, e2) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(e2, target_kw, Value::Bool(true)).unwrap();

    // Create a rule that matches entities with target tag
    let pattern = CompiledPattern {
        clauses: vec![CompiledClause {
            entity_var: "?e".to_string(),
            component: target_kw,
            binding: CompiledBinding::Wildcard,
        }],
        negations: vec![],
    };

    let rule = CompiledRule::new(rule_kw, pattern);
    let executor = TickExecutor::new().with_rules(vec![rule]);
    let mut executor = executor;

    let result = executor.tick(world, &[]).unwrap();

    // Should fire 2 activations (one per entity)
    assert_eq!(result.activations_fired, 2);
}

// =============================================================================
// Provenance Tracking
// =============================================================================

#[test]
fn tick_tracks_input_provenance() {
    let mut world = World::new(42);
    let health_kw = world.interner_mut().intern_keyword("health");

    let world = world
        .register_component(ComponentSchema::tag(health_kw))
        .unwrap();
    let (world, entity) = world.spawn(&LtMap::new()).unwrap();

    let inputs = vec![InputEvent::Set {
        entity,
        component: health_kw,
        value: Value::Bool(true),
    }];

    let mut executor = TickExecutor::new();
    executor.tick(world, &inputs).unwrap();

    // Provenance should be recorded
    let why = executor.provenance().why(entity, health_kw);
    assert!(why.is_some());
}

// =============================================================================
// Custom Events (pass-through)
// =============================================================================

#[test]
fn tick_custom_events_pass_through() {
    let mut world = World::new(42);
    let event_kw = world.interner_mut().intern_keyword("events/test");

    // Custom events don't modify world state at this level
    let inputs = vec![InputEvent::Custom {
        name: event_kw,
        payload: Value::Int(42),
    }];

    let mut executor = TickExecutor::new();
    let result = executor.tick(world, &inputs).unwrap();

    assert!(result.is_ok());
}
