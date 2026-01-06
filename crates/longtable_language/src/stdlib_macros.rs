//! Standard library macros.
//!
//! This module provides built-in macros that are registered automatically
//! when a new `MacroRegistry` is created with stdlib support.
//!
//! # Macros
//!
//! - `when` - Execute body if condition is truthy
//! - `when-not` - Execute body if condition is falsy
//! - `and` - Short-circuit logical AND
//! - `or` - Short-circuit logical OR
//! - `cond` - Multi-branch conditional
//! - `->` - Thread-first macro
//! - `->>` - Thread-last macro
//! - `if-let` - Bind and test in one form

use crate::macro_def::{MacroDef, MacroParam};
use crate::macro_registry::MacroRegistry;
use crate::parser::parse;
use crate::span::Span;

/// Registers all standard library macros into the given registry.
pub fn register_stdlib_macros(registry: &mut MacroRegistry) {
    // when - (when test body...) -> (if test (do body...) nil)
    register_macro(
        registry,
        "when",
        &["test"],
        Some("body"),
        "(if test (do ~@body) nil)",
    );

    // when-not - (when-not test body...) -> (if test nil (do body...))
    register_macro(
        registry,
        "when-not",
        &["test"],
        Some("body"),
        "(if test nil (do ~@body))",
    );

    // if-not - (if-not test then else?) -> (if test else? then)
    register_macro(
        registry,
        "if-not",
        &["test", "then"],
        Some("else-form"),
        "(if test (first else-form) then)",
    );

    // and - (and) -> true, (and x) -> x, (and x & more) -> (if x (and ~@more) x)
    // Note: This is a simplified version that expands one level at a time
    register_macro(registry, "and", &[], Some("forms"), "(and* ~@forms)");

    // or - (or) -> nil, (or x) -> x, (or x & more) -> (let [g# x] (if g# g# (or ~@more)))
    // Note: This is a simplified version
    register_macro(registry, "or", &[], Some("forms"), "(or* ~@forms)");

    // -> thread-first
    // (-> x) -> x
    // (-> x (f a b)) -> (f x a b)
    // (-> x (f a b) (g c)) -> (g (f x a b) c)
    register_macro(
        registry,
        "->",
        &["x"],
        Some("forms"),
        "(thread-first x ~@forms)",
    );

    // ->> thread-last
    // (->> x) -> x
    // (->> x (f a b)) -> (f a b x)
    // (->> x (f a b) (g c)) -> (g c (f a b x))
    register_macro(
        registry,
        "->>",
        &["x"],
        Some("forms"),
        "(thread-last x ~@forms)",
    );

    // cond - multi-way conditional
    // (cond) -> nil
    // (cond test1 expr1 test2 expr2 ...) -> (if test1 expr1 (if test2 expr2 ...))
    register_macro(registry, "cond", &[], Some("clauses"), "(cond* ~@clauses)");

    // doto - evaluate forms with first arg, return first arg
    // (doto x (f a) (g b)) -> (let [g# x] (f g# a) (g g# b) g#)
    register_macro(registry, "doto", &["x"], Some("forms"), "(doto* x ~@forms)");

    // comment - ignore forms, return nil
    register_macro(registry, "comment", &[], Some("body"), "nil");
}

