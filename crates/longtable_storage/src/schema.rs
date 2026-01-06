//! Schema definitions for components and relationships.
//!
//! Schemas define the structure and constraints for component and relationship types.

use longtable_foundation::{KeywordId, Type, Value};

/// Schema definition for a component type.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentSchema {
    /// Component name (e.g., `:health`, `:position`).
    pub name: KeywordId,
    /// Field definitions.
    pub fields: Vec<FieldSchema>,
    /// If true, this is a tag component (presence-only, no data).
    pub is_tag: bool,
}

impl ComponentSchema {
    /// Creates a new component schema.
    #[must_use]
    pub fn new(name: KeywordId) -> Self {
        Self {
            name,
            fields: Vec::new(),
            is_tag: false,
        }
    }

    /// Creates a tag component schema (no fields).
    #[must_use]
    pub fn tag(name: KeywordId) -> Self {
        Self {
            name,
            fields: Vec::new(),
            is_tag: true,
        }
    }

    /// Adds a field to the schema.
    #[must_use]
    pub fn with_field(mut self, field: FieldSchema) -> Self {
        self.fields.push(field);
        self
    }

    /// Returns the field schema by name.
    #[must_use]
    pub fn field(&self, name: KeywordId) -> Option<&FieldSchema> {
        self.fields.iter().find(|f| f.name == name)
    }
}

/// Schema definition for a component field.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldSchema {
    /// Field name.
    pub name: KeywordId,
    /// Field type.
    pub ty: Type,
    /// Default value if not provided.
    pub default: Option<Value>,
    /// Whether the field is required.
    pub required: bool,
}

impl FieldSchema {
    /// Creates a required field with no default.
    #[must_use]
    pub fn required(name: KeywordId, ty: Type) -> Self {
        Self {
            name,
            ty,
            default: None,
            required: true,
        }
    }

    /// Creates an optional field with a default value.
    #[must_use]
    pub fn optional(name: KeywordId, ty: Type, default: Value) -> Self {
        Self {
            name,
            ty,
            default: Some(default),
            required: false,
        }
    }

    /// Creates an optional field with no default (will be nil).
    #[must_use]
    pub fn optional_nil(name: KeywordId, ty: Type) -> Self {
        Self {
            name,
            ty,
            default: None,
            required: false,
        }
    }
}

/// Schema definition for a relationship type.
#[derive(Clone, Debug, PartialEq)]
pub struct RelationshipSchema {
    /// Relationship name (e.g., `:contains`, `:parent-of`).
    pub name: KeywordId,
    /// How the relationship is stored.
    pub storage: Storage,
    /// Cardinality constraint.
    pub cardinality: Cardinality,
    /// What happens when the target entity is deleted.
    pub on_target_delete: OnDelete,
    /// What happens when cardinality would be violated.
    pub on_violation: OnViolation,
    /// Attributes on the relationship edge (only for Entity storage).
    pub attributes: Vec<FieldSchema>,
}

impl RelationshipSchema {
    /// Creates a new relationship schema with default settings.
    #[must_use]
    pub fn new(name: KeywordId) -> Self {
        Self {
            name,
            storage: Storage::Field,
            cardinality: Cardinality::ManyToMany,
            on_target_delete: OnDelete::Remove,
            on_violation: OnViolation::Error,
            attributes: Vec::new(),
        }
    }

    /// Sets the storage type.
    #[must_use]
    pub fn with_storage(mut self, storage: Storage) -> Self {
        self.storage = storage;
        self
    }

    /// Sets the cardinality.
    #[must_use]
    pub fn with_cardinality(mut self, cardinality: Cardinality) -> Self {
        self.cardinality = cardinality;
        self
    }

    /// Sets the on-delete behavior.
    #[must_use]
    pub fn with_on_delete(mut self, on_delete: OnDelete) -> Self {
        self.on_target_delete = on_delete;
        self
    }

    /// Sets the on-violation behavior.
    #[must_use]
    pub fn with_on_violation(mut self, on_violation: OnViolation) -> Self {
        self.on_violation = on_violation;
        self
    }

    /// Adds an attribute to the relationship.
    #[must_use]
    pub fn with_attribute(mut self, attr: FieldSchema) -> Self {
        self.attributes.push(attr);
        self
    }
}

/// How a relationship is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Storage {
    /// Stored as a field on the source entity (efficient for 1:1 or N:1).
    Field,
    /// Stored as a separate entity (allows attributes on the edge).
    Entity,
}

/// Cardinality constraint for relationships.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cardinality {
    /// Each source has at most one target, each target has at most one source.
    OneToOne,
    /// Each source has at most one target, targets can have many sources.
    ManyToOne,
    /// Each source can have many targets, each target has at most one source.
    OneToMany,
    /// No cardinality constraints.
    ManyToMany,
}

/// What happens when the target of a relationship is deleted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OnDelete {
    /// Remove the relationship edge.
    Remove,
    /// Delete the source entity as well (cascade).
    Cascade,
    /// Set the relationship to nil (only for Field storage).
    Nullify,
}

/// What happens when a cardinality constraint would be violated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OnViolation {
    /// Return an error.
    Error,
    /// Replace the existing relationship.
    Replace,
}

#[cfg(test)]
mod tests {
    use super::*;
    use longtable_foundation::Interner;

    fn test_interner() -> Interner {
        Interner::new()
    }

    #[test]
    fn component_schema_basic() {
        let mut interner = test_interner();
        let health = interner.intern_keyword("health");
        let current = interner.intern_keyword("current");
        let max = interner.intern_keyword("max");

        let schema = ComponentSchema::new(health)
            .with_field(FieldSchema::required(current, Type::Int))
            .with_field(FieldSchema::optional(max, Type::Int, Value::Int(100)));

        assert_eq!(schema.name, health);
        assert_eq!(schema.fields.len(), 2);
        assert!(!schema.is_tag);

        let current_field = schema.field(current).unwrap();
        assert!(current_field.required);
        assert!(current_field.default.is_none());

        let max_field = schema.field(max).unwrap();
        assert!(!max_field.required);
        assert_eq!(max_field.default, Some(Value::Int(100)));
    }

    #[test]
    fn tag_component_schema() {
        let mut interner = test_interner();
        let player = interner.intern_keyword("player");

        let schema = ComponentSchema::tag(player);

        assert_eq!(schema.name, player);
        assert!(schema.is_tag);
        assert!(schema.fields.is_empty());
    }

    #[test]
    fn relationship_schema_basic() {
        let mut interner = test_interner();
        let contains = interner.intern_keyword("contains");

        let schema = RelationshipSchema::new(contains)
            .with_storage(Storage::Field)
            .with_cardinality(Cardinality::OneToMany)
            .with_on_delete(OnDelete::Cascade);

        assert_eq!(schema.name, contains);
        assert_eq!(schema.storage, Storage::Field);
        assert_eq!(schema.cardinality, Cardinality::OneToMany);
        assert_eq!(schema.on_target_delete, OnDelete::Cascade);
    }

    #[test]
    fn relationship_with_attributes() {
        let mut interner = test_interner();
        let equipped_in = interner.intern_keyword("equipped-in");
        let slot = interner.intern_keyword("slot");

        let schema = RelationshipSchema::new(equipped_in)
            .with_storage(Storage::Entity)
            .with_attribute(FieldSchema::required(slot, Type::Keyword));

        assert_eq!(schema.storage, Storage::Entity);
        assert_eq!(schema.attributes.len(), 1);
    }
}
