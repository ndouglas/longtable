//! Syntax matching tests.
//!
//! Tests for syntax pattern structures and specificity calculations.
//! Note: Full syntax matching requires vocabulary_lookup_word integration.

use longtable_foundation::KeywordId;
use longtable_parser::syntax::{CompiledSyntax, CompiledSyntaxElement};

#[test]
fn compiled_syntax_specificity_verb_only() {
    let syntax = CompiledSyntax {
        command: KeywordId::VALUE,
        action: KeywordId::REL_TYPE,
        elements: vec![CompiledSyntaxElement::Verb(KeywordId::VALUE)],
        priority: 0,
    };

    assert_eq!(syntax.specificity(), 1);
}

#[test]
fn compiled_syntax_specificity_verb_noun() {
    let syntax = CompiledSyntax {
        command: KeywordId::VALUE,
        action: KeywordId::REL_TYPE,
        elements: vec![
            CompiledSyntaxElement::Verb(KeywordId::VALUE),
            CompiledSyntaxElement::Noun {
                var: "target".to_string(),
                type_constraint: None,
            },
        ],
        priority: 0,
    };

    assert_eq!(syntax.specificity(), 2);
}

#[test]
fn compiled_syntax_specificity_verb_noun_prep_noun() {
    let syntax = CompiledSyntax {
        command: KeywordId::VALUE,
        action: KeywordId::REL_TYPE,
        elements: vec![
            CompiledSyntaxElement::Verb(KeywordId::VALUE),
            CompiledSyntaxElement::Noun {
                var: "target".to_string(),
                type_constraint: None,
            },
            CompiledSyntaxElement::Preposition(KeywordId::REL_SOURCE),
            CompiledSyntaxElement::Noun {
                var: "destination".to_string(),
                type_constraint: None,
            },
        ],
        priority: 0,
    };

    assert_eq!(syntax.specificity(), 4);
}

#[test]
fn compiled_syntax_specificity_excludes_optional() {
    let syntax = CompiledSyntax {
        command: KeywordId::VALUE,
        action: KeywordId::REL_TYPE,
        elements: vec![
            CompiledSyntaxElement::Verb(KeywordId::VALUE),
            CompiledSyntaxElement::Noun {
                var: "target".to_string(),
                type_constraint: None,
            },
            CompiledSyntaxElement::OptionalNoun {
                var: "with".to_string(),
                type_constraint: None,
            },
        ],
        priority: 0,
    };

    // Optional noun shouldn't count toward specificity
    assert_eq!(syntax.specificity(), 2);
}

#[test]
fn compiled_syntax_verb_extraction() {
    let verb_kw = KeywordId::VALUE;
    let syntax = CompiledSyntax {
        command: KeywordId::REL_TYPE,
        action: KeywordId::REL_SOURCE,
        elements: vec![CompiledSyntaxElement::Verb(verb_kw)],
        priority: 0,
    };

    assert_eq!(syntax.verb(), Some(verb_kw));
}

#[test]
fn compiled_syntax_no_verb() {
    let syntax = CompiledSyntax {
        command: KeywordId::VALUE,
        action: KeywordId::REL_TYPE,
        elements: vec![CompiledSyntaxElement::Literal("hello".to_string())],
        priority: 0,
    };

    assert_eq!(syntax.verb(), None);
}

#[test]
fn syntax_element_types() {
    // Ensure all syntax element types can be constructed
    let _verb = CompiledSyntaxElement::Verb(KeywordId::VALUE);
    let _literal = CompiledSyntaxElement::Literal("in".to_string());
    let _noun = CompiledSyntaxElement::Noun {
        var: "obj".to_string(),
        type_constraint: Some(KeywordId::REL_TYPE),
    };
    let _optional = CompiledSyntaxElement::OptionalNoun {
        var: "with".to_string(),
        type_constraint: None,
    };
    let _direction = CompiledSyntaxElement::Direction {
        var: "dir".to_string(),
    };
    let _prep = CompiledSyntaxElement::Preposition(KeywordId::REL_SOURCE);
}

#[test]
fn syntax_with_direction() {
    let syntax = CompiledSyntax {
        command: KeywordId::VALUE,
        action: KeywordId::REL_TYPE,
        elements: vec![
            CompiledSyntaxElement::Verb(KeywordId::VALUE),
            CompiledSyntaxElement::Direction {
                var: "dir".to_string(),
            },
        ],
        priority: 0,
    };

    assert_eq!(syntax.specificity(), 2);
    assert_eq!(syntax.verb(), Some(KeywordId::VALUE));
}

#[test]
fn syntax_with_type_constraint() {
    let takeable_type = KeywordId::REL_SOURCE;

    let syntax = CompiledSyntax {
        command: KeywordId::VALUE,
        action: KeywordId::REL_TYPE,
        elements: vec![
            CompiledSyntaxElement::Verb(KeywordId::VALUE),
            CompiledSyntaxElement::Noun {
                var: "target".to_string(),
                type_constraint: Some(takeable_type),
            },
        ],
        priority: 10,
    };

    assert_eq!(syntax.priority, 10);
    if let CompiledSyntaxElement::Noun {
        type_constraint, ..
    } = &syntax.elements[1]
    {
        assert_eq!(*type_constraint, Some(takeable_type));
    } else {
        panic!("Expected Noun element");
    }
}
