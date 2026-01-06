//! Relationship storage with bidirectional indices.
//!
//! Relationships connect entities with typed edges. Bidirectional indices
//! allow O(1) traversal in both directions.

use std::collections::{HashMap, HashSet};

use longtable_foundation::{EntityId, Error, ErrorKind, KeywordId, Result};

use crate::schema::{Cardinality, OnDelete, OnViolation, RelationshipSchema};

/// Stores relationship edges between entities.
///
/// Maintains bidirectional indices for efficient traversal:
/// - Forward: source -> relationship -> targets
/// - Reverse: target -> relationship -> sources
#[derive(Clone, Debug, Default)]
pub struct RelationshipStore {
    /// Registered schemas by relationship name.
    schemas: HashMap<KeywordId, RelationshipSchema>,
    /// Forward index: source -> relationship -> set of targets.
    forward: HashMap<EntityId, HashMap<KeywordId, HashSet<EntityId>>>,
    /// Reverse index: target -> relationship -> set of sources.
    reverse: HashMap<EntityId, HashMap<KeywordId, HashSet<EntityId>>>,
}

impl RelationshipStore {
    /// Creates a new empty relationship store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a relationship schema.
    ///
    /// # Errors
    ///
    /// Returns an error if a schema with the same name is already registered.
    pub fn register_schema(&mut self, schema: RelationshipSchema) -> Result<()> {
        if self.schemas.contains_key(&schema.name) {
            return Err(Error::new(ErrorKind::Internal(format!(
                "relationship schema already registered: {:?}",
                schema.name
            ))));
        }
        self.schemas.insert(schema.name, schema);
        Ok(())
    }

    /// Gets the schema for a relationship type.
    #[must_use]
    pub fn schema(&self, relationship: KeywordId) -> Option<&RelationshipSchema> {
        self.schemas.get(&relationship)
    }

    /// Creates a relationship edge.
    ///
    /// Linking an existing edge is idempotent (no-op).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The relationship is not registered
    /// - Cardinality would be violated and `on_violation` is `Error`
    #[allow(clippy::too_many_lines)]
    pub fn link(
        &mut self,
        source: EntityId,
        relationship: KeywordId,
        target: EntityId,
    ) -> Result<()> {
        let schema = self.schema(relationship).ok_or_else(|| {
            Error::new(ErrorKind::Internal(format!(
                "unknown relationship: {relationship:?}"
            )))
        })?;
        let cardinality = schema.cardinality;
        let on_violation = schema.on_violation;

        // Check for existing edge (idempotent)
        if self.has_edge(source, relationship, target) {
            return Ok(());
        }

        // Check cardinality constraints
        match cardinality {
            Cardinality::OneToOne => {
                // Source can have at most one target
                if let Some(existing_targets) =
                    self.forward.get(&source).and_then(|m| m.get(&relationship))
                {
                    if !existing_targets.is_empty() {
                        match on_violation {
                            OnViolation::Error => {
                                return Err(Error::new(ErrorKind::Internal(
                                    "cardinality violation: source already has a target"
                                        .to_string(),
                                )));
                            }
                            OnViolation::Replace => {
                                // Remove existing edges from this source
                                let old_targets: Vec<_> =
                                    existing_targets.iter().copied().collect();
                                for old_target in old_targets {
                                    self.unlink_internal(source, relationship, old_target);
                                }
                            }
                        }
                    }
                }
                // Target can have at most one source
                if let Some(existing_sources) =
                    self.reverse.get(&target).and_then(|m| m.get(&relationship))
                {
                    if !existing_sources.is_empty() {
                        match on_violation {
                            OnViolation::Error => {
                                return Err(Error::new(ErrorKind::Internal(
                                    "cardinality violation: target already has a source"
                                        .to_string(),
                                )));
                            }
                            OnViolation::Replace => {
                                let old_sources: Vec<_> =
                                    existing_sources.iter().copied().collect();
                                for old_source in old_sources {
                                    self.unlink_internal(old_source, relationship, target);
                                }
                            }
                        }
                    }
                }
            }
            Cardinality::ManyToOne => {
                // Source can have at most one target
                if let Some(existing_targets) =
                    self.forward.get(&source).and_then(|m| m.get(&relationship))
                {
                    if !existing_targets.is_empty() {
                        match on_violation {
                            OnViolation::Error => {
                                return Err(Error::new(ErrorKind::Internal(
                                    "cardinality violation: source already has a target"
                                        .to_string(),
                                )));
                            }
                            OnViolation::Replace => {
                                let old_targets: Vec<_> =
                                    existing_targets.iter().copied().collect();
                                for old_target in old_targets {
                                    self.unlink_internal(source, relationship, old_target);
                                }
                            }
                        }
                    }
                }
            }
            Cardinality::OneToMany => {
                // Target can have at most one source
                if let Some(existing_sources) =
                    self.reverse.get(&target).and_then(|m| m.get(&relationship))
                {
                    if !existing_sources.is_empty() {
                        match on_violation {
                            OnViolation::Error => {
                                return Err(Error::new(ErrorKind::Internal(
                                    "cardinality violation: target already has a source"
                                        .to_string(),
                                )));
                            }
                            OnViolation::Replace => {
                                let old_sources: Vec<_> =
                                    existing_sources.iter().copied().collect();
                                for old_source in old_sources {
                                    self.unlink_internal(old_source, relationship, target);
                                }
                            }
                        }
                    }
                }
            }
            Cardinality::ManyToMany => {
                // No constraints
            }
        }

        // Add the edge
        self.forward
            .entry(source)
            .or_default()
            .entry(relationship)
            .or_default()
            .insert(target);
        self.reverse
            .entry(target)
            .or_default()
            .entry(relationship)
            .or_default()
            .insert(source);

        Ok(())
    }

