//! Semantic declarations extracted from parsed AST.
//!
//! This module transforms raw AST (lists, vectors, symbols) into typed
//! declaration structures (Rules, Patterns, Components, etc.).
//!
//! The flow is: Source → Parser → AST → `DeclarationAnalyzer` → Declaration → Compiler

use crate::ast::Ast;
use crate::span::Span;
use longtable_foundation::{Error, ErrorKind, Result};

// =============================================================================
// Pattern Types
// =============================================================================

/// A pattern clause that matches entities.
///
/// Corresponds to `[?e :component/field ?value]` syntax.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternClause {
    /// The entity variable (e.g., "e" for `?e`)
    pub entity_var: String,
    /// The component keyword (e.g., "health/current")
    pub component: String,
    /// What to match/bind for the value
    pub value: PatternValue,
    /// Source span for error reporting
    pub span: Span,
}

/// What the value position of a pattern matches.
#[derive(Clone, Debug, PartialEq)]
pub enum PatternValue {
    /// Bind to a variable: `?hp`
    Variable(String),
    /// Match a literal value
    Literal(Ast),
    /// Wildcard match: `_`
    Wildcard,
}

/// A complete pattern (conjunction of clauses and negations).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Pattern {
    /// Positive clauses that must match
    pub clauses: Vec<PatternClause>,
    /// Negated patterns (entities must NOT match these)
    pub negations: Vec<PatternClause>,
}

impl Pattern {
    /// Creates an empty pattern.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns all variables bound by this pattern.
    #[must_use]
    pub fn bound_variables(&self) -> Vec<&str> {
        let mut vars = Vec::new();
        for clause in &self.clauses {
            vars.push(clause.entity_var.as_str());
            if let PatternValue::Variable(v) = &clause.value {
                vars.push(v.as_str());
            }
        }
        vars
    }
}

// =============================================================================
// Rule Declaration
// =============================================================================

/// A rule declaration extracted from AST.
///
/// Corresponds to:
/// ```clojure
/// (rule: name
///   :salience n
///   :once true/false
///   :where [[pattern clauses]]
///   :let [bindings]
///   :guard [conditions]
///   :then [effects])
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct RuleDecl {
    /// Rule name
    pub name: String,
    /// Priority (higher fires first), default 0
    pub salience: i32,
    /// Fire at most once per tick
    pub once: bool,
    /// Enabled flag
    pub enabled: bool,
    /// Pattern to match
    pub pattern: Pattern,
    /// Local bindings (let)
    pub bindings: Vec<(String, Ast)>,
    /// Guard conditions
    pub guards: Vec<Ast>,
    /// Effect expressions
    pub effects: Vec<Ast>,
    /// Source span
    pub span: Span,
}

impl RuleDecl {
    /// Creates a new rule with the given name.
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            salience: 0,
            once: false,
            enabled: true,
            pattern: Pattern::new(),
            bindings: Vec::new(),
            guards: Vec::new(),
            effects: Vec::new(),
            span,
        }
    }
}

// =============================================================================
// Declaration Analyzer
// =============================================================================

/// Analyzes AST and extracts typed declarations.
pub struct DeclarationAnalyzer;

impl DeclarationAnalyzer {
    /// Analyze a top-level form and return a rule if it's a rule declaration.
    pub fn analyze_rule(ast: &Ast) -> Result<Option<RuleDecl>> {
        let list = match ast {
            Ast::List(elements, span) => (elements, *span),
            _ => return Ok(None),
        };

        let (elements, span) = list;
        if elements.is_empty() {
            return Ok(None);
        }

        // Check for (rule: name ...) form
        match &elements[0] {
            Ast::Symbol(s, _) if s == "rule:" => {}
            _ => return Ok(None),
        }

        if elements.len() < 2 {
            return Err(Error::new(ErrorKind::ParseError {
                message: "rule: requires a name".to_string(),
                line: span.line,
                column: span.column,
                context: String::new(),
            }));
        }

        // Get rule name
        let name = match &elements[1] {
            Ast::Symbol(s, _) => s.clone(),
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!("rule name must be a symbol, got {}", other.type_name()),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        let mut rule = RuleDecl::new(name, span);

        // Parse keyword arguments
        let mut i = 2;
        while i < elements.len() {
            let key = match &elements[i] {
                Ast::Keyword(k, _) => k.as_str(),
                other => {
                    return Err(Error::new(ErrorKind::ParseError {
                        message: format!("expected keyword, got {}", other.type_name()),
                        line: other.span().line,
                        column: other.span().column,
                        context: String::new(),
                    }));
                }
            };

            i += 1;
            if i >= elements.len() {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!("missing value for :{key}"),
                    line: span.line,
                    column: span.column,
                    context: String::new(),
                }));
            }

