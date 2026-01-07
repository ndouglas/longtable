//! World serialization and deserialization using `MessagePack`.
//!
//! This module provides functions for saving and loading world state
//! to/from files using the `MessagePack` binary format.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

use longtable_foundation::{Error, ErrorKind, Result};
use longtable_storage::World;

/// Serializes a world to bytes using `MessagePack` format.
///
/// Uses named serialization to preserve struct field names.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn to_bytes(world: &World) -> Result<Vec<u8>> {
    rmp_serde::to_vec_named(world)
        .map_err(|e| Error::new(ErrorKind::SerializationError(e.to_string())))
}

/// Deserializes a world from `MessagePack` bytes.
///
/// # Errors
///
/// Returns an error if deserialization fails.
pub fn from_bytes(bytes: &[u8]) -> Result<World> {
    rmp_serde::from_slice(bytes)
        .map_err(|e| Error::new(ErrorKind::SerializationError(e.to_string())))
}

/// Saves a world to a file using `MessagePack` format.
///
/// Creates the file if it doesn't exist, or overwrites it if it does.
///
/// # Errors
///
/// Returns an error if the file cannot be created or written to,
/// or if serialization fails.
pub fn save_to_file<P: AsRef<Path>>(world: &World, path: P) -> Result<()> {
    let file = File::create(path.as_ref()).map_err(|e| {
        Error::new(ErrorKind::IoError(format!(
            "failed to create file '{}': {e}",
            path.as_ref().display()
        )))
    })?;

    let mut writer = BufWriter::new(file);
    let bytes = to_bytes(world)?;

    writer.write_all(&bytes).map_err(|e| {
        Error::new(ErrorKind::IoError(format!(
            "failed to write to file '{}': {e}",
            path.as_ref().display()
        )))
    })?;

    writer.flush().map_err(|e| {
        Error::new(ErrorKind::IoError(format!(
            "failed to flush file '{}': {e}",
            path.as_ref().display()
        )))
    })?;

    Ok(())
}

