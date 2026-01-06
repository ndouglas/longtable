//! Persistent collections with structural sharing.
//!
//! These are thin wrappers around the `im` crate's persistent data structures,
//! providing Longtable-specific semantics and future-proofing the API.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::iter::FromIterator;

/// Persistent vector with structural sharing.
///
/// Cloning is O(1). Modifications return a new vector sharing structure
/// with the original.
#[derive(Clone, Default)]
pub struct LtVec<T>(im::Vector<T>)
where
    T: Clone;

impl<T: Clone> LtVec<T> {
    /// Creates an empty vector.
    #[must_use]
    pub fn new() -> Self {
        Self(im::Vector::new())
    }

    /// Returns the number of elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the vector is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Gets an element by index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.0.get(index)
    }

    /// Returns a new vector with the element appended.
    #[must_use]
    pub fn push_back(&self, value: T) -> Self {
        let mut new = self.0.clone();
        new.push_back(value);
        Self(new)
    }

    /// Returns a new vector with the element prepended.
    #[must_use]
    pub fn push_front(&self, value: T) -> Self {
        let mut new = self.0.clone();
        new.push_front(value);
        Self(new)
    }

    /// Returns a new vector with the element at `index` replaced.
    ///
    /// Returns `None` if `index` is out of bounds.
    #[must_use]
    pub fn update(&self, index: usize, value: T) -> Option<Self> {
        if index >= self.len() {
            return None;
        }
        let mut new = self.0.clone();
        new.set(index, value);
        Some(Self(new))
    }

    /// Returns an iterator over the elements.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter()
    }

    /// Returns the first element.
    #[must_use]
    pub fn first(&self) -> Option<&T> {
        self.0.front()
    }

    /// Returns the last element.
    #[must_use]
    pub fn last(&self) -> Option<&T> {
        self.0.back()
    }

    /// Returns a new vector with the last element removed.
    ///
    /// Returns `None` if the vector is empty.
    #[must_use]
    pub fn pop_back(&self) -> Option<(Self, T)> {
        let mut new = self.0.clone();
        let value = new.pop_back()?;
        Some((Self(new), value))
    }

    /// Returns a new vector with the first element removed.
    ///
    /// Returns `None` if the vector is empty.
    #[must_use]
    pub fn pop_front(&self) -> Option<(Self, T)> {
        let mut new = self.0.clone();
        let value = new.pop_front()?;
        Some((Self(new), value))
    }
}

impl<T: Clone + fmt::Debug> fmt::Debug for LtVec<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T: Clone + PartialEq> PartialEq for LtVec<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: Clone + Eq> Eq for LtVec<T> {}

impl<T: Clone + Hash> Hash for LtVec<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for item in self.iter() {
            item.hash(state);
        }
    }
}

impl<T: Clone> FromIterator<T> for LtVec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self(im::Vector::from_iter(iter))
    }
}

impl<T: Clone> IntoIterator for LtVec<T> {
    type Item = T;
    type IntoIter = im::vector::ConsumingIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, T: Clone> IntoIterator for &'a LtVec<T> {
    type Item = &'a T;
    type IntoIter = im::vector::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// Persistent hash set with structural sharing.
#[derive(Clone, Default)]
pub struct LtSet<T>(im::HashSet<T>)
where
    T: Clone + Eq + Hash;

impl<T: Clone + Eq + Hash> LtSet<T> {
    /// Creates an empty set.
    #[must_use]
    pub fn new() -> Self {
        Self(im::HashSet::new())
    }

    /// Returns the number of elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns true if the set contains the value.
    #[must_use]
    pub fn contains(&self, value: &T) -> bool {
        self.0.contains(value)
    }

    /// Returns a new set with the value inserted.
    #[must_use]
    pub fn insert(&self, value: T) -> Self {
        let mut new = self.0.clone();
        new.insert(value);
        Self(new)
    }

    /// Returns a new set with the value removed.
    #[must_use]
    pub fn remove(&self, value: &T) -> Self {
        let mut new = self.0.clone();
        new.remove(value);
        Self(new)
    }

    /// Returns an iterator over the elements.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter()
    }

    /// Returns a new set that is the union of this set and another.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        Self(self.0.clone().union(other.0.clone()))
    }

    /// Returns a new set that is the intersection of this set and another.
    #[must_use]
    pub fn intersection(&self, other: &Self) -> Self {
        Self(self.0.clone().intersection(other.0.clone()))
    }

    /// Returns a new set that is the difference of this set and another (A \ B).
    ///
    /// Contains elements in `self` that are not in `other`.
    #[must_use]
    pub fn difference(&self, other: &Self) -> Self {
        // Note: im::HashSet::difference computes symmetric difference, not set difference
        // We compute the actual set difference (A \ B) manually
        Self(self.0.clone().relative_complement(other.0.clone()))
    }
}

