//! Integration tests for the parser
//!
//! Tests parsing of Longtable DSL to AST.

use longtable_language::{Ast, parse, parse_one};

// =============================================================================
// Literals
// =============================================================================

#[test]
fn parse_nil() {
    let ast = parse_one("nil").unwrap();
    assert!(matches!(ast, Ast::Nil(_)));
}

#[test]
fn parse_booleans() {
    let true_ast = parse_one("true").unwrap();
    let false_ast = parse_one("false").unwrap();

    assert!(matches!(true_ast, Ast::Bool(true, _)));
    assert!(matches!(false_ast, Ast::Bool(false, _)));
}

#[test]
fn parse_integers() {
    let ast = parse_one("42").unwrap();
    assert!(matches!(ast, Ast::Int(42, _)));

    let neg = parse_one("-17").unwrap();
    assert!(matches!(neg, Ast::Int(-17, _)));
}

#[test]
fn parse_floats() {
    let ast = parse_one("1.5").unwrap();
    if let Ast::Float(f, _) = ast {
        assert!((f - 1.5).abs() < 0.001);
    } else {
        panic!("Expected Float");
    }
}

#[test]
fn parse_string() {
    let ast = parse_one("\"hello\"").unwrap();
    if let Ast::String(s, _) = ast {
        assert_eq!(s, "hello");
    } else {
        panic!("Expected String");
    }
}

#[test]
fn parse_symbol() {
    let ast = parse_one("foo").unwrap();
    if let Ast::Symbol(s, _) = ast {
        assert_eq!(s, "foo");
    } else {
        panic!("Expected Symbol");
    }
}

#[test]
fn parse_keyword() {
    let ast = parse_one(":bar").unwrap();
    if let Ast::Keyword(s, _) = ast {
        assert_eq!(s, "bar");
    } else {
        panic!("Expected Keyword");
    }
}

// =============================================================================
// Collections
// =============================================================================

#[test]
fn parse_empty_list() {
    let ast = parse_one("()").unwrap();
    if let Ast::List(items, _) = ast {
        assert!(items.is_empty());
    } else {
        panic!("Expected List");
    }
}

#[test]
fn parse_list() {
    let ast = parse_one("(+ 1 2)").unwrap();
    if let Ast::List(items, _) = ast {
        assert_eq!(items.len(), 3);
    } else {
        panic!("Expected List");
    }
}

#[test]
fn parse_empty_vector() {
    let ast = parse_one("[]").unwrap();
    if let Ast::Vector(items, _) = ast {
        assert!(items.is_empty());
    } else {
        panic!("Expected Vector");
    }
}

#[test]
fn parse_vector() {
    let ast = parse_one("[1 2 3]").unwrap();
    if let Ast::Vector(items, _) = ast {
        assert_eq!(items.len(), 3);
    } else {
        panic!("Expected Vector");
    }
}

#[test]
fn parse_empty_map() {
    let ast = parse_one("{}").unwrap();
    if let Ast::Map(entries, _) = ast {
        assert!(entries.is_empty());
    } else {
        panic!("Expected Map");
    }
}

#[test]
fn parse_map() {
    let ast = parse_one("{:a 1 :b 2}").unwrap();
    if let Ast::Map(entries, _) = ast {
        assert_eq!(entries.len(), 2);
    } else {
        panic!("Expected Map");
    }
}

#[test]
fn parse_set() {
    let ast = parse_one("#{1 2 3}").unwrap();
    if let Ast::Set(items, _) = ast {
        assert_eq!(items.len(), 3);
    } else {
        panic!("Expected Set");
    }
}

// =============================================================================
// Nested Structures
// =============================================================================

#[test]
fn parse_nested_lists() {
    let ast = parse_one("((1 2) (3 4))").unwrap();
    if let Ast::List(items, _) = ast {
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[0], Ast::List(_, _)));
        assert!(matches!(&items[1], Ast::List(_, _)));
    } else {
        panic!("Expected List");
    }
}

#[test]
fn parse_mixed_collections() {
    let ast = parse_one("[{:a 1} {:b 2}]").unwrap();
    if let Ast::Vector(items, _) = ast {
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[0], Ast::Map(_, _)));
    } else {
        panic!("Expected Vector");
    }
}

// =============================================================================
// Special Forms
// =============================================================================

