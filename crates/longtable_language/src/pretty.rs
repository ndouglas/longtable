//! Pretty-printer for AST nodes.
//!
//! This module provides functionality to convert AST nodes back to
//! human-readable Longtable source code.
//!
//! # Example
//!
//! ```
//! use longtable_language::{parse, pretty::pretty_print};
//!
//! let ast = parse("(+ 1 2)").unwrap();
//! let source = pretty_print(&ast[0]);
//! assert_eq!(source, "(+ 1 2)");
//! ```

use std::fmt::Write;

use crate::ast::Ast;

/// Configuration for pretty-printing.
#[derive(Debug, Clone)]
pub struct PrettyConfig {
    /// Number of spaces for each indentation level.
    pub indent_width: usize,
    /// Maximum line width before breaking.
    pub max_width: usize,
    /// Whether to use multi-line formatting for collections.
    pub multi_line_collections: bool,
}

impl Default for PrettyConfig {
    fn default() -> Self {
        Self {
            indent_width: 2,
            max_width: 80,
            multi_line_collections: false,
        }
    }
}

/// Pretty-print an AST node to a string.
#[must_use]
pub fn pretty_print(ast: &Ast) -> String {
    let mut printer = PrettyPrinter::new(PrettyConfig::default());
    printer.print(ast);
    printer.output
}

/// Pretty-print an AST node with custom configuration.
#[must_use]
pub fn pretty_print_with_config(ast: &Ast, config: PrettyConfig) -> String {
    let mut printer = PrettyPrinter::new(config);
    printer.print(ast);
    printer.output
}

/// Pretty-print multiple AST nodes, one per line.
#[must_use]
pub fn pretty_print_all(asts: &[Ast]) -> String {
    asts.iter().map(pretty_print).collect::<Vec<_>>().join("\n")
}

