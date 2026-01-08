//! Entity lifecycle management with generational indices.
//!
//! The `EntityStore` manages entity allocation and tracks generations
//! to detect stale references to destroyed entities.

// Allow u64 to usize casts - we target 64-bit systems
#![allow(clippy::cast_possible_truncation)]

use longtable_foundation::{EntityId, Error, Result};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Manages entity lifecycle and generation tracking.
///
/// Entities are allocated from a free list when available, otherwise
/// new indices are allocated. When an entity is destroyed, its index
/// is added to the free list and its generation is incremented.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EntityStore {
    /// Generation counter for each entity index.
    /// Even generations are free, odd generations are alive.
    generations: Vec<u32>,
    /// Free list of indices available for reuse.
    free_list: Vec<u64>,
    /// Count of live entities.
    live_count: usize,
}

impl Default for EntityStore {
    fn default() -> Self {
        Self::new()
    }
}

impl EntityStore {
    /// Creates a new empty entity store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            generations: Vec::new(),
            free_list: Vec::new(),
            live_count: 0,
        }
    }

    /// Spawns a new entity, returns its ID.
    ///
    /// Reuses indices from the free list when available.
    pub fn spawn(&mut self) -> EntityId {
        self.live_count += 1;

        if let Some(index) = self.free_list.pop() {
            // Reuse an index from the free list
            let idx = index as usize;
            // Increment generation (was even/free, now odd/alive)
            self.generations[idx] += 1;
            EntityId::new(index, self.generations[idx])
        } else {
            // Allocate a new index
            let index = self.generations.len() as u64;
            // New entities start at generation 1 (odd = alive)
            self.generations.push(1);
            EntityId::new(index, 1)
        }
    }

    /// Destroys an entity.
    ///
    /// Returns `Ok(())` if the entity existed and was destroyed.
    /// Returns `Err` if the entity is stale or already destroyed.
    pub fn destroy(&mut self, id: EntityId) -> Result<()> {
        self.validate(id)?;

        let idx = id.index as usize;
        // Increment generation (was odd/alive, now even/free)
        self.generations[idx] += 1;
        self.free_list.push(id.index);
        self.live_count -= 1;

        Ok(())
    }

    /// Checks if an entity exists and is not stale.
    #[must_use]
    pub fn exists(&self, id: EntityId) -> bool {
        let idx = id.index as usize;
        if idx >= self.generations.len() {
            return false;
        }
        // Entity is alive if generation matches and is odd
        self.generations[idx] == id.generation && id.generation % 2 == 1
    }

    /// Validates that an entity is live.
    ///
    /// Returns `Ok(())` if the entity exists.
    /// Returns `Err` with context if the entity is stale or never existed.
    pub fn validate(&self, id: EntityId) -> Result<()> {
        let idx = id.index as usize;

        if idx >= self.generations.len() {
            return Err(Error::entity_not_found(id));
        }

        let current_gen = self.generations[idx];

        if current_gen != id.generation {
            // Generation mismatch - entity was destroyed and possibly reused
            return Err(Error::stale_entity(id));
        }

        if current_gen % 2 == 0 {
            // Even generation means the slot is free
            return Err(Error::entity_not_found(id));
        }

        Ok(())
    }

    /// Returns the total number of live entities.
    #[must_use]
    pub fn len(&self) -> usize {
        self.live_count
    }

    /// Returns true if there are no live entities.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.live_count == 0
    }

    /// Iterates over all live entity IDs.
    pub fn iter(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.generations
            .iter()
            .enumerate()
            .filter(|(_, generation)| *generation % 2 == 1) // Odd generation = alive
            .map(|(idx, generation)| EntityId::new(idx as u64, *generation))
    }

    /// Returns the current generation for an index, if it exists.
    ///
    /// This is useful for debugging and testing.
    #[must_use]
    pub fn generation(&self, index: u64) -> Option<u32> {
        self.generations.get(index as usize).copied()
    }

    /// Returns the generations slice for content hashing.
    ///
    /// The generations array is deterministically ordered by entity index.
    #[must_use]
    pub fn generations(&self) -> &[u32] {
        &self.generations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use longtable_foundation::ErrorKind;

    #[test]
    fn spawn_creates_unique_entities() {
        let mut store = EntityStore::new();

        let e1 = store.spawn();
        let e2 = store.spawn();
        let e3 = store.spawn();

        assert_ne!(e1, e2);
        assert_ne!(e2, e3);
        assert_ne!(e1, e3);
    }

    #[test]
    fn spawn_increments_index() {
        let mut store = EntityStore::new();

        let e1 = store.spawn();
        let e2 = store.spawn();
        let e3 = store.spawn();

        assert_eq!(e1.index, 0);
        assert_eq!(e2.index, 1);
        assert_eq!(e3.index, 2);
    }

    #[test]
    fn new_entities_have_generation_1() {
        let mut store = EntityStore::new();

        let e1 = store.spawn();
        let e2 = store.spawn();

        assert_eq!(e1.generation, 1);
        assert_eq!(e2.generation, 1);
    }

    #[test]
    fn exists_returns_true_for_live_entity() {
        let mut store = EntityStore::new();
        let e = store.spawn();

        assert!(store.exists(e));
    }

    #[test]
    fn exists_returns_false_for_destroyed_entity() {
        let mut store = EntityStore::new();
        let e = store.spawn();
        store.destroy(e).unwrap();

        assert!(!store.exists(e));
    }

    #[test]
    fn exists_returns_false_for_never_created_entity() {
        let store = EntityStore::new();
        let fake = EntityId::new(999, 1);

        assert!(!store.exists(fake));
    }

    #[test]
    fn destroy_increments_generation() {
        let mut store = EntityStore::new();
        let e = store.spawn();
        assert_eq!(e.generation, 1);

        store.destroy(e).unwrap();
        assert_eq!(store.generation(e.index), Some(2));
    }

    #[test]
    fn destroy_returns_error_for_stale_entity() {
        let mut store = EntityStore::new();
        let e = store.spawn();
        store.destroy(e).unwrap();

        let result = store.destroy(e);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err().kind,
            ErrorKind::StaleEntity(_)
        ));
    }

    #[test]
    fn spawn_reuses_freed_indices() {
        let mut store = EntityStore::new();

        let e1 = store.spawn();
        let _e2 = store.spawn();
        store.destroy(e1).unwrap();

        let e3 = store.spawn();

        // e3 should reuse e1's index with incremented generation
        assert_eq!(e3.index, e1.index);
        assert_eq!(e3.generation, 3); // Was 1, became 2 on destroy, became 3 on respawn
        assert_ne!(e3, e1); // But they're different entities
    }

    #[test]
    fn len_tracks_live_count() {
        let mut store = EntityStore::new();
        assert_eq!(store.len(), 0);

        let e1 = store.spawn();
        assert_eq!(store.len(), 1);

        let _e2 = store.spawn();
        assert_eq!(store.len(), 2);

        store.destroy(e1).unwrap();
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn iter_yields_only_live_entities() {
        let mut store = EntityStore::new();

        let e1 = store.spawn();
        let e2 = store.spawn();
        let e3 = store.spawn();
        store.destroy(e2).unwrap();

        let live: Vec<_> = store.iter().collect();
        assert_eq!(live.len(), 2);
        assert!(live.contains(&e1));
        assert!(live.contains(&e3));
        assert!(!live.contains(&e2));
    }

    #[test]
    fn validate_returns_ok_for_live_entity() {
        let mut store = EntityStore::new();
        let e = store.spawn();

        assert!(store.validate(e).is_ok());
    }

    #[test]
    fn validate_returns_error_for_stale_entity() {
        let mut store = EntityStore::new();
        let e = store.spawn();
        store.destroy(e).unwrap();

        let result = store.validate(e);
        assert!(result.is_err());
    }

    #[test]
    fn validate_returns_error_for_nonexistent_entity() {
        let store = EntityStore::new();
        let fake = EntityId::new(999, 1);

        let result = store.validate(fake);
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn spawned_entities_always_exist(count in 1usize..100) {
            let mut store = EntityStore::new();
            let entities: Vec<_> = (0..count).map(|_| store.spawn()).collect();

            for e in &entities {
                prop_assert!(store.exists(*e));
            }
            prop_assert_eq!(store.len(), count);
        }

        #[test]
        fn destroyed_entities_never_exist(count in 1usize..100) {
            let mut store = EntityStore::new();
            let entities: Vec<_> = (0..count).map(|_| store.spawn()).collect();

            for e in &entities {
                store.destroy(*e).unwrap();
            }

            for e in &entities {
                prop_assert!(!store.exists(*e));
            }
            prop_assert_eq!(store.len(), 0);
        }

        #[test]
        fn reused_indices_have_different_generations(cycles in 1usize..10) {
            let mut store = EntityStore::new();
            let mut prev_gen = 0u32;

            for _ in 0..cycles {
                let e = store.spawn();
                prop_assert!(e.generation > prev_gen);
                prev_gen = e.generation;
                store.destroy(e).unwrap();
            }
        }
    }
}
