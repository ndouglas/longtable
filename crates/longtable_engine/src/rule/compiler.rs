//! Rule compiler - transforms declaration AST into executable rules.
//!
//! Compiles `RuleDecl` from the language crate into `CompiledRule` for execution.

use longtable_foundation::{Interner, KeywordId, Result};
use longtable_language::declaration::RuleDecl;
use longtable_language::{Ast, Bytecode, compile_expression};

use crate::pattern::{CompiledPattern, PatternCompiler};

// =============================================================================
// Compiled Rule Body
// =============================================================================

/// A compiled rule body ready for execution.
#[derive(Clone, Debug)]
pub struct CompiledRuleBody {
    /// Compiled effect expressions as bytecode
    pub effects: Vec<Bytecode>,
    /// Compiled guard expressions as bytecode
    pub guards: Vec<Bytecode>,
}

impl Default for CompiledRuleBody {
    fn default() -> Self {
        Self::new()
    }
}

impl CompiledRuleBody {
    /// Create an empty compiled body.
    #[must_use]
    pub fn new() -> Self {
        Self {
            effects: Vec::new(),
            guards: Vec::new(),
        }
    }
}

// =============================================================================
// Full Compiled Rule (with body)
// =============================================================================

/// A fully compiled rule with pattern and body.
#[derive(Clone, Debug)]
pub struct FullCompiledRule {
    /// Rule name (interned keyword)
    pub name: KeywordId,
    /// Priority (higher fires first)
    pub salience: i32,
    /// Compiled pattern for matching
    pub pattern: CompiledPattern,
    /// Fire only once per tick
    pub once: bool,
    /// Whether rule is enabled
    pub enabled: bool,
    /// Compiled body (guards and effects)
    pub body: CompiledRuleBody,
    /// Local bindings from :let clause (name, AST)
    pub bindings: Vec<(String, Ast)>,
}

// =============================================================================
// Rule Compiler
// =============================================================================

/// Compiles rule declarations into executable rules.
pub struct RuleCompiler;

impl RuleCompiler {
    /// Compile a rule declaration into a full compiled rule.
    ///
    /// # Errors
    /// Returns an error if pattern or body compilation fails.
    pub fn compile(decl: &RuleDecl, interner: &mut Interner) -> Result<FullCompiledRule> {
        // Intern the rule name
        let name = interner.intern_keyword(&decl.name);

        // Compile the pattern
        let pattern = PatternCompiler::compile(&decl.pattern, interner)?;

        // Collect all binding variables from the pattern for use in expressions
        let binding_vars: Vec<String> = decl
            .pattern
            .bound_variables()
            .into_iter()
            .map(String::from)
            .collect();

        // Compile guard expressions
        let guards = decl
            .guards
            .iter()
            .map(|ast| Self::compile_expr(ast, &binding_vars))
            .collect::<Result<Vec<_>>>()?;

        // Compile effect expressions
        let effects = decl
            .effects
            .iter()
            .map(|ast| Self::compile_expr(ast, &binding_vars))
            .collect::<Result<Vec<_>>>()?;

        let body = CompiledRuleBody { effects, guards };

        Ok(FullCompiledRule {
            name,
            salience: decl.salience,
            pattern,
            once: decl.once,
            enabled: decl.enabled,
            body,
            bindings: decl.bindings.clone(),
        })
    }

    /// Compile a single AST expression to bytecode.
    fn compile_expr(ast: &Ast, binding_vars: &[String]) -> Result<Bytecode> {
        let compiled = compile_expression(ast, binding_vars)?;
        Ok(compiled.code)
    }

