//! VM context traits and types for World integration.
//!
//! The VM can optionally access a World via the [`VmContext`] trait. This enables
//! execution of ECS operations (reading components, spawning entities, etc.).
//!
//! The [`RuntimeContext`] trait extends this with mutation capabilities for
//! registering schemas, vocabulary, and other machine configuration.

use longtable_foundation::{EntityId, Error, ErrorKind, KeywordId, LtMap, Result, Value};
use longtable_storage::World;

// =============================================================================
// VmContext Trait
// =============================================================================

/// Provides read-only World access for VM execution.
///
/// Implement this trait to allow the VM to read entity data during rule evaluation.
pub trait VmContext {
    /// Gets a component value for an entity.
    fn get_component(&self, entity: EntityId, component: KeywordId) -> Result<Option<Value>>;

    /// Gets a specific field from a component.
    fn get_field(
        &self,
        entity: EntityId,
        component: KeywordId,
        field: KeywordId,
    ) -> Result<Option<Value>>;

    /// Checks if an entity exists.
    fn exists(&self, entity: EntityId) -> bool;

    /// Checks if an entity has a component.
    fn has_component(&self, entity: EntityId, component: KeywordId) -> bool;

    /// Resolves a keyword value to its `KeywordId` (for dynamic keyword access).
    fn resolve_keyword(&self, value: &Value) -> Option<KeywordId>;

    /// Returns all entities that have the given component.
    fn with_component(&self, component: KeywordId) -> Vec<EntityId>;

    /// Finds relationship entities matching the given criteria.
    ///
    /// - `rel_type`: Optional relationship type (e.g., `:contained-in`)
    /// - `source`: Optional source entity filter
    /// - `target`: Optional target entity filter
    ///
    /// Returns the relationship entity IDs (not the source/target entities).
    fn find_relationships(
        &self,
        rel_type: Option<KeywordId>,
        source: Option<EntityId>,
        target: Option<EntityId>,
    ) -> Vec<EntityId>;

    /// Gets the target entities of relationships from a source.
    fn targets(&self, source: EntityId, rel_type: KeywordId) -> Vec<EntityId>;

    /// Gets the source entities of relationships to a target.
    fn sources(&self, target: EntityId, rel_type: KeywordId) -> Vec<EntityId>;
}

// =============================================================================
// RuntimeContext Trait
// =============================================================================

/// Extends [`VmContext`] with mutation capabilities for the Longtable machine.
///
/// The Longtable VM is a non-Von Neumann machine with specialized structures:
/// - ECS (entities, components, relationships)
/// - Vocabulary (verbs, directions, prepositions, etc.)
/// - Parser (commands, actions, scopes)
/// - Rule engine
///
/// This trait provides access to modify these machine structures via opcodes.
pub trait RuntimeContext: VmContext {
    // =========================================================================
    // Schema Registration (ECS Structure)
    // =========================================================================

    /// Registers a component schema.
    ///
    /// Schema map should contain:
    /// - `:name` - keyword for the component name
    /// - `:fields` - vector of field definitions (optional for tags)
    /// - `:storage` - storage kind (`:sparse`, `:dense`, `:tag`)
    fn register_component_schema(&mut self, schema: &Value) -> Result<()>;

    /// Registers a relationship schema.
    ///
    /// Schema map should contain:
    /// - `:name` - keyword for the relationship name
    /// - `:cardinality` - `:one-to-one`, `:one-to-many`, `:many-to-one`, `:many-to-many`
    /// - `:on-delete` - `:cascade`, `:remove`, `:restrict`
    fn register_relationship_schema(&mut self, schema: &Value) -> Result<()>;

    // =========================================================================
    // Vocabulary Registration
    // =========================================================================

    /// Registers a verb.
    ///
    /// Data map should contain:
    /// - `:name` - keyword for the verb name
    /// - `:synonyms` - vector of synonym keywords (optional)
    fn register_verb(&mut self, data: &Value) -> Result<()>;

    /// Registers a direction.
    ///
    /// Data map should contain:
    /// - `:name` - keyword for the direction name
    /// - `:synonyms` - vector of synonym keywords (optional)
    /// - `:opposite` - keyword for opposite direction (optional)
    fn register_direction(&mut self, data: &Value) -> Result<()>;