/// Loads a world from a `MessagePack` file.
///
/// # Errors
///
/// Returns an error if the file cannot be read or if deserialization fails.
pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<World> {
    let file = File::open(path.as_ref()).map_err(|e| {
        Error::new(ErrorKind::IoError(format!(
            "failed to open file '{}': {e}",
            path.as_ref().display()
        )))
    })?;

    let mut reader = BufReader::new(file);
    let mut bytes = Vec::new();

    reader.read_to_end(&mut bytes).map_err(|e| {
        Error::new(ErrorKind::IoError(format!(
            "failed to read file '{}': {e}",
            path.as_ref().display()
        )))
    })?;

    from_bytes(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use longtable_foundation::Type;
    use longtable_foundation::{LtMap, Value};
    use longtable_storage::schema::{ComponentSchema, FieldSchema, RelationshipSchema};

    fn create_test_world() -> World {
        let mut world = World::new(42);

        // Register schemas
        let health = world.interner_mut().intern_keyword("health");
        let current = world.interner_mut().intern_keyword("current");
        let max = world.interner_mut().intern_keyword("max");
        let contains = world.interner_mut().intern_keyword("contains");

        let health_schema = ComponentSchema::new(health)
            .with_field(FieldSchema::required(current, Type::Int))
            .with_field(FieldSchema::optional(max, Type::Int, Value::Int(100)));

        world = world.register_component(health_schema).unwrap();
        world = world
            .register_relationship(RelationshipSchema::new(contains))
            .unwrap();

        // Create entities
        let mut comp = LtMap::new();
        comp = comp.insert(Value::Keyword(current), Value::Int(75));
        comp = comp.insert(Value::Keyword(max), Value::Int(100));

        let mut components = LtMap::new();
        components = components.insert(Value::Keyword(health), Value::Map(comp));

        let (w, player) = world.spawn(&components).unwrap();
        world = w;

        let (w, room) = world.spawn(&LtMap::new()).unwrap();
        world = w;

        // Create a relationship
        world = world.link(room, contains, player).unwrap();

        // Advance tick
        world = world.advance_tick();

        world
    }

    #[test]
    fn roundtrip_bytes() {
        let world = create_test_world();

        // Serialize
        let bytes = to_bytes(&world).expect("serialization failed");
        assert!(!bytes.is_empty());

        // Deserialize
        let restored = from_bytes(&bytes).expect("deserialization failed");

        // Verify state matches
        assert_eq!(restored.tick(), world.tick());
        assert_eq!(restored.seed(), world.seed());
        assert_eq!(restored.entity_count(), world.entity_count());
    }

    #[test]
    fn roundtrip_file() {
        let world = create_test_world();

        // Create temp file path
        let temp_path = std::env::temp_dir().join("longtable_test_world.msgpack");

        // Save to file
        save_to_file(&world, &temp_path).expect("save failed");

        // Load from file
        let restored = load_from_file(&temp_path).expect("load failed");

        // Verify state matches
        assert_eq!(restored.tick(), world.tick());
        assert_eq!(restored.seed(), world.seed());
        assert_eq!(restored.entity_count(), world.entity_count());

        // Clean up
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn entities_preserved() {
        let world = create_test_world();
        let bytes = to_bytes(&world).unwrap();
        let restored = from_bytes(&bytes).unwrap();

        // Verify entities exist
        let entities: Vec<_> = restored.entities().collect();
        assert_eq!(entities.len(), 2);

        // The interner state should be preserved
        assert_eq!(
            restored.interner().keyword_count(),
            world.interner().keyword_count()
        );
        assert_eq!(
            restored.interner().symbol_count(),
            world.interner().symbol_count()
        );
    }

    #[test]
    fn history_not_serialized() {
        let world = create_test_world();

        // The test world was advanced, so it has history
        assert!(world.previous().is_some());

        // Serialize and deserialize
        let bytes = to_bytes(&world).unwrap();
        let restored = from_bytes(&bytes).unwrap();

        // History should not be preserved
        assert!(restored.previous().is_none());
    }

    #[test]
    fn load_nonexistent_file_fails() {
        let result = load_from_file("/nonexistent/path/to/world.msgpack");
        assert!(result.is_err());
    }

    /// Comprehensive version compatibility test.
    ///
    /// This test creates a world with various content types and verifies
    /// that all data is preserved correctly through serialization/deserialization.
    #[test]
    fn version_compatibility() {
        // Create a world with diverse content
        let mut world = World::new(12345);

        // Register multiple component schemas
        let health = world.interner_mut().intern_keyword("health");
        let position = world.interner_mut().intern_keyword("position");
        let name = world.interner_mut().intern_keyword("name");
        let x = world.interner_mut().intern_keyword("x");
        let y = world.interner_mut().intern_keyword("y");
        let contains = world.interner_mut().intern_keyword("contains");
        let parent_of = world.interner_mut().intern_keyword("parent-of");

        // Health: int value
        let health_schema =
            ComponentSchema::new(health).with_field(FieldSchema::required(health, Type::Int));
        world = world.register_component(health_schema).unwrap();

        // Position: map with x, y
        let position_schema = ComponentSchema::new(position)
            .with_field(FieldSchema::required(x, Type::Float))
            .with_field(FieldSchema::optional(y, Type::Float, Value::Float(0.0)));
        world = world.register_component(position_schema).unwrap();

        // Name: string
        let name_schema =
            ComponentSchema::new(name).with_field(FieldSchema::required(name, Type::String));
        world = world.register_component(name_schema).unwrap();

        // Relationships
        world = world
            .register_relationship(RelationshipSchema::new(contains))
            .unwrap();
        world = world
            .register_relationship(RelationshipSchema::new(parent_of))
            .unwrap();

        // Create entities with different component combinations
        // Entity 1: Player with all components
        let mut player_health = LtMap::new();
        player_health = player_health.insert(Value::Keyword(health), Value::Int(100));
        let mut player_pos = LtMap::new();
        player_pos = player_pos.insert(Value::Keyword(x), Value::Float(10.5));
        player_pos = player_pos.insert(Value::Keyword(y), Value::Float(20.5));
        let mut player_name = LtMap::new();
        player_name = player_name.insert(Value::Keyword(name), Value::String("Hero".into()));

        let mut player_comps = LtMap::new();
        player_comps = player_comps.insert(Value::Keyword(health), Value::Map(player_health));
        player_comps = player_comps.insert(Value::Keyword(position), Value::Map(player_pos));
        player_comps = player_comps.insert(Value::Keyword(name), Value::Map(player_name));

        let (w, player) = world.spawn(&player_comps).unwrap();
        world = w;

        // Entity 2: Room (just a name)
        let mut room_name = LtMap::new();
        room_name = room_name.insert(Value::Keyword(name), Value::String("Dungeon".into()));
        let mut room_comps = LtMap::new();
        room_comps = room_comps.insert(Value::Keyword(name), Value::Map(room_name));

        let (w, room) = world.spawn(&room_comps).unwrap();
        world = w;

        // Entity 3: Item (just position)
        let mut item_pos = LtMap::new();
        item_pos = item_pos.insert(Value::Keyword(x), Value::Float(-5.0));
        let mut item_comps = LtMap::new();
        item_comps = item_comps.insert(Value::Keyword(position), Value::Map(item_pos));

        let (w, item) = world.spawn(&item_comps).unwrap();
        world = w;

        // Create relationships
        world = world.link(room, contains, player).unwrap();
        world = world.link(room, contains, item).unwrap();

        // Advance tick to create history
        world = world.advance_tick();
        world = world.advance_tick();

        // Serialize
        let bytes = to_bytes(&world).expect("serialization failed");
        assert!(!bytes.is_empty());

        // Deserialize
        let restored = from_bytes(&bytes).expect("deserialization failed");

        // Verify basic state
        assert_eq!(restored.tick(), world.tick());
        assert_eq!(restored.seed(), world.seed());
        assert_eq!(restored.entity_count(), world.entity_count());

        // Verify interner counts
        assert_eq!(
            restored.interner().keyword_count(),
            world.interner().keyword_count()
        );

        // Verify entities exist
        assert!(restored.exists(player));
        assert!(restored.exists(room));
        assert!(restored.exists(item));

        // Verify component data
        let player_health_val = restored.get(player, health).unwrap();
        assert!(player_health_val.is_some());

        let room_name_val = restored.get(room, name).unwrap();
        assert!(room_name_val.is_some());

        let item_pos_val = restored.get(item, position).unwrap();
        assert!(item_pos_val.is_some());

        // Verify relationships
        let room_contains: Vec<_> = restored.targets(room, contains).collect();
        assert_eq!(room_contains.len(), 2);
        assert!(room_contains.contains(&player));
        assert!(room_contains.contains(&item));

        // Verify history is NOT preserved (by design)
        assert!(restored.previous().is_none());
    }
}
