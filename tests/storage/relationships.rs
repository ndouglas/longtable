//! Integration tests for relationship storage
//!
//! Tests relationship linking, unlinking, traversal, and cardinality.

use longtable_foundation::LtMap;
use longtable_storage::{Cardinality, OnDelete, OnViolation, RelationshipSchema, World};

// =============================================================================
// Relationship Schema Registration
// =============================================================================

#[test]
fn register_relationship_schema() {
    let mut world = World::new(42);
    let owns_kw = world.interner_mut().intern_keyword("owns");

    let schema = RelationshipSchema::new(owns_kw);
    let world = world.register_relationship(schema).unwrap();

    assert!(world.relationship_schema(owns_kw).is_some());
}

#[test]
fn register_relationship_with_cardinality() {
    let mut world = World::new(42);
    let parent_kw = world.interner_mut().intern_keyword("parent-of");

    let schema = RelationshipSchema::new(parent_kw).with_cardinality(Cardinality::OneToMany);
    let world = world.register_relationship(schema).unwrap();

    let retrieved = world.relationship_schema(parent_kw).unwrap();
    assert_eq!(retrieved.cardinality, Cardinality::OneToMany);
}

// =============================================================================
// Basic Linking
// =============================================================================

#[test]
fn link_two_entities() {
    let mut world = World::new(42);
    let owns_kw = world.interner_mut().intern_keyword("owns");

    let world = world
        .register_relationship(RelationshipSchema::new(owns_kw))
        .unwrap();

    let (world, player) = world.spawn(&LtMap::new()).unwrap();
    let (world, sword) = world.spawn(&LtMap::new()).unwrap();

    let world = world.link(player, owns_kw, sword).unwrap();

    // Check relationship exists
    let targets: Vec<_> = world.targets(player, owns_kw).collect();
    assert_eq!(targets.len(), 1);
    assert!(targets.contains(&sword));
}

#[test]
fn link_multiple_targets() {
    let mut world = World::new(42);
    let owns_kw = world.interner_mut().intern_keyword("owns");

    let world = world
        .register_relationship(RelationshipSchema::new(owns_kw))
        .unwrap();

    let (world, player) = world.spawn(&LtMap::new()).unwrap();
    let (world, sword) = world.spawn(&LtMap::new()).unwrap();
    let (world, shield) = world.spawn(&LtMap::new()).unwrap();
    let (world, potion) = world.spawn(&LtMap::new()).unwrap();

    let world = world.link(player, owns_kw, sword).unwrap();
    let world = world.link(player, owns_kw, shield).unwrap();
    let world = world.link(player, owns_kw, potion).unwrap();

    let targets: Vec<_> = world.targets(player, owns_kw).collect();
    assert_eq!(targets.len(), 3);
    assert!(targets.contains(&sword));
    assert!(targets.contains(&shield));
    assert!(targets.contains(&potion));
}

// =============================================================================
// Unlinking
// =============================================================================

#[test]
fn unlink_removes_relationship() {
    let mut world = World::new(42);
    let owns_kw = world.interner_mut().intern_keyword("owns");

    let world = world
        .register_relationship(RelationshipSchema::new(owns_kw))
        .unwrap();

    let (world, player) = world.spawn(&LtMap::new()).unwrap();
    let (world, sword) = world.spawn(&LtMap::new()).unwrap();

    let world = world.link(player, owns_kw, sword).unwrap();
    assert_eq!(world.targets(player, owns_kw).count(), 1);

    let world = world.unlink(player, owns_kw, sword).unwrap();
    assert_eq!(world.targets(player, owns_kw).count(), 0);
}

#[test]
fn unlink_one_of_many() {
    let mut world = World::new(42);
    let owns_kw = world.interner_mut().intern_keyword("owns");

    let world = world
        .register_relationship(RelationshipSchema::new(owns_kw))
        .unwrap();

    let (world, player) = world.spawn(&LtMap::new()).unwrap();
    let (world, sword) = world.spawn(&LtMap::new()).unwrap();
    let (world, shield) = world.spawn(&LtMap::new()).unwrap();

    let world = world.link(player, owns_kw, sword).unwrap();
    let world = world.link(player, owns_kw, shield).unwrap();

    let world = world.unlink(player, owns_kw, sword).unwrap();

    let targets: Vec<_> = world.targets(player, owns_kw).collect();
    assert_eq!(targets.len(), 1);
    assert!(!targets.contains(&sword));
    assert!(targets.contains(&shield));
}