/// Helper function to register a macro with the given definition.
fn register_macro(
    registry: &mut MacroRegistry,
    name: &str,
    normal_params: &[&str],
    rest_param: Option<&str>,
    body_template: &str,
) {
    let mut params = Vec::new();

    for &param in normal_params {
        params.push(MacroParam::Normal(param.to_string()));
    }

    if let Some(rest) = rest_param {
        params.push(MacroParam::Rest(rest.to_string()));
    }

    // Parse the body template
    let body = match parse(body_template) {
        Ok(asts) => asts,
        Err(e) => {
            // This is a programming error - stdlib macros should always parse
            panic!("Failed to parse stdlib macro '{name}' body: {e}");
        }
    };

    let def = MacroDef::new(
        name.to_string(),
        "core".to_string(),
        params,
        body,
        Span::default(),
    );

    registry.register(def);
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Ast;
    use crate::macro_expander::MacroExpander;

    fn expand_with_stdlib(source: &str) -> crate::ast::Ast {
        let mut registry = MacroRegistry::new();
        register_stdlib_macros(&mut registry);
        let mut macro_expander = MacroExpander::new(&mut registry);
        let forms = parse(source).unwrap();
        let result = macro_expander.expand_all(&forms).unwrap();
        result.into_iter().next().unwrap()
    }

    #[test]
    fn stdlib_macros_registered() {
        let mut registry = MacroRegistry::new();
        register_stdlib_macros(&mut registry);

        assert!(registry.is_macro("when"));
        assert!(registry.is_macro("when-not"));
        assert!(registry.is_macro("if-not"));
        assert!(registry.is_macro("->"));
        assert!(registry.is_macro("->>"));
        assert!(registry.is_macro("cond"));
        assert!(registry.is_macro("comment"));
    }

    #[test]
    fn when_expands_correctly() {
        let result = expand_with_stdlib("(when true 1 2 3)");

        // Should expand to (if true (do 1 2 3) nil)
        match &result {
            Ast::List(elements, _) => {
                assert!(matches!(&elements[0], Ast::Symbol(s, _) if s == "if"));
                // The test condition
                assert!(matches!(&elements[1], Ast::Bool(true, _)));
                // The (do ...) form
                match &elements[2] {
                    Ast::List(do_elems, _) => {
                        assert!(matches!(&do_elems[0], Ast::Symbol(s, _) if s == "do"));
                        assert_eq!(do_elems.len(), 4); // do + 3 body forms
                    }
                    _ => panic!("Expected do form"),
                }
                // The nil else
                assert!(matches!(&elements[3], Ast::Nil(_)));
            }
            _ => panic!("Expected list"),
        }
    }

    #[test]
    fn when_not_expands_correctly() {
        let result = expand_with_stdlib("(when-not false 1 2)");

        // Should expand to (if false nil (do 1 2))
        match &result {
            Ast::List(elements, _) => {
                assert!(matches!(&elements[0], Ast::Symbol(s, _) if s == "if"));
                assert!(matches!(&elements[1], Ast::Bool(false, _)));
                assert!(matches!(&elements[2], Ast::Nil(_)));
                match &elements[3] {
                    Ast::List(do_elems, _) => {
                        assert!(matches!(&do_elems[0], Ast::Symbol(s, _) if s == "do"));
                    }
                    _ => panic!("Expected do form"),
                }
            }
            _ => panic!("Expected list"),
        }
    }

    #[test]
    fn comment_returns_nil() {
        let result = expand_with_stdlib("(comment (+ 1 2) (println \"hi\"))");

        // Should expand to nil
        assert!(matches!(result, Ast::Nil(_)));
    }

    #[test]
    fn if_not_expands_correctly() {
        let result = expand_with_stdlib("(if-not false 1 2)");

        // Should expand to (if false (first else-form) then)
        // where else-form is [2] and then is 1
        match &result {
            Ast::List(elements, _) => {
                assert!(matches!(&elements[0], Ast::Symbol(s, _) if s == "if"));
                assert!(matches!(&elements[1], Ast::Bool(false, _)));
            }
            _ => panic!("Expected list"),
        }
    }

    #[test]
    fn and_expands_to_and_star() {
        let result = expand_with_stdlib("(and true false)");

        // Should expand to (and* true false)
        match &result {
            Ast::List(elements, _) => {
                assert!(matches!(&elements[0], Ast::Symbol(s, _) if s == "and*"));
                assert!(matches!(&elements[1], Ast::Bool(true, _)));
                assert!(matches!(&elements[2], Ast::Bool(false, _)));
            }
            _ => panic!("Expected list"),
        }
    }

    #[test]
    fn or_expands_to_or_star() {
        let result = expand_with_stdlib("(or nil true)");

        // Should expand to (or* nil true)
        match &result {
            Ast::List(elements, _) => {
                assert!(matches!(&elements[0], Ast::Symbol(s, _) if s == "or*"));
                assert!(matches!(&elements[1], Ast::Nil(_)));
                assert!(matches!(&elements[2], Ast::Bool(true, _)));
            }
            _ => panic!("Expected list"),
        }
    }

    #[test]
    fn thread_first_expands_correctly() {
        let result = expand_with_stdlib("(-> 1 (+ 2) (* 3))");

        // Should expand to (thread-first 1 (+ 2) (* 3))
        match &result {
            Ast::List(elements, _) => {
                assert!(matches!(&elements[0], Ast::Symbol(s, _) if s == "thread-first"));
                assert!(matches!(&elements[1], Ast::Int(1, _)));
            }
            _ => panic!("Expected list"),
        }
    }

    #[test]
    fn thread_last_expands_correctly() {
        let result = expand_with_stdlib("(->> 1 (+ 2) (* 3))");

        // Should expand to (thread-last 1 (+ 2) (* 3))
        match &result {
            Ast::List(elements, _) => {
                assert!(matches!(&elements[0], Ast::Symbol(s, _) if s == "thread-last"));
                assert!(matches!(&elements[1], Ast::Int(1, _)));
            }
            _ => panic!("Expected list"),
        }
    }

    #[test]
    fn cond_expands_to_cond_star() {
        let result = expand_with_stdlib("(cond true 1 false 2)");

        // Should expand to (cond* true 1 false 2)
        match &result {
            Ast::List(elements, _) => {
                assert!(matches!(&elements[0], Ast::Symbol(s, _) if s == "cond*"));
                assert_eq!(elements.len(), 5); // cond* + 4 args
            }
            _ => panic!("Expected list"),
        }
    }

    #[test]
    fn doto_expands_to_doto_star() {
        let result = expand_with_stdlib("(doto x (f 1) (g 2))");

        // Should expand to (doto* x (f 1) (g 2))
        match &result {
            Ast::List(elements, _) => {
                assert!(matches!(&elements[0], Ast::Symbol(s, _) if s == "doto*"));
                assert!(matches!(&elements[1], Ast::Symbol(s, _) if s == "x"));
            }
            _ => panic!("Expected list"),
        }
    }

    // =========================================================================
    // End-to-end execution tests
    // =========================================================================

    use crate::vm::eval;
    use longtable_foundation::Value;

    #[test]
    fn eval_when_truthy() {
        let result = eval("(when true 1 2 3)").unwrap();
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn eval_when_falsy() {
        let result = eval("(when false 1 2 3)").unwrap();
        assert_eq!(result, Value::Nil);
    }

    #[test]
    fn eval_when_not_truthy() {
        let result = eval("(when-not true 1 2 3)").unwrap();
        assert_eq!(result, Value::Nil);
    }

    #[test]
    fn eval_when_not_falsy() {
        let result = eval("(when-not false 1 2 3)").unwrap();
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn eval_and_empty() {
        let result = eval("(and)").unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn eval_and_single() {
        let result = eval("(and 42)").unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn eval_and_all_truthy() {
        let result = eval("(and 1 2 3)").unwrap();
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn eval_and_short_circuit() {
        // Should return first falsy value
        let result = eval("(and 1 nil 3)").unwrap();
        assert_eq!(result, Value::Nil);
    }

    #[test]
    fn eval_and_false_value() {
        let result = eval("(and 1 false 3)").unwrap();
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn eval_or_empty() {
        let result = eval("(or)").unwrap();
        assert_eq!(result, Value::Nil);
    }

    #[test]
    fn eval_or_single() {
        let result = eval("(or 42)").unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn eval_or_first_truthy() {
        let result = eval("(or 1 2 3)").unwrap();
        assert_eq!(result, Value::Int(1));
    }

    #[test]
    fn eval_or_short_circuit() {
        // Should return first truthy value
        let result = eval("(or nil false 42)").unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn eval_or_all_falsy() {
        let result = eval("(or nil false)").unwrap();
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn eval_cond_empty() {
        let result = eval("(cond)").unwrap();
        assert_eq!(result, Value::Nil);
    }

    #[test]
    fn eval_cond_first_match() {
        let result = eval("(cond true 1 true 2)").unwrap();
        assert_eq!(result, Value::Int(1));
    }

    #[test]
    fn eval_cond_second_match() {
        let result = eval("(cond false 1 true 2)").unwrap();
        assert_eq!(result, Value::Int(2));
    }

    #[test]
    fn eval_cond_no_match() {
        let result = eval("(cond false 1 false 2)").unwrap();
        assert_eq!(result, Value::Nil);
    }

    #[test]
    fn eval_cond_with_else() {
        // :else is just a truthy keyword
        let result = eval("(cond false 1 :else 2)").unwrap();
        assert_eq!(result, Value::Int(2));
    }

    #[test]
    fn eval_thread_first_single() {
        let result = eval("(-> 1)").unwrap();
        assert_eq!(result, Value::Int(1));
    }

    #[test]
    fn eval_thread_first_one_form() {
        // (-> 1 (+ 2)) -> (+ 1 2) -> 3
        let result = eval("(-> 1 (+ 2))").unwrap();
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn eval_thread_first_multiple_forms() {
        // (-> 1 (+ 2) (* 3)) -> (* (+ 1 2) 3) -> (* 3 3) -> 9
        let result = eval("(-> 1 (+ 2) (* 3))").unwrap();
        assert_eq!(result, Value::Int(9));
    }

    #[test]
    fn eval_thread_last_single() {
        let result = eval("(->> 1)").unwrap();
        assert_eq!(result, Value::Int(1));
    }

    #[test]
    fn eval_thread_last_one_form() {
        // (->> 1 (+ 2)) -> (+ 2 1) -> 3
        let result = eval("(->> 1 (+ 2))").unwrap();
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn eval_thread_last_multiple_forms() {
        // (->> 2 (- 10) (* 3)) -> (* 3 (- 10 2)) -> (* 3 8) -> 24
        let result = eval("(->> 2 (- 10) (* 3))").unwrap();
        assert_eq!(result, Value::Int(24));
    }

    #[test]
    fn eval_comment() {
        let result = eval("(comment (+ 1 2) (/ 1 0))").unwrap();
        assert_eq!(result, Value::Nil);
    }

    #[test]
    fn eval_nested_macros() {
        // (when (and true true) (-> 1 (+ 2)))
        let result = eval("(when (and true true) (-> 1 (+ 2)))").unwrap();
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn eval_complex_cond() {
        // Use integers instead of keywords for cleaner test
        let result = eval("(cond (= 1 2) 10 (= 2 2) 20 true 30)").unwrap();
        // Should return 20 (second clause matches)
        assert_eq!(result, Value::Int(20));
    }
}
