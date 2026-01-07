//! Integration tests for pattern matching
//!
//! Tests pattern compilation and execution against world state.

use longtable_engine::pattern::{
    Bindings, CompiledBinding, CompiledClause, CompiledPattern, PatternMatcher,
};
use longtable_foundation::{LtMap, Value};
use longtable_storage::{ComponentSchema, World};

/// Helper to create a simple pattern that matches entities with a component
fn create_component_pattern(
    entity_var: &str,
    component: longtable_foundation::KeywordId,
) -> CompiledPattern {
    CompiledPattern {
        clauses: vec![CompiledClause {
            entity_var: entity_var.to_string(),
            component,
            binding: CompiledBinding::Wildcard,
        }],
        negations: vec![],
    }
}

// =============================================================================
// Basic Pattern Matching
// =============================================================================

#[test]
fn pattern_matches_entity_with_component() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");

    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();
    let (world, player) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(player, player_kw, Value::Bool(true)).unwrap();

    let pattern = create_component_pattern("?e", player_kw);
    let results = PatternMatcher::match_pattern(&pattern, &world);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].get_entity("?e"), Some(player));
}

#[test]
fn pattern_matches_multiple_entities() {
    let mut world = World::new(42);
    let enemy_kw = world.interner_mut().intern_keyword("tag/enemy");

    let world = world
        .register_component(ComponentSchema::tag(enemy_kw))
        .unwrap();
    let (world, e1) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(e1, enemy_kw, Value::Bool(true)).unwrap();
    let (world, e2) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(e2, enemy_kw, Value::Bool(true)).unwrap();
    let (world, e3) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(e3, enemy_kw, Value::Bool(true)).unwrap();

    let pattern = create_component_pattern("?e", enemy_kw);
    let results = PatternMatcher::match_pattern(&pattern, &world);

    assert_eq!(results.len(), 3);
}

#[test]
fn pattern_no_match_when_component_missing() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");
    let enemy_kw = world.interner_mut().intern_keyword("tag/enemy");

    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();
    let world = world
        .register_component(ComponentSchema::tag(enemy_kw))
        .unwrap();

    // Create entity with player tag only
    let (world, player) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(player, player_kw, Value::Bool(true)).unwrap();

    // Pattern looks for enemy tag
    let pattern = create_component_pattern("?e", enemy_kw);
    let results = PatternMatcher::match_pattern(&pattern, &world);

    assert_eq!(results.len(), 0);
}

// =============================================================================
// Multi-Clause Patterns
// =============================================================================

#[test]
fn pattern_with_multiple_components() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");
    let health_kw = world.interner_mut().intern_keyword("tag/health");

    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();
    let world = world
        .register_component(ComponentSchema::tag(health_kw))
        .unwrap();

    // Entity with both player and health tags
    let (world, e1) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(e1, player_kw, Value::Bool(true)).unwrap();
    let world = world.set(e1, health_kw, Value::Bool(true)).unwrap();

    // Entity with only player
    let (world, e2) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(e2, player_kw, Value::Bool(true)).unwrap();

    // Pattern requires both components
    let pattern = CompiledPattern {
        clauses: vec![
            CompiledClause {
                entity_var: "?e".to_string(),
                component: player_kw,
                binding: CompiledBinding::Wildcard,
            },
            CompiledClause {
                entity_var: "?e".to_string(),
                component: health_kw,
                binding: CompiledBinding::Wildcard,
            },
        ],
        negations: vec![],
    };

    let results = PatternMatcher::match_pattern(&pattern, &world);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].get_entity("?e"), Some(e1));
}

// =============================================================================
// Value Binding
// =============================================================================

