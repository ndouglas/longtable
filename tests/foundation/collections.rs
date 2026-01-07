//! Integration tests for persistent collections
//!
//! Tests LtVec, LtSet, LtMap with structural sharing and immutability.

use longtable_foundation::Value;
use longtable_foundation::collections::{LtMap, LtSet, LtVec};
use std::sync::Arc;

// =============================================================================
// LtVec
// =============================================================================

#[test]
fn vector_empty() {
    let v: LtVec<Value> = LtVec::new();
    assert!(v.is_empty());
    assert_eq!(v.len(), 0);
}

#[test]
fn vector_push_back() {
    let v = LtVec::new();
    let v = v.push_back(Value::Int(1));
    let v = v.push_back(Value::Int(2));

    assert_eq!(v.len(), 2);
    assert_eq!(v.get(0), Some(&Value::Int(1)));
    assert_eq!(v.get(1), Some(&Value::Int(2)));
}

#[test]
fn vector_immutability() {
    let v1 = LtVec::new().push_back(Value::Int(1));
    let v2 = v1.push_back(Value::Int(2));

    // v1 is unchanged
    assert_eq!(v1.len(), 1);
    assert_eq!(v2.len(), 2);
}

#[test]
fn vector_structural_sharing() {
    // Create a large vector
    let mut v = LtVec::new();
    for i in 0..1000 {
        v = v.push_back(Value::Int(i));
    }

    // Clone should be O(1) due to structural sharing
    let v2 = v.clone();
    assert_eq!(v.len(), v2.len());

    // Modify the clone - original unchanged
    let v3 = v2.push_back(Value::Int(1000));
    assert_eq!(v.len(), 1000);
    assert_eq!(v3.len(), 1001);
}

