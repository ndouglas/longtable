//! Component storage with archetype-based organization.
//!
//! Components are stored per-entity with schema validation.
//! Archetypes track which components each entity has for efficient querying.

use std::collections::HashMap;

use longtable_foundation::{EntityId, Error, ErrorKind, KeywordId, LtMap, Result, Type, Value};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::schema::{ComponentSchema, FieldSchema};

/// Represents a set of component types an entity has.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Archetype {
    /// Component types, sorted by `KeywordId` for consistent identity.
    components: Vec<KeywordId>,
}

impl Archetype {
    /// Creates a new empty archetype.
    #[must_use]
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
        }
    }

    /// Creates an archetype from a list of components.
    #[must_use]
    pub fn from_components(mut components: Vec<KeywordId>) -> Self {
        components.sort_by_key(|k| k.index());
        components.dedup();
        Self { components }
    }

    /// Returns the components in this archetype.
    #[must_use]
    pub fn components(&self) -> &[KeywordId] {
        &self.components
    }

    /// Checks if this archetype contains a component.
    #[must_use]
    pub fn contains(&self, component: KeywordId) -> bool {
        self.components
            .binary_search_by_key(&component.index(), |k| k.index())
            .is_ok()
    }

    /// Returns a new archetype with the component added.
    #[must_use]
    pub fn with_component(&self, component: KeywordId) -> Self {
        if self.contains(component) {
            return self.clone();
        }
        let mut components = self.components.clone();
        let pos = components
            .binary_search_by_key(&component.index(), |k| k.index())
            .unwrap_or_else(|p| p);
        components.insert(pos, component);
        Self { components }
    }

    /// Returns a new archetype with the component removed.
    #[must_use]
    pub fn without_component(&self, component: KeywordId) -> Self {
        let mut components = self.components.clone();
        if let Ok(pos) = components.binary_search_by_key(&component.index(), |k| k.index()) {
            components.remove(pos);
        }
        Self { components }
    }

    /// Checks if this archetype contains all components in another.
    #[must_use]
    pub fn contains_all(&self, other: &[KeywordId]) -> bool {
        other.iter().all(|c| self.contains(*c))
    }
}

/// Stores all component data for entities.
///
/// This is a simple entity-indexed storage. Components are stored
/// as maps from entity ID to value.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ComponentStore {
    /// Registered schemas by component name.
    schemas: HashMap<KeywordId, ComponentSchema>,
    /// Component data: component -> entity -> value.
    data: HashMap<KeywordId, HashMap<EntityId, Value>>,
    /// Archetype for each entity.
    archetypes: HashMap<EntityId, Archetype>,
}

impl ComponentStore {
    /// Creates a new empty component store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a component schema.
    ///
    /// Must be called before using a component type.
    ///
    /// # Errors
    ///
    /// Returns an error if a schema with the same name is already registered.
    pub fn register_schema(&mut self, schema: ComponentSchema) -> Result<()> {
        if self.schemas.contains_key(&schema.name) {
            return Err(Error::new(ErrorKind::Internal(format!(
                "component schema already registered: {:?}",
                schema.name
            ))));
        }
        self.schemas.insert(schema.name, schema);
        Ok(())
    }

    /// Gets the schema for a component type.
    #[must_use]
    pub fn schema(&self, component: KeywordId) -> Option<&ComponentSchema> {
        self.schemas.get(&component)
    }

    /// Sets a component on an entity.
    ///
    /// The value should be a map with field values for non-tag components,
    /// or `true` for tag components.
    ///
    /// # Errors
    ///
    /// Returns an error if the component is not registered or validation fails.
    pub fn set(&mut self, entity: EntityId, component: KeywordId, value: Value) -> Result<()> {
        let schema = self.schema(component).ok_or_else(|| {
            Error::new(ErrorKind::Internal(format!(
                "unknown component: {component:?}"
            )))
        })?;

        // Validate the value against the schema
        Self::validate_component_value(schema, &value)?;

        // Store the value
        self.data
            .entry(component)
            .or_default()
            .insert(entity, value);

        // Update archetype
        let archetype = self.archetypes.entry(entity).or_default();
        *archetype = archetype.with_component(component);

        Ok(())
    }

