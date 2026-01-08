//! Noun resolution tests.
//!
//! Tests for resolving noun phrases to entities in scope.

use longtable_foundation::{LtMap, LtVec, Type, Value};
use longtable_parser::noun_phrase::{NounPhrase, NounResolution, NounResolver, Quantifier};
use longtable_parser::vocabulary::VocabularyRegistry;
use longtable_storage::{ComponentSchema, FieldSchema, World};

fn make_vec(values: Vec<Value>) -> LtVec<Value> {
    values.into_iter().collect()
}

fn setup_world_with_items() -> (World, TestSetup) {
    let mut world = World::new(42);

    // Intern keywords
    let name_kw = world.interner_mut().intern_keyword("name");
    let aliases_kw = world.interner_mut().intern_keyword("aliases");
    let adjectives_kw = world.interner_mut().intern_keyword("adjectives");

    // Register schemas
    let world = world
        .register_component(
            ComponentSchema::new(name_kw).with_field(FieldSchema::required(name_kw, Type::String)),
        )
        .unwrap();
    let world = world
        .register_component(
            ComponentSchema::new(aliases_kw).with_field(FieldSchema::required(
                aliases_kw,
                Type::Vec(Box::new(Type::String)),
            )),
        )
        .unwrap();
    let world =
        world
            .register_component(ComponentSchema::new(adjectives_kw).with_field(
                FieldSchema::required(adjectives_kw, Type::Vec(Box::new(Type::String))),
            ))
            .unwrap();

    // Create sword entity
    let (world, sword) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(sword, name_kw, name_kw, Value::from("sword"))
        .unwrap();
    let world = world
        .set_field(
            sword,
            aliases_kw,
            aliases_kw,
            Value::Vec(make_vec(vec![Value::from("blade")])),
        )
        .unwrap();
    let world = world
        .set_field(
            sword,
            adjectives_kw,
            adjectives_kw,
            Value::Vec(make_vec(vec![Value::from("iron")])),
        )
        .unwrap();

    // Create lamp entity
    let (world, lamp) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(lamp, name_kw, name_kw, Value::from("lamp"))
        .unwrap();
    let world = world
        .set_field(
            lamp,
            adjectives_kw,
            adjectives_kw,
            Value::Vec(make_vec(vec![Value::from("brass")])),
        )
        .unwrap();

    // Create another lamp (for disambiguation tests)
    let (world, other_lamp) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(other_lamp, name_kw, name_kw, Value::from("lamp"))
        .unwrap();
    let world = world
        .set_field(
            other_lamp,
            adjectives_kw,
            adjectives_kw,
            Value::Vec(make_vec(vec![Value::from("rusty")])),
        )
        .unwrap();

    let setup = TestSetup {
        sword,
        lamp,
        other_lamp,
        name_kw,
        aliases_kw,
        adjectives_kw,
    };

    (world, setup)
}

struct TestSetup {
    sword: longtable_foundation::EntityId,
    lamp: longtable_foundation::EntityId,
    other_lamp: longtable_foundation::EntityId,
    name_kw: longtable_foundation::KeywordId,
    aliases_kw: longtable_foundation::KeywordId,
    adjectives_kw: longtable_foundation::KeywordId,
}

#[test]
fn resolve_by_exact_name() {
    let (world, setup) = setup_world_with_items();
    let vocab = VocabularyRegistry::new();
    let resolver = NounResolver::new(setup.name_kw, setup.aliases_kw, setup.adjectives_kw);

    let np = NounPhrase::new("sword");
    let scope = vec![setup.sword, setup.lamp, setup.other_lamp];

    let result = resolver.resolve(&np, None, &scope, &world, &vocab);

    assert!(matches!(result, NounResolution::Unique(id) if id == setup.sword));
}

#[test]
fn resolve_by_alias() {
    let (world, setup) = setup_world_with_items();
    let vocab = VocabularyRegistry::new();
    let resolver = NounResolver::new(setup.name_kw, setup.aliases_kw, setup.adjectives_kw);

    let np = NounPhrase::new("blade");
    let scope = vec![setup.sword, setup.lamp, setup.other_lamp];

    let result = resolver.resolve(&np, None, &scope, &world, &vocab);

    assert!(matches!(result, NounResolution::Unique(id) if id == setup.sword));
}

