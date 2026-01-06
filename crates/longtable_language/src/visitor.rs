//! AST visitor pattern for traversing and transforming syntax trees.
//!
//! This module provides two main traits:
//! - [`AstVisitor`] - For read-only traversal of ASTs
//! - [`AstTransform`] - For transforming ASTs into new ASTs
//!
//! # Example
//!
//! ```
//! use longtable_language::{parse, Ast};
//! use longtable_language::visitor::{AstVisitor, walk_ast};
//!
//! struct SymbolCounter(usize);
//!
//! impl AstVisitor for SymbolCounter {
//!     fn visit_symbol(&mut self, _name: &str, _span: longtable_language::Span) {
//!         self.0 += 1;
//!     }
//! }
//!
//! let ast = parse("(+ x y z)").unwrap();
//! let mut counter = SymbolCounter(0);
//! for node in &ast {
//!     walk_ast(&mut counter, node);
//! }
//! assert_eq!(counter.0, 4); // +, x, y, z
//! ```

use crate::ast::Ast;
use crate::span::Span;

// =============================================================================
// Read-Only Visitor
// =============================================================================

/// Trait for read-only AST visitors.
///
/// Implement specific `visit_*` methods to handle nodes of interest.
/// The default implementations do nothing.
///
/// Use [`walk_ast`] to traverse the AST and call visitor methods.
#[allow(unused_variables)]
pub trait AstVisitor {
    /// Called when entering any AST node (before type-specific visit).
    fn enter_node(&mut self, ast: &Ast) {}

    /// Called when leaving any AST node (after type-specific visit and children).
    fn leave_node(&mut self, ast: &Ast) {}

    /// Visit a nil literal.
    fn visit_nil(&mut self, span: Span) {}

    /// Visit a boolean literal.
    fn visit_bool(&mut self, value: bool, span: Span) {}

    /// Visit an integer literal.
    fn visit_int(&mut self, value: i64, span: Span) {}

    /// Visit a float literal.
    fn visit_float(&mut self, value: f64, span: Span) {}

    /// Visit a string literal.
    fn visit_string(&mut self, value: &str, span: Span) {}

    /// Visit a symbol.
    fn visit_symbol(&mut self, name: &str, span: Span) {}

    /// Visit a keyword.
    fn visit_keyword(&mut self, name: &str, span: Span) {}

    /// Called before visiting list elements.
    fn enter_list(&mut self, elements: &[Ast], span: Span) {}

    /// Called after visiting list elements.
    fn leave_list(&mut self, elements: &[Ast], span: Span) {}

    /// Called before visiting vector elements.
    fn enter_vector(&mut self, elements: &[Ast], span: Span) {}

    /// Called after visiting vector elements.
    fn leave_vector(&mut self, elements: &[Ast], span: Span) {}

    /// Called before visiting set elements.
    fn enter_set(&mut self, elements: &[Ast], span: Span) {}

    /// Called after visiting set elements.
    fn leave_set(&mut self, elements: &[Ast], span: Span) {}

    /// Called before visiting map entries.
    fn enter_map(&mut self, entries: &[(Ast, Ast)], span: Span) {}

    /// Called after visiting map entries.
    fn leave_map(&mut self, entries: &[(Ast, Ast)], span: Span) {}

    /// Called before visiting quoted expression.
    fn enter_quote(&mut self, inner: &Ast, span: Span) {}

    /// Called after visiting quoted expression.
    fn leave_quote(&mut self, inner: &Ast, span: Span) {}

    /// Called before visiting unquoted expression.
    fn enter_unquote(&mut self, inner: &Ast, span: Span) {}

    /// Called after visiting unquoted expression.
    fn leave_unquote(&mut self, inner: &Ast, span: Span) {}

    /// Called before visiting unquote-spliced expression.
    fn enter_unquote_splice(&mut self, inner: &Ast, span: Span) {}

    /// Called after visiting unquote-spliced expression.
    fn leave_unquote_splice(&mut self, inner: &Ast, span: Span) {}

    /// Called before visiting syntax-quoted expression.
    fn enter_syntax_quote(&mut self, inner: &Ast, span: Span) {}