#[test]
fn parse_quote() {
    let ast = parse_one("'foo").unwrap();
    if let Ast::Quote(inner, _) = ast {
        assert!(matches!(*inner, Ast::Symbol(_, _)));
    } else {
        panic!("Expected Quote");
    }
}

#[test]
fn parse_syntax_quote() {
    let ast = parse_one("`foo").unwrap();
    if let Ast::SyntaxQuote(inner, _) = ast {
        assert!(matches!(*inner, Ast::Symbol(_, _)));
    } else {
        panic!("Expected SyntaxQuote");
    }
}

#[test]
fn parse_unquote() {
    let ast = parse_one("~foo").unwrap();
    if let Ast::Unquote(inner, _) = ast {
        assert!(matches!(*inner, Ast::Symbol(_, _)));
    } else {
        panic!("Expected Unquote");
    }
}

#[test]
fn parse_unquote_splice() {
    let ast = parse_one("~@foo").unwrap();
    if let Ast::UnquoteSplice(inner, _) = ast {
        assert!(matches!(*inner, Ast::Symbol(_, _)));
    } else {
        panic!("Expected UnquoteSplice");
    }
}

// =============================================================================
// Multiple Expressions
// =============================================================================

#[test]
fn parse_multiple() {
    let asts = parse("1 2 3").unwrap();
    assert_eq!(asts.len(), 3);
}

#[test]
fn parse_multiple_forms() {
    let asts = parse("(def x 1) (def y 2)").unwrap();
    assert_eq!(asts.len(), 2);
    assert!(matches!(&asts[0], Ast::List(_, _)));
    assert!(matches!(&asts[1], Ast::List(_, _)));
}

// =============================================================================
// Error Cases
// =============================================================================

#[test]
fn parse_unbalanced_parens() {
    let result = parse_one("(+ 1 2");
    assert!(result.is_err());
}

#[test]
fn parse_unbalanced_brackets() {
    let result = parse_one("[1 2 3");
    assert!(result.is_err());
}

#[test]
fn parse_unbalanced_braces() {
    let result = parse_one("{:a 1");
    assert!(result.is_err());
}

#[test]
fn parse_odd_map_elements() {
    // Maps need even number of elements
    let result = parse_one("{:a 1 :b}");
    assert!(result.is_err());
}

// =============================================================================
// Span Information
// =============================================================================

#[test]
fn ast_has_span() {
    let ast = parse_one("foo").unwrap();
    if let Ast::Symbol(_, span) = ast {
        assert_eq!(span.start, 0);
        assert_eq!(span.end, 3);
    } else {
        panic!("Expected Symbol");
    }
}

#[test]
fn nested_spans() {
    let ast = parse_one("(+ 1 2)").unwrap();
    if let Ast::List(_, span) = ast {
        // List should span entire expression
        assert_eq!(span.start, 0);
        assert!(span.end >= 7);
    } else {
        panic!("Expected List");
    }
}

// =============================================================================
// Real-World DSL Examples
// =============================================================================

#[test]
fn parse_let_binding() {
    let ast = parse_one("(let [x 1 y 2] (+ x y))").unwrap();
    if let Ast::List(items, _) = ast {
        assert_eq!(items.len(), 3);
        // First item is 'let' symbol
        assert!(matches!(&items[0], Ast::Symbol(s, _) if s == "let"));
        // Second item is binding vector
        assert!(matches!(&items[1], Ast::Vector(_, _)));
    } else {
        panic!("Expected List");
    }
}

#[test]
fn parse_defn() {
    let ast = parse_one("(defn add [a b] (+ a b))").unwrap();
    if let Ast::List(items, _) = ast {
        assert!(items.len() >= 4);
        assert!(matches!(&items[0], Ast::Symbol(s, _) if s == "defn"));
    } else {
        panic!("Expected List");
    }
}

#[test]
fn parse_if_expression() {
    let ast = parse_one("(if (> x 0) \"positive\" \"non-positive\")").unwrap();
    if let Ast::List(items, _) = ast {
        assert_eq!(items.len(), 4);
        assert!(matches!(&items[0], Ast::Symbol(s, _) if s == "if"));
    } else {
        panic!("Expected List");
    }
}

#[test]
fn parse_pattern_variable() {
    // Used in rule patterns
    let ast = parse_one("?entity").unwrap();
    if let Ast::Symbol(s, _) = ast {
        assert_eq!(s, "?entity");
    } else {
        panic!("Expected Symbol");
    }
}
