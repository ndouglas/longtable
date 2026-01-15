//! Parser integration tests.
//!
//! Tests for the parser pipeline components.
//! Note: Full end-to-end parsing requires vocabulary_lookup_word integration.

use longtable_foundation::{EntityId, LtMap, LtVec, Type, Value};
use longtable_parser::NaturalLanguageParser;
use longtable_parser::noun_phrase::{NounPhrase, NounResolution, NounResolver, Quantifier};
use longtable_parser::parser::{ParseError, ParseResult};
use longtable_parser::pronouns::PronounState;
use longtable_parser::scope::{CompiledScope, ScopeEvaluator, ScopeKind};
use longtable_parser::vocabulary::VocabularyRegistry;
use longtable_storage::{ComponentSchema, FieldSchema, World};

fn make_vec(values: Vec<Value>) -> LtVec<Value> {
    values.into_iter().collect()
}

// =============================================================================
// Parser Construction Tests
// =============================================================================

#[test]
fn parser_new_creates_empty_parser() {
    let vocab = VocabularyRegistry::new();
    let parser = NaturalLanguageParser::new(vocab);

    // Parser should exist and have empty syntaxes/scopes
    assert!(
        parser
            .vocabulary()
            .lookup_verb(longtable_foundation::KeywordId::VALUE)
            .is_none()
    );
}

#[test]
fn parser_with_scope_evaluator() {
    let vocab = VocabularyRegistry::new();

    let location_kw = longtable_foundation::KeywordId::VALUE;
    let inventory_kw = longtable_foundation::KeywordId::REL_TYPE;

    let evaluator = ScopeEvaluator::new(
        location_kw,
        inventory_kw,
        location_kw,
        location_kw,
        location_kw,
    );

    let parser = NaturalLanguageParser::new(vocab).with_scope_evaluator(evaluator);

    // Should have the evaluator configured
    assert!(parser.vocabulary().lookup_verb(location_kw).is_none());
}

#[test]
fn parser_with_noun_resolver() {
    let vocab = VocabularyRegistry::new();

    let name_kw = longtable_foundation::KeywordId::VALUE;
    let value_kw = longtable_foundation::KeywordId::VALUE; // same as name_kw in tests
    let aliases_kw = longtable_foundation::KeywordId::REL_TYPE;
    let adjectives_kw = longtable_foundation::KeywordId::REL_SOURCE;

    let resolver = NounResolver::new(name_kw, value_kw, aliases_kw, adjectives_kw);

    let _parser = NaturalLanguageParser::new(vocab).with_noun_resolver(resolver);
}

// =============================================================================
// Parse Error Tests
// =============================================================================

#[test]
fn parse_empty_input_returns_error() {
    let vocab = VocabularyRegistry::new();
    let mut parser = NaturalLanguageParser::new(vocab);

    let actor = EntityId::new(1, 0);
    let world = World::new(42);

    let result = parser.parse("", actor, &world);

    assert!(matches!(result, ParseResult::Error(ParseError::EmptyInput)));
}

#[test]
fn parse_whitespace_only_returns_error() {
    let vocab = VocabularyRegistry::new();
    let mut parser = NaturalLanguageParser::new(vocab);

    let actor = EntityId::new(1, 0);
    let world = World::new(42);

    let result = parser.parse("   ", actor, &world);

    assert!(matches!(result, ParseResult::Error(ParseError::EmptyInput)));
}

#[test]
fn parse_no_syntax_match_returns_error() {
    let vocab = VocabularyRegistry::new();
    let mut parser = NaturalLanguageParser::new(vocab);

    let actor = EntityId::new(1, 0);
    let world = World::new(42);

    // No syntaxes registered, so any input should fail
    let result = parser.parse("take sword", actor, &world);

    assert!(matches!(result, ParseResult::Error(ParseError::NoMatch)));
}

// =============================================================================
// Pronoun State Tests
// =============================================================================

#[test]
fn pronoun_state_tracks_it() {
    let mut state = PronounState::new();
    let entity = EntityId::new(42, 0);

    state.set_it(entity);

    assert_eq!(state.get_it(), Some(entity));
}

#[test]
fn pronoun_state_tracks_him() {
    let mut state = PronounState::new();
    let entity = EntityId::new(42, 0);

    state.set_him(entity);

    assert_eq!(state.get_him(), Some(entity));
}

#[test]
fn pronoun_state_tracks_her() {
    let mut state = PronounState::new();
    let entity = EntityId::new(42, 0);

    state.set_her(entity);

    assert_eq!(state.get_her(), Some(entity));
}

#[test]
fn pronoun_state_tracks_them() {
    let mut state = PronounState::new();
    let entities = vec![EntityId::new(1, 0), EntityId::new(2, 0)];

    state.set_them(entities.clone());

    assert_eq!(state.get_them(), &entities);
}

#[test]
fn pronoun_state_initially_empty() {
    let state = PronounState::new();

    assert_eq!(state.get_it(), None);
    assert_eq!(state.get_him(), None);
    assert_eq!(state.get_her(), None);
    assert!(state.get_them().is_empty());
}

// =============================================================================
// Scope Evaluation Tests
// =============================================================================

#[test]
fn scope_kind_variants_exist() {
    let _same_location = ScopeKind::SameLocation;
    let _inventory = ScopeKind::Inventory;
    let _container = ScopeKind::ContainerContents {
        require_open: true,
        require_transparent: false,
    };
    let _union = ScopeKind::Union(vec![]);
    let _custom = ScopeKind::Custom;
}