// =============================================================================
// Reverse Traversal
// =============================================================================

#[test]
fn sources_traverses_backwards() {
    let mut world = World::new(42);
    let owns_kw = world.interner_mut().intern_keyword("owns");

    let world = world
        .register_relationship(RelationshipSchema::new(owns_kw))
        .unwrap();

    let (world, player) = world.spawn(&LtMap::new()).unwrap();
    let (world, sword) = world.spawn(&LtMap::new()).unwrap();

    let world = world.link(player, owns_kw, sword).unwrap();

    // Query backwards: who owns the sword?
    let sources: Vec<_> = world.sources(sword, owns_kw).collect();
    assert_eq!(sources.len(), 1);
    assert!(sources.contains(&player));
}

#[test]
fn multiple_sources() {
    let mut world = World::new(42);
    let likes_kw = world.interner_mut().intern_keyword("likes");

    let world = world
        .register_relationship(RelationshipSchema::new(likes_kw))
        .unwrap();

    let (world, alice) = world.spawn(&LtMap::new()).unwrap();
    let (world, bob) = world.spawn(&LtMap::new()).unwrap();
    let (world, cake) = world.spawn(&LtMap::new()).unwrap();

    let world = world.link(alice, likes_kw, cake).unwrap();
    let world = world.link(bob, likes_kw, cake).unwrap();

    let sources: Vec<_> = world.sources(cake, likes_kw).collect();
    assert_eq!(sources.len(), 2);
    assert!(sources.contains(&alice));
    assert!(sources.contains(&bob));
}

// =============================================================================
// Cardinality Enforcement
// =============================================================================

#[test]
fn one_to_one_enforced() {
    let mut world = World::new(42);
    let spouse_kw = world.interner_mut().intern_keyword("spouse");

    let schema = RelationshipSchema::new(spouse_kw)
        .with_cardinality(Cardinality::OneToOne)
        .with_on_violation(OnViolation::Error);
    let world = world.register_relationship(schema).unwrap();

    let (world, alice) = world.spawn(&LtMap::new()).unwrap();
    let (world, bob) = world.spawn(&LtMap::new()).unwrap();
    let (world, charlie) = world.spawn(&LtMap::new()).unwrap();

    // First link succeeds
    let world = world.link(alice, spouse_kw, bob).unwrap();

    // Second link from alice should fail (alice already has spouse)
    let result = world.link(alice, spouse_kw, charlie);
    assert!(result.is_err());
}

#[test]
fn many_to_one_allows_multiple_sources() {
    let mut world = World::new(42);
    let located_in_kw = world.interner_mut().intern_keyword("located-in");

    let schema = RelationshipSchema::new(located_in_kw).with_cardinality(Cardinality::ManyToOne);
    let world = world.register_relationship(schema).unwrap();

    let (world, room) = world.spawn(&LtMap::new()).unwrap();
    let (world, player) = world.spawn(&LtMap::new()).unwrap();
    let (world, enemy) = world.spawn(&LtMap::new()).unwrap();

    // Both player and enemy can be in the same room
    let world = world.link(player, located_in_kw, room).unwrap();
    let world = world.link(enemy, located_in_kw, room).unwrap();

    let sources: Vec<_> = world.sources(room, located_in_kw).collect();
    assert_eq!(sources.len(), 2);
}

#[test]
fn many_to_one_restricts_targets() {
    let mut world = World::new(42);
    let located_in_kw = world.interner_mut().intern_keyword("located-in");

    let schema = RelationshipSchema::new(located_in_kw)
        .with_cardinality(Cardinality::ManyToOne)
        .with_on_violation(OnViolation::Error);
    let world = world.register_relationship(schema).unwrap();

    let (world, room1) = world.spawn(&LtMap::new()).unwrap();
    let (world, room2) = world.spawn(&LtMap::new()).unwrap();
    let (world, player) = world.spawn(&LtMap::new()).unwrap();

    // Player in room1
    let world = world.link(player, located_in_kw, room1).unwrap();

    // Player can't also be in room2 (with Error policy)
    let result = world.link(player, located_in_kw, room2);
    assert!(result.is_err());
}

