//! Integration tests for the query system
//!
//! Tests query execution against world state.

use longtable_engine::pattern::{CompiledBinding, CompiledClause, CompiledPattern};
use longtable_engine::query::{CompiledQuery, QueryExecutor};
use longtable_foundation::{LtMap, Value};
use longtable_storage::{ComponentSchema, World};

/// Helper to create a simple query that finds entities with a component
fn simple_query(entity_var: &str, component: longtable_foundation::KeywordId) -> CompiledQuery {
    CompiledQuery {
        pattern: CompiledPattern {
            clauses: vec![CompiledClause {
                entity_var: entity_var.to_string(),
                component,
                binding: CompiledBinding::Wildcard,
            }],
            negations: vec![],
        },
        bindings: vec![],
        aggregates: vec![],
        group_by: vec![],
        guards: vec![],
        order_by: vec![],
        limit: None,
        return_expr: None,
        binding_vars: vec![entity_var.to_string()],
        warnings: vec![],
    }
}

// =============================================================================
// Basic Query Execution
// =============================================================================

#[test]
fn query_finds_matching_entities() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");

    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();
    let (world, _p1) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(_p1, player_kw, Value::Bool(true)).unwrap();
    let (world, _p2) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(_p2, player_kw, Value::Bool(true)).unwrap();

    let query = simple_query("?e", player_kw);
    let results = QueryExecutor::execute(&query, &world).unwrap();

    assert_eq!(results.len(), 2);
}

#[test]
fn query_count() {
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

    let query = simple_query("?e", enemy_kw);
    let count = QueryExecutor::count(&query, &world).unwrap();

    assert_eq!(count, 3);
}

#[test]
fn query_exists() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");
    let enemy_kw = world.interner_mut().intern_keyword("tag/enemy");

    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();
    let world = world
        .register_component(ComponentSchema::tag(enemy_kw))
        .unwrap();
    let (world, player) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(player, player_kw, Value::Bool(true)).unwrap();

    let player_query = simple_query("?e", player_kw);
    let enemy_query = simple_query("?e", enemy_kw);

    assert!(QueryExecutor::exists(&player_query, &world).unwrap());
    assert!(!QueryExecutor::exists(&enemy_query, &world).unwrap());
}

#[test]
fn query_execute_one() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");

    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();
    let (world, player) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(player, player_kw, Value::Bool(true)).unwrap();

    let query = simple_query("?e", player_kw);
    let result = QueryExecutor::execute_one(&query, &world).unwrap();

    assert!(result.is_some());
}

#[test]
fn query_execute_one_empty() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");

    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();

    let query = simple_query("?e", player_kw);
    let result = QueryExecutor::execute_one(&query, &world).unwrap();

    assert!(result.is_none());
}

// =============================================================================
// Query with Limit
// =============================================================================

#[test]
fn query_with_limit() {
    let mut world = World::new(42);
    let enemy_kw = world.interner_mut().intern_keyword("tag/enemy");

    let mut world = world
        .register_component(ComponentSchema::tag(enemy_kw))
        .unwrap();

    for _ in 0..10 {
        let (w, e) = world.spawn(&LtMap::new()).unwrap();
        let w = w.set(e, enemy_kw, Value::Bool(true)).unwrap();
        world = w;
    }

    let mut query = simple_query("?e", enemy_kw);
    query.limit = Some(3);

    let results = QueryExecutor::execute(&query, &world).unwrap();
    assert_eq!(results.len(), 3);
}

// =============================================================================
// Query on Empty World
// =============================================================================

#[test]
fn query_empty_world() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");
    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();

    let query = simple_query("?e", player_kw);

    let results = QueryExecutor::execute(&query, &world).unwrap();
    assert!(results.is_empty());

    let count = QueryExecutor::count(&query, &world).unwrap();
    assert_eq!(count, 0);

    let exists = QueryExecutor::exists(&query, &world).unwrap();
    assert!(!exists);
}
