//! World state management with immutable snapshots.
//!
//! The `World` is the unified interface to all storage systems.
//! It uses persistent data structures for O(1) cloning and structural sharing.

use std::sync::Arc;

use longtable_foundation::{EntityId, Interner, KeywordId, LtMap, Result, Value};

use crate::component::ComponentStore;
use crate::entity::EntityStore;
use crate::relationship::RelationshipStore;
use crate::schema::{ComponentSchema, RelationshipSchema};

#[cfg(feature = "serde")]
mod serde_support {
    use super::World;
    use serde::de::{self, MapAccess, Visitor};
    use serde::ser::SerializeStruct;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::fmt;
    use std::sync::Arc;

    impl Serialize for World {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            // Serialize the world state without the history (previous field)
            let mut state = serializer.serialize_struct("World", 6)?;
            state.serialize_field("entities", &*self.entities)?;
            state.serialize_field("components", &*self.components)?;
            state.serialize_field("relationships", &*self.relationships)?;
            state.serialize_field("interner", &*self.interner)?;
            state.serialize_field("tick", &self.tick)?;
            state.serialize_field("seed", &self.seed)?;
            state.end()
        }
    }

    impl<'de> Deserialize<'de> for World {
        fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            #[derive(Deserialize)]
            #[serde(field_identifier, rename_all = "lowercase")]
            enum Field {
                Entities,
                Components,
                Relationships,
                Interner,
                Tick,
                Seed,
            }

            struct WorldVisitor;

            impl<'de> Visitor<'de> for WorldVisitor {
                type Value = World;

                fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str("struct World")
                }

                fn visit_map<V>(self, mut map: V) -> std::result::Result<World, V::Error>
                where
                    V: MapAccess<'de>,
                {
                    let mut entities = None;
                    let mut components = None;
                    let mut relationships = None;
                    let mut interner = None;
                    let mut tick = None;
                    let mut seed = None;

                    while let Some(key) = map.next_key()? {
                        match key {
                            Field::Entities => {
                                if entities.is_some() {
                                    return Err(de::Error::duplicate_field("entities"));
                                }
                                entities = Some(map.next_value()?);
                            }
                            Field::Components => {
                                if components.is_some() {
                                    return Err(de::Error::duplicate_field("components"));
                                }
                                components = Some(map.next_value()?);
                            }
                            Field::Relationships => {
                                if relationships.is_some() {
                                    return Err(de::Error::duplicate_field("relationships"));
                                }
                                relationships = Some(map.next_value()?);
                            }
                            Field::Interner => {
                                if interner.is_some() {
                                    return Err(de::Error::duplicate_field("interner"));
                                }
                                interner = Some(map.next_value()?);
                            }
                            Field::Tick => {
                                if tick.is_some() {
                                    return Err(de::Error::duplicate_field("tick"));
                                }
                                tick = Some(map.next_value()?);
                            }
                            Field::Seed => {
                                if seed.is_some() {
                                    return Err(de::Error::duplicate_field("seed"));
                                }
                                seed = Some(map.next_value()?);
                            }
                        }
                    }

                    let entities = entities.ok_or_else(|| de::Error::missing_field("entities"))?;
                    let components =
                        components.ok_or_else(|| de::Error::missing_field("components"))?;
                    let relationships =
                        relationships.ok_or_else(|| de::Error::missing_field("relationships"))?;
                    let interner = interner.ok_or_else(|| de::Error::missing_field("interner"))?;
                    let tick = tick.ok_or_else(|| de::Error::missing_field("tick"))?;
                    let seed = seed.ok_or_else(|| de::Error::missing_field("seed"))?;

                    Ok(World {
                        entities: Arc::new(entities),
                        components: Arc::new(components),
                        relationships: Arc::new(relationships),
                        interner: Arc::new(interner),
                        tick,
                        seed,
                        previous: None, // History is not serialized
                    })
                }
            }

            const FIELDS: &[&str] = &[
                "entities",
                "components",
                "relationships",
                "interner",
                "tick",
                "seed",
            ];
            deserializer.deserialize_struct("World", FIELDS, WorldVisitor)
        }
    }
}

/// Immutable snapshot of simulation state.
///
/// Clone is O(1) due to structural sharing via `Arc`.
/// All mutation methods return a new `World` instance.
#[derive(Clone, Debug)]
pub struct World {
    /// Entity lifecycle management.
    entities: Arc<EntityStore>,
    /// Component data storage.
    components: Arc<ComponentStore>,
    /// Relationship edges.
    relationships: Arc<RelationshipStore>,
    /// String interner for symbols and keywords.
    interner: Arc<Interner>,
    /// Current tick number.
    tick: u64,
    /// Random seed for determinism.
    seed: u64,
    /// Previous world state (for history/undo).
    previous: Option<Arc<World>>,
}