    /// Registers a preposition.
    ///
    /// Data map should contain:
    /// - `:name` - keyword for the preposition
    /// - `:implies` - semantic role keyword (optional)
    fn register_preposition(&mut self, data: &Value) -> Result<()>;

    /// Registers a pronoun.
    ///
    /// Data map should contain:
    /// - `:name` - keyword for the pronoun
    /// - `:gender` - `:masculine`, `:feminine`, `:neuter`
    /// - `:number` - `:singular`, `:plural`
    fn register_pronoun(&mut self, data: &Value) -> Result<()>;

    /// Registers an adverb.
    ///
    /// Data map should contain:
    /// - `:name` - keyword for the adverb
    fn register_adverb(&mut self, data: &Value) -> Result<()>;

    // =========================================================================
    // Parser Configuration
    // =========================================================================

    /// Registers a type constraint.
    ///
    /// Data map should contain:
    /// - `:name` - keyword for the type name
    /// - `:extends` - vector of parent type keywords (optional)
    /// - `:pattern` - pattern to match entities (optional)
    fn register_type(&mut self, data: &Value) -> Result<()>;

    /// Registers a scope.
    ///
    /// Data map should contain:
    /// - `:name` - keyword for the scope name
    /// - `:resolver` - function or keyword for the resolver
    fn register_scope(&mut self, data: &Value) -> Result<()>;

    /// Registers a command syntax.
    ///
    /// Data map should contain:
    /// - `:name` - keyword for the command name
    /// - `:syntax` - vector of syntax elements
    /// - `:action` - keyword for the associated action
    /// - `:priority` - integer priority (optional)
    fn register_command(&mut self, data: &Value) -> Result<()>;

    /// Registers an action handler.
    ///
    /// Data map should contain:
    /// - `:name` - keyword for the action name
    /// - `:match` - map of match criteria
    /// - `:preconditions` - vector of precondition checks (optional)
    /// - `:handler` - vector of handler expressions
    fn register_action(&mut self, data: &Value) -> Result<()>;

    // =========================================================================
    // Rule Registration
    // =========================================================================

    /// Registers a rule as an entity.
    ///
    /// Data map should contain:
    /// - `:name` - keyword for the rule name
    /// - `:when` - vector of pattern clauses
    /// - `:then` - vector of action expressions
    /// - `:salience` - integer priority (optional, default 0)
    /// - `:once` - boolean for one-shot rules (optional)
    ///
    /// Returns the entity ID of the created rule entity.
    fn register_rule(&mut self, data: &Value) -> Result<EntityId>;

    // =========================================================================
    // Interner Access
    // =========================================================================

    /// Interns a keyword string and returns its ID.
    ///
    /// This is needed for creating keyword values at runtime.
    fn intern_keyword(&mut self, name: &str) -> KeywordId;
}

// =============================================================================
// VM Effects
// =============================================================================

/// An effect produced by VM execution.
///
/// Effects represent mutations that should be applied to the World after
/// successful rule execution. Effects are collected during execution and
/// can be retrieved via [`super::Vm::take_effects`].
#[derive(Clone, Debug, PartialEq)]
pub enum VmEffect {
    /// Spawn a new entity with components.
    Spawn {
        /// Initial components as a map of keyword -> value.
        components: LtMap<Value, Value>,
    },

    /// Destroy an entity.
    Destroy {
        /// The entity to destroy.
        entity: EntityId,
    },

    /// Set a component on an entity.
    SetComponent {
        /// The target entity.
        entity: EntityId,
        /// The component name.
        component: KeywordId,
        /// The component value.
        value: Value,
    },

    /// Set a field within a component.
    SetField {
        /// The target entity.
        entity: EntityId,
        /// The component name.
        component: KeywordId,
        /// The field name.
        field: KeywordId,
        /// The field value.
        value: Value,
    },

    /// Create a relationship.
    Link {
        /// The source entity.
        source: EntityId,
        /// The relationship type.
        relationship: KeywordId,
        /// The target entity.
        target: EntityId,
    },

