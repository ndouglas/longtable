//! Integration tests for entity storage
//!
//! Tests entity spawning, destruction, generational indices, and stale reference detection.

use longtable_foundation::LtMap;
use longtable_storage::World;

// =============================================================================
// Entity Spawning
// =============================================================================

#[test]
fn spawn_single_entity() {
    let world = World::new(42);
    let (world, entity) = world.spawn(&LtMap::new()).unwrap();

    assert!(world.exists(entity));
    assert_eq!(world.entity_count(), 1);
}

#[test]
fn spawn_multiple_entities() {
    let world = World::new(42);
    let (world, e1) = world.spawn(&LtMap::new()).unwrap();
    let (world, e2) = world.spawn(&LtMap::new()).unwrap();
    let (world, e3) = world.spawn(&LtMap::new()).unwrap();

    assert!(world.exists(e1));
    assert!(world.exists(e2));
    assert!(world.exists(e3));
    assert_eq!(world.entity_count(), 3);

    // Entities should have different IDs
    assert_ne!(e1, e2);
    assert_ne!(e2, e3);
    assert_ne!(e1, e3);
}

#[test]
fn spawned_entities_have_unique_indices() {
    let world = World::new(42);
    let (world, e1) = world.spawn(&LtMap::new()).unwrap();
    let (_world, e2) = world.spawn(&LtMap::new()).unwrap();

    assert_ne!(e1.index, e2.index);
}

// =============================================================================
// Entity Destruction
// =============================================================================

#[test]
fn destroy_entity() {
    let world = World::new(42);
    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    assert!(world.exists(entity));

    let world = world.destroy(entity).unwrap();
    assert!(!world.exists(entity));
    assert_eq!(world.entity_count(), 0);
}

#[test]
fn destroy_one_of_many() {
    let world = World::new(42);
    let (world, e1) = world.spawn(&LtMap::new()).unwrap();
    let (world, e2) = world.spawn(&LtMap::new()).unwrap();
    let (world, e3) = world.spawn(&LtMap::new()).unwrap();

    let world = world.destroy(e2).unwrap();

    assert!(world.exists(e1));
    assert!(!world.exists(e2));
    assert!(world.exists(e3));
    assert_eq!(world.entity_count(), 2);
}

#[test]
fn destroy_already_destroyed_fails() {
    let world = World::new(42);
    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let world = world.destroy(entity).unwrap();

    // Should fail - already destroyed
    let result = world.destroy(entity);
    assert!(result.is_err());
}

// =============================================================================
// Generational Indices
// =============================================================================

#[test]
fn generations_increment_on_reuse() {
    let world = World::new(42);

    // Spawn and destroy
    let (world, e1) = world.spawn(&LtMap::new()).unwrap();
    let old_gen = e1.generation;
    let world = world.destroy(e1).unwrap();

    // Spawn again - may reuse the same index
    let (world, e2) = world.spawn(&LtMap::new()).unwrap();

    if e2.index == e1.index {
        // If index was reused, generation must be higher
        assert!(e2.generation > old_gen);
    }

    // Either way, e1 should not exist (stale)
    assert!(!world.exists(e1));
    assert!(world.exists(e2));
}

#[test]
fn stale_reference_not_found() {
    let world = World::new(42);
    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let world = world.destroy(entity).unwrap();

    // Old entity ID should not exist
    assert!(!world.exists(entity));
}

// =============================================================================
// Entity Iteration
// =============================================================================

#[test]
fn iterate_all_entities() {
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

#[test]
fn iterate_after_destruction() {
    let world = World::new(42);
    let (world, e1) = world.spawn(&LtMap::new()).unwrap();
    let (world, e2) = world.spawn(&LtMap::new()).unwrap();
    let (world, e3) = world.spawn(&LtMap::new()).unwrap();
    let world = world.destroy(e2).unwrap();

    let entities: Vec<_> = world.entities().collect();
    assert_eq!(entities.len(), 2);
    assert!(entities.contains(&e1));
    assert!(!entities.contains(&e2));
    assert!(entities.contains(&e3));
}

// =============================================================================
// Entity Count
// =============================================================================

#[test]
fn entity_count_starts_at_zero() {
    let world = World::new(42);
    assert_eq!(world.entity_count(), 0);
}

#[test]
fn entity_count_increments() {
    let world = World::new(42);
    assert_eq!(world.entity_count(), 0);

    let (world, _) = world.spawn(&LtMap::new()).unwrap();
    assert_eq!(world.entity_count(), 1);

    let (world, _) = world.spawn(&LtMap::new()).unwrap();
    assert_eq!(world.entity_count(), 2);
}

#[test]
fn entity_count_decrements() {
    let world = World::new(42);
    let (world, e1) = world.spawn(&LtMap::new()).unwrap();
    let (world, _e2) = world.spawn(&LtMap::new()).unwrap();
    assert_eq!(world.entity_count(), 2);

    let world = world.destroy(e1).unwrap();
    assert_eq!(world.entity_count(), 1);
}
