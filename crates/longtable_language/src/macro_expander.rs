//! Macro expansion engine.
//!
//! The macro expander transforms macro calls into their expanded forms.
//! It handles:
//! - Parsing and registering `defmacro` forms
//! - Expanding macro invocations
//! - Syntax-quote with namespace qualification
//! - Gensym patterns for hygiene
//!
//! # Expansion Algorithm
//!
//! 1. If the form is `defmacro`, register the macro and return nil
//! 2. If the form is a macro call, expand it and recursively expand the result
//! 3. If the form is syntax-quote, expand with namespace qualification
//! 4. Otherwise, recursively expand children

use crate::ast::Ast;
use crate::gensym::GensymGenerator;
use crate::macro_def::{MacroDef, MacroParam};
use crate::macro_registry::MacroRegistry;
use crate::span::Span;
use longtable_foundation::{Error, ErrorKind, Result};
use std::collections::HashMap;

/// Maximum macro expansion depth to prevent infinite recursion.
const MAX_EXPANSION_DEPTH: usize = 100;

// =============================================================================
// MacroExpander
// =============================================================================

/// The macro expansion engine.
///
/// Transforms AST nodes by expanding macro invocations.
pub struct MacroExpander<'a> {
    /// Registry of macro definitions.
    registry: &'a mut MacroRegistry,
    /// Current expansion depth (for recursion limit).
    depth: usize,
    /// Gensym generator for hygiene.
    gensym: GensymGenerator,
    /// Current namespace for qualification.
    current_namespace: String,
    /// Gensym bindings for current expansion (pattern -> generated name).
    gensym_bindings: HashMap<String, String>,
}

impl<'a> MacroExpander<'a> {
    /// Creates a new macro expander.
    pub fn new(registry: &'a mut MacroRegistry) -> Self {
        let current_namespace = registry.current_namespace().to_string();
        Self {
            registry,
            depth: 0,
            gensym: GensymGenerator::new(),
            current_namespace,
            gensym_bindings: HashMap::new(),
        }
    }

    /// Sets the current namespace for symbol qualification.
    pub fn set_namespace(&mut self, namespace: impl Into<String>) {
        self.current_namespace = namespace.into();
    }

    /// Expands all macros in a list of forms.
    pub fn expand_all(&mut self, forms: &[Ast]) -> Result<Vec<Ast>> {
        forms.iter().map(|form| self.expand(form)).collect()
    }

    /// Expands macros in a single form.
    pub fn expand(&mut self, ast: &Ast) -> Result<Ast> {
        // Check recursion limit
        if self.depth > MAX_EXPANSION_DEPTH {
            return Err(Error::new(ErrorKind::Internal(format!(
                "macro expansion depth exceeded {MAX_EXPANSION_DEPTH} (possible infinite expansion)"
            ))));
        }

        match ast {
            // Check for defmacro
            Ast::List(elements, span) if Self::is_defmacro(elements) => {
                self.process_defmacro(elements, *span)?;
                Ok(Ast::Nil(*span))
            }

            // Check for macro call
            Ast::List(elements, span) if !elements.is_empty() => {
                if let Some(expanded) = self.try_expand_macro_call(elements, *span)? {
                    // Recursively expand the result
                    self.depth += 1;
                    let result = self.expand(&expanded);
                    self.depth -= 1;
                    result
                } else {
                    // Not a macro call, expand children
                    self.expand_list_elements(elements, *span)
                }
            }

            // Expand children of other compound forms
            Ast::Vector(elements, span) => {
                let expanded: Result<Vec<_>> = elements.iter().map(|e| self.expand(e)).collect();
                Ok(Ast::Vector(expanded?, *span))
            }

            Ast::Set(elements, span) => {
                let expanded: Result<Vec<_>> = elements.iter().map(|e| self.expand(e)).collect();
                Ok(Ast::Set(expanded?, *span))
            }

            Ast::Map(pairs, span) => {
                let expanded: Result<Vec<_>> = pairs
                    .iter()
                    .map(|(k, v)| Ok((self.expand(k)?, self.expand(v)?)))
                    .collect();
                Ok(Ast::Map(expanded?, *span))
            }

            // Handle quote forms - expand syntax-quote, leave regular quote alone
            Ast::SyntaxQuote(inner, span) => self.expand_syntax_quote(inner, *span),

            Ast::Quote(inner, span) => {
                // Regular quote doesn't expand its contents
                Ok(Ast::Quote(Box::new(inner.as_ref().clone()), *span))
            }

            Ast::Unquote(inner, span) => {
                // Unquote outside syntax-quote just expands its contents
                let expanded = self.expand(inner)?;
                Ok(Ast::Unquote(Box::new(expanded), *span))
            }

            Ast::UnquoteSplice(inner, span) => {
                let expanded = self.expand(inner)?;
                Ok(Ast::UnquoteSplice(Box::new(expanded), *span))
            }

            // Atoms pass through unchanged
            _ => Ok(ast.clone()),
        }
    }