    /// Called after visiting syntax-quoted expression.
    fn leave_syntax_quote(&mut self, inner: &Ast, span: Span) {}

    /// Called before visiting tagged literal.
    fn enter_tagged(&mut self, tag: &str, inner: &Ast, span: Span) {}

    /// Called after visiting tagged literal.
    fn leave_tagged(&mut self, tag: &str, inner: &Ast, span: Span) {}
}

/// Walk an AST node, calling appropriate visitor methods.
///
/// This function traverses the AST depth-first, calling:
/// 1. `enter_node` for every node
/// 2. Type-specific `visit_*` or `enter_*` method
/// 3. Recursively walks children
/// 4. Type-specific `leave_*` method (for container types)
/// 5. `leave_node` for every node
pub fn walk_ast<V: AstVisitor>(visitor: &mut V, ast: &Ast) {
    visitor.enter_node(ast);

    match ast {
        Ast::Nil(span) => visitor.visit_nil(*span),
        Ast::Bool(value, span) => visitor.visit_bool(*value, *span),
        Ast::Int(value, span) => visitor.visit_int(*value, *span),
        Ast::Float(value, span) => visitor.visit_float(*value, *span),
        Ast::String(value, span) => visitor.visit_string(value, *span),
        Ast::Symbol(name, span) => visitor.visit_symbol(name, *span),
        Ast::Keyword(name, span) => visitor.visit_keyword(name, *span),

        Ast::List(elements, span) => {
            visitor.enter_list(elements, *span);
            for elem in elements {
                walk_ast(visitor, elem);
            }
            visitor.leave_list(elements, *span);
        }

        Ast::Vector(elements, span) => {
            visitor.enter_vector(elements, *span);
            for elem in elements {
                walk_ast(visitor, elem);
            }
            visitor.leave_vector(elements, *span);
        }

        Ast::Set(elements, span) => {
            visitor.enter_set(elements, *span);
            for elem in elements {
                walk_ast(visitor, elem);
            }
            visitor.leave_set(elements, *span);
        }

        Ast::Map(entries, span) => {
            visitor.enter_map(entries, *span);
            for (key, value) in entries {
                walk_ast(visitor, key);
                walk_ast(visitor, value);
            }
            visitor.leave_map(entries, *span);
        }

        Ast::Quote(inner, span) => {
            visitor.enter_quote(inner, *span);
            walk_ast(visitor, inner);
            visitor.leave_quote(inner, *span);
        }

        Ast::Unquote(inner, span) => {
            visitor.enter_unquote(inner, *span);
            walk_ast(visitor, inner);
            visitor.leave_unquote(inner, *span);
        }

        Ast::UnquoteSplice(inner, span) => {
            visitor.enter_unquote_splice(inner, *span);
            walk_ast(visitor, inner);
            visitor.leave_unquote_splice(inner, *span);
        }

        Ast::SyntaxQuote(inner, span) => {
            visitor.enter_syntax_quote(inner, *span);
            walk_ast(visitor, inner);
            visitor.leave_syntax_quote(inner, *span);
        }

        Ast::Tagged(tag, inner, span) => {
            visitor.enter_tagged(tag, inner, *span);
            walk_ast(visitor, inner);
            visitor.leave_tagged(tag, inner, *span);
        }
    }

    visitor.leave_node(ast);
}

/// Walk multiple AST nodes in sequence.
pub fn walk_all<V: AstVisitor>(visitor: &mut V, asts: &[Ast]) {
    for ast in asts {
        walk_ast(visitor, ast);
    }
}

// =============================================================================
// Transforming Visitor
// =============================================================================

/// Trait for AST transformations.
///
/// Each method receives an AST node and returns a transformed version.
/// Default implementations clone the node unchanged while recursively
/// transforming children.
pub trait AstTransform {
    /// Transform a nil literal.
    fn transform_nil(&mut self, span: Span) -> Ast {
        Ast::Nil(span)
    }

    /// Transform a boolean literal.
    fn transform_bool(&mut self, value: bool, span: Span) -> Ast {
        Ast::Bool(value, span)
    }