#[test]
fn compiled_scope_construction() {
    let scope = CompiledScope {
        name: longtable_foundation::KeywordId::VALUE,
        parent: None,
        kind: ScopeKind::SameLocation,
    };

    assert!(scope.parent.is_none());
}

#[test]
fn compiled_scope_with_parent() {
    let parent_kw = longtable_foundation::KeywordId::VALUE;
    let child_kw = longtable_foundation::KeywordId::REL_TYPE;

    let scope = CompiledScope {
        name: child_kw,
        parent: Some(parent_kw),
        kind: ScopeKind::ContainerContents {
            require_open: false,
            require_transparent: true,
        },
    };

    assert_eq!(scope.parent, Some(parent_kw));
}

// =============================================================================
// Noun Resolution Tests (using NounResolver directly)
// =============================================================================

fn setup_resolution_world() -> (World, ResolutionSetup) {
    let mut world = World::new(42);

    let name_kw = world.interner_mut().intern_keyword("name");
    let value_kw = world.interner_mut().intern_keyword("value");
    let aliases_kw = world.interner_mut().intern_keyword("aliases");
    let adjectives_kw = world.interner_mut().intern_keyword("adjectives");

    // Register schemas (using :value as field, like the real game)
    let world = world
        .register_component(
            ComponentSchema::new(name_kw).with_field(FieldSchema::required(value_kw, Type::String)),
        )
        .unwrap();
    let world = world
        .register_component(
            ComponentSchema::new(aliases_kw).with_field(FieldSchema::required(
                value_kw,
                Type::Vec(Box::new(Type::String)),
            )),
        )
        .unwrap();
    let world =
        world
            .register_component(ComponentSchema::new(adjectives_kw).with_field(
                FieldSchema::required(value_kw, Type::Vec(Box::new(Type::String))),
            ))
            .unwrap();

    // Create sword
    let (world, sword) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(sword, name_kw, value_kw, Value::from("sword"))
        .unwrap();
    let world = world
        .set_field(
            sword,
            adjectives_kw,
            value_kw,
            Value::Vec(make_vec(vec![Value::from("iron")])),
        )
        .unwrap();

    // Create lamp
    let (world, lamp) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(lamp, name_kw, value_kw, Value::from("lamp"))
        .unwrap();
    let world = world
        .set_field(
            lamp,
            adjectives_kw,
            value_kw,
            Value::Vec(make_vec(vec![Value::from("brass")])),
        )
        .unwrap();

    let setup = ResolutionSetup {
        sword,
        lamp,
        name_kw,
        aliases_kw,
        adjectives_kw,
        value_kw,
    };

    (world, setup)
}

struct ResolutionSetup {
    sword: EntityId,
    lamp: EntityId,
    name_kw: longtable_foundation::KeywordId,
    aliases_kw: longtable_foundation::KeywordId,
    adjectives_kw: longtable_foundation::KeywordId,
    value_kw: longtable_foundation::KeywordId,
}

#[test]
fn resolver_finds_entity_by_name() {
    let (world, setup) = setup_resolution_world();
    let vocab = VocabularyRegistry::new();
    let resolver = NounResolver::new(
        setup.name_kw,
        setup.value_kw,
        setup.aliases_kw,
        setup.adjectives_kw,
    );

    let np = NounPhrase::new("sword");
    let scope = vec![setup.sword, setup.lamp];

    let result = resolver.resolve(&np, None, &scope, &world, &vocab);

    assert!(matches!(result, NounResolution::Unique(id) if id == setup.sword));
}

#[test]
fn resolver_returns_not_found() {
    let (world, setup) = setup_resolution_world();
    let vocab = VocabularyRegistry::new();
    let resolver = NounResolver::new(
        setup.name_kw,
        setup.value_kw,
        setup.aliases_kw,
        setup.adjectives_kw,
    );

    let np = NounPhrase::new("dragon");
    let scope = vec![setup.sword, setup.lamp];

    let result = resolver.resolve(&np, None, &scope, &world, &vocab);

    assert!(matches!(result, NounResolution::NotFound));
}

#[test]
fn resolver_with_adjective() {
    let (world, setup) = setup_resolution_world();
    let vocab = VocabularyRegistry::new();
    let resolver = NounResolver::new(
        setup.name_kw,
        setup.value_kw,
        setup.aliases_kw,
        setup.adjectives_kw,
    );

    let np = NounPhrase::new("sword").with_adjective("iron");
    let scope = vec![setup.sword, setup.lamp];

    let result = resolver.resolve(&np, None, &scope, &world, &vocab);

    assert!(matches!(result, NounResolution::Unique(id) if id == setup.sword));
}

#[test]
fn resolver_any_quantifier_returns_first() {
    let (world, setup) = setup_resolution_world();
    let vocab = VocabularyRegistry::new();
    let resolver = NounResolver::new(
        setup.name_kw,
        setup.value_kw,
        setup.aliases_kw,
        setup.adjectives_kw,
    );

    let np = NounPhrase::new("sword").with_quantifier(Quantifier::Any);
    let scope = vec![setup.sword, setup.lamp];

    let result = resolver.resolve(&np, None, &scope, &world, &vocab);

    // Should return unique (first match)
    assert!(matches!(result, NounResolution::Unique(_)));
}

#[test]
fn resolver_describes_entity() {
    let (world, setup) = setup_resolution_world();
    let resolver = NounResolver::new(
        setup.name_kw,
        setup.value_kw,
        setup.aliases_kw,
        setup.adjectives_kw,
    );

    let desc = resolver.describe(setup.lamp, &world);

    assert!(desc.contains("brass"));
    assert!(desc.contains("lamp"));
}