            let value = &elements[i];
            i += 1;

            match key {
                "salience" => {
                    rule.salience = match value {
                        Ast::Int(n, _) => *n as i32,
                        other => {
                            return Err(Error::new(ErrorKind::ParseError {
                                message: format!(
                                    ":salience must be an integer, got {}",
                                    other.type_name()
                                ),
                                line: other.span().line,
                                column: other.span().column,
                                context: String::new(),
                            }));
                        }
                    };
                }
                "once" => {
                    rule.once = match value {
                        Ast::Bool(b, _) => *b,
                        other => {
                            return Err(Error::new(ErrorKind::ParseError {
                                message: format!(
                                    ":once must be a boolean, got {}",
                                    other.type_name()
                                ),
                                line: other.span().line,
                                column: other.span().column,
                                context: String::new(),
                            }));
                        }
                    };
                }
                "enabled" => {
                    rule.enabled = match value {
                        Ast::Bool(b, _) => *b,
                        other => {
                            return Err(Error::new(ErrorKind::ParseError {
                                message: format!(
                                    ":enabled must be a boolean, got {}",
                                    other.type_name()
                                ),
                                line: other.span().line,
                                column: other.span().column,
                                context: String::new(),
                            }));
                        }
                    };
                }
                "where" => {
                    rule.pattern = Self::analyze_where_clause(value)?;
                }
                "let" => {
                    rule.bindings = Self::analyze_let_bindings(value)?;
                }
                "guard" => {
                    rule.guards = Self::analyze_guard_clause(value)?;
                }
                "then" => {
                    rule.effects = Self::analyze_then_clause(value)?;
                }
                other => {
                    return Err(Error::new(ErrorKind::ParseError {
                        message: format!("unknown rule clause :{other}"),
                        line: value.span().line,
                        column: value.span().column,
                        context: String::new(),
                    }));
                }
            }
        }

        Ok(Some(rule))
    }

    /// Analyze a :where clause into a Pattern.
    fn analyze_where_clause(ast: &Ast) -> Result<Pattern> {
        let patterns = match ast {
            Ast::Vector(elements, _) => elements,
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!(":where must be a vector, got {}", other.type_name()),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        let mut pattern = Pattern::new();

        for p in patterns {
            match p {
                // Regular pattern clause: [?e :component ?v]
                Ast::Vector(clause, span) => {
                    let pc = Self::analyze_pattern_clause(clause, *span)?;
                    pattern.clauses.push(pc);
                }
                // Negated pattern: (not [?e :component])
                Ast::List(elements, span) => {
                    if elements.is_empty() {
                        continue;
                    }
                    match &elements[0] {
                        Ast::Symbol(s, _) if s == "not" => {
                            if elements.len() != 2 {
                                return Err(Error::new(ErrorKind::ParseError {
                                    message: "not requires exactly one pattern".to_string(),
                                    line: span.line,
                                    column: span.column,
                                    context: String::new(),
                                }));
                            }
                            match &elements[1] {
                                Ast::Vector(clause, inner_span) => {
                                    let pc = Self::analyze_pattern_clause(clause, *inner_span)?;
                                    pattern.negations.push(pc);
                                }
                                other => {
                                    return Err(Error::new(ErrorKind::ParseError {
                                        message: format!(
                                            "not requires a pattern vector, got {}",
                                            other.type_name()
                                        ),
                                        line: other.span().line,
                                        column: other.span().column,
                                        context: String::new(),
                                    }));
                                }
                            }
                        }
                        _ => {
                            return Err(Error::new(ErrorKind::ParseError {
                                message: "expected pattern vector or (not [...])".to_string(),
                                line: span.line,
                                column: span.column,
                                context: String::new(),
                            }));
                        }
                    }
                }
                other => {
                    return Err(Error::new(ErrorKind::ParseError {
                        message: format!(
                            "pattern must be a vector or (not ...), got {}",
                            other.type_name()
                        ),
                        line: other.span().line,
                        column: other.span().column,
                        context: String::new(),
                    }));
                }
            }
        }

        Ok(pattern)
    }

    /// Analyze a single pattern clause: [?e :component ?v]
    fn analyze_pattern_clause(elements: &[Ast], span: Span) -> Result<PatternClause> {
        if elements.len() < 2 || elements.len() > 3 {
            return Err(Error::new(ErrorKind::ParseError {
                message: "pattern clause must have 2 or 3 elements: [?entity :component] or [?entity :component ?value]".to_string(),
                line: span.line,
                column: span.column,
                context: String::new(),
            }));
        }

        // Entity variable
        let entity_var = match &elements[0] {
            Ast::Symbol(s, _) if s.starts_with('?') => s[1..].to_string(),
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!(
                        "pattern entity must be a ?variable, got {}",
                        other.type_name()
                    ),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        // Component keyword
        let component = match &elements[1] {
            Ast::Keyword(k, _) => k.clone(),
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!(
                        "pattern component must be a keyword, got {}",
                        other.type_name()
                    ),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        // Value (optional)
        let value = if elements.len() == 3 {
            match &elements[2] {
                Ast::Symbol(s, _) if s == "_" => PatternValue::Wildcard,
                Ast::Symbol(s, _) if s.starts_with('?') => {
                    PatternValue::Variable(s[1..].to_string())
                }
                other => PatternValue::Literal(other.clone()),
            }
        } else {
            PatternValue::Wildcard
        };

        Ok(PatternClause {
            entity_var,
            component,
            value,
            span,
        })
    }

    /// Analyze a :let clause into bindings.
    fn analyze_let_bindings(ast: &Ast) -> Result<Vec<(String, Ast)>> {
        let bindings = match ast {
            Ast::Vector(elements, _) => elements,
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!(":let must be a vector, got {}", other.type_name()),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        if bindings.len() % 2 != 0 {
            return Err(Error::new(ErrorKind::ParseError {
                message: ":let bindings must be pairs".to_string(),
                line: ast.span().line,
                column: ast.span().column,
                context: String::new(),
            }));
        }

        let mut result = Vec::new();
        for chunk in bindings.chunks(2) {
            let name = match &chunk[0] {
                Ast::Symbol(s, _) => s.clone(),
                other => {
                    return Err(Error::new(ErrorKind::ParseError {
                        message: format!(
                            "binding name must be a symbol, got {}",
                            other.type_name()
                        ),
                        line: other.span().line,
                        column: other.span().column,
                        context: String::new(),
                    }));
                }
            };
            result.push((name, chunk[1].clone()));
        }

        Ok(result)
    }

    /// Analyze a :guard clause.
    fn analyze_guard_clause(ast: &Ast) -> Result<Vec<Ast>> {
        match ast {
            Ast::Vector(elements, _) => Ok(elements.clone()),
            other => Err(Error::new(ErrorKind::ParseError {
                message: format!(":guard must be a vector, got {}", other.type_name()),
                line: other.span().line,
                column: other.span().column,
                context: String::new(),
            })),
        }
    }

    /// Analyze a :then clause.
    fn analyze_then_clause(ast: &Ast) -> Result<Vec<Ast>> {
        match ast {
            Ast::Vector(elements, _) => Ok(elements.clone()),
            other => Err(Error::new(ErrorKind::ParseError {
                message: format!(":then must be a vector, got {}", other.type_name()),
                line: other.span().line,
                column: other.span().column,
                context: String::new(),
            })),
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn parse(src: &str) -> Ast {
        parser::parse(src).unwrap().remove(0)
    }

    #[test]
    fn analyze_simple_rule() {
        let ast = parse(
            r#"(rule: my-rule
                 :where [[?e :health ?hp]]
                 :then [(print! ?hp)])"#,
        );

        let rule = DeclarationAnalyzer::analyze_rule(&ast).unwrap().unwrap();

        assert_eq!(rule.name, "my-rule");
        assert_eq!(rule.salience, 0);
        assert!(!rule.once);
        assert_eq!(rule.pattern.clauses.len(), 1);
        assert_eq!(rule.pattern.clauses[0].entity_var, "e");
        assert_eq!(rule.pattern.clauses[0].component, "health");
        assert_eq!(
            rule.pattern.clauses[0].value,
            PatternValue::Variable("hp".to_string())
        );
        assert_eq!(rule.effects.len(), 1);
    }

    #[test]
    fn analyze_rule_with_options() {
        let ast = parse(
            r#"(rule: priority-rule
                 :salience 100
                 :once true
                 :where [[?e :tag/player true]]
                 :then [])"#,
        );

        let rule = DeclarationAnalyzer::analyze_rule(&ast).unwrap().unwrap();

        assert_eq!(rule.name, "priority-rule");
        assert_eq!(rule.salience, 100);
        assert!(rule.once);
        // Check that value is a literal Bool(true) regardless of span
        match &rule.pattern.clauses[0].value {
            PatternValue::Literal(Ast::Bool(true, _)) => {}
            other => panic!("expected Literal(Bool(true, _)), got {:?}", other),
        }
    }

    #[test]
    fn analyze_rule_with_negation() {
        let ast = parse(
            r#"(rule: no-velocity
                 :where [[?e :position _]
                         (not [?e :velocity])]
                 :then [])"#,
        );

        let rule = DeclarationAnalyzer::analyze_rule(&ast).unwrap().unwrap();

        assert_eq!(rule.pattern.clauses.len(), 1);
        assert_eq!(rule.pattern.negations.len(), 1);
        assert_eq!(rule.pattern.negations[0].entity_var, "e");
        assert_eq!(rule.pattern.negations[0].component, "velocity");
    }

    #[test]
    fn analyze_rule_with_let_and_guard() {
        let ast = parse(
            r#"(rule: guarded
                 :where [[?e :health ?hp]]
                 :let [threshold 10]
                 :guard [(< ?hp threshold)]
                 :then [(print! "low health")])"#,
        );

        let rule = DeclarationAnalyzer::analyze_rule(&ast).unwrap().unwrap();

        assert_eq!(rule.bindings.len(), 1);
        assert_eq!(rule.bindings[0].0, "threshold");
        assert_eq!(rule.guards.len(), 1);
    }

    #[test]
    fn analyze_wildcard_pattern() {
        let ast = parse(
            r#"(rule: wildcard
                 :where [[?e :position _]]
                 :then [])"#,
        );

        let rule = DeclarationAnalyzer::analyze_rule(&ast).unwrap().unwrap();

        assert_eq!(rule.pattern.clauses[0].value, PatternValue::Wildcard);
    }

    #[test]
    fn analyze_multiple_patterns() {
        let ast = parse(
            r#"(rule: multi
                 :where [[?e :position ?pos]
                         [?e :velocity ?vel]]
                 :then [])"#,
        );

        let rule = DeclarationAnalyzer::analyze_rule(&ast).unwrap().unwrap();

        assert_eq!(rule.pattern.clauses.len(), 2);
        assert_eq!(rule.pattern.clauses[0].component, "position");
        assert_eq!(rule.pattern.clauses[1].component, "velocity");
    }

    #[test]
    fn non_rule_returns_none() {
        let ast = parse("(+ 1 2)");
        let result = DeclarationAnalyzer::analyze_rule(&ast).unwrap();
        assert!(result.is_none());
    }
}