#[test]
fn replace_on_violation() {
    let mut world = World::new(42);
    let located_in_kw = world.interner_mut().intern_keyword("located-in");

    let schema = RelationshipSchema::new(located_in_kw)
        .with_cardinality(Cardinality::ManyToOne)
        .with_on_violation(OnViolation::Replace);
    let world = world.register_relationship(schema).unwrap();

    let (world, room1) = world.spawn(&LtMap::new()).unwrap();
    let (world, room2) = world.spawn(&LtMap::new()).unwrap();
    let (world, player) = world.spawn(&LtMap::new()).unwrap();

    // Player in room1
    let world = world.link(player, located_in_kw, room1).unwrap();
    assert!(world.targets(player, located_in_kw).any(|t| t == room1));

    // Move player to room2 (Replace policy)
    let world = world.link(player, located_in_kw, room2).unwrap();

    let targets: Vec<_> = world.targets(player, located_in_kw).collect();
    assert_eq!(targets.len(), 1);
    assert!(targets.contains(&room2));
    assert!(!targets.contains(&room1)); // old relationship removed
}

// =============================================================================
// Cascade Delete
// =============================================================================

#[test]
fn cascade_delete_removes_related() {
    let mut world = World::new(42);
    let in_room_kw = world.interner_mut().intern_keyword("in-room");

    let schema = RelationshipSchema::new(in_room_kw).with_on_delete(OnDelete::Cascade);
    let world = world.register_relationship(schema).unwrap();

    let (world, room) = world.spawn(&LtMap::new()).unwrap();
    let (world, item1) = world.spawn(&LtMap::new()).unwrap();
    let (world, item2) = world.spawn(&LtMap::new()).unwrap();

    let world = world.link(item1, in_room_kw, room).unwrap();
    let world = world.link(item2, in_room_kw, room).unwrap();

    // Destroy the room - items should cascade delete
    let world = world.destroy(room).unwrap();

    assert!(!world.exists(room));
    assert!(!world.exists(item1));
    assert!(!world.exists(item2));
}

#[test]
fn remove_on_delete_keeps_related() {
    let mut world = World::new(42);
    let owns_kw = world.interner_mut().intern_keyword("owns");

    let schema = RelationshipSchema::new(owns_kw).with_on_delete(OnDelete::Remove);
    let world = world.register_relationship(schema).unwrap();

    let (world, player) = world.spawn(&LtMap::new()).unwrap();
    let (world, sword) = world.spawn(&LtMap::new()).unwrap();

    let world = world.link(player, owns_kw, sword).unwrap();

    // Destroy the sword - player should still exist
    let world = world.destroy(sword).unwrap();

    assert!(world.exists(player));
    assert!(!world.exists(sword));
    assert_eq!(world.targets(player, owns_kw).count(), 0);
}

// =============================================================================
// Link/Unlink on Nonexistent Entities
// =============================================================================

#[test]
fn link_nonexistent_source_fails() {
    let mut world = World::new(42);
    let owns_kw = world.interner_mut().intern_keyword("owns");

    let world = world
        .register_relationship(RelationshipSchema::new(owns_kw))
        .unwrap();

    let (world, source) = world.spawn(&LtMap::new()).unwrap();
    let (world, target) = world.spawn(&LtMap::new()).unwrap();
    let world = world.destroy(source).unwrap();

    let result = world.link(source, owns_kw, target);
    assert!(result.is_err());
}

#[test]
fn link_nonexistent_target_fails() {
    let mut world = World::new(42);
    let owns_kw = world.interner_mut().intern_keyword("owns");

    let world = world
        .register_relationship(RelationshipSchema::new(owns_kw))
        .unwrap();

    let (world, source) = world.spawn(&LtMap::new()).unwrap();
    let (world, target) = world.spawn(&LtMap::new()).unwrap();
    let world = world.destroy(target).unwrap();

    let result = world.link(source, owns_kw, target);
    assert!(result.is_err());
}

// =============================================================================
// Idempotent Linking
// =============================================================================

#[test]
fn duplicate_link_is_idempotent() {
    let mut world = World::new(42);
    let owns_kw = world.interner_mut().intern_keyword("owns");

    let world = world
        .register_relationship(RelationshipSchema::new(owns_kw))
        .unwrap();

    let (world, player) = world.spawn(&LtMap::new()).unwrap();
    let (world, sword) = world.spawn(&LtMap::new()).unwrap();

    let world = world.link(player, owns_kw, sword).unwrap();
    let world = world.link(player, owns_kw, sword).unwrap(); // duplicate

    // Should still have just one relationship
    let targets: Vec<_> = world.targets(player, owns_kw).collect();
    assert_eq!(targets.len(), 1);
}