/// Pretty-print multiple AST nodes with custom configuration.
#[must_use]
pub fn pretty_print_all_with_config(asts: &[Ast], config: &PrettyConfig) -> String {
    asts.iter()
        .map(|ast| pretty_print_with_config(ast, config.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Pretty-printer state.
struct PrettyPrinter {
    config: PrettyConfig,
    output: String,
    indent_level: usize,
}

impl PrettyPrinter {
    fn new(config: PrettyConfig) -> Self {
        Self {
            config,
            output: String::new(),
            indent_level: 0,
        }
    }

    fn print(&mut self, ast: &Ast) {
        match ast {
            Ast::Nil(_) => self.output.push_str("nil"),
            Ast::Bool(true, _) => self.output.push_str("true"),
            Ast::Bool(false, _) => self.output.push_str("false"),
            Ast::Int(n, _) => self.output.push_str(&n.to_string()),
            Ast::Float(n, _) => self.print_float(*n),
            Ast::String(s, _) => self.print_string(s),
            Ast::Symbol(s, _) => self.output.push_str(s),
            Ast::Keyword(s, _) => {
                self.output.push(':');
                self.output.push_str(s);
            }
            Ast::List(elements, _) => self.print_list(elements),
            Ast::Vector(elements, _) => self.print_vector(elements),
            Ast::Set(elements, _) => self.print_set(elements),
            Ast::Map(entries, _) => self.print_map(entries),
            Ast::Quote(inner, _) => {
                self.output.push('\'');
                self.print(inner);
            }
            Ast::Unquote(inner, _) => {
                self.output.push('~');
                self.print(inner);
            }
            Ast::UnquoteSplice(inner, _) => {
                self.output.push_str("~@");
                self.print(inner);
            }
            Ast::SyntaxQuote(inner, _) => {
                self.output.push('`');
                self.print(inner);
            }
            Ast::Tagged(tag, inner, _) => {
                self.output.push('#');
                self.output.push_str(tag);
                self.print(inner);
            }
        }
    }

    fn print_float(&mut self, n: f64) {
        // Ensure we print floats with decimal point
        let s = n.to_string();
        self.output.push_str(&s);
        if !s.contains('.') && !s.contains('e') && !s.contains('E') {
            self.output.push_str(".0");
        }
    }

    fn print_string(&mut self, s: &str) {
        self.output.push('"');
        for c in s.chars() {
            match c {
                '"' => self.output.push_str("\\\""),
                '\\' => self.output.push_str("\\\\"),
                '\n' => self.output.push_str("\\n"),
                '\r' => self.output.push_str("\\r"),
                '\t' => self.output.push_str("\\t"),
                c if c.is_control() => {
                    // Print control characters as \uXXXX
                    let code = c as u32;
                    if code <= 0xFFFF {
                        let _ = write!(self.output, "\\u{code:04X}");
                    } else {
                        let _ = write!(self.output, "\\u{{{code:X}}}");
                    }
                }
                c => self.output.push(c),
            }
        }
        self.output.push('"');
    }

    fn print_list(&mut self, elements: &[Ast]) {
        self.output.push('(');
        self.print_elements(elements);
        self.output.push(')');
    }

    fn print_vector(&mut self, elements: &[Ast]) {
        self.output.push('[');
        self.print_elements(elements);
        self.output.push(']');
    }

    fn print_set(&mut self, elements: &[Ast]) {
        self.output.push_str("#{");
        self.print_elements(elements);
        self.output.push('}');
    }

    fn print_elements(&mut self, elements: &[Ast]) {
        for (i, elem) in elements.iter().enumerate() {
            if i > 0 {
                self.output.push(' ');
            }
            self.print(elem);
        }
    }

    fn print_map(&mut self, entries: &[(Ast, Ast)]) {
        self.output.push('{');
        for (i, (key, value)) in entries.iter().enumerate() {
            if i > 0 {
                self.output.push(' ');
            }
            self.print(key);
            self.output.push(' ');
            self.print(value);
        }
        self.output.push('}');
    }

    #[allow(dead_code)]
    fn indent(&self) -> String {
        " ".repeat(self.indent_level * self.config.indent_width)
    }

    #[allow(dead_code)]
    fn push_indent(&mut self) {
        self.indent_level += 1;
    }

    #[allow(dead_code)]
    fn pop_indent(&mut self) {
        self.indent_level = self.indent_level.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    // =========================================================================
    // Basic Type Round-Trips
    // =========================================================================

    #[test]
    fn roundtrip_nil() {
        assert_roundtrip("nil");
    }

    #[test]
    fn roundtrip_bool() {
        assert_roundtrip("true");
        assert_roundtrip("false");
    }

    #[test]
    fn roundtrip_int() {
        assert_roundtrip("42");
        assert_roundtrip("-123");
        assert_roundtrip("0");
    }

    #[test]
    fn roundtrip_float() {
        assert_roundtrip("3.14");
        assert_roundtrip("-2.5");
        assert_roundtrip("0.001");
        assert_roundtrip("1000.0");
    }

    #[test]
    fn roundtrip_string() {
        assert_roundtrip(r#""hello""#);
        assert_roundtrip(r#""world""#);
        assert_roundtrip(r#""""#); // empty string
    }

    #[test]
    fn roundtrip_string_escapes() {
        assert_roundtrip(r#""hello\nworld""#);
        assert_roundtrip(r#""tab\there""#);
        assert_roundtrip(r#""quote\"here""#);
        assert_roundtrip(r#""back\\slash""#);
    }

    #[test]
    fn roundtrip_symbol() {
        assert_roundtrip("foo");
        assert_roundtrip("bar/baz");
        assert_roundtrip("+");
        assert_roundtrip("->");
    }

    #[test]
    fn roundtrip_keyword() {
        assert_roundtrip(":foo");
        assert_roundtrip(":bar/baz");
        assert_roundtrip(":a");
    }

    // =========================================================================
    // Collection Round-Trips
    // =========================================================================

    #[test]
    fn roundtrip_list() {
        assert_roundtrip("()");
        assert_roundtrip("(a)");
        assert_roundtrip("(a b c)");
        assert_roundtrip("(+ 1 2)");
    }

    #[test]
    fn roundtrip_vector() {
        assert_roundtrip("[]");
        assert_roundtrip("[a]");
        assert_roundtrip("[1 2 3]");
        assert_roundtrip("[a b c]");
    }

    #[test]
    fn roundtrip_set() {
        assert_roundtrip("#{}");
        assert_roundtrip("#{a}");
        assert_roundtrip("#{1 2 3}");
    }

    #[test]
    fn roundtrip_map() {
        assert_roundtrip("{}");
        assert_roundtrip("{:a 1}");
        assert_roundtrip("{:a 1 :b 2}");
        assert_roundtrip("{\"key\" \"value\"}");
    }

    // =========================================================================
    // Quote Forms Round-Trips
    // =========================================================================

    #[test]
    fn roundtrip_quote() {
        assert_roundtrip("'x");
        assert_roundtrip("'(a b c)");
    }

    #[test]
    fn roundtrip_unquote() {
        assert_roundtrip("~x");
        assert_roundtrip("~(foo bar)");
    }

    #[test]
    fn roundtrip_unquote_splice() {
        assert_roundtrip("~@xs");
        assert_roundtrip("~@(foo bar)");
    }

    #[test]
    fn roundtrip_syntax_quote() {
        assert_roundtrip("`x");
        assert_roundtrip("`(a b ~c)");
    }

    // =========================================================================
    // Nested Structures Round-Trips
    // =========================================================================

    #[test]
    fn roundtrip_nested_lists() {
        assert_roundtrip("((a))");
        assert_roundtrip("(a (b (c)))");
        assert_roundtrip("((a b) (c d))");
    }

    #[test]
    fn roundtrip_nested_vectors() {
        assert_roundtrip("[[a]]");
        assert_roundtrip("[a [b [c]]]");
    }

    #[test]
    fn roundtrip_mixed_collections() {
        assert_roundtrip("(a [b] {:c 1})");
        assert_roundtrip("[{:a 1} {:b 2}]");
        assert_roundtrip("{:vec [1 2] :list (3 4)}");
    }

    // =========================================================================
    // Complex Expressions Round-Trips
    // =========================================================================

    #[test]
    fn roundtrip_def() {
        assert_roundtrip("(def x 42)");
        assert_roundtrip("(def greet \"hello\")");
    }

    #[test]
    fn roundtrip_fn() {
        assert_roundtrip("(fn [x] x)");
        assert_roundtrip("(fn [x y] (+ x y))");
    }

    #[test]
    fn roundtrip_let() {
        assert_roundtrip("(let [x 1] x)");
        assert_roundtrip("(let [x 1 y 2] (+ x y))");
    }

    #[test]
    fn roundtrip_if() {
        assert_roundtrip("(if true 1 2)");
        assert_roundtrip("(if (> x 0) \"pos\" \"neg\")");
    }

    // =========================================================================
    // Tagged Literals Round-Trips
    // =========================================================================

    #[test]
    fn roundtrip_tagged() {
        assert_roundtrip("#entity[1 2]");
        assert_roundtrip("#inst\"2024-01-01\"");
    }

    // =========================================================================
    // Semantic Equivalence Tests
    // =========================================================================

    #[test]
    fn semantic_equivalence_complex() {
        // Test that parse → print → parse yields semantically equivalent AST
        let sources = [
            "(defn add [a b] (+ a b))",
            "(let [x 1 y 2] (* x y))",
            "{:name \"Alice\" :age 30}",
            "[(fn [x] (* x x)) (fn [x] (+ x x))]",
            "'(quote form ~unquote ~@splice)",
        ];

        for source in sources {
            let ast1 = parse(source).expect("first parse failed");
            let printed = pretty_print_all(&ast1);
            let ast2 = parse(&printed).expect("second parse failed");

            assert_eq!(
                ast1.len(),
                ast2.len(),
                "AST length mismatch for: {source}\nPrinted: {printed}"
            );

            for (a, b) in ast1.iter().zip(ast2.iter()) {
                assert_ast_equivalent(a, b, source);
            }
        }
    }

    // =========================================================================
    // Helper Functions
    // =========================================================================

    /// Assert that source round-trips through parse → print → parse.
    fn assert_roundtrip(source: &str) {
        let ast1 = parse(source).unwrap_or_else(|e| panic!("Failed to parse '{source}': {e}"));
        assert_eq!(ast1.len(), 1, "Expected single expression for: {source}");

        let printed = pretty_print(&ast1[0]);
        let ast2 = parse(&printed)
            .unwrap_or_else(|e| panic!("Failed to re-parse '{printed}' (from '{source}'): {e}"));
        assert_eq!(
            ast2.len(),
            1,
            "Expected single expression after re-parse for: {source}"
        );

        assert_ast_equivalent(&ast1[0], &ast2[0], source);
    }

    /// Assert two AST nodes are semantically equivalent (ignoring spans).
    fn assert_ast_equivalent(a: &Ast, b: &Ast, context: &str) {
        match (a, b) {
            (Ast::Nil(_), Ast::Nil(_)) => {}
            (Ast::Bool(a, _), Ast::Bool(b, _)) => assert_eq!(a, b, "bool mismatch: {context}"),
            (Ast::Int(a, _), Ast::Int(b, _)) => assert_eq!(a, b, "int mismatch: {context}"),
            (Ast::Float(a, _), Ast::Float(b, _)) => {
                assert!(
                    (a - b).abs() < f64::EPSILON || (a.is_nan() && b.is_nan()),
                    "float mismatch: {a} != {b}: {context}"
                );
            }
            (Ast::String(a, _), Ast::String(b, _)) => {
                assert_eq!(a, b, "string mismatch: {context}");
            }
            (Ast::Symbol(a, _), Ast::Symbol(b, _)) => {
                assert_eq!(a, b, "symbol mismatch: {context}");
            }
            (Ast::Keyword(a, _), Ast::Keyword(b, _)) => {
                assert_eq!(a, b, "keyword mismatch: {context}");
            }
            (Ast::List(a, _), Ast::List(b, _)) => {
                assert_eq!(a.len(), b.len(), "list length mismatch: {context}");
                for (a, b) in a.iter().zip(b.iter()) {
                    assert_ast_equivalent(a, b, context);
                }
            }
            (Ast::Vector(a, _), Ast::Vector(b, _)) => {
                assert_eq!(a.len(), b.len(), "vector length mismatch: {context}");
                for (a, b) in a.iter().zip(b.iter()) {
                    assert_ast_equivalent(a, b, context);
                }
            }
            (Ast::Set(a, _), Ast::Set(b, _)) => {
                assert_eq!(a.len(), b.len(), "set length mismatch: {context}");
                for (a, b) in a.iter().zip(b.iter()) {
                    assert_ast_equivalent(a, b, context);
                }
            }
            (Ast::Map(a, _), Ast::Map(b, _)) => {
                assert_eq!(a.len(), b.len(), "map length mismatch: {context}");
                for ((ka, va), (kb, vb)) in a.iter().zip(b.iter()) {
                    assert_ast_equivalent(ka, kb, context);
                    assert_ast_equivalent(va, vb, context);
                }
            }
            (Ast::Quote(a, _), Ast::Quote(b, _)) => assert_ast_equivalent(a, b, context),
            (Ast::Unquote(a, _), Ast::Unquote(b, _)) => assert_ast_equivalent(a, b, context),
            (Ast::UnquoteSplice(a, _), Ast::UnquoteSplice(b, _)) => {
                assert_ast_equivalent(a, b, context);
            }
            (Ast::SyntaxQuote(a, _), Ast::SyntaxQuote(b, _)) => {
                assert_ast_equivalent(a, b, context);
            }
            (Ast::Tagged(ta, a, _), Ast::Tagged(tb, b, _)) => {
                assert_eq!(ta, tb, "tag mismatch: {context}");
                assert_ast_equivalent(a, b, context);
            }
            _ => panic!(
                "AST type mismatch: {} vs {}: {context}",
                a.type_name(),
                b.type_name()
            ),
        }
    }
}