    /// Sets a specific field within a component.
    ///
    /// If the component doesn't exist on the entity, creates it with defaults.
    ///
    /// # Errors
    ///
    /// Returns an error if the component or field is not registered.
    pub fn set_field(
        &mut self,
        entity: EntityId,
        component: KeywordId,
        field: KeywordId,
        value: Value,
    ) -> Result<()> {
        // Clone schema to avoid borrow issues
        let schema = self
            .schema(component)
            .ok_or_else(|| {
                Error::new(ErrorKind::Internal(format!(
                    "unknown component: {component:?}"
                )))
            })?
            .clone();

        // Verify field exists in schema
        let field_schema = schema.field(field).ok_or_else(|| {
            Error::new(ErrorKind::AttributeNotFound {
                component: format!("{component:?}"),
                attribute: format!("{field:?}"),
            })
        })?;

        // Validate field value type
        Self::validate_field_value(field_schema, &value)?;

        // Create default value if needed (before mutating data)
        let default_value = Self::create_default_component(&schema);

        // Get or create the component value
        let comp_data = self.data.entry(component).or_default();
        let comp_value = comp_data.entry(entity).or_insert(default_value);

        // Update the field
        if let Value::Map(map) = comp_value {
            let new_map = map.insert(Value::Keyword(field), value);
            *comp_value = Value::Map(new_map);
        }

        // Update archetype
        let archetype = self.archetypes.entry(entity).or_default();
        *archetype = archetype.with_component(component);

        Ok(())
    }

    /// Gets a component value for an entity.
    #[must_use]
    pub fn get(&self, entity: EntityId, component: KeywordId) -> Option<&Value> {
        self.data.get(&component)?.get(&entity)
    }

    /// Gets a specific field from a component.
    #[must_use]
    pub fn get_field(
        &self,
        entity: EntityId,
        component: KeywordId,
        field: KeywordId,
    ) -> Option<&Value> {
        let comp_value = self.get(entity, component)?;
        if let Value::Map(map) = comp_value {
            map.get(&Value::Keyword(field))
        } else {
            None
        }
    }

    /// Checks if an entity has a component.
    #[must_use]
    pub fn has(&self, entity: EntityId, component: KeywordId) -> bool {
        self.data
            .get(&component)
            .is_some_and(|m| m.contains_key(&entity))
    }

    /// Removes a component from an entity.
    ///
    /// Returns the removed value if it existed.
    pub fn remove(&mut self, entity: EntityId, component: KeywordId) -> Option<Value> {
        let value = self.data.get_mut(&component)?.remove(&entity);

        if value.is_some() {
            if let Some(archetype) = self.archetypes.get_mut(&entity) {
                *archetype = archetype.without_component(component);
            }
        }

        value
    }

    /// Removes all components for an entity.
    ///
    /// Called when an entity is destroyed.
    pub fn remove_entity(&mut self, entity: EntityId) {
        for comp_data in self.data.values_mut() {
            comp_data.remove(&entity);
        }
        self.archetypes.remove(&entity);
    }

    /// Gets the archetype for an entity.
    #[must_use]
    pub fn archetype(&self, entity: EntityId) -> Option<&Archetype> {
        self.archetypes.get(&entity)
    }

