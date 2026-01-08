//! Scope evaluation for noun resolution.
//!
//! Determines which entities are visible for noun resolution based on
//! the actor's location and game state.

use std::collections::HashSet;

use longtable_foundation::{EntityId, KeywordId};
use longtable_storage::World;

/// A compiled scope definition.
#[derive(Clone, Debug)]
pub struct CompiledScope {
    /// Scope name
    pub name: KeywordId,
    /// Parent scope to extend
    pub parent: Option<KeywordId>,
    /// Scope kind determines the evaluation strategy
    pub kind: ScopeKind,
}

/// The kind of scope, determining how entities are gathered.
#[derive(Clone, Debug)]
pub enum ScopeKind {
    /// Entities in the same location as the actor
    SameLocation,
    /// Entities in the actor's inventory
    Inventory,
    /// Contents of containers in scope
    ContainerContents {
        /// Only include if container is open
        require_open: bool,
        /// Only include if container is transparent
        require_transparent: bool,
    },
    /// Combined scopes
    Union(Vec<KeywordId>),
    /// Custom pattern-based scope (uses compiled patterns)
    Custom,
}

/// Evaluates entity visibility scopes.
#[derive(Clone, Debug)]
pub struct ScopeEvaluator {
    /// Keyword for location component
    location: KeywordId,
    /// Keyword for inventory relationship
    inventory: KeywordId,
    /// Keyword for container/open component
    container_open: KeywordId,
    /// Keyword for transparent component
    transparent: KeywordId,
    /// Keyword for location/in relationship
    location_in: KeywordId,
}

impl ScopeEvaluator {
    /// Creates a new scope evaluator with the given keywords.
    #[must_use]
    pub fn new(
        location: KeywordId,
        inventory: KeywordId,
        container_open: KeywordId,
        transparent: KeywordId,
        location_in: KeywordId,
    ) -> Self {
        Self {
            location,
            inventory,
            container_open,
            transparent,
            location_in,
        }
    }

    /// Gets all entities visible to an actor given the scope definitions.
    ///
    /// Default scopes:
    /// - `immediate`: Room contents + inventory
    /// - `visible`: immediate + transparent containers
    /// - `reachable`: visible + open containers
    #[must_use]
    pub fn visible_entities(
        &self,
        actor: EntityId,
        world: &World,
        scopes: &[CompiledScope],
    ) -> Vec<EntityId> {
        let mut visible = HashSet::new();

        for scope in scopes {
            self.evaluate_scope(actor, world, scope, scopes, &mut visible);
        }

        visible.into_iter().collect()
    }

    /// Gets entities for a specific named scope.
    #[must_use]
    pub fn entities_in_scope(
        &self,
        actor: EntityId,
        world: &World,
        scope_name: KeywordId,
        scopes: &[CompiledScope],
    ) -> Vec<EntityId> {
        let mut visible = HashSet::new();

        if let Some(scope) = scopes.iter().find(|s| s.name == scope_name) {
            self.evaluate_scope(actor, world, scope, scopes, &mut visible);
        }

        visible.into_iter().collect()
    }

    /// Evaluates a single scope definition.
    fn evaluate_scope(
        &self,
        actor: EntityId,
        world: &World,
        scope: &CompiledScope,
        all_scopes: &[CompiledScope],
        result: &mut HashSet<EntityId>,
    ) {
        // First, include parent scope if any
        if let Some(parent_name) = scope.parent {
            if let Some(parent) = all_scopes.iter().find(|s| s.name == parent_name) {
                self.evaluate_scope(actor, world, parent, all_scopes, result);
            }
        }

        // Then evaluate this scope's kind
        match &scope.kind {
            ScopeKind::SameLocation => {
                self.add_same_location(actor, world, result);
            }
            ScopeKind::Inventory => {
                self.add_inventory(actor, world, result);
            }
            ScopeKind::ContainerContents {
                require_open,
                require_transparent,
            } => {
                self.add_container_contents(world, result, *require_open, *require_transparent);
            }
            ScopeKind::Union(scope_names) => {
                for name in scope_names {
                    if let Some(sub_scope) = all_scopes.iter().find(|s| s.name == *name) {
                        self.evaluate_scope(actor, world, sub_scope, all_scopes, result);
                    }
                }
            }
            ScopeKind::Custom => {
                // Custom scopes need pattern evaluation - placeholder for now
            }
        }
    }

    /// Adds entities in the same location as the actor.
    fn add_same_location(&self, actor: EntityId, world: &World, result: &mut HashSet<EntityId>) {
        // Get actor's location
        if let Ok(Some(longtable_foundation::Value::EntityRef(location))) =
            world.get(actor, self.location)
        {
            // Add all entities in this location
            for entity in world.entities() {
                if entity != actor {
                    if let Ok(Some(longtable_foundation::Value::EntityRef(loc))) =
                        world.get(entity, self.location)
                    {
                        if loc == location {
                            result.insert(entity);
                        }
                    }
                }
            }
        }
    }

    /// Adds entities in the actor's inventory.
    fn add_inventory(&self, actor: EntityId, world: &World, result: &mut HashSet<EntityId>) {
        // Get inventory targets (entities the actor is carrying)
        for entity in world.targets(actor, self.inventory) {
            result.insert(entity);
        }
    }

    /// Adds contents of containers that are in the current result set.
    fn add_container_contents(
        &self,
        world: &World,
        result: &mut HashSet<EntityId>,
        require_open: bool,
        require_transparent: bool,
    ) {
        // Collect containers that are already visible
        let containers: Vec<EntityId> = result.iter().copied().collect();

        for container in containers {
            let should_include = if require_open {
                // Check if container is open
                matches!(
                    world.get(container, self.container_open),
                    Ok(Some(longtable_foundation::Value::Bool(true)))
                )
            } else if require_transparent {
                // Check if container is transparent
                matches!(
                    world.get(container, self.transparent),
                    Ok(Some(longtable_foundation::Value::Bool(true)))
                )
            } else {
                true
            };

            if should_include {
                // Add contents of this container
                for entity in world.targets(container, self.location_in) {
                    result.insert(entity);
                }
            }
        }
    }
}

/// Creates default adventure game scopes.
#[must_use]
pub fn default_scopes(
    immediate: KeywordId,
    visible: KeywordId,
    reachable: KeywordId,
) -> Vec<CompiledScope> {
    vec![
        CompiledScope {
            name: immediate,
            parent: None,
            kind: ScopeKind::Union(vec![]), // Will combine SameLocation + Inventory
        },
        CompiledScope {
            name: visible,
            parent: Some(immediate),
            kind: ScopeKind::ContainerContents {
                require_open: false,
                require_transparent: true,
            },
        },
        CompiledScope {
            name: reachable,
            parent: Some(visible),
            kind: ScopeKind::ContainerContents {
                require_open: true,
                require_transparent: false,
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_kind_variants() {
        let kind = ScopeKind::SameLocation;
        assert!(matches!(kind, ScopeKind::SameLocation));

        let kind = ScopeKind::ContainerContents {
            require_open: true,
            require_transparent: false,
        };
        assert!(matches!(
            kind,
            ScopeKind::ContainerContents {
                require_open: true,
                ..
            }
        ));
    }
}