    /// Compile multiple rule declarations.
    ///
    /// # Errors
    /// Returns an error if any rule compilation fails.
    pub fn compile_all(
        decls: &[RuleDecl],
        interner: &mut Interner,
    ) -> Result<Vec<FullCompiledRule>> {
        decls.iter().map(|d| Self::compile(d, interner)).collect()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use longtable_language::declaration::{
        Pattern as DeclPattern, PatternClause as DeclClause, PatternValue,
    };
    use longtable_language::{Span, parse};

    fn make_clause(entity_var: &str, component: &str, value: PatternValue) -> DeclClause {
        DeclClause {
            entity_var: entity_var.to_string(),
            component: component.to_string(),
            value,
            span: Span::default(),
        }
    }

    #[test]
    fn compile_simple_rule() {
        let mut interner = Interner::new();

        let decl = RuleDecl {
            name: "test-rule".to_string(),
            salience: 10,
            once: false,
            enabled: true,
            pattern: DeclPattern {
                clauses: vec![make_clause(
                    "e",
                    "health",
                    PatternValue::Variable("hp".to_string()),
                )],
                negations: vec![],
            },
            bindings: vec![],
            guards: vec![],
            effects: vec![],
            span: Span::default(),
        };

        let compiled = RuleCompiler::compile(&decl, &mut interner).unwrap();

        assert_eq!(compiled.salience, 10);
        assert!(!compiled.once);
        assert!(compiled.enabled);
        assert_eq!(compiled.pattern.clauses.len(), 1);
        assert!(compiled.body.guards.is_empty());
        assert!(compiled.body.effects.is_empty());
    }

    #[test]
    fn compile_rule_with_guards() {
        let mut interner = Interner::new();

        // Parse a guard expression
        let guard_ast = parse("(< ?hp 50)").unwrap().remove(0);

        let decl = RuleDecl {
            name: "guarded-rule".to_string(),
            salience: 0,
            once: true,
            enabled: true,
            pattern: DeclPattern {
                clauses: vec![make_clause(
                    "e",
                    "health",
                    PatternValue::Variable("hp".to_string()),
                )],
                negations: vec![],
            },
            bindings: vec![],
            guards: vec![guard_ast],
            effects: vec![],
            span: Span::default(),
        };

        let compiled = RuleCompiler::compile(&decl, &mut interner).unwrap();

        assert!(compiled.once);
        assert_eq!(compiled.body.guards.len(), 1);
    }

    #[test]
    fn compile_rule_with_effects() {
        let mut interner = Interner::new();

        // Parse effect expressions
        let effect1 = parse("(print \"firing!\")").unwrap().remove(0);
        let effect2 = parse("(+ 1 2)").unwrap().remove(0);

        let decl = RuleDecl {
            name: "effect-rule".to_string(),
            salience: 100,
            once: false,
            enabled: true,
            pattern: DeclPattern {
                clauses: vec![make_clause("e", "tag", PatternValue::Wildcard)],
                negations: vec![],
            },
            bindings: vec![],
            guards: vec![],
            effects: vec![effect1, effect2],
            span: Span::default(),
        };

        let compiled = RuleCompiler::compile(&decl, &mut interner).unwrap();

        assert_eq!(compiled.salience, 100);
        assert_eq!(compiled.body.effects.len(), 2);
    }

    #[test]
    fn compile_rule_with_bindings() {
        let mut interner = Interner::new();

        let binding_value = parse("42").unwrap().remove(0);

        let decl = RuleDecl {
            name: "binding-rule".to_string(),
            salience: 0,
            once: false,
            enabled: true,
            pattern: DeclPattern::default(),
            bindings: vec![("threshold".to_string(), binding_value)],
            guards: vec![],
            effects: vec![],
            span: Span::default(),
        };

        let compiled = RuleCompiler::compile(&decl, &mut interner).unwrap();

        assert_eq!(compiled.bindings.len(), 1);
        assert_eq!(compiled.bindings[0].0, "threshold");
    }

    #[test]
    fn compile_multiple_rules() {
        let mut interner = Interner::new();

        let decls = vec![
            RuleDecl::new("rule1", Span::default()),
            RuleDecl::new("rule2", Span::default()),
            RuleDecl::new("rule3", Span::default()),
        ];

        let compiled = RuleCompiler::compile_all(&decls, &mut interner).unwrap();

        assert_eq!(compiled.len(), 3);
    }
}