#[test]
fn pattern_binds_value() {
    let mut world = World::new(42);
    let tag_kw = world.interner_mut().intern_keyword("active");

    let world = world
        .register_component(ComponentSchema::tag(tag_kw))
        .unwrap();
    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(entity, tag_kw, Value::Bool(true)).unwrap();

    let pattern = CompiledPattern {
        clauses: vec![CompiledClause {
            entity_var: "?e".to_string(),
            component: tag_kw,
            binding: CompiledBinding::Variable("?active".to_string()),
        }],
        negations: vec![],
    };

    let results = PatternMatcher::match_pattern(&pattern, &world);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].get("?active"), Some(&Value::Bool(true)));
}

#[test]
fn pattern_literal_matching() {
    let mut world = World::new(42);
    let active_kw = world.interner_mut().intern_keyword("active");
    let inactive_kw = world.interner_mut().intern_keyword("inactive");

    let world = world
        .register_component(ComponentSchema::tag(active_kw))
        .unwrap();
    let world = world
        .register_component(ComponentSchema::tag(inactive_kw))
        .unwrap();

    let (world, e1) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(e1, active_kw, Value::Bool(true)).unwrap();

    let (world, e2) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(e2, inactive_kw, Value::Bool(true)).unwrap();

    // Pattern matching literal value true on active component
    let pattern = CompiledPattern {
        clauses: vec![CompiledClause {
            entity_var: "?e".to_string(),
            component: active_kw,
            binding: CompiledBinding::Literal(Value::Bool(true)),
        }],
        negations: vec![],
    };

    let results = PatternMatcher::match_pattern(&pattern, &world);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].get_entity("?e"), Some(e1));
}

// =============================================================================
// Negation Patterns
// =============================================================================

#[test]
fn pattern_negation() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");
    let dead_kw = world.interner_mut().intern_keyword("tag/dead");

    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();
    let world = world
        .register_component(ComponentSchema::tag(dead_kw))
        .unwrap();

    // Living player
    let (world, alive) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(alive, player_kw, Value::Bool(true)).unwrap();

    // Dead player
    let (world, dead) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(dead, player_kw, Value::Bool(true)).unwrap();
    let world = world.set(dead, dead_kw, Value::Bool(true)).unwrap();

    // Pattern: player but NOT dead
    let pattern = CompiledPattern {
        clauses: vec![CompiledClause {
            entity_var: "?e".to_string(),
            component: player_kw,
            binding: CompiledBinding::Wildcard,
        }],
        negations: vec![CompiledClause {
            entity_var: "?e".to_string(),
            component: dead_kw,
            binding: CompiledBinding::Wildcard,
        }],
    };

    let results = PatternMatcher::match_pattern(&pattern, &world);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].get_entity("?e"), Some(alive));
}

// =============================================================================
// Bindings API
// =============================================================================

#[test]
fn bindings_iteration() {
    let mut bindings = Bindings::new();
    bindings.set("?x".to_string(), Value::Int(1));
    bindings.set("?y".to_string(), Value::Int(2));

    let entries: Vec<_> = bindings.iter().collect();
    assert_eq!(entries.len(), 2);
}

#[test]
fn bindings_refraction_key() {
    use longtable_foundation::EntityId;

    let mut b1 = Bindings::new();
    b1.set("?e".to_string(), Value::EntityRef(EntityId::new(1, 0)));

    let mut b2 = Bindings::new();
    b2.set("?e".to_string(), Value::EntityRef(EntityId::new(1, 0)));

    let mut b3 = Bindings::new();
    b3.set("?e".to_string(), Value::EntityRef(EntityId::new(2, 0)));

    // Same entity bindings = same refraction key
    assert_eq!(b1.refraction_key(), b2.refraction_key());
    // Different entity bindings = different refraction key
    assert_ne!(b1.refraction_key(), b3.refraction_key());
}

// =============================================================================
// Empty World
// =============================================================================

#[test]
fn pattern_on_empty_world() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");
    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();

    let pattern = create_component_pattern("?e", player_kw);
    let results = PatternMatcher::match_pattern(&pattern, &world);

    assert_eq!(results.len(), 0);
}