    /// Transform an integer literal.
    fn transform_int(&mut self, value: i64, span: Span) -> Ast {
        Ast::Int(value, span)
    }

    /// Transform a float literal.
    fn transform_float(&mut self, value: f64, span: Span) -> Ast {
        Ast::Float(value, span)
    }

    /// Transform a string literal.
    fn transform_string(&mut self, value: String, span: Span) -> Ast {
        Ast::String(value, span)
    }

    /// Transform a symbol.
    fn transform_symbol(&mut self, name: String, span: Span) -> Ast {
        Ast::Symbol(name, span)
    }

    /// Transform a keyword.
    fn transform_keyword(&mut self, name: String, span: Span) -> Ast {
        Ast::Keyword(name, span)
    }

    /// Transform a list.
    fn transform_list(&mut self, elements: Vec<Ast>, span: Span) -> Ast {
        let transformed: Vec<_> = elements
            .into_iter()
            .map(|e| transform_ast(self, e))
            .collect();
        Ast::List(transformed, span)
    }

    /// Transform a vector.
    fn transform_vector(&mut self, elements: Vec<Ast>, span: Span) -> Ast {
        let transformed: Vec<_> = elements
            .into_iter()
            .map(|e| transform_ast(self, e))
            .collect();
        Ast::Vector(transformed, span)
    }

    /// Transform a set.
    fn transform_set(&mut self, elements: Vec<Ast>, span: Span) -> Ast {
        let transformed: Vec<_> = elements
            .into_iter()
            .map(|e| transform_ast(self, e))
            .collect();
        Ast::Set(transformed, span)
    }

    /// Transform a map.
    fn transform_map(&mut self, entries: Vec<(Ast, Ast)>, span: Span) -> Ast {
        let transformed: Vec<_> = entries
            .into_iter()
            .map(|(k, v)| (transform_ast(self, k), transform_ast(self, v)))
            .collect();
        Ast::Map(transformed, span)
    }

    /// Transform a quoted expression.
    fn transform_quote(&mut self, inner: Ast, span: Span) -> Ast {
        Ast::Quote(Box::new(transform_ast(self, inner)), span)
    }

    /// Transform an unquoted expression.
    fn transform_unquote(&mut self, inner: Ast, span: Span) -> Ast {
        Ast::Unquote(Box::new(transform_ast(self, inner)), span)
    }

    /// Transform an unquote-spliced expression.
    fn transform_unquote_splice(&mut self, inner: Ast, span: Span) -> Ast {
        Ast::UnquoteSplice(Box::new(transform_ast(self, inner)), span)
    }

    /// Transform a syntax-quoted expression.
    fn transform_syntax_quote(&mut self, inner: Ast, span: Span) -> Ast {
        Ast::SyntaxQuote(Box::new(transform_ast(self, inner)), span)
    }

    /// Transform a tagged literal.
    fn transform_tagged(&mut self, tag: String, inner: Ast, span: Span) -> Ast {
        Ast::Tagged(tag, Box::new(transform_ast(self, inner)), span)
    }
}

/// Transform an AST node using a transformer.
pub fn transform_ast<T: AstTransform + ?Sized>(transformer: &mut T, ast: Ast) -> Ast {
    match ast {
        Ast::Nil(span) => transformer.transform_nil(span),
        Ast::Bool(value, span) => transformer.transform_bool(value, span),
        Ast::Int(value, span) => transformer.transform_int(value, span),
        Ast::Float(value, span) => transformer.transform_float(value, span),
        Ast::String(value, span) => transformer.transform_string(value, span),
        Ast::Symbol(name, span) => transformer.transform_symbol(name, span),
        Ast::Keyword(name, span) => transformer.transform_keyword(name, span),
        Ast::List(elements, span) => transformer.transform_list(elements, span),
        Ast::Vector(elements, span) => transformer.transform_vector(elements, span),
        Ast::Set(elements, span) => transformer.transform_set(elements, span),
        Ast::Map(entries, span) => transformer.transform_map(entries, span),
        Ast::Quote(inner, span) => transformer.transform_quote(*inner, span),
        Ast::Unquote(inner, span) => transformer.transform_unquote(*inner, span),
        Ast::UnquoteSplice(inner, span) => transformer.transform_unquote_splice(*inner, span),
        Ast::SyntaxQuote(inner, span) => transformer.transform_syntax_quote(*inner, span),
        Ast::Tagged(tag, inner, span) => transformer.transform_tagged(tag, *inner, span),
    }
}

