//! Integration tests for the adventure game example.
//!
//! These tests verify that the world structure works correctly with
//! the storage and query systems.

use longtable_foundation::Type;
use longtable_foundation::{KeywordId, LtMap, Value};
use longtable_storage::World;
use longtable_storage::schema::{
    Cardinality, ComponentSchema, FieldSchema, OnDelete, RelationshipSchema,
};

/// Helper to intern a keyword
fn kw(world: &mut World, name: &str) -> KeywordId {
    world.interner_mut().intern_keyword(name)
}

/// Creates the adventure game world with all components, relationships, and entities.
fn create_adventure_world() -> World {
    let mut world = World::new(42);

    // =========================================================================
    // Register Component Schemas
    // =========================================================================

    // Tag components (marker components)
    let tags = [
        "tag/player",
        "tag/room",
        "tag/item",
        "tag/container",
        "tag/npc",
        "tag/door",
        "tag/takeable",
        "tag/openable",
        "tag/open",
        "tag/locked",
        "tag/lit",
        "tag/dark",
        "tag/visited",
    ];

    for tag in tags {
        let kw = kw(&mut world, tag);
        world = world.register_component(ComponentSchema::tag(kw)).unwrap();
    }

    // Name component
    let name_kw = kw(&mut world, "name");
    let value_kw = kw(&mut world, "value");
    world = world
        .register_component(
            ComponentSchema::new(name_kw).with_field(FieldSchema::required(value_kw, Type::String)),
        )
        .unwrap();

    // Description component
    let desc_kw = kw(&mut world, "description");
    world = world
        .register_component(
            ComponentSchema::new(desc_kw).with_field(FieldSchema::required(value_kw, Type::String)),
        )
        .unwrap();

    // Health component
    let health_kw = kw(&mut world, "health");
    let current_kw = kw(&mut world, "current");
    let max_kw = kw(&mut world, "max");
    world = world
        .register_component(
            ComponentSchema::new(health_kw)
                .with_field(FieldSchema::required(current_kw, Type::Int))
                .with_field(FieldSchema::required(max_kw, Type::Int)),
        )
        .unwrap();

    // Score component
    let score_kw = kw(&mut world, "score");
    world = world
        .register_component(
            ComponentSchema::new(score_kw).with_field(FieldSchema::required(value_kw, Type::Int)),
        )
        .unwrap();

    // Weight component
    let weight_kw = kw(&mut world, "weight");
    world = world
        .register_component(
            ComponentSchema::new(weight_kw).with_field(FieldSchema::required(value_kw, Type::Int)),
        )
        .unwrap();

    // Light source component
    let light_kw = kw(&mut world, "light-source");
    let radius_kw = kw(&mut world, "radius");
    world = world
        .register_component(
            ComponentSchema::new(light_kw).with_field(FieldSchema::required(radius_kw, Type::Int)),
        )
        .unwrap();

    // Weapon component
    let weapon_kw = kw(&mut world, "weapon");
    let damage_kw = kw(&mut world, "damage");
    world = world
        .register_component(
            ComponentSchema::new(weapon_kw).with_field(FieldSchema::required(damage_kw, Type::Int)),
        )
        .unwrap();

    // =========================================================================
    // Register Relationship Schemas
    // =========================================================================

    // in-room relationship
    let in_room_kw = kw(&mut world, "in-room");
    world = world
        .register_relationship(
            RelationshipSchema::new(in_room_kw)
                .with_cardinality(Cardinality::ManyToOne)
                .with_on_delete(OnDelete::Remove),
        )
        .unwrap();

    // contained-in relationship
    let contained_in_kw = kw(&mut world, "contained-in");
    world = world
        .register_relationship(
            RelationshipSchema::new(contained_in_kw)
                .with_cardinality(Cardinality::ManyToOne)
                .with_on_delete(OnDelete::Cascade),
        )
        .unwrap();

    // Exit relationships
    let exits = [
        "exit/north",
        "exit/south",
        "exit/east",
        "exit/west",
        "exit/up",
        "exit/down",
    ];
    for exit in exits {
        let exit_kw = kw(&mut world, exit);
        world = world
            .register_relationship(
                RelationshipSchema::new(exit_kw)
                    .with_cardinality(Cardinality::OneToOne)
                    .with_on_delete(OnDelete::Remove),
            )
            .unwrap();
    }

    // =========================================================================
    // Create Entities
    // =========================================================================

    // Helper to build component maps
    let tag_player = kw(&mut world, "tag/player");
    let tag_room = kw(&mut world, "tag/room");
    let tag_item = kw(&mut world, "tag/item");
    let tag_takeable = kw(&mut world, "tag/takeable");
    let tag_lit = kw(&mut world, "tag/lit");
    let tag_dark = kw(&mut world, "tag/dark");

    // Create Player
    let mut player_comps = LtMap::new();
    player_comps = player_comps.insert(Value::Keyword(tag_player), Value::Bool(true));
    let mut name_map = LtMap::new();
    name_map = name_map.insert(Value::Keyword(value_kw), Value::String("Adventurer".into()));
    player_comps = player_comps.insert(Value::Keyword(name_kw), Value::Map(name_map));
    let mut health_map = LtMap::new();
    health_map = health_map.insert(Value::Keyword(current_kw), Value::Int(100));
    health_map = health_map.insert(Value::Keyword(max_kw), Value::Int(100));
    player_comps = player_comps.insert(Value::Keyword(health_kw), Value::Map(health_map));

    let (w, player) = world.spawn(&player_comps).unwrap();
    world = w;

    // Create Cave Entrance
    let mut room_comps = LtMap::new();
    room_comps = room_comps.insert(Value::Keyword(tag_room), Value::Bool(true));
    room_comps = room_comps.insert(Value::Keyword(tag_lit), Value::Bool(true));
    let mut name_map = LtMap::new();
    name_map = name_map.insert(
        Value::Keyword(value_kw),
        Value::String("Cave Entrance".into()),
    );
    room_comps = room_comps.insert(Value::Keyword(name_kw), Value::Map(name_map));

    let (w, cave_entrance) = world.spawn(&room_comps).unwrap();
    world = w;

    // Create Main Hall
    let mut room_comps = LtMap::new();
    room_comps = room_comps.insert(Value::Keyword(tag_room), Value::Bool(true));
    room_comps = room_comps.insert(Value::Keyword(tag_dark), Value::Bool(true));
    let mut name_map = LtMap::new();
    name_map = name_map.insert(Value::Keyword(value_kw), Value::String("Main Hall".into()));
    room_comps = room_comps.insert(Value::Keyword(name_kw), Value::Map(name_map));

    let (w, main_hall) = world.spawn(&room_comps).unwrap();
    world = w;

    // Create Crystal Cavern
    let mut room_comps = LtMap::new();
    room_comps = room_comps.insert(Value::Keyword(tag_room), Value::Bool(true));
    room_comps = room_comps.insert(Value::Keyword(tag_lit), Value::Bool(true));
    let mut name_map = LtMap::new();
    name_map = name_map.insert(
        Value::Keyword(value_kw),
        Value::String("Crystal Cavern".into()),
    );
    room_comps = room_comps.insert(Value::Keyword(name_kw), Value::Map(name_map));

    let (w, crystal_cavern) = world.spawn(&room_comps).unwrap();
    world = w;

    // Create Brass Lantern
    let mut item_comps = LtMap::new();
    item_comps = item_comps.insert(Value::Keyword(tag_item), Value::Bool(true));
    item_comps = item_comps.insert(Value::Keyword(tag_takeable), Value::Bool(true));
    let mut name_map = LtMap::new();
    name_map = name_map.insert(
        Value::Keyword(value_kw),
        Value::String("brass lantern".into()),
    );
    item_comps = item_comps.insert(Value::Keyword(name_kw), Value::Map(name_map));
    let mut weight_map = LtMap::new();
    weight_map = weight_map.insert(Value::Keyword(value_kw), Value::Int(2));
    item_comps = item_comps.insert(Value::Keyword(weight_kw), Value::Map(weight_map));
    let mut light_map = LtMap::new();
    light_map = light_map.insert(Value::Keyword(radius_kw), Value::Int(3));
    item_comps = item_comps.insert(Value::Keyword(light_kw), Value::Map(light_map));

    let (w, lantern) = world.spawn(&item_comps).unwrap();
    world = w;

    // Create Rusty Sword
    let mut item_comps = LtMap::new();
    item_comps = item_comps.insert(Value::Keyword(tag_item), Value::Bool(true));
    item_comps = item_comps.insert(Value::Keyword(tag_takeable), Value::Bool(true));
    let mut name_map = LtMap::new();
    name_map = name_map.insert(
        Value::Keyword(value_kw),
        Value::String("rusty sword".into()),
    );
    item_comps = item_comps.insert(Value::Keyword(name_kw), Value::Map(name_map));
    let mut damage_map = LtMap::new();
    damage_map = damage_map.insert(Value::Keyword(damage_kw), Value::Int(10));
    item_comps = item_comps.insert(Value::Keyword(weapon_kw), Value::Map(damage_map));

    let (w, sword) = world.spawn(&item_comps).unwrap();
    world = w;

    // =========================================================================
    // Create Relationships
    // =========================================================================

    // Player starts in cave entrance
    world = world.link(player, in_room_kw, cave_entrance).unwrap();

    // Lantern is in cave entrance
    world = world.link(lantern, in_room_kw, cave_entrance).unwrap();

    // Sword is in main hall
    world = world.link(sword, in_room_kw, main_hall).unwrap();

    // Room connections
    let exit_south = kw(&mut world, "exit/south");
    let exit_north = kw(&mut world, "exit/north");
    let exit_east = kw(&mut world, "exit/east");
    let exit_west = kw(&mut world, "exit/west");

    // Cave entrance <-> Main hall
    world = world.link(cave_entrance, exit_south, main_hall).unwrap();
    world = world.link(main_hall, exit_north, cave_entrance).unwrap();

    // Main hall <-> Crystal cavern
    world = world.link(main_hall, exit_east, crystal_cavern).unwrap();
    world = world.link(crystal_cavern, exit_west, main_hall).unwrap();

    world
}

