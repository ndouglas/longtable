//! Integration tests for component storage
//!
//! Tests component get/set, field access, schema validation, and archetype iteration.

use longtable_foundation::{LtMap, Type, Value};
use longtable_storage::{ComponentSchema, FieldSchema, World};

// =============================================================================
// Component Schema Registration
// =============================================================================

#[test]
fn register_component_schema() {
    let mut world = World::new(42);
    let health_kw = world.interner_mut().intern_keyword("health");

    let schema = ComponentSchema::tag(health_kw);
    let world = world.register_component(schema).unwrap();

    assert!(world.component_schema(health_kw).is_some());
}

#[test]
fn register_component_with_fields() {
    let mut world = World::new(42);
    let position_kw = world.interner_mut().intern_keyword("position");
    let x_kw = world.interner_mut().intern_keyword("x");
    let y_kw = world.interner_mut().intern_keyword("y");

    let schema = ComponentSchema::new(position_kw)
        .with_field(FieldSchema::required(x_kw, Type::Float))
        .with_field(FieldSchema::required(y_kw, Type::Float));

    let world = world.register_component(schema).unwrap();

    let retrieved = world.component_schema(position_kw).unwrap();
    assert_eq!(retrieved.fields.len(), 2);
}

// =============================================================================
// Tag Components (No Fields)
// =============================================================================

#[test]
fn set_tag_component() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");

    // Register tag schema
    let schema = ComponentSchema::tag(player_kw);
    let world = world.register_component(schema).unwrap();

    // Spawn entity and set tag
    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(entity, player_kw, Value::Bool(true)).unwrap();

    assert!(world.has(entity, player_kw));
}

#[test]
fn has_tag_component() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");
    let enemy_kw = world.interner_mut().intern_keyword("tag/enemy");

    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();
    let world = world
        .register_component(ComponentSchema::tag(enemy_kw))
        .unwrap();

    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(entity, player_kw, Value::Bool(true)).unwrap();

    assert!(world.has(entity, player_kw));
    assert!(!world.has(entity, enemy_kw));
}

// =============================================================================
// Structured Components (With Fields)
// =============================================================================

#[test]
fn set_structured_component() {
    let mut world = World::new(42);
    let health_kw = world.interner_mut().intern_keyword("health");
    let current_kw = world.interner_mut().intern_keyword("current");
    let max_kw = world.interner_mut().intern_keyword("max");

    let schema = ComponentSchema::new(health_kw)
        .with_field(FieldSchema::required(current_kw, Type::Int))
        .with_field(FieldSchema::required(max_kw, Type::Int));
    let world = world.register_component(schema).unwrap();

    let (world, entity) = world.spawn(&LtMap::new()).unwrap();

    let health_data = LtMap::new()
        .insert(Value::Keyword(current_kw), Value::Int(80))
        .insert(Value::Keyword(max_kw), Value::Int(100));

    let world = world
        .set(entity, health_kw, Value::Map(health_data))
        .unwrap();

    let value = world.get(entity, health_kw).unwrap().unwrap();
    if let Value::Map(map) = value {
        assert_eq!(map.get(&Value::Keyword(current_kw)), Some(&Value::Int(80)));
        assert_eq!(map.get(&Value::Keyword(max_kw)), Some(&Value::Int(100)));
    } else {
        panic!("Expected Map value");
    }
}

#[test]
fn get_component_field() {
    let mut world = World::new(42);
    let health_kw = world.interner_mut().intern_keyword("health");
    let current_kw = world.interner_mut().intern_keyword("current");
    let max_kw = world.interner_mut().intern_keyword("max");

    let schema = ComponentSchema::new(health_kw)
        .with_field(FieldSchema::required(current_kw, Type::Int))
        .with_field(FieldSchema::required(max_kw, Type::Int));
    let world = world.register_component(schema).unwrap();

    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let health_data = LtMap::new()
        .insert(Value::Keyword(current_kw), Value::Int(80))
        .insert(Value::Keyword(max_kw), Value::Int(100));
    let world = world
        .set(entity, health_kw, Value::Map(health_data))
        .unwrap();

    let current = world.get_field(entity, health_kw, current_kw).unwrap();
    assert_eq!(current, Some(Value::Int(80)));
}

