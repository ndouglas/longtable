//! World state management with immutable snapshots.
//!
//! The `World` is the unified interface to all storage systems.
//! It uses persistent data structures for O(1) cloning and structural sharing.

use std::sync::Arc;

use longtable_foundation::{EntityId, Error, ErrorKind, Interner, KeywordId, LtMap, Result, Value};

use crate::component::ComponentStore;
use crate::entity::EntityStore;
use crate::relationship::RelationshipStore;
use crate::schema::{ComponentSchema, OnDelete, RelationshipSchema};

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
    ///
    /// The world is pre-configured with reserved component schemas for
    /// relationship entities (`:rel/type`, `:rel/source`, `:rel/target`).
    ///
    /// # Panics
    ///
    /// Panics if reserved component schemas fail to register (should never
    /// happen unless there's an internal bug).
    #[must_use]
    pub fn new(seed: u64) -> Self {
        use crate::schema::{ComponentSchema, FieldSchema};
        use longtable_foundation::Type;

        let mut components = ComponentStore::new();

        // Register reserved relationship component schemas
        // :rel/type - stores the relationship keyword (e.g., :exit/north)
        let rel_type_schema = ComponentSchema::new(KeywordId::REL_TYPE)
            .with_field(FieldSchema::required(KeywordId::VALUE, Type::Keyword));
        components
            .register_schema(rel_type_schema)
            .expect("failed to register :rel/type schema");

        // :rel/source - stores the source entity
        let rel_source_schema = ComponentSchema::new(KeywordId::REL_SOURCE)
            .with_field(FieldSchema::required(KeywordId::VALUE, Type::EntityRef));
        components
            .register_schema(rel_source_schema)
            .expect("failed to register :rel/source schema");

        // :rel/target - stores the target entity
        let rel_target_schema = ComponentSchema::new(KeywordId::REL_TARGET)
            .with_field(FieldSchema::required(KeywordId::VALUE, Type::EntityRef));
        components
            .register_schema(rel_target_schema)
            .expect("failed to register :rel/target schema");

        Self {
            entities: Arc::new(EntityStore::new()),
            components: Arc::new(components),
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

    /// Replaces the interner with a new one.
    ///
    /// Used when the compiler's interner needs to be synchronized back.
    pub fn set_interner(&mut self, interner: Interner) {
        self.interner = Arc::new(interner);
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

    /// Spawns a relationship entity linking source to target.
    ///
    /// This creates a new entity with `:rel/type`, `:rel/source`, and `:rel/target`
    /// components set appropriately.
    ///
    /// Returns a new World and the relationship entity ID.
    ///
    /// # Errors
    ///
    /// Returns an error if source or target entities don't exist.
    pub fn spawn_relationship(
        &self,
        rel_type: KeywordId,
        source: EntityId,
        target: EntityId,
    ) -> Result<(World, EntityId)> {
        // Validate source and target exist
        self.entities.validate(source)?;
        self.entities.validate(target)?;

        // Spawn the relationship entity
        let mut new_entities = (*self.entities).clone();
        let rel_entity = new_entities.spawn();

        let mut new_components = (*self.components).clone();

        // Set :rel/type
        let type_value =
            LtMap::new().insert(Value::Keyword(KeywordId::VALUE), Value::Keyword(rel_type));
        new_components.set(rel_entity, KeywordId::REL_TYPE, Value::Map(type_value))?;

        // Set :rel/source
        let source_value =
            LtMap::new().insert(Value::Keyword(KeywordId::VALUE), Value::EntityRef(source));
        new_components.set(rel_entity, KeywordId::REL_SOURCE, Value::Map(source_value))?;

        // Set :rel/target
        let target_value =
            LtMap::new().insert(Value::Keyword(KeywordId::VALUE), Value::EntityRef(target));
        new_components.set(rel_entity, KeywordId::REL_TARGET, Value::Map(target_value))?;

        let new_world = World {
            entities: Arc::new(new_entities),
            components: Arc::new(new_components),
            previous: Some(Arc::new(self.clone())),
            ..self.clone()
        };

        Ok((new_world, rel_entity))
    }

    /// Creates a relationship entity with cardinality enforcement.
    ///
    /// This is the preferred way to create relationships. It:
    /// 1. Looks up the relationship schema
    /// 2. Checks cardinality constraints
    /// 3. Handles violations according to schema settings (Error or Replace)
    /// 4. Creates the relationship entity
    ///
    /// # Cardinality Rules
    ///
    /// - `OneToOne`: Source can have at most one target, target can have at most one source
    /// - `ManyToOne`: Source can have at most one target (many sources per target OK)
    /// - `OneToMany`: Target can have at most one source (many targets per source OK)
    /// - `ManyToMany`: No constraints
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Source or target entities don't exist
    /// - Relationship type is not registered
    /// - Cardinality would be violated and `on_violation` is `Error`
    pub fn create_relationship(
        &self,
        rel_type: KeywordId,
        source: EntityId,
        target: EntityId,
    ) -> Result<(World, EntityId)> {
        use crate::schema::{Cardinality, OnViolation};

        // Look up the relationship schema
        let schema = self.relationships.schema(rel_type).ok_or_else(|| {
            Error::new(ErrorKind::Internal(format!(
                "unknown relationship type: {rel_type:?}"
            )))
        })?;

        let cardinality = schema.cardinality;
        let on_violation = schema.on_violation;

        // Check for existing identical relationship (idempotent)
        let existing = self.find_relationships(Some(rel_type), Some(source), Some(target));
        if !existing.is_empty() {
            // Relationship already exists, return unchanged world and existing entity
            return Ok((self.clone(), existing[0]));
        }

        // Collect relationship entities to remove before creating new one
        let mut to_remove: Vec<EntityId> = Vec::new();

        match cardinality {
            Cardinality::OneToOne => {
                // Source can have at most one target
                let existing_from_source =
                    self.find_relationships(Some(rel_type), Some(source), None);
                if !existing_from_source.is_empty() {
                    match on_violation {
                        OnViolation::Error => {
                            return Err(Error::new(ErrorKind::Internal(
                                "cardinality violation: source already has a target".to_string(),
                            )));
                        }
                        OnViolation::Replace => {
                            to_remove.extend(existing_from_source);
                        }
                    }
                }

                // Target can have at most one source
                let existing_to_target =
                    self.find_relationships(Some(rel_type), None, Some(target));
                if !existing_to_target.is_empty() {
                    match on_violation {
                        OnViolation::Error => {
                            return Err(Error::new(ErrorKind::Internal(
                                "cardinality violation: target already has a source".to_string(),
                            )));
                        }
                        OnViolation::Replace => {
                            to_remove.extend(existing_to_target);
                        }
                    }
                }
            }
            Cardinality::ManyToOne => {
                // Source can have at most one target
                let existing_from_source =
                    self.find_relationships(Some(rel_type), Some(source), None);
                if !existing_from_source.is_empty() {
                    match on_violation {
                        OnViolation::Error => {
                            return Err(Error::new(ErrorKind::Internal(
                                "cardinality violation: source already has a target".to_string(),
                            )));
                        }
                        OnViolation::Replace => {
                            to_remove.extend(existing_from_source);
                        }
                    }
                }
            }
            Cardinality::OneToMany => {
                // Target can have at most one source
                let existing_to_target =
                    self.find_relationships(Some(rel_type), None, Some(target));
                if !existing_to_target.is_empty() {
                    match on_violation {
                        OnViolation::Error => {
                            return Err(Error::new(ErrorKind::Internal(
                                "cardinality violation: target already has a source".to_string(),
                            )));
                        }
                        OnViolation::Replace => {
                            to_remove.extend(existing_to_target);
                        }
                    }
                }
            }
            Cardinality::ManyToMany => {
                // No constraints
            }
        }

        // Remove old relationships if Replace strategy
        let mut world = self.clone();
        for rel_entity in to_remove {
            world = world.destroy(rel_entity)?;
        }

        // Create the new relationship entity
        world.spawn_relationship(rel_type, source, target)
    }

    /// Finds relationship entities matching the given criteria.
    ///
    /// All parameters are optional filters:
    /// - `rel_type`: Only return relationships of this type
    /// - `source`: Only return relationships from this source entity
    /// - `target`: Only return relationships to this target entity
    ///
    /// Returns entity IDs of matching relationship entities.
    ///
    /// Note: This is currently O(n) over all entities. Will be optimized
    /// with indexes in Phase 5.6.
    #[must_use]
    pub fn find_relationships(
        &self,
        rel_type: Option<KeywordId>,
        source: Option<EntityId>,
        target: Option<EntityId>,
    ) -> Vec<EntityId> {
        // Find all entities that have :rel/type component (i.e., are relationship entities)
        self.components
            .with_component(KeywordId::REL_TYPE)
            .filter(|&entity| {
                // Check rel_type filter
                if let Some(expected_type) = rel_type {
                    if let Some(Value::Map(map)) = self.components.get(entity, KeywordId::REL_TYPE)
                    {
                        if let Some(Value::Keyword(actual_type)) =
                            map.get(&Value::Keyword(KeywordId::VALUE))
                        {
                            if *actual_type != expected_type {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                // Check source filter
                if let Some(expected_source) = source {
                    if let Some(Value::Map(map)) =
                        self.components.get(entity, KeywordId::REL_SOURCE)
                    {
                        if let Some(Value::EntityRef(actual_source)) =
                            map.get(&Value::Keyword(KeywordId::VALUE))
                        {
                            if *actual_source != expected_source {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                // Check target filter
                if let Some(expected_target) = target {
                    if let Some(Value::Map(map)) =
                        self.components.get(entity, KeywordId::REL_TARGET)
                    {
                        if let Some(Value::EntityRef(actual_target)) =
                            map.get(&Value::Keyword(KeywordId::VALUE))
                        {
                            if *actual_target != expected_target {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                true
            })
            .collect()
    }

    /// Finds relationship entities where the type name starts with the given prefix.
    ///
    /// This is useful for querying relationships by namespace, e.g., `"exit/"` to find
    /// all exit relationships (`:exit/north`, `:exit/south`, etc.).
    ///
    /// Parameters are optional filters:
    /// - `prefix`: String prefix to match against relationship type names
    /// - `source`: Only return relationships from this source entity
    /// - `target`: Only return relationships to this target entity
    ///
    /// Returns entity IDs of matching relationship entities.
    ///
    /// Note: This is currently O(n) over all entities. Will be optimized
    /// with indexes in Phase 5.6.
    #[must_use]
    pub fn find_relationships_by_prefix(
        &self,
        prefix: &str,
        source: Option<EntityId>,
        target: Option<EntityId>,
    ) -> Vec<EntityId> {
        // First, get all relationships filtered by source/target
        let candidates = self.find_relationships(None, source, target);

        // Then filter by prefix
        candidates
            .into_iter()
            .filter(|&entity| {
                if let Some(Value::Map(map)) = self.components.get(entity, KeywordId::REL_TYPE) {
                    if let Some(Value::Keyword(kw)) = map.get(&Value::Keyword(KeywordId::VALUE)) {
                        if let Some(name) = self.interner.get_keyword(*kw) {
                            return name.starts_with(prefix);
                        }
                    }
                }
                false
            })
            .collect()
    }

    /// Checks if an entity has an outgoing relationship of the given type.
    ///
    /// Note: This is currently O(n) over all relationship entities.
    /// Will be optimized with indexes in Phase 5.6.
    #[must_use]
    pub fn has_outgoing(&self, source: EntityId, rel_type: KeywordId) -> bool {
        !self
            .find_relationships(Some(rel_type), Some(source), None)
            .is_empty()
    }

    /// Checks if an entity has an incoming relationship of the given type.
    ///
    /// Note: This is currently O(n) over all relationship entities.
    /// Will be optimized with indexes in Phase 5.6.
    #[must_use]
    pub fn has_incoming(&self, target: EntityId, rel_type: KeywordId) -> bool {
        !self
            .find_relationships(Some(rel_type), None, Some(target))
            .is_empty()
    }

    /// Gets the relationship type from a relationship entity.
    fn get_relationship_type(&self, rel_entity: EntityId) -> Option<KeywordId> {
        if let Some(Value::Map(map)) = self.components.get(rel_entity, KeywordId::REL_TYPE) {
            if let Some(Value::Keyword(kw)) = map.get(&Value::Keyword(KeywordId::VALUE)) {
                return Some(*kw);
            }
        }
        None
    }

    /// Gets the source entity from a relationship entity.
    fn get_relationship_source(&self, rel_entity: EntityId) -> Option<EntityId> {
        if let Some(Value::Map(map)) = self.components.get(rel_entity, KeywordId::REL_SOURCE) {
            if let Some(Value::EntityRef(id)) = map.get(&Value::Keyword(KeywordId::VALUE)) {
                return Some(*id);
            }
        }
        None
    }

    /// Destroys an entity and all its components/relationships.
    ///
    /// Returns a new World with the entity removed.
    pub fn destroy(&self, entity: EntityId) -> Result<World> {
        self.entities.validate(entity)?;

        let mut new_entities = (*self.entities).clone();
        let mut new_components = (*self.components).clone();

        // Find relationship entities where this entity is source OR target
        let rel_entities_as_source = self.find_relationships(None, Some(entity), None);
        let rel_entities_as_target = self.find_relationships(None, None, Some(entity));

        // Collect cascade victims: entities that should be deleted because of OnDelete::Cascade
        // When entity E is deleted, find relationships where E is TARGET with Cascade policy
        let mut cascade_victims = Vec::new();
        for rel_entity in &rel_entities_as_target {
            if let Some(rel_type) = self.get_relationship_type(*rel_entity) {
                if let Some(schema) = self.relationships.schema(rel_type) {
                    if schema.on_target_delete == OnDelete::Cascade {
                        if let Some(source) = self.get_relationship_source(*rel_entity) {
                            if !cascade_victims.contains(&source) {
                                cascade_victims.push(source);
                            }
                        }
                    }
                }
            }
        }

        // Collect all relationship entities to destroy (deduplicated)
        let mut rel_entities_to_destroy: Vec<EntityId> = rel_entities_as_source;
        for rel in rel_entities_as_target {
            if !rel_entities_to_destroy.contains(&rel) {
                rel_entities_to_destroy.push(rel);
            }
        }

        // Destroy relationship entities
        for rel_entity in rel_entities_to_destroy {
            if new_entities.exists(rel_entity) {
                new_components.remove_entity(rel_entity);
                let _ = new_entities.destroy(rel_entity);
            }
        }

        // Remove all components
        new_components.remove_entity(entity);

        // Destroy the entity
        new_entities.destroy(entity)?;

        // Process cascade deletions recursively
        // Note: We build new World first, then call destroy on cascade victims
        let mut world = World {
            entities: Arc::new(new_entities),
            components: Arc::new(new_components),
            relationships: Arc::clone(&self.relationships),
            previous: Some(Arc::new(self.clone())),
            interner: Arc::clone(&self.interner),
            tick: self.tick,
            seed: self.seed,
        };

        for victim in cascade_victims {
            if world.exists(victim) {
                world = world.destroy(victim)?;
            }
        }

        Ok(world)
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

    /// Returns the components that an entity has.
    ///
    /// Returns an empty slice if the entity doesn't exist or has no components.
    #[must_use]
    pub fn entity_components(&self, entity: EntityId) -> &[KeywordId] {
        self.components
            .archetype(entity)
            .map_or(&[], |arch| arch.components())
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
    ///
    /// Uses `create_relationship` which handles cardinality enforcement.
    pub fn link(
        &self,
        source: EntityId,
        relationship: KeywordId,
        target: EntityId,
    ) -> Result<World> {
        self.entities.validate(source)?;
        self.entities.validate(target)?;

        // Create relationship entity (handles cardinality enforcement)
        let (world, _rel_entity) = self.create_relationship(relationship, source, target)?;

        Ok(world)
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

        // Find and destroy the relationship entity
        let rel_entities = self.find_relationships(Some(relationship), Some(source), Some(target));

        let mut world = self.clone();
        for rel_entity in rel_entities {
            world = world.destroy(rel_entity)?;
        }

        // Update previous reference
        Ok(World {
            previous: Some(Arc::new(self.clone())),
            ..world
        })
    }

    /// Gets targets of a relationship from a source.
    ///
    /// # Read Migration (Phase 5.5.5)
    ///
    /// Now reads from relationship entities instead of the old `RelationshipStore`.
    /// Finds relationship entities where `:rel/type = relationship` and `:rel/source = source`,
    /// then extracts `:rel/target` from each.
    pub fn targets(
        &self,
        source: EntityId,
        relationship: KeywordId,
    ) -> impl Iterator<Item = EntityId> + '_ {
        // Find relationship entities matching type and source
        let rel_entities = self.find_relationships(Some(relationship), Some(source), None);

        // Extract targets from relationship entities
        rel_entities.into_iter().filter_map(|rel_entity| {
            self.get(rel_entity, KeywordId::REL_TARGET)
                .ok()
                .flatten()
                .and_then(|value| {
                    if let Value::Map(map) = value {
                        if let Some(Value::EntityRef(target)) =
                            map.get(&Value::Keyword(KeywordId::VALUE))
                        {
                            return Some(*target);
                        }
                    }
                    None
                })
        })
    }

    /// Gets sources pointing to a target.
    ///
    /// # Read Migration (Phase 5.5.5)
    ///
    /// Now reads from relationship entities instead of the old `RelationshipStore`.
    /// Finds relationship entities where `:rel/type = relationship` and `:rel/target = target`,
    /// then extracts `:rel/source` from each.
    pub fn sources(
        &self,
        target: EntityId,
        relationship: KeywordId,
    ) -> impl Iterator<Item = EntityId> + '_ {
        // Find relationship entities matching type and target
        let rel_entities = self.find_relationships(Some(relationship), None, Some(target));

        // Extract sources from relationship entities
        rel_entities.into_iter().filter_map(|rel_entity| {
            self.get(rel_entity, KeywordId::REL_SOURCE)
                .ok()
                .flatten()
                .and_then(|value| {
                    if let Value::Map(map) = value {
                        if let Some(Value::EntityRef(source)) =
                            map.get(&Value::Keyword(KeywordId::VALUE))
                        {
                            return Some(*source);
                        }
                    }
                    None
                })
        })
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
    /// This includes tick, seed, entity generations, and all component data
    /// in a deterministic order.
    #[must_use]
    pub fn content_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        // Hash basic fields
        self.tick.hash(&mut hasher);
        self.seed.hash(&mut hasher);

        // Hash entity generations (deterministically ordered by index)
        self.entities.generations().hash(&mut hasher);

        // Hash all component data in sorted order
        for (component, entity, value) in self.components.sorted_data() {
            component.hash(&mut hasher);
            entity.hash(&mut hasher);
            value.hash(&mut hasher);
        }

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
    use crate::schema::{
        Cardinality, ComponentSchema, FieldSchema, OnViolation, RelationshipSchema,
    };
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

    // --- Relationship Entity Tests ---

    #[test]
    fn spawn_relationship_creates_entity_with_components() {
        let world = setup_world();

        // Create source and target entities
        let (world, source) = world.spawn(&LtMap::new()).unwrap();
        let (world, target) = world.spawn(&LtMap::new()).unwrap();

        // Intern a relationship type
        let mut world = world;
        let exit_north = world.interner_mut().intern_keyword("exit/north");

        // Spawn a relationship
        let (world, rel_entity) = world
            .spawn_relationship(exit_north, source, target)
            .unwrap();

        // Verify the relationship entity exists and has correct components
        assert!(world.exists(rel_entity));
        assert_eq!(world.entity_count(), 3); // source, target, relationship

        // Check :rel/type
        let rel_type = world.get(rel_entity, KeywordId::REL_TYPE).unwrap().unwrap();
        if let Value::Map(map) = rel_type {
            let value = map.get(&Value::Keyword(KeywordId::VALUE)).unwrap();
            assert_eq!(*value, Value::Keyword(exit_north));
        } else {
            panic!("expected map for :rel/type");
        }

        // Check :rel/source
        let rel_source = world
            .get(rel_entity, KeywordId::REL_SOURCE)
            .unwrap()
            .unwrap();
        if let Value::Map(map) = rel_source {
            let value = map.get(&Value::Keyword(KeywordId::VALUE)).unwrap();
            assert_eq!(*value, Value::EntityRef(source));
        } else {
            panic!("expected map for :rel/source");
        }

        // Check :rel/target
        let rel_target = world
            .get(rel_entity, KeywordId::REL_TARGET)
            .unwrap()
            .unwrap();
        if let Value::Map(map) = rel_target {
            let value = map.get(&Value::Keyword(KeywordId::VALUE)).unwrap();
            assert_eq!(*value, Value::EntityRef(target));
        } else {
            panic!("expected map for :rel/target");
        }
    }

    #[test]
    fn find_relationships_by_type() {
        let world = setup_world();

        let (world, a) = world.spawn(&LtMap::new()).unwrap();
        let (world, b) = world.spawn(&LtMap::new()).unwrap();
        let (world, c) = world.spawn(&LtMap::new()).unwrap();

        let mut world = world;
        let exit_north = world.interner_mut().intern_keyword("exit/north");
        let exit_south = world.interner_mut().intern_keyword("exit/south");

        // Create relationships
        let (world, rel1) = world.spawn_relationship(exit_north, a, b).unwrap();
        let (world, _rel2) = world.spawn_relationship(exit_south, b, a).unwrap();
        let (world, rel3) = world.spawn_relationship(exit_north, b, c).unwrap();

        // Find all :exit/north relationships
        let north_rels = world.find_relationships(Some(exit_north), None, None);
        assert_eq!(north_rels.len(), 2);
        assert!(north_rels.contains(&rel1));
        assert!(north_rels.contains(&rel3));
    }

    #[test]
    fn find_relationships_by_source() {
        let world = setup_world();

        let (world, a) = world.spawn(&LtMap::new()).unwrap();
        let (world, b) = world.spawn(&LtMap::new()).unwrap();
        let (world, c) = world.spawn(&LtMap::new()).unwrap();

        let mut world = world;
        let exit_north = world.interner_mut().intern_keyword("exit/north");

        let (world, rel1) = world.spawn_relationship(exit_north, a, b).unwrap();
        let (world, rel2) = world.spawn_relationship(exit_north, a, c).unwrap();
        let (world, _rel3) = world.spawn_relationship(exit_north, b, c).unwrap();

        // Find relationships from entity a
        let from_a = world.find_relationships(None, Some(a), None);
        assert_eq!(from_a.len(), 2);
        assert!(from_a.contains(&rel1));
        assert!(from_a.contains(&rel2));
    }

    #[test]
    fn find_relationships_by_target() {
        let world = setup_world();

        let (world, a) = world.spawn(&LtMap::new()).unwrap();
        let (world, b) = world.spawn(&LtMap::new()).unwrap();
        let (world, c) = world.spawn(&LtMap::new()).unwrap();

        let mut world = world;
        let exit_north = world.interner_mut().intern_keyword("exit/north");

        let (world, _rel1) = world.spawn_relationship(exit_north, a, c).unwrap();
        let (world, rel2) = world.spawn_relationship(exit_north, b, c).unwrap();
        let (world, rel3) = world.spawn_relationship(exit_north, a, c).unwrap();

        // Find relationships to entity c
        let to_c = world.find_relationships(None, None, Some(c));
        assert_eq!(to_c.len(), 3);
        assert!(to_c.contains(&rel2));
        assert!(to_c.contains(&rel3));
    }

    #[test]
    fn has_outgoing_and_incoming() {
        let world = setup_world();

        let (world, a) = world.spawn(&LtMap::new()).unwrap();
        let (world, b) = world.spawn(&LtMap::new()).unwrap();

        let mut world = world;
        let exit_north = world.interner_mut().intern_keyword("exit/north");
        let exit_south = world.interner_mut().intern_keyword("exit/south");

        // Before any relationships
        assert!(!world.has_outgoing(a, exit_north));
        assert!(!world.has_incoming(b, exit_north));

        // Create a->b via exit/north
        let (world, _) = world.spawn_relationship(exit_north, a, b).unwrap();

        // Now a has outgoing :exit/north, b has incoming :exit/north
        assert!(world.has_outgoing(a, exit_north));
        assert!(world.has_incoming(b, exit_north));

        // But not for :exit/south
        assert!(!world.has_outgoing(a, exit_south));
        assert!(!world.has_incoming(b, exit_south));

        // And not in reverse direction
        assert!(!world.has_outgoing(b, exit_north));
        assert!(!world.has_incoming(a, exit_north));
    }

    #[test]
    fn reserved_component_schemas_are_registered() {
        let world = setup_world();

        // Verify reserved schemas exist
        assert!(world.component_schema(KeywordId::REL_TYPE).is_some());
        assert!(world.component_schema(KeywordId::REL_SOURCE).is_some());
        assert!(world.component_schema(KeywordId::REL_TARGET).is_some());
    }

    // --- Cardinality Enforcement Tests ---

    #[test]
    fn create_relationship_one_to_one_errors_on_duplicate_source() {
        let mut world = setup_world();

        let in_room = world.interner_mut().intern_keyword("in-room");
        world = world
            .register_relationship(
                RelationshipSchema::new(in_room).with_cardinality(Cardinality::OneToOne),
            )
            .unwrap();

        let (world, player) = world.spawn(&LtMap::new()).unwrap();
        let (world, room1) = world.spawn(&LtMap::new()).unwrap();
        let (world, room2) = world.spawn(&LtMap::new()).unwrap();

        // First link succeeds
        let (world, _) = world.create_relationship(in_room, player, room1).unwrap();

        // Second link from same source should error (OneToOne)
        let result = world.create_relationship(in_room, player, room2);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("source already has a target")
        );
    }

    #[test]
    fn create_relationship_one_to_one_errors_on_duplicate_target() {
        let mut world = setup_world();

        let in_room = world.interner_mut().intern_keyword("in-room");
        world = world
            .register_relationship(
                RelationshipSchema::new(in_room).with_cardinality(Cardinality::OneToOne),
            )
            .unwrap();

        let (world, player1) = world.spawn(&LtMap::new()).unwrap();
        let (world, player2) = world.spawn(&LtMap::new()).unwrap();
        let (world, room) = world.spawn(&LtMap::new()).unwrap();

        // First link succeeds
        let (world, _) = world.create_relationship(in_room, player1, room).unwrap();

        // Second link to same target should error (OneToOne)
        let result = world.create_relationship(in_room, player2, room);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("target already has a source")
        );
    }

    #[test]
    fn create_relationship_many_to_one_errors_on_duplicate_source() {
        let mut world = setup_world();

        let in_room = world.interner_mut().intern_keyword("in-room");
        world = world
            .register_relationship(
                RelationshipSchema::new(in_room).with_cardinality(Cardinality::ManyToOne),
            )
            .unwrap();

        let (world, player) = world.spawn(&LtMap::new()).unwrap();
        let (world, room1) = world.spawn(&LtMap::new()).unwrap();
        let (world, room2) = world.spawn(&LtMap::new()).unwrap();

        // First link succeeds
        let (world, _) = world.create_relationship(in_room, player, room1).unwrap();

        // Second link from same source should error (ManyToOne)
        let result = world.create_relationship(in_room, player, room2);
        assert!(result.is_err());
    }

    #[test]
    fn create_relationship_many_to_one_allows_multiple_sources() {
        let mut world = setup_world();

        let in_room = world.interner_mut().intern_keyword("in-room");
        world = world
            .register_relationship(
                RelationshipSchema::new(in_room).with_cardinality(Cardinality::ManyToOne),
            )
            .unwrap();

        let (world, player1) = world.spawn(&LtMap::new()).unwrap();
        let (world, player2) = world.spawn(&LtMap::new()).unwrap();
        let (world, room) = world.spawn(&LtMap::new()).unwrap();

        // Both links should succeed (many sources to one target is allowed)
        let (world, _) = world.create_relationship(in_room, player1, room).unwrap();
        let (world, _) = world.create_relationship(in_room, player2, room).unwrap();

        // Verify both relationships exist
        let rels = world.find_relationships(Some(in_room), None, Some(room));
        assert_eq!(rels.len(), 2);
    }

    #[test]
    fn create_relationship_one_to_many_errors_on_duplicate_target() {
        let mut world = setup_world();

        let contains = world.interner_mut().intern_keyword("contains");
        world = world
            .register_relationship(
                RelationshipSchema::new(contains).with_cardinality(Cardinality::OneToMany),
            )
            .unwrap();

        let (world, container1) = world.spawn(&LtMap::new()).unwrap();
        let (world, container2) = world.spawn(&LtMap::new()).unwrap();
        let (world, item) = world.spawn(&LtMap::new()).unwrap();

        // First link succeeds
        let (world, _) = world
            .create_relationship(contains, container1, item)
            .unwrap();

        // Second link to same target should error (OneToMany)
        let result = world.create_relationship(contains, container2, item);
        assert!(result.is_err());
    }

    #[test]
    fn create_relationship_one_to_many_allows_multiple_targets() {
        let mut world = setup_world();

        let contains = world.interner_mut().intern_keyword("contains");
        world = world
            .register_relationship(
                RelationshipSchema::new(contains).with_cardinality(Cardinality::OneToMany),
            )
            .unwrap();

        let (world, container) = world.spawn(&LtMap::new()).unwrap();
        let (world, item1) = world.spawn(&LtMap::new()).unwrap();
        let (world, item2) = world.spawn(&LtMap::new()).unwrap();

        // Both links should succeed (one source to many targets is allowed)
        let (world, _) = world
            .create_relationship(contains, container, item1)
            .unwrap();
        let (world, _) = world
            .create_relationship(contains, container, item2)
            .unwrap();

        // Verify both relationships exist
        let rels = world.find_relationships(Some(contains), Some(container), None);
        assert_eq!(rels.len(), 2);
    }

    #[test]
    fn create_relationship_many_to_many_allows_all() {
        let mut world = setup_world();

        let likes = world.interner_mut().intern_keyword("likes");
        world = world
            .register_relationship(
                RelationshipSchema::new(likes).with_cardinality(Cardinality::ManyToMany),
            )
            .unwrap();

        let (world, a) = world.spawn(&LtMap::new()).unwrap();
        let (world, b) = world.spawn(&LtMap::new()).unwrap();
        let (world, c) = world.spawn(&LtMap::new()).unwrap();

        // All combinations should work
        let (world, _) = world.create_relationship(likes, a, b).unwrap();
        let (world, _) = world.create_relationship(likes, a, c).unwrap();
        let (world, _) = world.create_relationship(likes, b, c).unwrap();
        let (world, _) = world.create_relationship(likes, c, a).unwrap();

        let rels = world.find_relationships(Some(likes), None, None);
        assert_eq!(rels.len(), 4);
    }

    #[test]
    fn create_relationship_replace_removes_old_one_to_one() {
        let mut world = setup_world();

        let in_room = world.interner_mut().intern_keyword("in-room");
        world = world
            .register_relationship(
                RelationshipSchema::new(in_room)
                    .with_cardinality(Cardinality::OneToOne)
                    .with_on_violation(OnViolation::Replace),
            )
            .unwrap();

        let (world, player) = world.spawn(&LtMap::new()).unwrap();
        let (world, room1) = world.spawn(&LtMap::new()).unwrap();
        let (world, room2) = world.spawn(&LtMap::new()).unwrap();

        // First link
        let (world, rel1) = world.create_relationship(in_room, player, room1).unwrap();
        assert!(world.exists(rel1));

        // Second link should replace the first
        let (world, rel2) = world.create_relationship(in_room, player, room2).unwrap();

        // Old relationship should be gone
        assert!(!world.exists(rel1));
        // New one should exist
        assert!(world.exists(rel2));

        // Only one relationship from player
        let found = world.find_relationships(Some(in_room), Some(player), None);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], rel2);
    }

    #[test]
    fn create_relationship_is_idempotent() {
        let mut world = setup_world();

        let in_room = world.interner_mut().intern_keyword("in-room");
        world = world
            .register_relationship(
                RelationshipSchema::new(in_room).with_cardinality(Cardinality::OneToOne),
            )
            .unwrap();

        let (world, player) = world.spawn(&LtMap::new()).unwrap();
        let (world, room) = world.spawn(&LtMap::new()).unwrap();

        // Create relationship
        let (world, rel1) = world.create_relationship(in_room, player, room).unwrap();

        // Creating same relationship again should be idempotent
        let (world, rel2) = world.create_relationship(in_room, player, room).unwrap();

        // Should return same entity
        assert_eq!(rel1, rel2);

        // Should still be only one relationship
        let found = world.find_relationships(Some(in_room), Some(player), Some(room));
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn create_relationship_requires_registered_schema() {
        let world = setup_world();

        let (world, a) = world.spawn(&LtMap::new()).unwrap();
        let (world, b) = world.spawn(&LtMap::new()).unwrap();

        let mut world = world;
        let unregistered = world.interner_mut().intern_keyword("unregistered");

        // Should error because relationship type is not registered
        let result = world.create_relationship(unregistered, a, b);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("unknown relationship")
        );
    }

    // --- Orphan Cleanup Tests ---

    #[test]
    fn destroy_source_removes_relationship_entity() {
        let mut world = setup_world();

        let in_room = world.interner_mut().intern_keyword("in-room");
        world = world
            .register_relationship(
                RelationshipSchema::new(in_room).with_cardinality(Cardinality::ManyToOne),
            )
            .unwrap();

        let (world, player) = world.spawn(&LtMap::new()).unwrap();
        let (world, room) = world.spawn(&LtMap::new()).unwrap();

        // Create relationship: player -[in-room]-> room
        let (world, rel_entity) = world.create_relationship(in_room, player, room).unwrap();

        // Verify relationship entity exists
        assert!(world.exists(rel_entity));
        assert_eq!(world.entity_count(), 3); // player, room, relationship

        // Destroy the source entity (player)
        let world = world.destroy(player).unwrap();

        // Relationship entity should also be destroyed
        assert!(!world.exists(rel_entity));
        assert_eq!(world.entity_count(), 1); // only room remains
    }

    #[test]
    fn destroy_target_removes_relationship_entity() {
        let mut world = setup_world();

        let in_room = world.interner_mut().intern_keyword("in-room");
        world = world
            .register_relationship(
                RelationshipSchema::new(in_room).with_cardinality(Cardinality::ManyToOne),
            )
            .unwrap();

        let (world, player) = world.spawn(&LtMap::new()).unwrap();
        let (world, room) = world.spawn(&LtMap::new()).unwrap();

        // Create relationship: player -[in-room]-> room
        let (world, rel_entity) = world.create_relationship(in_room, player, room).unwrap();

        // Verify relationship entity exists
        assert!(world.exists(rel_entity));

        // Destroy the target entity (room)
        let world = world.destroy(room).unwrap();

        // Relationship entity should also be destroyed
        assert!(!world.exists(rel_entity));
        assert_eq!(world.entity_count(), 1); // only player remains
    }

    #[test]
    fn destroy_cleans_up_multiple_relationships() {
        let mut world = setup_world();

        let in_room = world.interner_mut().intern_keyword("in-room");
        let owns = world.interner_mut().intern_keyword("owns");
        world = world
            .register_relationship(
                RelationshipSchema::new(in_room).with_cardinality(Cardinality::ManyToOne),
            )
            .unwrap();
        world = world
            .register_relationship(
                RelationshipSchema::new(owns).with_cardinality(Cardinality::OneToMany),
            )
            .unwrap();

        let (world, player) = world.spawn(&LtMap::new()).unwrap();
        let (world, room) = world.spawn(&LtMap::new()).unwrap();
        let (world, item) = world.spawn(&LtMap::new()).unwrap();

        // Create relationships:
        // player -[in-room]-> room
        // player -[owns]-> item
        let (world, rel1) = world.create_relationship(in_room, player, room).unwrap();
        let (world, rel2) = world.create_relationship(owns, player, item).unwrap();

        assert_eq!(world.entity_count(), 5); // player, room, item, 2 relationships

        // Destroy player (source of both relationships)
        let world = world.destroy(player).unwrap();

        // Both relationship entities should be destroyed
        assert!(!world.exists(rel1));
        assert!(!world.exists(rel2));
        assert_eq!(world.entity_count(), 2); // only room and item remain
    }
}