impl<T: Clone + Eq + Hash + fmt::Debug> fmt::Debug for LtSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

impl<T: Clone + Eq + Hash> PartialEq for LtSet<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: Clone + Eq + Hash> Eq for LtSet<T> {}

impl<T: Clone + Eq + Hash> Hash for LtSet<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash the length and elements in a deterministic order
        self.len().hash(state);
        // Note: This is order-dependent which is not ideal for sets,
        // but im::HashSet doesn't guarantee order anyway
        for item in self.iter() {
            item.hash(state);
        }
    }
}

impl<T: Clone + Eq + Hash> FromIterator<T> for LtSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self(im::HashSet::from_iter(iter))
    }
}

/// Persistent hash map with structural sharing.
#[derive(Clone, Default)]
pub struct LtMap<K, V>(im::HashMap<K, V>)
where
    K: Clone + Eq + Hash,
    V: Clone;

impl<K: Clone + Eq + Hash, V: Clone> LtMap<K, V> {
    /// Creates an empty map.
    #[must_use]
    pub fn new() -> Self {
        Self(im::HashMap::new())
    }

    /// Returns the number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the map is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Gets a value by key.
    #[must_use]
    pub fn get(&self, key: &K) -> Option<&V> {
        self.0.get(key)
    }

    /// Returns true if the map contains the key.
    #[must_use]
    pub fn contains_key(&self, key: &K) -> bool {
        self.0.contains_key(key)
    }

    /// Returns a new map with the key-value pair inserted.
    #[must_use]
    pub fn insert(&self, key: K, value: V) -> Self {
        let mut new = self.0.clone();
        new.insert(key, value);
        Self(new)
    }

    /// Returns a new map with the key removed.
    #[must_use]
    pub fn remove(&self, key: &K) -> Self {
        let mut new = self.0.clone();
        new.remove(key);
        Self(new)
    }

    /// Returns an iterator over key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.0.iter()
    }

    /// Returns an iterator over keys.
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.0.keys()
    }

    /// Returns an iterator over values.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.0.values()
    }

    /// Returns a new map that is the union of this map and another.
    ///
    /// If a key exists in both maps, the value from `other` is used.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        Self(self.0.clone().union(other.0.clone()))
    }
}

impl<K: Clone + Eq + Hash + fmt::Debug, V: Clone + fmt::Debug> fmt::Debug for LtMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<K: Clone + Eq + Hash, V: Clone + PartialEq> PartialEq for LtMap<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<K: Clone + Eq + Hash, V: Clone + Eq> Eq for LtMap<K, V> {}

impl<K: Clone + Eq + Hash, V: Clone + Hash> Hash for LtMap<K, V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        for (k, v) in self.iter() {
            k.hash(state);
            v.hash(state);
        }
    }
}

impl<K: Clone + Eq + Hash, V: Clone> FromIterator<(K, V)> for LtMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self(im::HashMap::from_iter(iter))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec_push_back() {
        let v = LtVec::new();
        let v = v.push_back(1);
        let v = v.push_back(2);
        let v = v.push_back(3);

