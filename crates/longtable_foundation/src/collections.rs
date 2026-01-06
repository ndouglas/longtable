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

    /// Returns a new set that is the difference of this set and another.
    #[must_use]
    pub fn difference(&self, other: &Self) -> Self {
        Self(self.0.clone().difference(other.0.clone()))
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
}
