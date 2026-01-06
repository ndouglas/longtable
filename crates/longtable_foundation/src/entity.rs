//! Entity identifiers with generational indices.

use std::fmt;

/// Entity identifier with generational index for stale reference detection.
///
/// The generation counter increments when an entity index is reused after destruction,
/// allowing detection of stale references to destroyed entities.
///
/// # Layout
/// - `index`: 64-bit index into entity storage
/// - `generation`: 32-bit generation counter
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct EntityId {
    /// Index into entity storage.
    pub index: u64,
    /// Generation counter for stale reference detection.
    pub generation: u32,
}

impl EntityId {
    /// Creates a new entity ID with the given index and generation.
    #[must_use]
    pub const fn new(index: u64, generation: u32) -> Self {
        Self { index, generation }
    }

    /// Returns a sentinel value representing "no entity".
    ///
    /// This uses `u64::MAX` as the index, which should never be allocated.
    #[must_use]
    pub const fn null() -> Self {
        Self {
            index: u64::MAX,
            generation: 0,
        }
    }

    /// Returns true if this is the null sentinel value.
    #[must_use]
    pub const fn is_null(self) -> bool {
        self.index == u64::MAX
    }
}

impl fmt::Debug for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_null() {
            write!(f, "EntityId(null)")
        } else {
            write!(f, "EntityId({}v{})", self.index, self.generation)
        }
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_null() {
            write!(f, "Entity(null)")
        } else {
            write!(f, "Entity({})", self.index)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_id_equality() {
        let a = EntityId::new(1, 0);
        let b = EntityId::new(1, 0);
        let c = EntityId::new(1, 1);
        let d = EntityId::new(2, 0);

        assert_eq!(a, b);
        assert_ne!(a, c); // Different generation
        assert_ne!(a, d); // Different index
    }

    #[test]
    fn entity_id_null() {
        let null = EntityId::null();
        assert!(null.is_null());

        let normal = EntityId::new(0, 0);
        assert!(!normal.is_null());
    }

    #[test]
    fn entity_id_debug_format() {
        let e = EntityId::new(42, 3);
        assert_eq!(format!("{e:?}"), "EntityId(42v3)");

        let null = EntityId::null();
        assert_eq!(format!("{null:?}"), "EntityId(null)");
    }

    #[test]
    fn entity_id_display_format() {
        let e = EntityId::new(42, 3);
        assert_eq!(format!("{e}"), "Entity(42)");
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn hash_entity(e: &EntityId) -> u64 {
        let mut hasher = DefaultHasher::new();
        e.hash(&mut hasher);
        hasher.finish()
    }

    proptest! {
        #[test]
        fn eq_reflexivity(index in any::<u64>(), generation in any::<u32>()) {
            let e = EntityId::new(index, generation);
            prop_assert_eq!(e, e);
        }

        #[test]
        fn eq_hash_consistency(index in any::<u64>(), generation in any::<u32>()) {
            let e = EntityId::new(index, generation);
            let h1 = hash_entity(&e);
            let h2 = hash_entity(&e);
            prop_assert_eq!(h1, h2);
        }

        #[test]
        fn equality_requires_both_fields(
            idx1 in any::<u64>(),
            idx2 in any::<u64>(),
            gen1 in any::<u32>(),
            gen2 in any::<u32>()
        ) {
            let e1 = EntityId::new(idx1, gen1);
            let e2 = EntityId::new(idx2, gen2);
            if idx1 == idx2 && gen1 == gen2 {
                prop_assert_eq!(e1, e2);
                prop_assert_eq!(hash_entity(&e1), hash_entity(&e2));
            } else {
                prop_assert_ne!(e1, e2);
            }
        }
    }
}
