//! Integration tests for World state management
//!
//! Tests world immutability, structural sharing, history, and tick management.

use longtable_foundation::LtMap;
use longtable_storage::World;

// =============================================================================
// World Creation
// =============================================================================

#[test]
fn world_starts_empty() {
    let world = World::new(42);
    assert_eq!(world.entity_count(), 0);
    assert_eq!(world.tick(), 0);
}

#[test]
fn world_has_seed() {
    let world = World::new(12345);
    assert_eq!(world.seed(), 12345);
}

#[test]
fn world_has_interner() {
    let world = World::new(42);
    // Should be able to access interner
    let _interner = world.interner();
}

// =============================================================================
// World Immutability
// =============================================================================

#[test]
fn spawn_returns_new_world() {
    let world1 = World::new(42);
    let (world2, _entity) = world1.spawn(&LtMap::new()).unwrap();

    // Original world unchanged
    assert_eq!(world1.entity_count(), 0);
    // New world has the entity
    assert_eq!(world2.entity_count(), 1);
}

#[test]
fn destroy_returns_new_world() {
    let world1 = World::new(42);
    let (world2, entity) = world1.spawn(&LtMap::new()).unwrap();
    let world3 = world2.destroy(entity).unwrap();

    // world2 still has the entity
    assert!(world2.exists(entity));
    // world3 doesn't
    assert!(!world3.exists(entity));
}

#[test]
fn set_component_returns_new_world() {
    let mut world1 = World::new(42);
    let tag_kw = world1.interner_mut().intern_keyword("tag");
    let world1 = world1
        .register_component(longtable_storage::ComponentSchema::tag(tag_kw))
        .unwrap();

    let (world2, entity) = world1.spawn(&LtMap::new()).unwrap();
    let world3 = world2
        .set(entity, tag_kw, longtable_foundation::Value::Bool(true))
        .unwrap();

    // world2 doesn't have the component
    assert!(!world2.has(entity, tag_kw));
    // world3 does
    assert!(world3.has(entity, tag_kw));
}

// =============================================================================
// World Clone (Structural Sharing)
// =============================================================================

#[test]
fn world_clone_is_cheap() {
    let world = World::new(42);

    // Spawn many entities
    let mut w = world;
    for _ in 0..1000 {
        let (new_w, _) = w.spawn(&LtMap::new()).unwrap();
        w = new_w;
    }

    // Clone should be O(1) due to structural sharing
    let w2 = w.clone();
    assert_eq!(w.entity_count(), w2.entity_count());
}

#[test]
fn modified_clone_shares_unchanged_parts() {
    let world = World::new(42);
    let (world, e1) = world.spawn(&LtMap::new()).unwrap();
    let (world, e2) = world.spawn(&LtMap::new()).unwrap();

    // Clone and modify
    let world2 = world.destroy(e2).unwrap();

    // e1 still exists in both
    assert!(world.exists(e1));
    assert!(world2.exists(e1));

    // e2 only exists in original
    assert!(world.exists(e2));
    assert!(!world2.exists(e2));
}

// =============================================================================
// World History
// =============================================================================

#[test]
fn world_tracks_previous() {
    let world1 = World::new(42);
    assert!(world1.previous().is_none());

    let (world2, _) = world1.spawn(&LtMap::new()).unwrap();
    assert!(world2.previous().is_some());

    // Previous should be the original state
    let prev = world2.previous().unwrap();
    assert_eq!(prev.entity_count(), 0);
}

#[test]
fn history_chain() {
    let world1 = World::new(42);
    let (world2, _) = world1.spawn(&LtMap::new()).unwrap();
    let (world3, _) = world2.spawn(&LtMap::new()).unwrap();
    let (world4, _) = world3.spawn(&LtMap::new()).unwrap();

    // Can walk back through history
    let w3 = world4.previous().unwrap();
    assert_eq!(w3.entity_count(), 2);

    let w2 = w3.previous().unwrap();
    assert_eq!(w2.entity_count(), 1);

    let w1 = w2.previous().unwrap();
    assert_eq!(w1.entity_count(), 0);

    assert!(w1.previous().is_none());
}

// =============================================================================
// Tick Management
// =============================================================================

#[test]
fn tick_starts_at_zero() {
    let world = World::new(42);
    assert_eq!(world.tick(), 0);
}

#[test]
fn advance_tick() {
    let world = World::new(42);
    let world = world.advance_tick();
    assert_eq!(world.tick(), 1);

    let world = world.advance_tick();
    assert_eq!(world.tick(), 2);
}

#[test]
fn advance_tick_preserves_state() {
    let world = World::new(42);
    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let world = world.advance_tick();

    // Entity still exists after tick
    assert!(world.exists(entity));
    assert_eq!(world.tick(), 1);
}

// =============================================================================
// Interner Access
// =============================================================================

#[test]
fn interner_mut_works() {
    let mut world = World::new(42);
    let kw = world.interner_mut().intern_keyword("test");

    // Can look up the keyword
    let name = world.interner().get_keyword(kw);
    assert_eq!(name, Some("test"));
}

#[test]
fn interner_shared_across_clones() {
    let mut world1 = World::new(42);
    let kw = world1.interner_mut().intern_keyword("shared");

    let (world2, _) = world1.spawn(&LtMap::new()).unwrap();

    // Both worlds can resolve the keyword
    assert_eq!(world1.interner().get_keyword(kw), Some("shared"));
    assert_eq!(world2.interner().get_keyword(kw), Some("shared"));
}

// =============================================================================
// Reserved Schemas
// =============================================================================

#[test]
fn world_has_reserved_relationship_schemas() {
    use longtable_foundation::KeywordId;

    let world = World::new(42);

    // Reserved schemas should be registered
    assert!(world.component_schema(KeywordId::REL_TYPE).is_some());
    assert!(world.component_schema(KeywordId::REL_SOURCE).is_some());
    assert!(world.component_schema(KeywordId::REL_TARGET).is_some());
}

// =============================================================================
// Entity Iteration
// =============================================================================

#[test]
fn entities_iterator() {
    let world = World::new(42);
    let (world, e1) = world.spawn(&LtMap::new()).unwrap();
    let (world, e2) = world.spawn(&LtMap::new()).unwrap();
    let (world, e3) = world.spawn(&LtMap::new()).unwrap();

    let entities: Vec<_> = world.entities().collect();
    assert_eq!(entities.len(), 3);
    assert!(entities.contains(&e1));
    assert!(entities.contains(&e2));
    assert!(entities.contains(&e3));
}

// =============================================================================
// Complex Scenarios
// =============================================================================

#[test]
fn multiple_mutations_chain_correctly() {
    let mut world = World::new(42);
    let tag_kw = world.interner_mut().intern_keyword("tag");
    let world = world
        .register_component(longtable_storage::ComponentSchema::tag(tag_kw))
        .unwrap();

    // Chain of mutations
    let (world, e1) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set(e1, tag_kw, longtable_foundation::Value::Bool(true))
        .unwrap();
    let (world, e2) = world.spawn(&LtMap::new()).unwrap();
    let world = world.destroy(e1).unwrap();

    // Final state: e2 exists, e1 doesn't
    assert!(!world.exists(e1));
    assert!(world.exists(e2));
    assert_eq!(world.entity_count(), 1);
}