    /// Remove a relationship.
    Unlink {
        /// The source entity.
        source: EntityId,
        /// The relationship type.
        relationship: KeywordId,
        /// The target entity.
        target: EntityId,
    },
}

// =============================================================================
// WorldContext (VmContext implementation for World)
// =============================================================================

/// A context that provides access to a World for VM execution.
///
/// This allows the VM to read entity data during rule evaluation.
pub struct WorldContext<'a> {
    /// Reference to the World.
    world: &'a World,
}

impl<'a> WorldContext<'a> {
    /// Creates a new `WorldContext` wrapping a World reference.
    #[must_use]
    pub fn new(world: &'a World) -> Self {
        Self { world }
    }

    /// Returns a reference to the underlying World.
    #[must_use]
    pub fn world(&self) -> &World {
        self.world
    }
}

impl VmContext for WorldContext<'_> {
    fn get_component(&self, entity: EntityId, component: KeywordId) -> Result<Option<Value>> {
        self.world.get(entity, component)
    }

    fn get_field(
        &self,
        entity: EntityId,
        component: KeywordId,
        field: KeywordId,
    ) -> Result<Option<Value>> {
        self.world.get_field(entity, component, field)
    }

    fn exists(&self, entity: EntityId) -> bool {
        self.world.exists(entity)
    }

    fn has_component(&self, entity: EntityId, component: KeywordId) -> bool {
        self.world.has(entity, component)
    }

    fn resolve_keyword(&self, value: &Value) -> Option<KeywordId> {
        // Keywords are already interned and carry their ID
        if let Value::Keyword(k) = value {
            Some(*k)
        } else {
            None
        }
    }

    fn with_component(&self, component: KeywordId) -> Vec<EntityId> {
        self.world.with_component(component).collect()
    }

    fn find_relationships(
        &self,
        rel_type: Option<KeywordId>,
        source: Option<EntityId>,
        target: Option<EntityId>,
    ) -> Vec<EntityId> {
        self.world.find_relationships(rel_type, source, target)
    }

    fn targets(&self, source: EntityId, rel_type: KeywordId) -> Vec<EntityId> {
        self.world.targets(source, rel_type).collect()
    }

    fn sources(&self, target: EntityId, rel_type: KeywordId) -> Vec<EntityId> {
        self.world.sources(target, rel_type).collect()
    }
}

// =============================================================================
// NoContext (for pure evaluation without World)
// =============================================================================

/// A no-op context that returns errors for World operations.
///
/// Used internally when executing without a World context.
pub(crate) struct NoContext;

impl VmContext for NoContext {
    fn get_component(&self, _entity: EntityId, _component: KeywordId) -> Result<Option<Value>> {
        Err(Error::new(ErrorKind::Internal(
            "world operations not available in this context".to_string(),
        )))
    }

    fn get_field(
        &self,
        _entity: EntityId,
        _component: KeywordId,
        _field: KeywordId,
    ) -> Result<Option<Value>> {
        Err(Error::new(ErrorKind::Internal(
            "world operations not available in this context".to_string(),
        )))
    }

    fn exists(&self, _entity: EntityId) -> bool {
        false
    }

    fn has_component(&self, _entity: EntityId, _component: KeywordId) -> bool {
        false
    }

    fn resolve_keyword(&self, _value: &Value) -> Option<KeywordId> {
        None
    }

    fn with_component(&self, _component: KeywordId) -> Vec<EntityId> {
        Vec::new()
    }

    fn find_relationships(
        &self,
        _rel_type: Option<KeywordId>,
        _source: Option<EntityId>,
        _target: Option<EntityId>,
    ) -> Vec<EntityId> {
        Vec::new()
    }

    fn targets(&self, _source: EntityId, _rel_type: KeywordId) -> Vec<EntityId> {
        Vec::new()
    }

    fn sources(&self, _target: EntityId, _rel_type: KeywordId) -> Vec<EntityId> {
        Vec::new()
    }
}

// =============================================================================
// NoRuntimeContext (for execution without full runtime)
// =============================================================================