    /// Iterates entities with a specific component.
    pub fn with_component(&self, component: KeywordId) -> impl Iterator<Item = EntityId> + '_ {
        self.data
            .get(&component)
            .into_iter()
            .flat_map(|m| m.keys().copied())
    }

    /// Iterates entities having all specified components.
    pub fn with_archetype<'a>(
        &'a self,
        components: &'a [KeywordId],
    ) -> impl Iterator<Item = EntityId> + 'a {
        self.archetypes
            .iter()
            .filter(move |(_, arch)| arch.contains_all(components))
            .map(|(id, _)| *id)
    }

    // --- Private helpers ---

    fn validate_component_value(schema: &ComponentSchema, value: &Value) -> Result<()> {
        if schema.is_tag {
            // Tag components accept true or a map
            match value {
                Value::Bool(true) | Value::Map(_) => Ok(()),
                _ => Err(Error::type_mismatch(Type::Bool, value.value_type())),
            }
        } else {
            // Non-tag components must be maps
            match value {
                Value::Map(map) => {
                    // Validate required fields are present
                    for field in &schema.fields {
                        if field.required {
                            let key = Value::Keyword(field.name);
                            if !map.contains_key(&key) {
                                return Err(Error::new(ErrorKind::AttributeNotFound {
                                    component: format!("{:?}", schema.name),
                                    attribute: format!("{:?}", field.name),
                                }));
                            }
                        }
                    }
                    Ok(())
                }
                _ => Err(Error::type_mismatch(
                    Type::map(Type::Keyword, Type::Any),
                    value.value_type(),
                )),
            }
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    fn validate_field_value(_field_schema: &FieldSchema, _value: &Value) -> Result<()> {
        // TODO: Implement proper type checking
        // For now, accept any value
        Ok(())
    }

    fn create_default_component(schema: &ComponentSchema) -> Value {
        if schema.is_tag {
            Value::Bool(true)
        } else {
            let mut map = LtMap::new();
            for field in &schema.fields {
                if let Some(default) = &field.default {
                    map = map.insert(Value::Keyword(field.name), default.clone());
                }
            }
            Value::Map(map)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use longtable_foundation::Interner;

    fn setup() -> (ComponentStore, Interner) {
        let store = ComponentStore::new();
        let interner = Interner::new();
        (store, interner)
    }

    #[test]
    fn archetype_contains() {
        let mut interner = Interner::new();
        let health = interner.intern_keyword("health");
        let position = interner.intern_keyword("position");
        let velocity = interner.intern_keyword("velocity");

        let arch = Archetype::from_components(vec![health, position]);

        assert!(arch.contains(health));
        assert!(arch.contains(position));
        assert!(!arch.contains(velocity));
    }

    #[test]
    fn archetype_with_component() {
        let mut interner = Interner::new();
        let health = interner.intern_keyword("health");
        let position = interner.intern_keyword("position");

        let arch = Archetype::new()
            .with_component(health)
            .with_component(position);

        assert!(arch.contains(health));
        assert!(arch.contains(position));

        // Adding same component is idempotent
        let arch2 = arch.with_component(health);
        assert_eq!(arch.components().len(), arch2.components().len());
    }

    #[test]
    fn archetype_without_component() {
        let mut interner = Interner::new();
        let health = interner.intern_keyword("health");
        let position = interner.intern_keyword("position");

        let arch = Archetype::from_components(vec![health, position]);
        let arch2 = arch.without_component(health);

        assert!(!arch2.contains(health));
        assert!(arch2.contains(position));
    }

    #[test]
    fn register_and_get_schema() {
        let (mut store, mut interner) = setup();
        let health = interner.intern_keyword("health");
        let current = interner.intern_keyword("current");

        let schema = ComponentSchema::new(health)
            .with_field(crate::schema::FieldSchema::required(current, Type::Int));

        store.register_schema(schema.clone()).unwrap();

        let retrieved = store.schema(health).unwrap();
        assert_eq!(retrieved.name, health);
    }

    #[test]
    fn set_and_get_component() {
        let (mut store, mut interner) = setup();
        let health = interner.intern_keyword("health");
        let current = interner.intern_keyword("current");

        let schema = ComponentSchema::new(health)
            .with_field(crate::schema::FieldSchema::required(current, Type::Int));
        store.register_schema(schema).unwrap();

        let entity = EntityId::new(0, 1);
        let mut map = LtMap::new();
        map = map.insert(Value::Keyword(current), Value::Int(100));
        let value = Value::Map(map);

        store.set(entity, health, value.clone()).unwrap();

        let retrieved = store.get(entity, health).unwrap();
        assert_eq!(retrieved, &value);
    }

    #[test]
    fn set_and_get_field() {
        let (mut store, mut interner) = setup();
        let health = interner.intern_keyword("health");
        let current = interner.intern_keyword("current");
        let max = interner.intern_keyword("max");

        let schema = ComponentSchema::new(health)
            .with_field(crate::schema::FieldSchema::optional(
                current,
                Type::Int,
                Value::Int(100),
            ))
            .with_field(crate::schema::FieldSchema::optional(
                max,
                Type::Int,
                Value::Int(100),
            ));
        store.register_schema(schema).unwrap();

        let entity = EntityId::new(0, 1);
        store
            .set_field(entity, health, current, Value::Int(50))
            .unwrap();

        let retrieved = store.get_field(entity, health, current).unwrap();
        assert_eq!(retrieved, &Value::Int(50));

        // Default should be applied
        let max_val = store.get_field(entity, health, max).unwrap();
        assert_eq!(max_val, &Value::Int(100));
    }

    #[test]
    fn has_component() {
        let (mut store, mut interner) = setup();
        let health = interner.intern_keyword("health");
        let position = interner.intern_keyword("position");

        store.register_schema(ComponentSchema::tag(health)).unwrap();
        store
            .register_schema(ComponentSchema::tag(position))
            .unwrap();

        let entity = EntityId::new(0, 1);
        store.set(entity, health, Value::Bool(true)).unwrap();

        assert!(store.has(entity, health));
        assert!(!store.has(entity, position));
    }

    #[test]
    fn remove_component() {
        let (mut store, mut interner) = setup();
        let health = interner.intern_keyword("health");

        store.register_schema(ComponentSchema::tag(health)).unwrap();

        let entity = EntityId::new(0, 1);
        store.set(entity, health, Value::Bool(true)).unwrap();
        assert!(store.has(entity, health));

        let removed = store.remove(entity, health);
        assert!(removed.is_some());
        assert!(!store.has(entity, health));
    }

    #[test]
    fn with_component_iteration() {
        let (mut store, mut interner) = setup();
        let health = interner.intern_keyword("health");

        store.register_schema(ComponentSchema::tag(health)).unwrap();

        let e1 = EntityId::new(0, 1);
        let e2 = EntityId::new(1, 1);
        let e3 = EntityId::new(2, 1);

        store.set(e1, health, Value::Bool(true)).unwrap();
        store.set(e3, health, Value::Bool(true)).unwrap();

        let entities: Vec<_> = store.with_component(health).collect();
        assert_eq!(entities.len(), 2);
        assert!(entities.contains(&e1));
        assert!(entities.contains(&e3));
        assert!(!entities.contains(&e2));
    }

    #[test]
    fn with_archetype_iteration() {
        let (mut store, mut interner) = setup();
        let health = interner.intern_keyword("health");
        let position = interner.intern_keyword("position");

        store.register_schema(ComponentSchema::tag(health)).unwrap();
        store
            .register_schema(ComponentSchema::tag(position))
            .unwrap();

        let e1 = EntityId::new(0, 1);
        let e2 = EntityId::new(1, 1);
        let e3 = EntityId::new(2, 1);

        // e1 has health only
        store.set(e1, health, Value::Bool(true)).unwrap();
        // e2 has both
        store.set(e2, health, Value::Bool(true)).unwrap();
        store.set(e2, position, Value::Bool(true)).unwrap();
        // e3 has position only
        store.set(e3, position, Value::Bool(true)).unwrap();

        let with_both: Vec<_> = store.with_archetype(&[health, position]).collect();
        assert_eq!(with_both.len(), 1);
        assert!(with_both.contains(&e2));
    }

    #[test]
    fn remove_entity() {
        let (mut store, mut interner) = setup();
        let health = interner.intern_keyword("health");
        let position = interner.intern_keyword("position");

        store.register_schema(ComponentSchema::tag(health)).unwrap();
        store
            .register_schema(ComponentSchema::tag(position))
            .unwrap();

        let entity = EntityId::new(0, 1);
        store.set(entity, health, Value::Bool(true)).unwrap();
        store.set(entity, position, Value::Bool(true)).unwrap();

        assert!(store.has(entity, health));
        assert!(store.has(entity, position));

        store.remove_entity(entity);

        assert!(!store.has(entity, health));
        assert!(!store.has(entity, position));
        assert!(store.archetype(entity).is_none());
    }
}