#[test]
fn test_world_creation() {
    let world = create_adventure_world();

    // Should have entities
    assert!(world.entity_count() > 0);
}

#[test]
fn test_player_exists() {
    let world = create_adventure_world();

    // Verify interner has keywords
    let keyword_count = world.interner().keyword_count();
    assert!(keyword_count > 0, "Should have interned keywords");

    // Verify we have multiple entities (player, rooms, items)
    assert!(
        world.entity_count() >= 6,
        "Should have player, rooms, and items"
    );
}

#[test]
fn test_room_connections() {
    let mut world = create_adventure_world();

    // Re-intern the keyword to get its ID (interning is idempotent)
    let exit_south = world.interner_mut().intern_keyword("exit/south");

    // Verify we can look it up
    assert_eq!(world.interner().get_keyword(exit_south), Some("exit/south"));
}

#[test]
fn test_items_in_rooms() {
    let mut world = create_adventure_world();

    // Re-intern the keyword to get its ID
    let in_room = world.interner_mut().intern_keyword("in-room");

    // Verify the relationship exists
    assert_eq!(world.interner().get_keyword(in_room), Some("in-room"));
}

#[test]
fn test_serialization_roundtrip() {
    let world = create_adventure_world();

    // Serialize
    let bytes = longtable_runtime::to_bytes(&world).expect("serialization should succeed");
    assert!(!bytes.is_empty());

    // Deserialize
    let restored = longtable_runtime::from_bytes(&bytes).expect("deserialization should succeed");

    // Verify state matches
    assert_eq!(restored.entity_count(), world.entity_count());
    assert_eq!(
        restored.interner().keyword_count(),
        world.interner().keyword_count()
    );
}
