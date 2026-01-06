//! VM context traits and types for World integration.
//!
//! The VM can optionally access a World via the [`VmContext`] trait. This enables
//! execution of ECS operations (reading components, spawning entities, etc.).

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
}