/// A no-op runtime context that errors on registration operations.
///
/// Used when the VM is executed without a full runtime environment.
/// Read operations work (delegated to `NoContext` behavior), but
/// registration operations fail with an error.
#[allow(dead_code)]
pub(crate) struct NoRuntimeContext;

impl VmContext for NoRuntimeContext {
    fn get_component(&self, _entity: EntityId, _component: KeywordId) -> Result<Option<Value>> {
        Err(Error::new(ErrorKind::Internal(
            "world operations not available in this context".to_string(),
        )))
    }

    fn get_field(
        &self,
        _entity: EntityId,
        _component: KeywordId,
        _field: KeywordId,
    ) -> Result<Option<Value>> {
        Err(Error::new(ErrorKind::Internal(
            "world operations not available in this context".to_string(),
        )))
    }

    fn exists(&self, _entity: EntityId) -> bool {
        false
    }

    fn has_component(&self, _entity: EntityId, _component: KeywordId) -> bool {
        false
    }

    fn resolve_keyword(&self, _value: &Value) -> Option<KeywordId> {
        None
    }

    fn with_component(&self, _component: KeywordId) -> Vec<EntityId> {
        Vec::new()
    }

    fn find_relationships(
        &self,
        _rel_type: Option<KeywordId>,
        _source: Option<EntityId>,
        _target: Option<EntityId>,
    ) -> Vec<EntityId> {
        Vec::new()
    }

    fn targets(&self, _source: EntityId, _rel_type: KeywordId) -> Vec<EntityId> {
        Vec::new()
    }

    fn sources(&self, _target: EntityId, _rel_type: KeywordId) -> Vec<EntityId> {
        Vec::new()
    }
}

impl RuntimeContext for NoRuntimeContext {
    fn register_component_schema(&mut self, _schema: &Value) -> Result<()> {
        Err(Error::new(ErrorKind::Internal(
            "schema registration not available in this context".to_string(),
        )))
    }

    fn register_relationship_schema(&mut self, _schema: &Value) -> Result<()> {
        Err(Error::new(ErrorKind::Internal(
            "schema registration not available in this context".to_string(),
        )))
    }

    fn register_verb(&mut self, _data: &Value) -> Result<()> {
        Err(Error::new(ErrorKind::Internal(
            "vocabulary registration not available in this context".to_string(),
        )))
    }

    fn register_direction(&mut self, _data: &Value) -> Result<()> {
        Err(Error::new(ErrorKind::Internal(
            "vocabulary registration not available in this context".to_string(),
        )))
    }

    fn register_preposition(&mut self, _data: &Value) -> Result<()> {
        Err(Error::new(ErrorKind::Internal(
            "vocabulary registration not available in this context".to_string(),
        )))
    }

    fn register_pronoun(&mut self, _data: &Value) -> Result<()> {
        Err(Error::new(ErrorKind::Internal(
            "vocabulary registration not available in this context".to_string(),
        )))
    }

    fn register_adverb(&mut self, _data: &Value) -> Result<()> {
        Err(Error::new(ErrorKind::Internal(
            "vocabulary registration not available in this context".to_string(),
        )))
    }

    fn register_type(&mut self, _data: &Value) -> Result<()> {
        Err(Error::new(ErrorKind::Internal(
            "type registration not available in this context".to_string(),
        )))
    }

    fn register_scope(&mut self, _data: &Value) -> Result<()> {
        Err(Error::new(ErrorKind::Internal(
            "scope registration not available in this context".to_string(),
        )))
    }

    fn register_command(&mut self, _data: &Value) -> Result<()> {
        Err(Error::new(ErrorKind::Internal(
            "command registration not available in this context".to_string(),
        )))
    }

    fn register_action(&mut self, _data: &Value) -> Result<()> {
        Err(Error::new(ErrorKind::Internal(
            "action registration not available in this context".to_string(),
        )))
    }

    fn register_rule(&mut self, _data: &Value) -> Result<EntityId> {
        Err(Error::new(ErrorKind::Internal(
            "rule registration not available in this context".to_string(),
        )))
    }

    fn intern_keyword(&mut self, _name: &str) -> KeywordId {
        // This should never be called in NoRuntimeContext
        panic!("intern_keyword not available in NoRuntimeContext")
    }
}