#[test]
fn set_component_field() {
    let mut world = World::new(42);
    let health_kw = world.interner_mut().intern_keyword("health");
    let current_kw = world.interner_mut().intern_keyword("current");
    let max_kw = world.interner_mut().intern_keyword("max");

    let schema = ComponentSchema::new(health_kw)
        .with_field(FieldSchema::required(current_kw, Type::Int))
        .with_field(FieldSchema::required(max_kw, Type::Int));
    let world = world.register_component(schema).unwrap();

    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let health_data = LtMap::new()
        .insert(Value::Keyword(current_kw), Value::Int(80))
        .insert(Value::Keyword(max_kw), Value::Int(100));
    let world = world
        .set(entity, health_kw, Value::Map(health_data))
        .unwrap();

    // Update just the current field
    let world = world
        .set_field(entity, health_kw, current_kw, Value::Int(60))
        .unwrap();

    let current = world.get_field(entity, health_kw, current_kw).unwrap();
    let max = world.get_field(entity, health_kw, max_kw).unwrap();
    assert_eq!(current, Some(Value::Int(60)));
    assert_eq!(max, Some(Value::Int(100))); // unchanged
}

// =============================================================================
// Component Queries
// =============================================================================

#[test]
fn with_component_filters_entities() {
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

    let (world, enemy1) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(enemy1, enemy_kw, Value::Bool(true)).unwrap();

    let (world, enemy2) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(enemy2, enemy_kw, Value::Bool(true)).unwrap();

    // Query for players
    let players: Vec<_> = world.with_component(player_kw).collect();
    assert_eq!(players.len(), 1);
    assert!(players.contains(&player));

    // Query for enemies
    let enemies: Vec<_> = world.with_component(enemy_kw).collect();
    assert_eq!(enemies.len(), 2);
    assert!(enemies.contains(&enemy1));
    assert!(enemies.contains(&enemy2));
}

#[test]
fn with_components_requires_all() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");
    let health_kw = world.interner_mut().intern_keyword("health");

    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();
    let world = world
        .register_component(ComponentSchema::tag(health_kw))
        .unwrap();

    // Entity with just player tag
    let (world, e1) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(e1, player_kw, Value::Bool(true)).unwrap();

    // Entity with both player and health
    let (world, e2) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(e2, player_kw, Value::Bool(true)).unwrap();
    let world = world.set(e2, health_kw, Value::Bool(true)).unwrap();

    // Entity with just health
    let (world, e3) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(e3, health_kw, Value::Bool(true)).unwrap();

    // Query for entities with both player AND health
    let both: Vec<_> = world.with_components(&[player_kw, health_kw]).collect();
    assert_eq!(both.len(), 1);
    assert!(both.contains(&e2));
}

// =============================================================================
// Component on Nonexistent Entity
// =============================================================================

#[test]
fn get_on_nonexistent_entity_fails() {
    let mut world = World::new(42);
    let health_kw = world.interner_mut().intern_keyword("health");

    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let world = world.destroy(entity).unwrap();

    // Should fail - entity doesn't exist
    let result = world.get(entity, health_kw);
    assert!(result.is_err());
}

#[test]
fn set_on_nonexistent_entity_fails() {
    let mut world = World::new(42);
    let health_kw = world.interner_mut().intern_keyword("health");
    let world = world
        .register_component(ComponentSchema::tag(health_kw))
        .unwrap();

    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let world = world.destroy(entity).unwrap();

    // Should fail - entity doesn't exist
    let result = world.set(entity, health_kw, Value::Bool(true));
    assert!(result.is_err());
}

// =============================================================================
// Entity Components List
// =============================================================================

#[test]
fn entity_components_returns_component_list() {
    let mut world = World::new(42);
    let player_kw = world.interner_mut().intern_keyword("tag/player");
    let health_kw = world.interner_mut().intern_keyword("health");

    let world = world
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();
    let world = world
        .register_component(ComponentSchema::tag(health_kw))
        .unwrap();

    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(entity, player_kw, Value::Bool(true)).unwrap();
    let world = world.set(entity, health_kw, Value::Bool(true)).unwrap();

    let components = world.entity_components(entity);
    assert_eq!(components.len(), 2);
    assert!(components.contains(&player_kw));
    assert!(components.contains(&health_kw));
}
