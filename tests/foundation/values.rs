//! Integration tests for Value types
//!
//! Tests Value enum variants, equality, hashing, display, and type coercion.

use longtable_foundation::collections::{LtMap, LtSet, LtVec};
use longtable_foundation::{EntityId, Interner, Value};
use std::collections::HashSet;
use std::sync::Arc;

// =============================================================================
// Value Construction
// =============================================================================

#[test]
fn value_nil() {
    let v = Value::Nil;
    assert!(v.is_nil());
    assert!(!v.is_truthy());
}

#[test]
fn value_bool_true() {
    let v = Value::Bool(true);
    assert!(v.is_truthy());
    assert_eq!(v.as_bool(), Some(true));
}

#[test]
fn value_bool_false() {
    let v = Value::Bool(false);
    assert!(!v.is_truthy());
    assert_eq!(v.as_bool(), Some(false));
}

#[test]
fn value_int() {
    let v = Value::Int(42);
    assert!(v.is_truthy());
    assert_eq!(v.as_int(), Some(42));
    assert_eq!(v.as_float(), None);
}

#[test]
fn value_float() {
    let v = Value::Float(1.5);
    assert!(v.is_truthy());
    assert_eq!(v.as_float(), Some(1.5));
    assert_eq!(v.as_int(), None);
}

#[test]
fn value_string() {
    let v = Value::String(Arc::from("hello"));
    assert!(v.is_truthy());
    assert_eq!(v.as_str(), Some("hello"));
}

#[test]
fn value_empty_string_is_truthy() {
    // In Longtable, empty string is still truthy (only nil and false are falsy)
    let v = Value::String(Arc::from(""));
    assert!(v.is_truthy());
}

#[test]
fn value_symbol() {
    let mut interner = Interner::new();
    let sym_id = interner.intern_symbol("foo");
    let v = Value::Symbol(sym_id);
    assert!(v.is_truthy());
    assert_eq!(v.as_symbol(), Some(sym_id));
}

#[test]
fn value_keyword() {
    let mut interner = Interner::new();
    let kw_id = interner.intern_keyword("bar");
    let v = Value::Keyword(kw_id);
    assert!(v.is_truthy());
    assert_eq!(v.as_keyword(), Some(kw_id));
}

#[test]
fn value_entity_ref() {
    let id = EntityId::new(1, 0);
    let v = Value::EntityRef(id);
    assert!(v.is_truthy());
    assert_eq!(v.as_entity(), Some(id));
}

// =============================================================================
// Value Equality
// =============================================================================

#[test]
fn value_equality_nil() {
    assert_eq!(Value::Nil, Value::Nil);
}

#[test]
fn value_equality_bool() {
    assert_eq!(Value::Bool(true), Value::Bool(true));
    assert_eq!(Value::Bool(false), Value::Bool(false));
    assert_ne!(Value::Bool(true), Value::Bool(false));
}

#[test]
fn value_equality_int() {
    assert_eq!(Value::Int(42), Value::Int(42));
    assert_ne!(Value::Int(42), Value::Int(43));
}

#[test]
fn value_equality_float() {
    assert_eq!(Value::Float(1.5), Value::Float(1.5));
    assert_ne!(Value::Float(1.5), Value::Float(2.5));
}

#[test]
fn value_equality_int_float_not_equal() {
    // Int and Float are different types, even with same numeric value
    assert_ne!(Value::Int(42), Value::Float(42.0));
}

#[test]
fn value_equality_string() {
    assert_eq!(
        Value::String(Arc::from("hello")),
        Value::String(Arc::from("hello"))
    );
    assert_ne!(
        Value::String(Arc::from("hello")),
        Value::String(Arc::from("world"))
    );
}

#[test]
fn value_equality_symbol() {
    let mut interner = Interner::new();
    let foo1 = interner.intern_symbol("foo");
    let foo2 = interner.intern_symbol("foo"); // same symbol
    let bar = interner.intern_symbol("bar");

    assert_eq!(Value::Symbol(foo1), Value::Symbol(foo2));
    assert_ne!(Value::Symbol(foo1), Value::Symbol(bar));
}

#[test]
fn value_equality_keyword() {
    let mut interner = Interner::new();
    let foo1 = interner.intern_keyword("foo");
    let foo2 = interner.intern_keyword("foo");
    let bar = interner.intern_keyword("bar");

    assert_eq!(Value::Keyword(foo1), Value::Keyword(foo2));
    assert_ne!(Value::Keyword(foo1), Value::Keyword(bar));
}

#[test]
fn value_equality_symbol_vs_keyword() {
    let mut interner = Interner::new();
    let sym_foo = interner.intern_symbol("foo");
    let kw_foo = interner.intern_keyword("foo");

    // Symbols and keywords are different even with same name
    assert_ne!(Value::Symbol(sym_foo), Value::Keyword(kw_foo));
}

#[test]
fn value_equality_entity_ref() {
    let id1 = EntityId::new(1, 0);
    let id2 = EntityId::new(1, 0);
    let id3 = EntityId::new(2, 0);
    assert_eq!(Value::EntityRef(id1), Value::EntityRef(id2));
    assert_ne!(Value::EntityRef(id1), Value::EntityRef(id3));
}

// =============================================================================
// Value Hashing (for use in HashSet/HashMap)
// =============================================================================

#[test]
#[allow(clippy::mutable_key_type)]
fn value_hash_consistency() {
    // Equal values must have equal hashes
    let v1 = Value::Int(42);
    let v2 = Value::Int(42);
    assert_eq!(v1, v2);

    let mut set = HashSet::new();
    set.insert(v1.clone());
    assert!(set.contains(&v2));
}