/// Transform multiple AST nodes.
pub fn transform_all<T: AstTransform + ?Sized>(transformer: &mut T, asts: Vec<Ast>) -> Vec<Ast> {
    asts.into_iter()
        .map(|ast| transform_ast(transformer, ast))
        .collect()
}

// =============================================================================
// Utility Visitors
// =============================================================================

/// Collects all symbols referenced in an AST.
#[derive(Debug, Default)]
pub struct SymbolCollector {
    /// Collected symbol names.
    pub symbols: Vec<String>,
}

impl AstVisitor for SymbolCollector {
    fn visit_symbol(&mut self, name: &str, _span: Span) {
        self.symbols.push(name.to_string());
    }
}

/// Built-in special forms and known functions (for free variable analysis).
const BUILTINS: &[&str] = &[
    "if", "do", "let", "let*", "fn", "fn*", "def", "defn", "quote", "loop", "recur", "and", "or",
    "not", "+", "-", "*", "/", "=", "<", ">", "<=", ">=", "nil", "true", "false",
];

/// Collects all free variables (symbols not defined locally).
#[derive(Debug, Default)]
pub struct FreeVariableCollector {
    /// Symbols that appear to be free variables.
    pub free_vars: Vec<String>,
    /// Symbols that are defined (bound).
    pub bound: Vec<String>,
}

