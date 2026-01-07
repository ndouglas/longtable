//! Scope evaluation for noun resolution.
//!
//! Determines which entities are visible for noun resolution based on
//! the actor's location and game state.

use longtable_foundation::{EntityId, KeywordId};

/// A compiled scope definition.
#[derive(Clone, Debug)]
pub struct CompiledScope {
    /// Scope name
    pub name: KeywordId,
    /// Parent scope to extend
    pub parent: Option<KeywordId>,
    // pattern: CompiledPattern, // Added later when we integrate with engine
}

/// Evaluates entity visibility scopes.
pub struct ScopeEvaluator;

impl ScopeEvaluator {
    /// Gets all entities visible to an actor given the scope definitions.
    ///
    /// Default scopes:
    /// - `immediate`: Room contents + inventory
    /// - `visible`: + transparent containers
    /// - `known`: + remembered entities (memory system)
    #[must_use]
    pub fn visible_entities(
        _actor: EntityId,
        // world: &World,
        _scopes: &[CompiledScope],
    ) -> Vec<EntityId> {
        // TODO: Implement scope evaluation
        Vec::new()
    }
}