    /// Removes a relationship edge.
    ///
    /// Unlinking a non-existent edge is idempotent (no-op).
    pub fn unlink(&mut self, source: EntityId, relationship: KeywordId, target: EntityId) {
        self.unlink_internal(source, relationship, target);
    }

    fn unlink_internal(&mut self, source: EntityId, relationship: KeywordId, target: EntityId) {
        if let Some(rels) = self.forward.get_mut(&source) {
            if let Some(targets) = rels.get_mut(&relationship) {
                targets.remove(&target);
            }
        }
        if let Some(rels) = self.reverse.get_mut(&target) {
            if let Some(sources) = rels.get_mut(&relationship) {
                sources.remove(&source);
            }
        }
    }

    /// Gets targets of a relationship from a source (forward traversal).
    pub fn targets(
        &self,
        source: EntityId,
        relationship: KeywordId,
    ) -> impl Iterator<Item = EntityId> + '_ {
        self.forward
            .get(&source)
            .and_then(|m| m.get(&relationship))
            .into_iter()
            .flat_map(|s| s.iter().copied())
    }

    /// Gets sources pointing to a target (reverse traversal).
    pub fn sources(
        &self,
        target: EntityId,
        relationship: KeywordId,
    ) -> impl Iterator<Item = EntityId> + '_ {
        self.reverse
            .get(&target)
            .and_then(|m| m.get(&relationship))
            .into_iter()
            .flat_map(|s| s.iter().copied())
    }

    /// Checks if a specific edge exists.
    #[must_use]
    pub fn has_edge(&self, source: EntityId, relationship: KeywordId, target: EntityId) -> bool {
        self.forward
            .get(&source)
            .and_then(|m| m.get(&relationship))
            .is_some_and(|s| s.contains(&target))
    }

    /// Handles entity destruction according to `on_target_delete` policies.
    ///
    /// Returns a list of entities that should be cascade-deleted.
    pub fn on_entity_destroyed(&mut self, entity: EntityId) -> Vec<EntityId> {
        let mut cascade_victims = Vec::new();

        // Process edges where this entity is the target
        if let Some(reverse_rels) = self.reverse.remove(&entity) {
            for (rel_id, sources) in reverse_rels {
                if let Some(schema) = self.schemas.get(&rel_id) {
                    match schema.on_target_delete {
                        OnDelete::Remove => {
                            // Just remove the edges (already done by removing from reverse)
                            for source in sources {
                                if let Some(fwd) = self.forward.get_mut(&source) {
                                    if let Some(targets) = fwd.get_mut(&rel_id) {
                                        targets.remove(&entity);
                                    }
                                }
                            }
                        }
                        OnDelete::Cascade => {
                            // Mark sources for deletion
                            for source in sources {
                                if let Some(fwd) = self.forward.get_mut(&source) {
                                    if let Some(targets) = fwd.get_mut(&rel_id) {
                                        targets.remove(&entity);
                                    }
                                }
                                cascade_victims.push(source);
                            }
                        }
                        OnDelete::Nullify => {
                            // For Field storage, would set the field to nil
                            // For now, just remove the edges
                            for source in sources {
                                if let Some(fwd) = self.forward.get_mut(&source) {
                                    if let Some(targets) = fwd.get_mut(&rel_id) {
                                        targets.remove(&entity);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Process edges where this entity is the source
        if let Some(forward_rels) = self.forward.remove(&entity) {
            for (rel_id, targets) in forward_rels {
                for target in targets {
                    if let Some(rev) = self.reverse.get_mut(&target) {
                        if let Some(sources) = rev.get_mut(&rel_id) {
                            sources.remove(&entity);
                        }
                    }
                }
            }
        }

        cascade_victims
    }

    /// Returns all relationships involving an entity.
    #[must_use]
    pub fn relationships_for(&self, entity: EntityId) -> Vec<(KeywordId, EntityId, bool)> {
        let mut result = Vec::new();

        // Forward relationships (entity is source)
        if let Some(fwd) = self.forward.get(&entity) {
            for (rel, targets) in fwd {
                for target in targets {
                    result.push((*rel, *target, true)); // true = forward
                }
            }
        }

        // Reverse relationships (entity is target)
        if let Some(rev) = self.reverse.get(&entity) {
            for (rel, sources) in rev {
                for source in sources {
                    result.push((*rel, *source, false)); // false = reverse
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use longtable_foundation::Interner;

    fn setup() -> (RelationshipStore, Interner) {
        let store = RelationshipStore::new();
        let interner = Interner::new();
        (store, interner)
    }

    #[test]
    fn link_and_check_edge() {
        let (mut store, mut interner) = setup();
        let contains = interner.intern_keyword("contains");

        store
            .register_schema(RelationshipSchema::new(contains))
            .unwrap();

        let room = EntityId::new(0, 1);
        let item = EntityId::new(1, 1);

        store.link(room, contains, item).unwrap();

        assert!(store.has_edge(room, contains, item));
    }

    #[test]
    fn link_is_idempotent() {
        let (mut store, mut interner) = setup();
        let contains = interner.intern_keyword("contains");

        store
            .register_schema(RelationshipSchema::new(contains))
            .unwrap();

        let room = EntityId::new(0, 1);
        let item = EntityId::new(1, 1);

        store.link(room, contains, item).unwrap();
        store.link(room, contains, item).unwrap(); // Should not fail

        let targets: Vec<_> = store.targets(room, contains).collect();
        assert_eq!(targets.len(), 1);
    }

    #[test]
    fn forward_traversal() {
        let (mut store, mut interner) = setup();
        let contains = interner.intern_keyword("contains");

        store
            .register_schema(RelationshipSchema::new(contains))
            .unwrap();

        let room = EntityId::new(0, 1);
        let item1 = EntityId::new(1, 1);
        let item2 = EntityId::new(2, 1);

        store.link(room, contains, item1).unwrap();
        store.link(room, contains, item2).unwrap();

        let targets: HashSet<_> = store.targets(room, contains).collect();
        assert_eq!(targets.len(), 2);
        assert!(targets.contains(&item1));
        assert!(targets.contains(&item2));
    }

    #[test]
    fn reverse_traversal() {
        let (mut store, mut interner) = setup();
        let contains = interner.intern_keyword("contains");

        store
            .register_schema(RelationshipSchema::new(contains))
            .unwrap();

        let room1 = EntityId::new(0, 1);
        let room2 = EntityId::new(1, 1);
        let item = EntityId::new(2, 1);

        store.link(room1, contains, item).unwrap();
        store.link(room2, contains, item).unwrap();

        let sources: HashSet<_> = store.sources(item, contains).collect();
        assert_eq!(sources.len(), 2);
        assert!(sources.contains(&room1));
        assert!(sources.contains(&room2));
    }

    #[test]
    fn unlink_removes_edge() {
        let (mut store, mut interner) = setup();
        let contains = interner.intern_keyword("contains");

        store
            .register_schema(RelationshipSchema::new(contains))
            .unwrap();

        let room = EntityId::new(0, 1);
        let item = EntityId::new(1, 1);

        store.link(room, contains, item).unwrap();
        assert!(store.has_edge(room, contains, item));

        store.unlink(room, contains, item);
        assert!(!store.has_edge(room, contains, item));
    }

    #[test]
    fn unlink_is_idempotent() {
        let (mut store, mut interner) = setup();
        let contains = interner.intern_keyword("contains");

        store
            .register_schema(RelationshipSchema::new(contains))
            .unwrap();

        let room = EntityId::new(0, 1);
        let item = EntityId::new(1, 1);

        // Unlinking non-existent edge should not fail
        store.unlink(room, contains, item);
    }

    #[test]
    fn one_to_one_cardinality_error() {
        let (mut store, mut interner) = setup();
        let parent = interner.intern_keyword("parent");

        store
            .register_schema(
                RelationshipSchema::new(parent)
                    .with_cardinality(Cardinality::OneToOne)
                    .with_on_violation(OnViolation::Error),
            )
            .unwrap();

        let child = EntityId::new(0, 1);
        let parent1 = EntityId::new(1, 1);
        let parent2 = EntityId::new(2, 1);

        store.link(child, parent, parent1).unwrap();
        let result = store.link(child, parent, parent2);

        assert!(result.is_err());
    }

    #[test]
    fn one_to_one_cardinality_replace() {
        let (mut store, mut interner) = setup();
        let parent = interner.intern_keyword("parent");

        store
            .register_schema(
                RelationshipSchema::new(parent)
                    .with_cardinality(Cardinality::OneToOne)
                    .with_on_violation(OnViolation::Replace),
            )
            .unwrap();

        let child = EntityId::new(0, 1);
        let parent1 = EntityId::new(1, 1);
        let parent2 = EntityId::new(2, 1);

        store.link(child, parent, parent1).unwrap();
        store.link(child, parent, parent2).unwrap();

        // Old edge should be gone
        assert!(!store.has_edge(child, parent, parent1));
        // New edge should exist
        assert!(store.has_edge(child, parent, parent2));
    }

    #[test]
    fn on_entity_destroyed_removes_edges() {
        let (mut store, mut interner) = setup();
        let contains = interner.intern_keyword("contains");

        store
            .register_schema(RelationshipSchema::new(contains))
            .unwrap();

        let room = EntityId::new(0, 1);
        let item = EntityId::new(1, 1);

        store.link(room, contains, item).unwrap();
        store.on_entity_destroyed(item);

        assert!(!store.has_edge(room, contains, item));
    }

    #[test]
    fn on_entity_destroyed_cascade() {
        let (mut store, mut interner) = setup();
        let child_of = interner.intern_keyword("child-of");

        store
            .register_schema(RelationshipSchema::new(child_of).with_on_delete(OnDelete::Cascade))
            .unwrap();

        let parent = EntityId::new(0, 1);
        let child = EntityId::new(1, 1);

        store.link(child, child_of, parent).unwrap();
        let victims = store.on_entity_destroyed(parent);

        assert!(victims.contains(&child));
    }
}