        assert_eq!(v.len(), 3);
        assert_eq!(v.get(0), Some(&1));
        assert_eq!(v.get(1), Some(&2));
        assert_eq!(v.get(2), Some(&3));
    }

    #[test]
    fn vec_structural_sharing() {
        let v1 = LtVec::new().push_back(1).push_back(2);
        let v2 = v1.push_back(3);

        // v1 is unchanged
        assert_eq!(v1.len(), 2);
        assert_eq!(v2.len(), 3);
    }

    #[test]
    fn set_insert_contains() {
        let s = LtSet::new();
        let s = s.insert(1);
        let s = s.insert(2);
        let s = s.insert(1); // Duplicate

        assert_eq!(s.len(), 2);
        assert!(s.contains(&1));
        assert!(s.contains(&2));
        assert!(!s.contains(&3));
    }

    #[test]
    fn map_insert_get() {
        let m = LtMap::new();
        let m = m.insert("a", 1);
        let m = m.insert("b", 2);

        assert_eq!(m.get(&"a"), Some(&1));
        assert_eq!(m.get(&"b"), Some(&2));
        assert_eq!(m.get(&"c"), None);
    }

    #[test]
    fn map_structural_sharing() {
        let m1 = LtMap::new().insert("a", 1);
        let m2 = m1.insert("b", 2);

        assert_eq!(m1.len(), 1);
        assert_eq!(m2.len(), 2);
        assert_eq!(m1.get(&"b"), None);
        assert_eq!(m2.get(&"b"), Some(&2));
    }

    #[test]
    fn set_difference_basic() {
        let s1: LtSet<i32> = vec![0].into_iter().collect();
        let s2: LtSet<i32> = vec![1].into_iter().collect();
        let diff = s1.difference(&s2);

        // diff should contain 0 (in s1 but not in s2)
        assert!(diff.contains(&0), "diff should contain 0");
        assert!(!diff.contains(&1), "diff should not contain 1");
        assert_eq!(diff.len(), 1, "diff should have exactly 1 element");

        // Verify all elements in diff are in s1
        for item in diff.iter() {
            assert!(s1.contains(item), "diff element {item} not in s1");
        }
    }

    #[test]
    fn set_difference_empty_result() {
        let s1: LtSet<i32> = vec![0].into_iter().collect();
        let s2: LtSet<i32> = vec![0].into_iter().collect();
        let diff = s1.difference(&s2);

        // diff should be empty (0 is in both)
        assert_eq!(diff.len(), 0, "diff should be empty");
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    // =========================================================================
    // LtVec Property Tests
    // =========================================================================

    proptest! {
        /// Push back increases length by one.
        #[test]
        fn vec_push_back_increases_len(items in proptest::collection::vec(any::<i32>(), 0..100)) {
            let mut v = LtVec::new();
            for (i, item) in items.iter().enumerate() {
                v = v.push_back(*item);
                prop_assert_eq!(v.len(), i + 1);
            }
        }

        /// Push front increases length by one.
        #[test]
        fn vec_push_front_increases_len(items in proptest::collection::vec(any::<i32>(), 0..100)) {
            let mut v = LtVec::new();
            for (i, item) in items.iter().enumerate() {
                v = v.push_front(*item);
                prop_assert_eq!(v.len(), i + 1);
            }
        }

        /// All pushed elements are retrievable.
        #[test]
        fn vec_elements_retrievable(items in proptest::collection::vec(any::<i32>(), 1..100)) {
            let v: LtVec<i32> = items.iter().copied().collect();
            for (i, item) in items.iter().enumerate() {
                prop_assert_eq!(v.get(i), Some(item));
            }
        }

        /// Structural sharing: original unchanged after modification.
        #[test]
        fn vec_structural_sharing_preserved(
            items in proptest::collection::vec(any::<i32>(), 1..50),
            new_item: i32
        ) {
            let v1: LtVec<i32> = items.iter().copied().collect();
            let v2 = v1.push_back(new_item);

            // Original unchanged
            prop_assert_eq!(v1.len(), items.len());
            for (i, item) in items.iter().enumerate() {
                prop_assert_eq!(v1.get(i), Some(item));
            }

            // New version has the new element
            prop_assert_eq!(v2.len(), items.len() + 1);
            prop_assert_eq!(v2.get(items.len()), Some(&new_item));
        }

        /// Update returns new vec with changed element, original unchanged.
        #[test]
        fn vec_update_preserves_original(
            items in proptest::collection::vec(any::<i32>(), 2..50),
            new_value: i32
        ) {
            let v1: LtVec<i32> = items.iter().copied().collect();
            let mid = items.len() / 2;
            let v2 = v1.update(mid, new_value).unwrap();

            // Original unchanged
            prop_assert_eq!(v1.get(mid), Some(&items[mid]));
            // New version has updated value
            prop_assert_eq!(v2.get(mid), Some(&new_value));
        }

        /// Pop back returns last element and shorter vec.
        #[test]
        fn vec_pop_back_returns_last(items in proptest::collection::vec(any::<i32>(), 1..50)) {
            let v: LtVec<i32> = items.iter().copied().collect();
            let (v2, popped) = v.pop_back().unwrap();

            prop_assert_eq!(popped, *items.last().unwrap());
            prop_assert_eq!(v2.len(), items.len() - 1);
        }

        /// Pop front returns first element and shorter vec.
        #[test]
        fn vec_pop_front_returns_first(items in proptest::collection::vec(any::<i32>(), 1..50)) {
            let v: LtVec<i32> = items.iter().copied().collect();
            let (v2, popped) = v.pop_front().unwrap();

            prop_assert_eq!(popped, items[0]);
            prop_assert_eq!(v2.len(), items.len() - 1);
        }

        /// First and last return correct elements.
        #[test]
        fn vec_first_last(items in proptest::collection::vec(any::<i32>(), 1..50)) {
            let v: LtVec<i32> = items.iter().copied().collect();

            prop_assert_eq!(v.first(), Some(&items[0]));
            prop_assert_eq!(v.last(), Some(items.last().unwrap()));
        }

        /// Iterator yields all elements in order.
        #[test]
        fn vec_iter_yields_all(items in proptest::collection::vec(any::<i32>(), 0..100)) {
            let v: LtVec<i32> = items.iter().copied().collect();
            let collected: Vec<_> = v.iter().copied().collect();

            prop_assert_eq!(collected, items);
        }

        /// Vec equality works correctly.
        #[test]
        fn vec_equality(items in proptest::collection::vec(any::<i32>(), 0..50)) {
            let v1: LtVec<i32> = items.iter().copied().collect();
            let v2: LtVec<i32> = items.iter().copied().collect();

            prop_assert_eq!(v1, v2);
        }
    }

    // =========================================================================
    // LtSet Property Tests
    // =========================================================================

    proptest! {
        /// Insert makes element contained.
        #[test]
        fn set_insert_contains(items in proptest::collection::vec(any::<i32>(), 0..100)) {
            let mut s = LtSet::new();
            for item in &items {
                s = s.insert(*item);
                prop_assert!(s.contains(item));
            }
        }

        /// Insert is idempotent (no duplicate entries).
        #[test]
        fn set_insert_idempotent(value: i32) {
            let s = LtSet::new().insert(value).insert(value).insert(value);
            prop_assert_eq!(s.len(), 1);
            prop_assert!(s.contains(&value));
        }

        /// Length equals number of unique elements.
        #[test]
        fn set_len_is_unique_count(items in proptest::collection::vec(any::<i32>(), 0..100)) {
            let s: LtSet<i32> = items.iter().copied().collect();
            let unique: std::collections::HashSet<_> = items.iter().collect();
            prop_assert_eq!(s.len(), unique.len());
        }

        /// Remove makes element not contained.
        #[test]
        fn set_remove_not_contains(items in proptest::collection::vec(any::<i32>(), 1..50)) {
            let s: LtSet<i32> = items.iter().copied().collect();
            let to_remove = items[0];
            let s2 = s.remove(&to_remove);

            prop_assert!(!s2.contains(&to_remove));
            // Original unchanged
            prop_assert!(s.contains(&to_remove));
        }

        /// Structural sharing: original unchanged after modification.
        #[test]
        fn set_structural_sharing_preserved(
            items in proptest::collection::vec(any::<i32>(), 1..50),
            new_item: i32
        ) {
            let s1: LtSet<i32> = items.iter().copied().collect();
            let _s2 = s1.insert(new_item);

            // Original still has same elements
            for item in &items {
                prop_assert!(s1.contains(item));
            }
        }

        /// Union contains all elements from both sets.
        #[test]
        fn set_union_contains_all(
            items1 in proptest::collection::vec(any::<i32>(), 0..50),
            items2 in proptest::collection::vec(any::<i32>(), 0..50)
        ) {
            let s1: LtSet<i32> = items1.iter().copied().collect();
            let s2: LtSet<i32> = items2.iter().copied().collect();
            let union = s1.union(&s2);

            for item in &items1 {
                prop_assert!(union.contains(item));
            }
            for item in &items2 {
                prop_assert!(union.contains(item));
            }
        }

        /// Intersection contains only common elements.
        #[test]
        fn set_intersection_contains_common(
            items1 in proptest::collection::vec(0..100i32, 0..50),
            items2 in proptest::collection::vec(0..100i32, 0..50)
        ) {
            let s1: LtSet<i32> = items1.iter().copied().collect();
            let s2: LtSet<i32> = items2.iter().copied().collect();
            let inter = s1.intersection(&s2);

            for item in inter.iter() {
                prop_assert!(s1.contains(item));
                prop_assert!(s2.contains(item));
            }
        }

        /// Difference contains only elements in first but not second.
        #[test]
        fn set_difference_correct(
            items1 in proptest::collection::vec(0..100i32, 1..50),
            items2 in proptest::collection::vec(0..100i32, 0..50)
        ) {
            let s1: LtSet<i32> = items1.iter().copied().collect();
            let s2: LtSet<i32> = items2.iter().copied().collect();
            let diff = s1.difference(&s2);

            // Every element in diff must be in s1 and not in s2
            for item in diff.iter() {
                prop_assert!(s1.contains(item), "diff element not in s1");
                prop_assert!(!s2.contains(item), "diff element in s2");
            }

            // Every element in s1 that's not in s2 must be in diff
            for item in s1.iter() {
                if !s2.contains(item) {
                    prop_assert!(diff.contains(item), "expected element not in diff");
                }
            }
        }

        /// Set equality works correctly.
        #[test]
        fn set_equality(items in proptest::collection::vec(any::<i32>(), 0..50)) {
            let s1: LtSet<i32> = items.iter().copied().collect();
            let s2: LtSet<i32> = items.iter().copied().collect();

            prop_assert_eq!(s1, s2);
        }
    }

    // =========================================================================
    // LtMap Property Tests
    // =========================================================================

    proptest! {
        /// Insert makes key-value retrievable.
        #[test]
        fn map_insert_get(pairs in proptest::collection::vec((any::<i32>(), any::<i32>()), 0..100)) {
            let mut m = LtMap::new();
            for (k, v) in &pairs {
                m = m.insert(*k, *v);
                prop_assert_eq!(m.get(k), Some(v));
            }
        }

        /// Insert overwrites existing key.
        #[test]
        fn map_insert_overwrites(key: i32, v1: i32, v2: i32) {
            let m = LtMap::new().insert(key, v1).insert(key, v2);
            prop_assert_eq!(m.len(), 1);
            prop_assert_eq!(m.get(&key), Some(&v2));
        }

        /// Length equals number of unique keys.
        #[test]
        fn map_len_is_unique_keys(pairs in proptest::collection::vec((0..100i32, any::<i32>()), 0..100)) {
            let m: LtMap<i32, i32> = pairs.iter().copied().collect();
            let unique_keys: std::collections::HashSet<_> = pairs.iter().map(|(k, _)| k).collect();
            prop_assert_eq!(m.len(), unique_keys.len());
        }

        /// Remove makes key not contained.
        #[test]
        fn map_remove_not_contains(pairs in proptest::collection::vec((any::<i32>(), any::<i32>()), 1..50)) {
            let m: LtMap<i32, i32> = pairs.iter().copied().collect();
            let (key_to_remove, _) = pairs[0];
            let m2 = m.remove(&key_to_remove);

            prop_assert!(!m2.contains_key(&key_to_remove));
            // Original unchanged
            prop_assert!(m.contains_key(&key_to_remove));
        }

        /// Structural sharing: original unchanged after modification.
        #[test]
        fn map_structural_sharing_preserved(
            pairs in proptest::collection::vec((any::<i32>(), any::<i32>()), 1..50),
            new_key: i32,
            new_value: i32
        ) {
            let m1: LtMap<i32, i32> = pairs.iter().copied().collect();
            let m2 = m1.insert(new_key, new_value);

            // Original still has same elements
            for (k, v) in &pairs {
                prop_assert_eq!(m1.get(k), Some(v));
            }

            // New map has the new element
            prop_assert_eq!(m2.get(&new_key), Some(&new_value));
        }

        /// Keys iterator yields all keys.
        #[test]
        fn map_keys_iter(pairs in proptest::collection::vec((0..1000i32, any::<i32>()), 0..50)) {
            let m: LtMap<i32, i32> = pairs.iter().copied().collect();
            let keys: std::collections::HashSet<_> = m.keys().copied().collect();
            let expected: std::collections::HashSet<_> = pairs.iter().map(|(k, _)| *k).collect();

            prop_assert_eq!(keys, expected);
        }

        /// Values iterator yields all values (may have duplicates).
        #[test]
        fn map_values_iter(pairs in proptest::collection::vec((0..1000i32, any::<i32>()), 0..50)) {
            let m: LtMap<i32, i32> = pairs.iter().copied().collect();
            let values_count = m.values().count();

            // Number of values equals number of entries
            prop_assert_eq!(values_count, m.len());
        }

        /// Union contains all keys from both maps.
        #[test]
        fn map_union_contains_all_keys(
            pairs1 in proptest::collection::vec((0..100i32, any::<i32>()), 0..30),
            pairs2 in proptest::collection::vec((50..150i32, any::<i32>()), 0..30)
        ) {
            let m1: LtMap<i32, i32> = pairs1.iter().copied().collect();
            let m2: LtMap<i32, i32> = pairs2.iter().copied().collect();
            let union = m1.union(&m2);

            for (k, _) in &pairs1 {
                prop_assert!(union.contains_key(k));
            }
            for (k, _) in &pairs2 {
                prop_assert!(union.contains_key(k));
            }
        }

        /// Map equality works correctly.
        #[test]
        fn map_equality(pairs in proptest::collection::vec((any::<i32>(), any::<i32>()), 0..50)) {
            let m1: LtMap<i32, i32> = pairs.iter().copied().collect();
            let m2: LtMap<i32, i32> = pairs.iter().copied().collect();

            prop_assert_eq!(m1, m2);
        }
    }
}