#[test]
fn resolve_ambiguous_returns_multiple() {
    let (world, setup) = setup_world_with_items();
    let vocab = VocabularyRegistry::new();
    let resolver = NounResolver::new(setup.name_kw, setup.aliases_kw, setup.adjectives_kw);

    let np = NounPhrase::new("lamp"); // Both lamps match
    let scope = vec![setup.sword, setup.lamp, setup.other_lamp];

    let result = resolver.resolve(&np, None, &scope, &world, &vocab);

    if let NounResolution::Ambiguous(entities) = result {
        assert_eq!(entities.len(), 2);
        assert!(entities.contains(&setup.lamp));
        assert!(entities.contains(&setup.other_lamp));
    } else {
        panic!("Expected Ambiguous, got {result:?}");
    }
}

#[test]
fn resolve_with_adjective_disambiguation() {
    let (world, setup) = setup_world_with_items();
    let vocab = VocabularyRegistry::new();
    let resolver = NounResolver::new(setup.name_kw, setup.aliases_kw, setup.adjectives_kw);

    let np = NounPhrase::new("lamp").with_adjective("brass");
    let scope = vec![setup.sword, setup.lamp, setup.other_lamp];

    let result = resolver.resolve(&np, None, &scope, &world, &vocab);

    assert!(matches!(result, NounResolution::Unique(id) if id == setup.lamp));
}

#[test]
fn resolve_not_found() {
    let (world, setup) = setup_world_with_items();
    let vocab = VocabularyRegistry::new();
    let resolver = NounResolver::new(setup.name_kw, setup.aliases_kw, setup.adjectives_kw);

    let np = NounPhrase::new("dragon");
    let scope = vec![setup.sword, setup.lamp, setup.other_lamp];

    let result = resolver.resolve(&np, None, &scope, &world, &vocab);

    assert!(matches!(result, NounResolution::NotFound));
}

#[test]
fn resolve_not_in_scope() {
    let (world, setup) = setup_world_with_items();
    let vocab = VocabularyRegistry::new();
    let resolver = NounResolver::new(setup.name_kw, setup.aliases_kw, setup.adjectives_kw);

    let np = NounPhrase::new("sword");
    let scope = vec![setup.lamp, setup.other_lamp]; // Sword not in scope

    let result = resolver.resolve(&np, None, &scope, &world, &vocab);

    assert!(matches!(result, NounResolution::NotFound));
}

#[test]
fn resolve_all_quantifier() {
    let (world, setup) = setup_world_with_items();
    let vocab = VocabularyRegistry::new();
    let resolver = NounResolver::new(setup.name_kw, setup.aliases_kw, setup.adjectives_kw);

    let np = NounPhrase::new("lamp").with_quantifier(Quantifier::All);
    let scope = vec![setup.sword, setup.lamp, setup.other_lamp];

    let result = resolver.resolve(&np, None, &scope, &world, &vocab);

    if let NounResolution::Multiple(entities) = result {
        assert_eq!(entities.len(), 2);
        assert!(entities.contains(&setup.lamp));
        assert!(entities.contains(&setup.other_lamp));
    } else {
        panic!("Expected Multiple, got {result:?}");
    }
}

#[test]
fn resolve_any_quantifier() {
    let (world, setup) = setup_world_with_items();
    let vocab = VocabularyRegistry::new();
    let resolver = NounResolver::new(setup.name_kw, setup.aliases_kw, setup.adjectives_kw);

    let np = NounPhrase::new("lamp").with_quantifier(Quantifier::Any);
    let scope = vec![setup.sword, setup.lamp, setup.other_lamp];

    let result = resolver.resolve(&np, None, &scope, &world, &vocab);

    // With Any quantifier, ambiguous matches should return first match
    assert!(
        matches!(result, NounResolution::Unique(id) if id == setup.lamp || id == setup.other_lamp)
    );
}

#[test]
fn describe_entity() {
    let (world, setup) = setup_world_with_items();
    let resolver = NounResolver::new(setup.name_kw, setup.aliases_kw, setup.adjectives_kw);

    let desc = resolver.describe(setup.lamp, &world);

    // Should include adjectives and name
    assert!(desc.contains("brass"));
    assert!(desc.contains("lamp"));
}

#[test]
fn case_insensitive_matching() {
    let (world, setup) = setup_world_with_items();
    let vocab = VocabularyRegistry::new();
    let resolver = NounResolver::new(setup.name_kw, setup.aliases_kw, setup.adjectives_kw);

    let np = NounPhrase::new("SWORD");
    let scope = vec![setup.sword, setup.lamp];

    let result = resolver.resolve(&np, None, &scope, &world, &vocab);

    assert!(matches!(result, NounResolution::Unique(id) if id == setup.sword));
}
