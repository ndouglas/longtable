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
}