#[test]
fn vector_iteration() {
    let v = LtVec::new()
        .push_back(Value::Int(1))
        .push_back(Value::Int(2))
        .push_back(Value::Int(3));

    let collected: Vec<_> = v.iter().cloned().collect();
    assert_eq!(collected, vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
}

#[test]
fn vector_from_iter() {
    let v: LtVec<Value> = vec![Value::Int(1), Value::Int(2)].into_iter().collect();
    assert_eq!(v.len(), 2);
}

#[test]
fn vector_update() {
    let v = LtVec::new()
        .push_back(Value::Int(1))
        .push_back(Value::Int(2));

    let v2 = v.update(0, Value::Int(10)).unwrap();
    assert_eq!(v.get(0), Some(&Value::Int(1))); // original unchanged
    assert_eq!(v2.get(0), Some(&Value::Int(10)));
}

#[test]
fn vector_update_out_of_bounds() {
    let v = LtVec::new().push_back(Value::Int(1));
    assert!(v.update(5, Value::Int(10)).is_none());
}

#[test]
fn vector_first_last() {
    let v = LtVec::new()
        .push_back(Value::Int(1))
        .push_back(Value::Int(2))
        .push_back(Value::Int(3));

    assert_eq!(v.first(), Some(&Value::Int(1)));
    assert_eq!(v.last(), Some(&Value::Int(3)));
}

#[test]
fn vector_pop() {
    let v = LtVec::new()
        .push_back(Value::Int(1))
        .push_back(Value::Int(2));

    let (v2, popped) = v.pop_back().unwrap();
    assert_eq!(popped, Value::Int(2));
    assert_eq!(v2.len(), 1);

    // Original unchanged
    assert_eq!(v.len(), 2);
}

// =============================================================================
// LtSet
// =============================================================================

#[test]
fn set_empty() {
    let s: LtSet<Value> = LtSet::new();
    assert!(s.is_empty());
    assert_eq!(s.len(), 0);
}

#[test]
fn set_insert() {
    let s = LtSet::new();
    let s = s.insert(Value::Int(1));
    let s = s.insert(Value::Int(2));

    assert_eq!(s.len(), 2);
    assert!(s.contains(&Value::Int(1)));
    assert!(s.contains(&Value::Int(2)));
}

#[test]
fn set_no_duplicates() {
    let s = LtSet::new();
    let s = s.insert(Value::Int(1));
    let s = s.insert(Value::Int(1)); // duplicate

    assert_eq!(s.len(), 1);
}

#[test]
fn set_immutability() {
    let s1 = LtSet::new().insert(Value::Int(1));
    let s2 = s1.insert(Value::Int(2));

    assert_eq!(s1.len(), 1);
    assert_eq!(s2.len(), 2);
}

#[test]
fn set_remove() {
    let s = LtSet::new()
        .insert(Value::Int(1))
        .insert(Value::Int(2))
        .insert(Value::Int(3));

    let s2 = s.remove(&Value::Int(2));

    assert!(s.contains(&Value::Int(2))); // original unchanged
    assert!(!s2.contains(&Value::Int(2)));
    assert_eq!(s2.len(), 2);
}

#[test]
fn set_iteration() {
    let s = LtSet::new()
        .insert(Value::Int(1))
        .insert(Value::Int(2))
        .insert(Value::Int(3));

    let collected: Vec<_> = s.iter().cloned().collect();
    assert_eq!(collected.len(), 3);
    assert!(collected.contains(&Value::Int(1)));
    assert!(collected.contains(&Value::Int(2)));
    assert!(collected.contains(&Value::Int(3)));
}

#[test]
fn set_union() {
    let s1 = LtSet::new().insert(Value::Int(1)).insert(Value::Int(2));
    let s2 = LtSet::new().insert(Value::Int(2)).insert(Value::Int(3));

    let union = s1.union(&s2);
    assert_eq!(union.len(), 3);
}

#[test]
fn set_intersection() {
    let s1 = LtSet::new().insert(Value::Int(1)).insert(Value::Int(2));
    let s2 = LtSet::new().insert(Value::Int(2)).insert(Value::Int(3));

    let intersection = s1.intersection(&s2);
    assert_eq!(intersection.len(), 1);
    assert!(intersection.contains(&Value::Int(2)));
}

#[test]
fn set_difference() {
    let s1 = LtSet::new()
        .insert(Value::Int(1))
        .insert(Value::Int(2))
        .insert(Value::Int(3));
    let s2 = LtSet::new().insert(Value::Int(2));

    let diff = s1.difference(&s2);
    assert_eq!(diff.len(), 2);
    assert!(diff.contains(&Value::Int(1)));
    assert!(diff.contains(&Value::Int(3)));
}

// =============================================================================
// LtMap
// =============================================================================

#[test]
fn map_empty() {
    let m: LtMap<Value, Value> = LtMap::new();
    assert!(m.is_empty());
    assert_eq!(m.len(), 0);
}

#[test]
fn map_insert_get() {
    let m = LtMap::new();
    let m = m.insert(Value::String(Arc::from("a")), Value::Int(1));
    let m = m.insert(Value::String(Arc::from("b")), Value::Int(2));

    assert_eq!(m.len(), 2);
    assert_eq!(m.get(&Value::String(Arc::from("a"))), Some(&Value::Int(1)));
    assert_eq!(m.get(&Value::String(Arc::from("b"))), Some(&Value::Int(2)));
    assert_eq!(m.get(&Value::String(Arc::from("c"))), None);
}

#[test]
fn map_overwrite() {
    let m = LtMap::new();
    let m = m.insert(Value::String(Arc::from("a")), Value::Int(1));
    let m = m.insert(Value::String(Arc::from("a")), Value::Int(10));

    assert_eq!(m.len(), 1);
    assert_eq!(m.get(&Value::String(Arc::from("a"))), Some(&Value::Int(10)));
}

#[test]
fn map_immutability() {
    let m1 = LtMap::new().insert(Value::String(Arc::from("a")), Value::Int(1));
    let m2 = m1.insert(Value::String(Arc::from("b")), Value::Int(2));

    assert_eq!(m1.len(), 1);
    assert_eq!(m2.len(), 2);
}

#[test]
fn map_remove() {
    let m = LtMap::new()
        .insert(Value::String(Arc::from("a")), Value::Int(1))
        .insert(Value::String(Arc::from("b")), Value::Int(2));

    let m2 = m.remove(&Value::String(Arc::from("a")));

    assert!(m.get(&Value::String(Arc::from("a"))).is_some()); // original unchanged
    assert!(m2.get(&Value::String(Arc::from("a"))).is_none());
    assert_eq!(m2.len(), 1);
}

#[test]
fn map_contains_key() {
    let m = LtMap::new().insert(Value::String(Arc::from("a")), Value::Int(1));

    assert!(m.contains_key(&Value::String(Arc::from("a"))));
    assert!(!m.contains_key(&Value::String(Arc::from("b"))));
}

#[test]
fn map_keys() {
    let m = LtMap::new()
        .insert(Value::String(Arc::from("a")), Value::Int(1))
        .insert(Value::String(Arc::from("b")), Value::Int(2));

    let keys: Vec<_> = m.keys().cloned().collect();
    assert_eq!(keys.len(), 2);
}

#[test]
fn map_values() {
    let m = LtMap::new()
        .insert(Value::String(Arc::from("a")), Value::Int(1))
        .insert(Value::String(Arc::from("b")), Value::Int(2));

    let values: Vec<_> = m.values().cloned().collect();
    assert_eq!(values.len(), 2);
}

#[test]
fn map_iteration() {
    let m = LtMap::new()
        .insert(Value::String(Arc::from("a")), Value::Int(1))
        .insert(Value::String(Arc::from("b")), Value::Int(2));

    let entries: Vec<_> = m.iter().collect();
    assert_eq!(entries.len(), 2);
}

// =============================================================================
// Structural Sharing at Scale
// =============================================================================

#[test]
fn large_vector_clone_is_cheap() {
    let mut v = LtVec::new();
    for i in 0..10_000 {
        v = v.push_back(Value::Int(i));
    }

    // This should be essentially instant due to structural sharing
    let v2 = v.clone();
    assert_eq!(v.len(), v2.len());

    // Modifications create new nodes, don't affect original
    let v3 = v2.push_back(Value::Int(10_000));
    assert_eq!(v.len(), 10_000);
    assert_eq!(v3.len(), 10_001);
}

#[test]
fn large_map_clone_is_cheap() {
    let mut m = LtMap::new();
    for i in 0..10_000 {
        m = m.insert(Value::Int(i), Value::Int(i * 2));
    }

    let m2 = m.clone();
    assert_eq!(m.len(), m2.len());

    // Verify data integrity
    assert_eq!(m2.get(&Value::Int(5000)), Some(&Value::Int(10_000)));
}

// =============================================================================
// Collection Equality
// =============================================================================

#[test]
fn vector_equality() {
    let v1 = LtVec::new()
        .push_back(Value::Int(1))
        .push_back(Value::Int(2));
    let v2 = LtVec::new()
        .push_back(Value::Int(1))
        .push_back(Value::Int(2));
    let v3 = LtVec::new()
        .push_back(Value::Int(1))
        .push_back(Value::Int(3));

    assert_eq!(v1, v2);
    assert_ne!(v1, v3);
}

#[test]
fn set_equality() {
    let s1 = LtSet::new().insert(Value::Int(1)).insert(Value::Int(2));
    let s2 = LtSet::new().insert(Value::Int(2)).insert(Value::Int(1)); // different order
    let s3 = LtSet::new().insert(Value::Int(1)).insert(Value::Int(3));

    assert_eq!(s1, s2); // order doesn't matter for sets
    assert_ne!(s1, s3);
}

#[test]
fn map_equality() {
    let m1 = LtMap::new()
        .insert(Value::String(Arc::from("a")), Value::Int(1))
        .insert(Value::String(Arc::from("b")), Value::Int(2));
    let m2 = LtMap::new()
        .insert(Value::String(Arc::from("b")), Value::Int(2))
        .insert(Value::String(Arc::from("a")), Value::Int(1)); // different order
    let m3 = LtMap::new()
        .insert(Value::String(Arc::from("a")), Value::Int(1))
        .insert(Value::String(Arc::from("b")), Value::Int(3)); // different value

    assert_eq!(m1, m2); // order doesn't matter for maps
    assert_ne!(m1, m3);
}
