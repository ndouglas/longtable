//! Vocabulary registry tests.
//!
//! Tests for registering and looking up vocabulary definitions.

use std::collections::HashSet;

use longtable_foundation::KeywordId;
use longtable_parser::vocabulary::{Direction, NounType, Preposition, Verb, VocabularyRegistry};

fn make_test_keywords() -> (KeywordId, KeywordId, KeywordId, KeywordId) {
    // Use reserved keywords for testing
    (
        KeywordId::VALUE,      // verb
        KeywordId::REL_TYPE,   // synonym
        KeywordId::REL_SOURCE, // prep
        KeywordId::REL_TARGET, // direction
    )
}

#[test]
fn register_and_lookup_verb() {
    let mut vocab = VocabularyRegistry::new();
    let (verb_kw, synonym_kw, _, _) = make_test_keywords();

    let verb = Verb {
        name: verb_kw,
        synonyms: HashSet::from([synonym_kw]),
    };

    vocab.register_verb(verb.clone());

    let found = vocab.lookup_verb(verb_kw);
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, verb_kw);
    assert!(found.unwrap().synonyms.contains(&synonym_kw));
}

#[test]
fn lookup_verb_by_synonym() {
    let mut vocab = VocabularyRegistry::new();
    let (verb_kw, synonym_kw, _, _) = make_test_keywords();

    let verb = Verb {
        name: verb_kw,
        synonyms: HashSet::from([synonym_kw]),
    };

    vocab.register_verb(verb);

    // Should find by synonym
    let found = vocab.lookup_verb(synonym_kw);
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, verb_kw);
}

#[test]
fn register_and_lookup_preposition() {
    let mut vocab = VocabularyRegistry::new();
    let (_, _, prep_kw, _) = make_test_keywords();

    let prep = Preposition {
        name: prep_kw,
        implies: None,
    };

    vocab.register_preposition(prep);

    let found = vocab.lookup_preposition(prep_kw);
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, prep_kw);
}

#[test]
fn register_and_lookup_direction() {
    let mut vocab = VocabularyRegistry::new();
    let (_, _, _, dir_kw) = make_test_keywords();
    let opposite_kw = KeywordId::REL_TARGET;

    let direction = Direction {
        name: dir_kw,
        synonyms: HashSet::new(),
        opposite: Some(opposite_kw),
    };

    vocab.register_direction(direction);

    let found = vocab.lookup_direction(dir_kw);
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, dir_kw);
    assert_eq!(found.unwrap().opposite, Some(opposite_kw));
}

#[test]
fn lookup_nonexistent_verb() {
    let vocab = VocabularyRegistry::new();
    let (verb_kw, _, _, _) = make_test_keywords();

    let found = vocab.lookup_verb(verb_kw);
    assert!(found.is_none());
}

#[test]
fn register_type() {
    let mut vocab = VocabularyRegistry::new();
    let type_kw = KeywordId::VALUE;

    let noun_type = NounType {
        name: type_kw,
        extends: vec![],
        pattern_source: String::new(),
    };

    vocab.register_type(noun_type);

    let found = vocab.lookup_type(type_kw);
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, type_kw);
}

#[test]
fn register_multiple_verbs() {
    let mut vocab = VocabularyRegistry::new();

    let take_kw = KeywordId::VALUE;
    let drop_kw = KeywordId::REL_TYPE;
    let get_kw = KeywordId::REL_SOURCE;

    vocab.register_verb(Verb {
        name: take_kw,
        synonyms: HashSet::from([get_kw]), // "get" is synonym of "take"
    });

    vocab.register_verb(Verb {
        name: drop_kw,
        synonyms: HashSet::new(),
    });

    // Both should be findable
    assert!(vocab.lookup_verb(take_kw).is_some());
    assert!(vocab.lookup_verb(drop_kw).is_some());
    assert!(vocab.lookup_verb(get_kw).is_some()); // Via synonym

    // "get" should resolve to "take"
    let found = vocab.lookup_verb(get_kw).unwrap();
    assert_eq!(found.name, take_kw);
}

#[test]
fn register_direction_with_synonyms() {
    let mut vocab = VocabularyRegistry::new();

    let north_kw = KeywordId::VALUE;
    let n_kw = KeywordId::REL_TYPE;
    let south_kw = KeywordId::REL_SOURCE;

    vocab.register_direction(Direction {
        name: north_kw,
        synonyms: HashSet::from([n_kw]),
        opposite: Some(south_kw),
    });

    // Should find by canonical name
    let found = vocab.lookup_direction(north_kw);
    assert!(found.is_some());
    assert_eq!(found.unwrap().opposite, Some(south_kw));

    // Should find by synonym
    let found = vocab.lookup_direction(n_kw);
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, north_kw);
}

#[test]
fn register_adverb() {
    let mut vocab = VocabularyRegistry::new();
    let quietly_kw = KeywordId::VALUE;

    vocab.register_adverb(quietly_kw);

    assert!(vocab.is_adverb(quietly_kw));
    assert!(!vocab.is_adverb(KeywordId::REL_TYPE));
}