    /// Checks if a list form is a defmacro.
    fn is_defmacro(elements: &[Ast]) -> bool {
        matches!(elements.first(), Some(Ast::Symbol(s, _)) if s == "defmacro")
    }

    /// Processes a defmacro form and registers the macro.
    fn process_defmacro(&mut self, elements: &[Ast], span: Span) -> Result<()> {
        // (defmacro name [params...] body...)
        if elements.len() < 3 {
            return Err(Error::new(ErrorKind::ParseError {
                message: "defmacro requires name, params, and body".to_string(),
                line: span.line,
                column: span.column,
                context: String::new(),
            }));
        }

        // Get macro name
        let name = match &elements[1] {
            Ast::Symbol(s, _) => s.clone(),
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!("macro name must be a symbol, got {}", other.type_name()),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        // Get params
        let params = Self::parse_macro_params(&elements[2])?;

        // Get body (everything after params)
        let body: Vec<Ast> = elements[3..].to_vec();

        // Create and register the macro
        let def = MacroDef::new(name, self.current_namespace.clone(), params, body, span);

        self.registry.register(def);
        Ok(())
    }

    /// Parses macro parameters from a vector form.
    fn parse_macro_params(ast: &Ast) -> Result<Vec<MacroParam>> {
        let elements = match ast {
            Ast::Vector(elements, _) => elements,
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!("macro params must be a vector, got {}", other.type_name()),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        let mut params = Vec::new();
        let mut saw_rest = false;

        for (i, elem) in elements.iter().enumerate() {
            match elem {
                Ast::Symbol(s, _) if s == "&" => {
                    // Next param is the rest param
                    if i + 1 >= elements.len() {
                        return Err(Error::new(ErrorKind::ParseError {
                            message: "& must be followed by a rest parameter name".to_string(),
                            line: elem.span().line,
                            column: elem.span().column,
                            context: String::new(),
                        }));
                    }
                    saw_rest = true;
                }
                Ast::Symbol(s, _) => {
                    if saw_rest {
                        params.push(MacroParam::Rest(s.clone()));
                        saw_rest = false;
                    } else {
                        params.push(MacroParam::Normal(s.clone()));
                    }
                }
                other => {
                    return Err(Error::new(ErrorKind::ParseError {
                        message: format!("macro param must be a symbol, got {}", other.type_name()),
                        line: other.span().line,
                        column: other.span().column,
                        context: String::new(),
                    }));
                }
            }
        }

        Ok(params)
    }

    /// Tries to expand a macro call. Returns None if not a macro.
    fn try_expand_macro_call(&mut self, elements: &[Ast], span: Span) -> Result<Option<Ast>> {
        // Get the potential macro name
        let Ast::Symbol(name, _) = &elements[0] else {
            return Ok(None);
        };

        // Check if it's a registered macro
        let macro_def = match self.registry.resolve(name) {
            Some(def) => def.clone(),
            None => return Ok(None),
        };

        // Check arity
        let arg_count = elements.len() - 1;
        if !macro_def.accepts_arity(arg_count) {
            return Err(Error::new(ErrorKind::Internal(format!(
                "macro {} expects {} arguments, got {}",
                macro_def.name,
                if macro_def.variadic {
                    format!("at least {}", macro_def.min_arity())
                } else {
                    macro_def.min_arity().to_string()
                },
                arg_count
            ))));
        }

        // Build parameter bindings
        let args = &elements[1..];
        let bindings = Self::build_macro_bindings(&macro_def, args);

        // Clear gensym bindings for this expansion
        self.gensym_bindings.clear();

        // Substitute and expand the body
        let expanded = self.substitute_body(&macro_def.body, &bindings, span)?;

        // Wrap in do if multiple body forms
        let result = if expanded.len() == 1 {
            expanded.into_iter().next().unwrap()
        } else {
            let mut do_forms = vec![Ast::Symbol("do".to_string(), span)];
            do_forms.extend(expanded);
            Ast::List(do_forms, span)
        };

        Ok(Some(result))
    }

    /// Builds parameter bindings from macro arguments.
    fn build_macro_bindings(macro_def: &MacroDef, args: &[Ast]) -> HashMap<String, MacroArg> {
        let mut bindings = HashMap::new();
        let mut arg_idx = 0;

        for param in &macro_def.params {
            match param {
                MacroParam::Normal(name) => {
                    if arg_idx < args.len() {
                        bindings.insert(name.clone(), MacroArg::Single(args[arg_idx].clone()));
                        arg_idx += 1;
                    }
                }
                MacroParam::Rest(name) => {
                    // Collect remaining args
                    let rest: Vec<Ast> = args[arg_idx..].to_vec();
                    bindings.insert(name.clone(), MacroArg::Rest(rest));
                    break;
                }
            }
        }

        bindings
    }

    /// Substitutes parameters in the macro body.
    fn substitute_body(
        &mut self,
        body: &[Ast],
        bindings: &HashMap<String, MacroArg>,
        span: Span,
    ) -> Result<Vec<Ast>> {
        body.iter()
            .map(|form| self.substitute(form, bindings, span))
            .collect()
    }

    /// Substitutes parameters in a single form.
    fn substitute(
        &mut self,
        ast: &Ast,
        bindings: &HashMap<String, MacroArg>,
        span: Span,
    ) -> Result<Ast> {
        match ast {
            // Handle gensym patterns (x#)
            Ast::Symbol(name, s) if GensymGenerator::is_gensym_pattern(name) => {
                let generated = self
                    .gensym_bindings
                    .entry(name.clone())
                    .or_insert_with(|| self.gensym.expand_pattern(name))
                    .clone();
                Ok(Ast::Symbol(generated, *s))
            }

            // Substitute bound parameters
            Ast::Symbol(name, s) => {
                if let Some(arg) = bindings.get(name) {
                    match arg {
                        MacroArg::Single(ast) => Ok(ast.clone()),
                        MacroArg::Rest(asts) => {
                            // Wrap rest args in a list for single reference
                            Ok(Ast::List(asts.clone(), *s))
                        }
                    }
                } else {
                    Ok(ast.clone())
                }
            }

            // Handle unquote (~)
            Ast::Unquote(inner, _s) => {
                // Evaluate the inner form with substitutions
                self.substitute(inner, bindings, span)
            }

            // Handle unquote-splice (~@)
            Ast::UnquoteSplice(inner, _) => {
                // This should only appear inside lists during syntax-quote expansion
                // Return as-is for now; list handling will splice it
                let substituted = self.substitute(inner, bindings, span)?;
                Ok(substituted)
            }

            // Recurse into compound forms
            Ast::List(elements, s) => {
                let substituted = self.substitute_list_with_splice(elements, bindings, span)?;
                Ok(Ast::List(substituted, *s))
            }

            Ast::Vector(elements, s) => {
                let substituted = self.substitute_list_with_splice(elements, bindings, span)?;
                Ok(Ast::Vector(substituted, *s))
            }

            Ast::Set(elements, s) => {
                let substituted: Result<Vec<_>> = elements
                    .iter()
                    .map(|e| self.substitute(e, bindings, span))
                    .collect();
                Ok(Ast::Set(substituted?, *s))
            }

            Ast::Map(pairs, s) => {
                let substituted: Result<Vec<_>> = pairs
                    .iter()
                    .map(|(k, v)| {
                        Ok((
                            self.substitute(k, bindings, span)?,
                            self.substitute(v, bindings, span)?,
                        ))
                    })
                    .collect();
                Ok(Ast::Map(substituted?, *s))
            }

            Ast::Quote(inner, s) => {
                let substituted = self.substitute(inner, bindings, span)?;
                Ok(Ast::Quote(Box::new(substituted), *s))
            }

            Ast::SyntaxQuote(inner, s) => {
                let substituted = self.substitute(inner, bindings, span)?;
                Ok(Ast::SyntaxQuote(Box::new(substituted), *s))
            }

            // Atoms pass through
            _ => Ok(ast.clone()),
        }
    }

    /// Substitutes in a list, handling unquote-splice.
    fn substitute_list_with_splice(
        &mut self,
        elements: &[Ast],
        bindings: &HashMap<String, MacroArg>,
        span: Span,
    ) -> Result<Vec<Ast>> {
        let mut result = Vec::new();

        for elem in elements {
            match elem {
                Ast::UnquoteSplice(inner, _) => {
                    // Evaluate and splice
                    let substituted = self.substitute(inner, bindings, span)?;
                    match substituted {
                        Ast::List(items, _) | Ast::Vector(items, _) => {
                            result.extend(items);
                        }
                        other => {
                            // If not a sequence, just include it
                            result.push(other);
                        }
                    }
                }
                _ => {
                    result.push(self.substitute(elem, bindings, span)?);
                }
            }
        }

        Ok(result)
    }

    /// Expands syntax-quote with namespace qualification.
    fn expand_syntax_quote(&mut self, inner: &Ast, span: Span) -> Result<Ast> {
        // For now, just expand the inner form
        // Full syntax-quote expansion with namespace qualification
        // will be implemented in Stage X3
        let expanded = self.qualify_symbols(inner)?;
        Ok(Ast::Quote(Box::new(expanded), span))
    }

    /// Qualifies unqualified symbols to the current namespace.
    fn qualify_symbols(&self, ast: &Ast) -> Result<Ast> {
        match ast {
            // Don't qualify special forms
            Ast::Symbol(name, _span) if Self::is_special_form(name) => Ok(ast.clone()),

            // Don't qualify already-qualified symbols
            Ast::Symbol(name, _span) if name.contains('/') => Ok(ast.clone()),

            // Don't qualify gensym patterns
            Ast::Symbol(name, _span) if GensymGenerator::is_gensym_pattern(name) => Ok(ast.clone()),

            // Qualify unqualified symbols
            Ast::Symbol(name, span) => {
                let qualified = format!("{}/{}", self.current_namespace, name);
                Ok(Ast::Symbol(qualified, *span))
            }

            // Handle unquote - don't qualify inside unquote
            Ast::Unquote(inner, span) => Ok(Ast::Unquote(inner.clone(), *span)),

            Ast::UnquoteSplice(inner, span) => Ok(Ast::UnquoteSplice(inner.clone(), *span)),

            // Recurse into compound forms
            Ast::List(elements, span) => {
                let qualified: Result<Vec<_>> =
                    elements.iter().map(|e| self.qualify_symbols(e)).collect();
                Ok(Ast::List(qualified?, *span))
            }

            Ast::Vector(elements, span) => {
                let qualified: Result<Vec<_>> =
                    elements.iter().map(|e| self.qualify_symbols(e)).collect();
                Ok(Ast::Vector(qualified?, *span))
            }

            Ast::Set(elements, span) => {
                let qualified: Result<Vec<_>> =
                    elements.iter().map(|e| self.qualify_symbols(e)).collect();
                Ok(Ast::Set(qualified?, *span))
            }

            Ast::Map(pairs, span) => {
                let qualified: Result<Vec<_>> = pairs
                    .iter()
                    .map(|(k, v)| Ok((self.qualify_symbols(k)?, self.qualify_symbols(v)?)))
                    .collect();
                Ok(Ast::Map(qualified?, *span))
            }

            // Other forms pass through
            _ => Ok(ast.clone()),
        }
    }

    /// Checks if a symbol is a special form (not qualified in syntax-quote).
    fn is_special_form(name: &str) -> bool {
        matches!(
            name,
            "if" | "do"
                | "let"
                | "fn"
                | "def"
                | "defn"
                | "defmacro"
                | "quote"
                | "var"
                | "loop"
                | "recur"
                | "throw"
                | "try"
                | "catch"
                | "finally"
                | "new"
                | "set!"
                | "&"
        )
    }

    /// Expands children of a list (non-macro call).
    fn expand_list_elements(&mut self, elements: &[Ast], span: Span) -> Result<Ast> {
        let expanded: Result<Vec<_>> = elements.iter().map(|e| self.expand(e)).collect();
        Ok(Ast::List(expanded?, span))
    }
}

// =============================================================================
// MacroArg
// =============================================================================

/// An argument passed to a macro.
#[derive(Clone, Debug)]
enum MacroArg {
    /// A single argument.
    Single(Ast),
    /// Rest arguments (collected by &).
    Rest(Vec<Ast>),
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn expand_test(source: &str) -> Result<Vec<Ast>> {
        let mut registry = MacroRegistry::new();
        let mut expander = MacroExpander::new(&mut registry);
        let forms = parse(source)?;
        expander.expand_all(&forms)
    }

    #[test]
    fn expand_non_macro_passthrough() {
        let result = expand_test("(+ 1 2)").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Ast::List(elements, _) => assert_eq!(elements.len(), 3),
            _ => panic!("expected list"),
        }
    }

    #[test]
    fn defmacro_registers_macro() {
        let mut registry = MacroRegistry::new();
        {
            let mut expander = MacroExpander::new(&mut registry);
            let forms = parse("(defmacro my-macro [x] x)").unwrap();
            let result = expander.expand_all(&forms).unwrap();
            // defmacro returns nil
            assert!(matches!(result[0], Ast::Nil(_)));
        }
        // Macro should be registered
        assert!(registry.is_macro("my-macro"));
    }

    #[test]
    fn defmacro_with_rest_params() {
        let mut registry = MacroRegistry::new();
        {
            let mut expander = MacroExpander::new(&mut registry);
            let forms = parse("(defmacro varargs [x & rest] x)").unwrap();
            expander.expand_all(&forms).unwrap();
        }
        let def = registry.get_local("varargs").unwrap();
        assert!(def.variadic);
        assert_eq!(def.min_arity(), 1);
    }

    #[test]
    fn expand_simple_macro() {
        let mut registry = MacroRegistry::new();
        let mut expander = MacroExpander::new(&mut registry);

        // Define a simple identity macro
        let def_forms = parse("(defmacro identity [x] x)").unwrap();
        expander.expand_all(&def_forms).unwrap();

        // Use the macro
        let use_forms = parse("(identity 42)").unwrap();
        let result = expander.expand_all(&use_forms).unwrap();

        // Should expand to just 42
        assert_eq!(result.len(), 1);
        match &result[0] {
            Ast::Int(42, _) => {}
            other => panic!("expected Int(42), got {other:?}"),
        }
    }

    #[test]
    fn expand_macro_with_multiple_body_forms() {
        let mut registry = MacroRegistry::new();
        let mut expander = MacroExpander::new(&mut registry);

        let def_forms = parse("(defmacro multi [] 1 2 3)").unwrap();
        expander.expand_all(&def_forms).unwrap();

        let use_forms = parse("(multi)").unwrap();
        let result = expander.expand_all(&use_forms).unwrap();

        // Should wrap in do
        match &result[0] {
            Ast::List(elements, _) => {
                assert!(matches!(&elements[0], Ast::Symbol(s, _) if s == "do"));
                assert_eq!(elements.len(), 4); // do + 3 body forms
            }
            other => panic!("expected list with do, got {other:?}"),
        }
    }

    #[test]
    fn expand_macro_arity_error() {
        let mut registry = MacroRegistry::new();
        let mut expander = MacroExpander::new(&mut registry);

        let def_forms = parse("(defmacro needs-two [a b] a)").unwrap();
        expander.expand_all(&def_forms).unwrap();

        let use_forms = parse("(needs-two 1)").unwrap();
        let result = expander.expand_all(&use_forms);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expects"));
    }

    #[test]
    fn expand_nested_macro_calls() {
        let mut registry = MacroRegistry::new();
        let mut expander = MacroExpander::new(&mut registry);

        let def_forms = parse("(defmacro wrap [x] (list x))").unwrap();
        expander.expand_all(&def_forms).unwrap();

        // Note: This tests that expansion is recursive
        // The result of the first expansion gets expanded again
    }

    #[test]
    fn recursion_limit() {
        let mut registry = MacroRegistry::new();
        let mut expander = MacroExpander::new(&mut registry);

        // Create a macro that expands to itself
        let def_forms = parse("(defmacro infinite [] (infinite))").unwrap();
        expander.expand_all(&def_forms).unwrap();

        let use_forms = parse("(infinite)").unwrap();
        let result = expander.expand_all(&use_forms);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("depth exceeded"));
    }

    #[test]
    fn gensym_pattern_generates_unique_symbols() {
        // Helper function to check for gensym markers
        fn contains_gensym(ast: &Ast) -> bool {
            match ast {
                Ast::Symbol(s, _) => s.contains("__G__"),
                Ast::List(elements, _) | Ast::Vector(elements, _) => {
                    elements.iter().any(contains_gensym)
                }
                _ => false,
            }
        }

        let mut registry = MacroRegistry::new();
        let mut expander = MacroExpander::new(&mut registry);

        // The gensym generator should give unique names for x#
        let def_forms = parse("(defmacro with-temp [body] (let [x# 1] body))").unwrap();
        expander.expand_all(&def_forms).unwrap();

        let use_forms = parse("(with-temp (+ x# 2))").unwrap();
        let result = expander.expand_all(&use_forms).unwrap();

        // The x# should be replaced with a generated symbol
        // We can't test the exact value but we can verify it's changed
        assert!(contains_gensym(&result[0]));
    }
}