#[test]
#[allow(clippy::mutable_key_type)]
fn value_hash_string() {
    let mut set = HashSet::new();
    set.insert(Value::String(Arc::from("hello")));
    assert!(set.contains(&Value::String(Arc::from("hello"))));
    assert!(!set.contains(&Value::String(Arc::from("world"))));
}

#[test]
#[allow(clippy::mutable_key_type)]
fn value_hash_symbol() {
    let mut interner = Interner::new();
    let foo = interner.intern_symbol("foo");
    let bar = interner.intern_symbol("bar");

    let mut set = HashSet::new();
    set.insert(Value::Symbol(foo));
    assert!(set.contains(&Value::Symbol(foo)));
    assert!(!set.contains(&Value::Symbol(bar)));
}

#[test]
#[allow(clippy::mutable_key_type)]
fn value_hash_mixed_types() {
    let mut set = HashSet::new();
    set.insert(Value::Nil);
    set.insert(Value::Bool(true));
    set.insert(Value::Int(42));
    set.insert(Value::String(Arc::from("hello")));

    assert_eq!(set.len(), 4);
    assert!(set.contains(&Value::Nil));
    assert!(set.contains(&Value::Bool(true)));
    assert!(set.contains(&Value::Int(42)));
    assert!(set.contains(&Value::String(Arc::from("hello"))));
}

// =============================================================================
// Value Display
// =============================================================================

#[test]
fn value_display_nil() {
    assert_eq!(format!("{}", Value::Nil), "nil");
}

#[test]
fn value_display_bool() {
    assert_eq!(format!("{}", Value::Bool(true)), "true");
    assert_eq!(format!("{}", Value::Bool(false)), "false");
}

#[test]
fn value_display_int() {
    assert_eq!(format!("{}", Value::Int(42)), "42");
    assert_eq!(format!("{}", Value::Int(-17)), "-17");
}

#[test]
fn value_display_float() {
    let display = format!("{}", Value::Float(1.5));
    assert!(display.starts_with("1.5"));
}

#[test]
fn value_display_string() {
    // Display for string shows the raw string (not quoted)
    assert_eq!(format!("{}", Value::String(Arc::from("hello"))), "hello");
}

#[test]
fn value_display_entity_ref() {
    let id = EntityId::new(42, 3);
    let v = Value::EntityRef(id);
    let display = format!("{v}");
    assert!(display.contains("42"));
}

// =============================================================================
// Value Collections
// =============================================================================

#[test]
fn value_vector() {
    let vec = LtVec::new()
        .push_back(Value::Int(1))
        .push_back(Value::Int(2))
        .push_back(Value::Int(3));
    let v = Value::Vec(vec);
    assert!(v.is_truthy());
    if let Value::Vec(vec) = v {
        assert_eq!(vec.len(), 3);
    } else {
        panic!("Expected Vec");
    }
}

#[test]
fn value_empty_vector() {
    let v = Value::Vec(LtVec::new());
    assert!(v.is_truthy()); // Empty collections are truthy
    if let Value::Vec(vec) = v {
        assert!(vec.is_empty());
    } else {
        panic!("Expected Vec");
    }
}

#[test]
fn value_set() {
    let set: LtSet<Value> = [Value::Int(1), Value::Int(2), Value::Int(1)]
        .into_iter()
        .collect();
    let v = Value::Set(set);
    if let Value::Set(set) = v {
        assert_eq!(set.len(), 2); // duplicates removed
    } else {
        panic!("Expected Set");
    }
}

#[test]
fn value_map() {
    let mut interner = Interner::new();
    let key_a = interner.intern_keyword("a");
    let key_b = interner.intern_keyword("b");

    let map = LtMap::new()
        .insert(Value::Keyword(key_a), Value::Int(1))
        .insert(Value::Keyword(key_b), Value::Int(2));
    let v = Value::Map(map);

    if let Value::Map(map) = v {
        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&Value::Keyword(key_a)), Some(&Value::Int(1)));
    } else {
        panic!("Expected Map");
    }
}

// =============================================================================
// EntityId
// =============================================================================

#[test]
fn entity_id_construction() {
    let id = EntityId::new(42, 3);
    assert_eq!(id.index, 42);
    assert_eq!(id.generation, 3);
}

#[test]
fn entity_id_equality() {
    let id1 = EntityId::new(1, 0);
    let id2 = EntityId::new(1, 0);
    let id3 = EntityId::new(1, 1); // different generation
    let id4 = EntityId::new(2, 0); // different index

    assert_eq!(id1, id2);
    assert_ne!(id1, id3);
    assert_ne!(id1, id4);
}

#[test]
fn entity_id_hash() {
    let mut set = HashSet::new();
    set.insert(EntityId::new(1, 0));
    set.insert(EntityId::new(2, 0));
    set.insert(EntityId::new(1, 0)); // duplicate

    assert_eq!(set.len(), 2);
}

#[test]
fn entity_id_generation_prevents_stale_reference() {
    // Same index but different generation = different entity
    let old_id = EntityId::new(5, 0);
    let new_id = EntityId::new(5, 1);

    assert_ne!(old_id, new_id);
}

#[test]
fn entity_id_null() {
    let null = EntityId::null();
    assert!(null.is_null());

    let normal = EntityId::new(0, 0);
    assert!(!normal.is_null());
}