impl AstVisitor for FreeVariableCollector {
    fn enter_list(&mut self, elements: &[Ast], _span: Span) {
        // Check for binding forms like (let [x 1] ...) or (fn [x] ...)
        if let Some(Ast::Symbol(head, _)) = elements.first() {
            match head.as_str() {
                "let" | "let*" | "loop" => {
                    // Binding vector is the second element
                    if let Some(Ast::Vector(bindings, _)) = elements.get(1) {
                        // Bindings are alternating name/value pairs
                        for (i, binding) in bindings.iter().enumerate() {
                            if i % 2 == 0 {
                                if let Ast::Symbol(name, _) = binding {
                                    self.bound.push(name.clone());
                                }
                            }
                        }
                    }
                }
                "fn" | "fn*" => {
                    // Parameter vector is the second element
                    if let Some(Ast::Vector(params, _)) = elements.get(1) {
                        for param in params {
                            if let Ast::Symbol(name, _) = param {
                                self.bound.push(name.clone());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn visit_symbol(&mut self, name: &str, _span: Span) {
        // Skip special symbols
        if name.starts_with(':') {
            return;
        }

        // Skip built-in special forms and known functions
        if BUILTINS.contains(&name) {
            return;
        }

        // If not bound, it's free
        if !self.bound.contains(&name.to_string()) && !self.free_vars.contains(&name.to_string()) {
            self.free_vars.push(name.to_string());
        }
    }
}

/// Computes the maximum depth of an AST.
#[derive(Debug, Default)]
pub struct DepthCalculator {
    current_depth: usize,
    /// Maximum depth encountered.
    pub max_depth: usize,
}

impl AstVisitor for DepthCalculator {
    fn enter_node(&mut self, _ast: &Ast) {
        self.current_depth += 1;
        if self.current_depth > self.max_depth {
            self.max_depth = self.current_depth;
        }
    }

    fn leave_node(&mut self, _ast: &Ast) {
        self.current_depth -= 1;
    }
}

/// Counts nodes by type.
#[derive(Debug, Default)]
pub struct NodeCounter {
    /// Count of nil nodes.
    pub nil_count: usize,
    /// Count of bool nodes.
    pub bool_count: usize,
    /// Count of int nodes.
    pub int_count: usize,
    /// Count of float nodes.
    pub float_count: usize,
    /// Count of string nodes.
    pub string_count: usize,
    /// Count of symbol nodes.
    pub symbol_count: usize,
    /// Count of keyword nodes.
    pub keyword_count: usize,
    /// Count of list nodes.
    pub list_count: usize,
    /// Count of vector nodes.
    pub vector_count: usize,
    /// Count of set nodes.
    pub set_count: usize,
    /// Count of map nodes.
    pub map_count: usize,
    /// Count of quote nodes.
    pub quote_count: usize,
    /// Count of unquote nodes.
    pub unquote_count: usize,
    /// Count of unquote-splice nodes.
    pub unquote_splice_count: usize,
    /// Count of syntax-quote nodes.
    pub syntax_quote_count: usize,
    /// Count of tagged literal nodes.
    pub tagged_count: usize,
}

impl NodeCounter {
    /// Returns the total number of nodes counted.
    #[must_use]
    pub fn total(&self) -> usize {
        self.nil_count
            + self.bool_count
            + self.int_count
            + self.float_count
            + self.string_count
            + self.symbol_count
            + self.keyword_count
            + self.list_count
            + self.vector_count
            + self.set_count
            + self.map_count
            + self.quote_count
            + self.unquote_count
            + self.unquote_splice_count
            + self.syntax_quote_count
            + self.tagged_count
    }
}

impl AstVisitor for NodeCounter {
    fn visit_nil(&mut self, _span: Span) {
        self.nil_count += 1;
    }
    fn visit_bool(&mut self, _value: bool, _span: Span) {
        self.bool_count += 1;
    }
    fn visit_int(&mut self, _value: i64, _span: Span) {
        self.int_count += 1;
    }
    fn visit_float(&mut self, _value: f64, _span: Span) {
        self.float_count += 1;
    }
    fn visit_string(&mut self, _value: &str, _span: Span) {
        self.string_count += 1;
    }
    fn visit_symbol(&mut self, _name: &str, _span: Span) {
        self.symbol_count += 1;
    }
    fn visit_keyword(&mut self, _name: &str, _span: Span) {
        self.keyword_count += 1;
    }
    fn enter_list(&mut self, _elements: &[Ast], _span: Span) {
        self.list_count += 1;
    }
    fn enter_vector(&mut self, _elements: &[Ast], _span: Span) {
        self.vector_count += 1;
    }
    fn enter_set(&mut self, _elements: &[Ast], _span: Span) {
        self.set_count += 1;
    }
    fn enter_map(&mut self, _entries: &[(Ast, Ast)], _span: Span) {
        self.map_count += 1;
    }
    fn enter_quote(&mut self, _inner: &Ast, _span: Span) {
        self.quote_count += 1;
    }
    fn enter_unquote(&mut self, _inner: &Ast, _span: Span) {
        self.unquote_count += 1;
    }
    fn enter_unquote_splice(&mut self, _inner: &Ast, _span: Span) {
        self.unquote_splice_count += 1;
    }
    fn enter_syntax_quote(&mut self, _inner: &Ast, _span: Span) {
        self.syntax_quote_count += 1;
    }
    fn enter_tagged(&mut self, _tag: &str, _inner: &Ast, _span: Span) {
        self.tagged_count += 1;
    }
}

// =============================================================================
// Utility Transformers
// =============================================================================

/// Renames symbols according to a mapping.
pub struct SymbolRenamer<F>
where
    F: FnMut(&str) -> Option<String>,
{
    rename_fn: F,
}

impl<F> SymbolRenamer<F>
where
    F: FnMut(&str) -> Option<String>,
{
    /// Create a new symbol renamer with the given rename function.
    pub fn new(rename_fn: F) -> Self {
        Self { rename_fn }
    }
}

impl<F> AstTransform for SymbolRenamer<F>
where
    F: FnMut(&str) -> Option<String>,
{
    fn transform_symbol(&mut self, name: String, span: Span) -> Ast {
        if let Some(new_name) = (self.rename_fn)(&name) {
            Ast::Symbol(new_name, span)
        } else {
            Ast::Symbol(name, span)
        }
    }
}

/// Strips all span information (sets to default).
#[derive(Default)]
pub struct SpanStripper;

impl AstTransform for SpanStripper {
    fn transform_nil(&mut self, _span: Span) -> Ast {
        Ast::Nil(Span::default())
    }
    fn transform_bool(&mut self, value: bool, _span: Span) -> Ast {
        Ast::Bool(value, Span::default())
    }
    fn transform_int(&mut self, value: i64, _span: Span) -> Ast {
        Ast::Int(value, Span::default())
    }
    fn transform_float(&mut self, value: f64, _span: Span) -> Ast {
        Ast::Float(value, Span::default())
    }
    fn transform_string(&mut self, value: String, _span: Span) -> Ast {
        Ast::String(value, Span::default())
    }
    fn transform_symbol(&mut self, name: String, _span: Span) -> Ast {
        Ast::Symbol(name, Span::default())
    }
    fn transform_keyword(&mut self, name: String, _span: Span) -> Ast {
        Ast::Keyword(name, Span::default())
    }
    fn transform_list(&mut self, elements: Vec<Ast>, _span: Span) -> Ast {
        let transformed: Vec<_> = elements
            .into_iter()
            .map(|e| transform_ast(self, e))
            .collect();
        Ast::List(transformed, Span::default())
    }
    fn transform_vector(&mut self, elements: Vec<Ast>, _span: Span) -> Ast {
        let transformed: Vec<_> = elements
            .into_iter()
            .map(|e| transform_ast(self, e))
            .collect();
        Ast::Vector(transformed, Span::default())
    }
    fn transform_set(&mut self, elements: Vec<Ast>, _span: Span) -> Ast {
        let transformed: Vec<_> = elements
            .into_iter()
            .map(|e| transform_ast(self, e))
            .collect();
        Ast::Set(transformed, Span::default())
    }
    fn transform_map(&mut self, entries: Vec<(Ast, Ast)>, _span: Span) -> Ast {
        let transformed: Vec<_> = entries
            .into_iter()
            .map(|(k, v)| (transform_ast(self, k), transform_ast(self, v)))
            .collect();
        Ast::Map(transformed, Span::default())
    }
    fn transform_quote(&mut self, inner: Ast, _span: Span) -> Ast {
        Ast::Quote(Box::new(transform_ast(self, inner)), Span::default())
    }
    fn transform_unquote(&mut self, inner: Ast, _span: Span) -> Ast {
        Ast::Unquote(Box::new(transform_ast(self, inner)), Span::default())
    }
    fn transform_unquote_splice(&mut self, inner: Ast, _span: Span) -> Ast {
        Ast::UnquoteSplice(Box::new(transform_ast(self, inner)), Span::default())
    }
    fn transform_syntax_quote(&mut self, inner: Ast, _span: Span) -> Ast {
        Ast::SyntaxQuote(Box::new(transform_ast(self, inner)), Span::default())
    }
    fn transform_tagged(&mut self, tag: String, inner: Ast, _span: Span) -> Ast {
        Ast::Tagged(tag, Box::new(transform_ast(self, inner)), Span::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[test]
    fn symbol_collector_gathers_all_symbols() {
        let ast = parse("(+ x (* y z))").unwrap();
        let mut collector = SymbolCollector::default();
        walk_all(&mut collector, &ast);

        assert_eq!(collector.symbols, vec!["+", "x", "*", "y", "z"]);
    }

    #[test]
    fn depth_calculator_computes_max_depth() {
        // (+ 1 2) has depth 2: list -> elements
        let ast = parse("(+ 1 2)").unwrap();
        let mut calc = DepthCalculator::default();
        walk_all(&mut calc, &ast);
        assert_eq!(calc.max_depth, 2);

        // ((a)) has depth 3
        let ast = parse("((a))").unwrap();
        let mut calc = DepthCalculator::default();
        walk_all(&mut calc, &ast);
        assert_eq!(calc.max_depth, 3);
    }

    #[test]
    fn node_counter_counts_all_types() {
        let ast = parse("(foo 1 2.5 \"hi\" :key [a b] {:x 1})").unwrap();
        let mut counter = NodeCounter::default();
        walk_all(&mut counter, &ast);

        assert_eq!(counter.list_count, 1);
        assert_eq!(counter.symbol_count, 3); // foo, a, b
        assert_eq!(counter.int_count, 2); // 1, 1
        assert_eq!(counter.float_count, 1); // 2.5
        assert_eq!(counter.string_count, 1); // "hi"
        assert_eq!(counter.keyword_count, 2); // :key, :x
        assert_eq!(counter.vector_count, 1); // [a b]
        assert_eq!(counter.map_count, 1); // {:x 1}
    }

    #[test]
    fn symbol_renamer_renames_matching_symbols() {
        let ast = parse("(+ x y)").unwrap();
        let mut renamer = SymbolRenamer::new(|name| {
            if name == "x" {
                Some("renamed_x".to_string())
            } else {
                None
            }
        });
        let transformed = transform_all(&mut renamer, ast);

        // Check the result has renamed_x
        let mut collector = SymbolCollector::default();
        walk_all(&mut collector, &transformed);
        assert!(collector.symbols.contains(&"renamed_x".to_string()));
        assert!(!collector.symbols.contains(&"x".to_string()));
    }

    #[test]
    fn span_stripper_removes_all_spans() {
        let ast = parse("(+ 1 2)").unwrap();
        // Original has non-default spans
        assert_ne!(ast[0].span(), Span::default());

        let mut stripper = SpanStripper;
        let transformed = transform_all(&mut stripper, ast);

        // All spans are now default
        assert_eq!(transformed[0].span(), Span::default());
    }

    #[test]
    fn free_variable_collector_finds_free_vars() {
        // x is free, y is bound by let
        let ast = parse("(let [y 1] (+ x y))").unwrap();
        let mut collector = FreeVariableCollector::default();
        walk_all(&mut collector, &ast);

        assert!(collector.free_vars.contains(&"x".to_string()));
        assert!(!collector.free_vars.contains(&"y".to_string()));
    }

    #[test]
    fn visitor_handles_nested_structures() {
        let ast = parse("[[1 2] [3 [4 5]]]").unwrap();
        let mut counter = NodeCounter::default();
        walk_all(&mut counter, &ast);

        assert_eq!(counter.vector_count, 4); // Outer, [1 2], [3 [4 5]], [4 5]
        assert_eq!(counter.int_count, 5); // 1, 2, 3, 4, 5
    }

    #[test]
    fn visitor_handles_maps() {
        let ast = parse("{:a 1 :b {:c 2}}").unwrap();
        let mut counter = NodeCounter::default();
        walk_all(&mut counter, &ast);

        assert_eq!(counter.map_count, 2);
        assert_eq!(counter.keyword_count, 3); // :a, :b, :c
        assert_eq!(counter.int_count, 2); // 1, 2
    }

    #[test]
    fn visitor_handles_quotes() {
        let ast = parse("'(a b c)").unwrap();
        let mut counter = NodeCounter::default();
        walk_all(&mut counter, &ast);

        assert_eq!(counter.quote_count, 1);
        assert_eq!(counter.list_count, 1);
        assert_eq!(counter.symbol_count, 3);
    }

    #[test]
    fn enter_leave_called_in_order() {
        #[derive(Default)]
        struct OrderTracker {
            events: Vec<String>,
        }

        impl AstVisitor for OrderTracker {
            fn enter_list(&mut self, _elements: &[Ast], _span: Span) {
                self.events.push("enter_list".to_string());
            }
            fn leave_list(&mut self, _elements: &[Ast], _span: Span) {
                self.events.push("leave_list".to_string());
            }
            fn visit_symbol(&mut self, name: &str, _span: Span) {
                self.events.push(format!("symbol:{name}"));
            }
        }

        let ast = parse("(a b)").unwrap();
        let mut tracker = OrderTracker::default();
        walk_all(&mut tracker, &ast);

        assert_eq!(
            tracker.events,
            vec!["enter_list", "symbol:a", "symbol:b", "leave_list"]
        );
    }
}