impl World {
    /// Creates a new empty world with the given seed.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            entities: Arc::new(EntityStore::new()),
            components: Arc::new(ComponentStore::new()),
            relationships: Arc::new(RelationshipStore::new()),
            interner: Arc::new(Interner::new()),
            tick: 0,
            seed,
            previous: None,
        }
    }

    /// Returns the current tick number.
    #[must_use]
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Returns the world seed.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Returns the number of live entities.
    #[must_use]
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// Returns a reference to the previous world state, if any.
    #[must_use]
    pub fn previous(&self) -> Option<&World> {
        self.previous.as_ref().map(Arc::as_ref)
    }

    /// Returns a reference to the interner.
    #[must_use]
    pub fn interner(&self) -> &Interner {
        &self.interner
    }

    /// Returns a mutable reference to the interner.
    ///
    /// This requires cloning the Arc if it's shared.
    pub fn interner_mut(&mut self) -> &mut Interner {
        Arc::make_mut(&mut self.interner)
    }

    // --- Schema Registration ---

    /// Registers a component schema.
    ///
    /// Returns a new World with the schema registered.
    pub fn register_component(&self, schema: ComponentSchema) -> Result<World> {
        let mut new_components = (*self.components).clone();
        new_components.register_schema(schema)?;
        Ok(World {
            components: Arc::new(new_components),
            ..self.clone()
        })
    }

    /// Registers a relationship schema.
    ///
    /// Returns a new World with the schema registered.
    pub fn register_relationship(&self, schema: RelationshipSchema) -> Result<World> {
        let mut new_relationships = (*self.relationships).clone();
        new_relationships.register_schema(schema)?;
        Ok(World {
            relationships: Arc::new(new_relationships),
            ..self.clone()
        })
    }

    /// Gets a component schema by name.
    #[must_use]
    pub fn component_schema(&self, name: KeywordId) -> Option<&ComponentSchema> {
        self.components.schema(name)
    }

    /// Gets a relationship schema by name.
    #[must_use]
    pub fn relationship_schema(&self, name: KeywordId) -> Option<&RelationshipSchema> {
        self.relationships.schema(name)
    }

    // --- Entity Operations ---

    /// Spawns a new entity with optional initial components.
    ///
    /// The components map should have keyword keys (component names)
    /// and map values (component data).
    ///
    /// Returns a new World and the spawned entity ID.
    pub fn spawn(&self, components: &LtMap<Value, Value>) -> Result<(World, EntityId)> {
        let mut new_entities = (*self.entities).clone();
        let id = new_entities.spawn();

        let mut new_components = (*self.components).clone();

        // Set initial components
        for (key, value) in components.iter() {
            if let Value::Keyword(comp_name) = key {
                new_components.set(id, *comp_name, value.clone())?;
            }
        }

        let new_world = World {
            entities: Arc::new(new_entities),
            components: Arc::new(new_components),
            previous: Some(Arc::new(self.clone())),
            ..self.clone()
        };

        Ok((new_world, id))
    }

    /// Destroys an entity and all its components/relationships.
    ///
    /// Returns a new World with the entity removed.
    pub fn destroy(&self, entity: EntityId) -> Result<World> {
        self.entities.validate(entity)?;

        let mut new_entities = (*self.entities).clone();
        let mut new_components = (*self.components).clone();
        let mut new_relationships = (*self.relationships).clone();

        // Remove all relationships
        let cascade_victims = new_relationships.on_entity_destroyed(entity);

        // Remove all components
        new_components.remove_entity(entity);

        // Destroy the entity
        new_entities.destroy(entity)?;

        // Process cascade deletions
        for victim in cascade_victims {
            if new_entities.exists(victim) {
                new_relationships.on_entity_destroyed(victim);
                new_components.remove_entity(victim);
                let _ = new_entities.destroy(victim);
            }
        }

        Ok(World {
            entities: Arc::new(new_entities),
            components: Arc::new(new_components),
            relationships: Arc::new(new_relationships),
            previous: Some(Arc::new(self.clone())),
            ..self.clone()
        })
    }

    /// Checks if an entity exists.
    #[must_use]
    pub fn exists(&self, entity: EntityId) -> bool {
        self.entities.exists(entity)
    }

    /// Iterates all live entity IDs.
    pub fn entities(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.entities.iter()
    }

    // --- Component Operations ---

    /// Gets a component value for an entity.
    pub fn get(&self, entity: EntityId, component: KeywordId) -> Result<Option<Value>> {
        self.entities.validate(entity)?;
        Ok(self.components.get(entity, component).cloned())
    }

    /// Gets a specific field from a component.
    pub fn get_field(
        &self,
        entity: EntityId,
        component: KeywordId,
        field: KeywordId,
    ) -> Result<Option<Value>> {
        self.entities.validate(entity)?;
        Ok(self.components.get_field(entity, component, field).cloned())
    }

    /// Sets a component on an entity.
    ///
    /// Returns a new World with the component set.
    pub fn set(&self, entity: EntityId, component: KeywordId, value: Value) -> Result<World> {
        self.entities.validate(entity)?;

        let mut new_components = (*self.components).clone();
        new_components.set(entity, component, value)?;

        Ok(World {
            components: Arc::new(new_components),
            previous: Some(Arc::new(self.clone())),
            ..self.clone()
        })
    }

    /// Sets a specific field in a component.
    ///
    /// Returns a new World with the field updated.
    pub fn set_field(
        &self,
        entity: EntityId,
        component: KeywordId,
        field: KeywordId,
        value: Value,
    ) -> Result<World> {
        self.entities.validate(entity)?;

        let mut new_components = (*self.components).clone();
        new_components.set_field(entity, component, field, value)?;

        Ok(World {
            components: Arc::new(new_components),
            previous: Some(Arc::new(self.clone())),
            ..self.clone()
        })
    }

    /// Checks if an entity has a component.
    #[must_use]
    pub fn has(&self, entity: EntityId, component: KeywordId) -> bool {
        self.entities.exists(entity) && self.components.has(entity, component)
    }

    /// Iterates entities with a specific component.
    pub fn with_component(&self, component: KeywordId) -> impl Iterator<Item = EntityId> + '_ {
        self.components.with_component(component)
    }

    /// Iterates entities with all specified components.
    pub fn with_components<'a>(
        &'a self,
        components: &'a [KeywordId],
    ) -> impl Iterator<Item = EntityId> + 'a {
        self.components.with_archetype(components)
    }

    // --- Relationship Operations ---

    /// Creates a relationship edge.
    ///
    /// Returns a new World with the relationship added.
    pub fn link(
        &self,
        source: EntityId,
        relationship: KeywordId,
        target: EntityId,
    ) -> Result<World> {
        self.entities.validate(source)?;
        self.entities.validate(target)?;

        let mut new_relationships = (*self.relationships).clone();
        new_relationships.link(source, relationship, target)?;

        Ok(World {
            relationships: Arc::new(new_relationships),
            previous: Some(Arc::new(self.clone())),
            ..self.clone()
        })
    }

    /// Removes a relationship edge.
    ///
    /// Returns a new World with the relationship removed.
    pub fn unlink(
        &self,
        source: EntityId,
        relationship: KeywordId,
        target: EntityId,
    ) -> Result<World> {
        self.entities.validate(source)?;
        self.entities.validate(target)?;

        let mut new_relationships = (*self.relationships).clone();
        new_relationships.unlink(source, relationship, target);

        Ok(World {
            relationships: Arc::new(new_relationships),
            previous: Some(Arc::new(self.clone())),
            ..self.clone()
        })
    }

    /// Gets targets of a relationship from a source.
    pub fn targets(
        &self,
        source: EntityId,
        relationship: KeywordId,
    ) -> impl Iterator<Item = EntityId> + '_ {
        self.relationships.targets(source, relationship)
    }

    /// Gets sources pointing to a target.
    pub fn sources(
        &self,
        target: EntityId,
        relationship: KeywordId,
    ) -> impl Iterator<Item = EntityId> + '_ {
        self.relationships.sources(target, relationship)
    }

    // --- Tick Operations ---

    /// Advances to the next tick.
    ///
    /// Returns a new World with incremented tick and history link.
    #[must_use]
    pub fn advance_tick(&self) -> World {
        World {
            tick: self.tick + 1,
            previous: Some(Arc::new(self.clone())),
            ..self.clone()
        }
    }

    /// Creates a content hash for memoization.
    ///
    /// Two worlds with identical content will have the same hash.
    #[must_use]
    pub fn content_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.tick.hash(&mut hasher);
        self.seed.hash(&mut hasher);
        self.entity_count().hash(&mut hasher);
        // TODO: Add more thorough content hashing
        hasher.finish()
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{ComponentSchema, FieldSchema, RelationshipSchema};
    use longtable_foundation::Type;

    fn setup_world() -> World {
        World::new(42)
    }

    #[test]
    fn new_world_is_empty() {
        let world = setup_world();
        assert_eq!(world.entity_count(), 0);
        assert_eq!(world.tick(), 0);
        assert_eq!(world.seed(), 42);
    }

    #[test]
    fn spawn_creates_entity() {
        let world = setup_world();
        let (world, entity) = world.spawn(&LtMap::new()).unwrap();

        assert_eq!(world.entity_count(), 1);
        assert!(world.exists(entity));
    }

    #[test]
    fn spawn_with_components() {
        let mut world = setup_world();

        // Register schema
        let health = world.interner_mut().intern_keyword("health");
        let current = world.interner_mut().intern_keyword("current");
        let schema =
            ComponentSchema::new(health).with_field(FieldSchema::required(current, Type::Int));
        world = world.register_component(schema).unwrap();

        // Spawn with component
        let mut components = LtMap::new();
        let mut comp_data = LtMap::new();
        comp_data = comp_data.insert(Value::Keyword(current), Value::Int(100));
        components = components.insert(Value::Keyword(health), Value::Map(comp_data));

        let (world, entity) = world.spawn(&components).unwrap();

        assert!(world.has(entity, health));
        let value = world.get_field(entity, health, current).unwrap();
        assert_eq!(value, Some(Value::Int(100)));
    }

    #[test]
    fn destroy_removes_entity() {
        let world = setup_world();
        let (world, entity) = world.spawn(&LtMap::new()).unwrap();
        let world = world.destroy(entity).unwrap();

        assert_eq!(world.entity_count(), 0);
        assert!(!world.exists(entity));
    }

    #[test]
    fn set_and_get_component() {
        let mut world = setup_world();

        let health = world.interner_mut().intern_keyword("health");
        let current = world.interner_mut().intern_keyword("current");
        let schema =
            ComponentSchema::new(health).with_field(FieldSchema::required(current, Type::Int));
        world = world.register_component(schema).unwrap();

        let (world, entity) = world.spawn(&LtMap::new()).unwrap();

        let mut comp_data = LtMap::new();
        comp_data = comp_data.insert(Value::Keyword(current), Value::Int(50));
        let world = world.set(entity, health, Value::Map(comp_data)).unwrap();

        let value = world.get_field(entity, health, current).unwrap();
        assert_eq!(value, Some(Value::Int(50)));
    }

    #[test]
    fn link_and_traverse() {
        let mut world = setup_world();

        let contains = world.interner_mut().intern_keyword("contains");
        world = world
            .register_relationship(RelationshipSchema::new(contains))
            .unwrap();

        let (world, room) = world.spawn(&LtMap::new()).unwrap();
        let (world, item) = world.spawn(&LtMap::new()).unwrap();

        let world = world.link(room, contains, item).unwrap();

        let targets: Vec<_> = world.targets(room, contains).collect();
        assert_eq!(targets, vec![item]);

        let sources: Vec<_> = world.sources(item, contains).collect();
        assert_eq!(sources, vec![room]);
    }

    #[test]
    fn world_clone_is_cheap() {
        let world = setup_world();
        let (world, _) = world.spawn(&LtMap::new()).unwrap();

        // Clone should be O(1) - just Arc clones
        let world2 = world.clone();

        assert_eq!(world.entity_count(), world2.entity_count());
        assert_eq!(world.tick(), world2.tick());
    }

    #[test]
    fn advance_tick_preserves_history() {
        let world = setup_world();
        let (world, entity) = world.spawn(&LtMap::new()).unwrap();
        let world = world.advance_tick();

        assert_eq!(world.tick(), 1);
        assert!(world.previous().is_some());
        assert_eq!(world.previous().unwrap().tick(), 0);

        // Entity should still exist in current world
        assert!(world.exists(entity));
    }

    #[test]
    fn with_component_iteration() {
        let mut world = setup_world();

        let player = world.interner_mut().intern_keyword("player");
        world = world
            .register_component(ComponentSchema::tag(player))
            .unwrap();

        let (world, e1) = world.spawn(&LtMap::new()).unwrap();
        let (world, e2) = world.spawn(&LtMap::new()).unwrap();
        let (world, _e3) = world.spawn(&LtMap::new()).unwrap();

        // Mark e1 and e2 as players
        let world = world.set(e1, player, Value::Bool(true)).unwrap();
        let world = world.set(e2, player, Value::Bool(true)).unwrap();

        let players: Vec<_> = world.with_component(player).collect();
        assert_eq!(players.len(), 2);
        assert!(players.contains(&e1));
        assert!(players.contains(&e2));
    }
}
