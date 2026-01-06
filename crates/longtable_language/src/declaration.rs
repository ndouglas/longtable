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
// Component Declaration
// =============================================================================

/// A field in a component schema.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldDecl {
    /// Field name (e.g., "current" for `:current`)
    pub name: String,
    /// Field type (e.g., "int", "float", "string", "entity-ref")
    pub ty: String,
    /// Default value, if any
    pub default: Option<Ast>,
    /// Source span
    pub span: Span,
}

/// A component declaration.
///
/// Corresponds to:
/// ```clojure
/// (component: health
///   :current :int
///   :max :int :default 100)
///
/// ;; Tag shorthand
/// (component: tag/player :bool :default true)
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentDecl {
    /// Component name (e.g., "health", "tag/player")
    pub name: String,
    /// Fields (empty for tag shorthand, parsed from type directly)
    pub fields: Vec<FieldDecl>,
    /// Whether this is a tag (single-field boolean shorthand)
    pub is_tag: bool,
    /// Source span
    pub span: Span,
}

impl ComponentDecl {
    /// Creates a new component declaration.
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            fields: Vec::new(),
            is_tag: false,
            span,
        }
    }
}

// =============================================================================
// Relationship Declaration
// =============================================================================

/// Storage strategy for a relationship.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StorageKind {
    /// Lightweight, stored as a component field
    #[default]
    Field,
    /// Heavyweight, stored as a separate entity with attributes
    Entity,
}

/// Cardinality of a relationship.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Cardinality {
    /// One source to one target
    OneToOne,
    /// One source to many targets
    OneToMany,
    /// Many sources to one target
    #[default]
    ManyToOne,
    /// Many sources to many targets
    ManyToMany,
}

/// Behavior when the target of a relationship is deleted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OnTargetDelete {
    /// Remove the relationship
    #[default]
    Remove,
    /// Destroy the source entity
    Cascade,
    /// Set to nil (only valid if not required)
    Nullify,
}

/// Behavior when a cardinality constraint is violated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OnViolation {
    /// Return an error
    #[default]
    Error,
    /// Replace the old relationship with the new one
    Replace,
}

/// A relationship declaration.
///
/// Corresponds to:
/// ```clojure
/// (relationship: follows
///   :storage :field
///   :cardinality :many-to-many
///   :on-target-delete :remove)
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct RelationshipDecl {
    /// Relationship name
    pub name: String,
    /// Storage strategy
    pub storage: StorageKind,
    /// Cardinality constraint
    pub cardinality: Cardinality,
    /// Behavior when target is deleted
    pub on_target_delete: OnTargetDelete,
    /// Behavior when cardinality is violated
    pub on_violation: OnViolation,
    /// Whether this relationship is required
    pub required: bool,
    /// Attributes (only for entity storage)
    pub attributes: Vec<FieldDecl>,
    /// Source span
    pub span: Span,
}

impl RelationshipDecl {
    /// Creates a new relationship declaration.
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            storage: StorageKind::default(),
            cardinality: Cardinality::default(),
            on_target_delete: OnTargetDelete::default(),
            on_violation: OnViolation::default(),
            required: true,
            attributes: Vec::new(),
            span,
        }
    }
}

// =============================================================================
// Derived Component Declaration
// =============================================================================

/// A derived component declaration.
///
/// Corresponds to:
/// ```clojure
/// (derived: health/percent
///   :for ?self
///   :where [[?self :health/current ?curr]
///           [?self :health/max ?max]]
///   :value (/ (* ?curr 100) ?max))
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct DerivedDecl {
    /// Derived component name
    pub name: String,
    /// Entity variable this is computed for (e.g., "self" for `:for ?self`)
    pub for_var: String,
    /// Pattern to match for inputs
    pub pattern: Pattern,
    /// Local bindings
    pub bindings: Vec<(String, Ast)>,
    /// Aggregations (e.g., `{:total (sum ?p)}`)
    pub aggregates: Vec<(String, Ast)>,
    /// Value expression
    pub value: Ast,
    /// Source span
    pub span: Span,
}

impl DerivedDecl {
    /// Creates a new derived component declaration.
    pub fn new(
        name: impl Into<String>,
        for_var: impl Into<String>,
        value: Ast,
        span: Span,
    ) -> Self {
        Self {
            name: name.into(),
            for_var: for_var.into(),
            pattern: Pattern::new(),
            bindings: Vec::new(),
            aggregates: Vec::new(),
            value,
            span,
        }
    }
}

// =============================================================================
// Constraint Declaration
// =============================================================================

/// Behavior when a constraint is violated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ConstraintViolation {
    /// Rollback the entire tick
    #[default]
    Rollback,
    /// Log a warning and continue
    Warn,
}

/// A constraint declaration.
///
/// Corresponds to:
/// ```clojure
/// (constraint: health-bounds
///   :where [[?e :health/current ?hp]
///           [?e :health/max ?max]]
///   :check [(>= ?hp 0) (<= ?hp ?max)]
///   :on-violation :rollback)
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct ConstraintDecl {
    /// Constraint name
    pub name: String,
    /// Pattern to match for checking
    pub pattern: Pattern,
    /// Local bindings
    pub bindings: Vec<(String, Ast)>,
    /// Aggregations
    pub aggregates: Vec<(String, Ast)>,
    /// Guard conditions (additional filtering before check)
    pub guards: Vec<Ast>,
    /// Check expressions (all must be true)
    pub checks: Vec<Ast>,
    /// Behavior on violation
    pub on_violation: ConstraintViolation,
    /// Source span
    pub span: Span,
}

impl ConstraintDecl {
    /// Creates a new constraint declaration.
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            pattern: Pattern::new(),
            bindings: Vec::new(),
            aggregates: Vec::new(),
            guards: Vec::new(),
            checks: Vec::new(),
            on_violation: ConstraintViolation::default(),
            span,
        }
    }
}

// =============================================================================
// Query Declaration
// =============================================================================

/// Order direction for query results.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OrderDirection {
    /// Ascending order (smallest first)
    #[default]
    Asc,
    /// Descending order (largest first)
    Desc,
}

/// A query expression.
///
/// Corresponds to:
/// ```clojure
/// (query
///   :where [[?e :health/current ?hp]]
///   :order-by [[?hp :desc]]
///   :limit 10
///   :return ?e)
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct QueryDecl {
    /// Pattern to match
    pub pattern: Pattern,
    /// Local bindings
    pub bindings: Vec<(String, Ast)>,
    /// Aggregations
    pub aggregates: Vec<(String, Ast)>,
    /// Group-by variables
    pub group_by: Vec<String>,
    /// Guard conditions
    pub guards: Vec<Ast>,
    /// Order-by clauses
    pub order_by: Vec<(String, OrderDirection)>,
    /// Result limit
    pub limit: Option<usize>,
    /// Return expression
    pub return_expr: Option<Ast>,
    /// Source span
    pub span: Span,
}

impl QueryDecl {
    /// Creates a new query declaration.
    #[must_use]
    pub fn new(span: Span) -> Self {
        Self {
            pattern: Pattern::new(),
            bindings: Vec::new(),
            aggregates: Vec::new(),
            group_by: Vec::new(),
            guards: Vec::new(),
            order_by: Vec::new(),
            limit: None,
            return_expr: None,
            span,
        }
    }
}

// =============================================================================
// Unified Declaration Enum
// =============================================================================

/// Any top-level declaration.
#[derive(Clone, Debug, PartialEq)]
pub enum Declaration {
    /// A component schema declaration.
    Component(ComponentDecl),
    /// A relationship declaration.
    Relationship(RelationshipDecl),
    /// A rule declaration.
    Rule(RuleDecl),
    /// A derived component declaration.
    Derived(DerivedDecl),
    /// A constraint declaration.
    Constraint(ConstraintDecl),
    /// A query expression.
    Query(QueryDecl),
}

// =============================================================================
// Declaration Analyzer
// =============================================================================

/// Analyzes AST and extracts typed declarations.
pub struct DeclarationAnalyzer;

impl DeclarationAnalyzer {
    /// Analyze a top-level form and return a rule if it's a rule declaration.
    #[allow(clippy::too_many_lines)]
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
                    #[allow(clippy::cast_possible_truncation)]
                    {
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

    // =========================================================================
    // Component Declaration Analysis
    // =========================================================================

    /// Analyze a top-level form and return a component if it's a component declaration.
    ///
    /// Handles both full form and tag shorthand:
    /// ```clojure
    /// (component: health :current :int :max :int :default 100)
    /// (component: tag/player :bool :default true)
    /// ```
    #[allow(clippy::too_many_lines)]
    pub fn analyze_component(ast: &Ast) -> Result<Option<ComponentDecl>> {
        let list = match ast {
            Ast::List(elements, span) => (elements, *span),
            _ => return Ok(None),
        };

        let (elements, span) = list;
        if elements.is_empty() {
            return Ok(None);
        }

        // Check for (component: name ...) form
        match &elements[0] {
            Ast::Symbol(s, _) if s == "component:" => {}
            _ => return Ok(None),
        }

        if elements.len() < 2 {
            return Err(Error::new(ErrorKind::ParseError {
                message: "component: requires a name".to_string(),
                line: span.line,
                column: span.column,
                context: String::new(),
            }));
        }

        // Get component name
        let name = match &elements[1] {
            Ast::Symbol(s, _) => s.clone(),
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!("component name must be a symbol, got {}", other.type_name()),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        let mut component = ComponentDecl::new(name, span);

        // Check for tag shorthand: (component: tag/player :bool :default true)
        // This is when the third element is a type keyword like :bool, :int, etc.
        if elements.len() >= 3 {
            if let Ast::Keyword(ty, _) = &elements[2] {
                if Self::is_type_keyword(ty) {
                    // Tag shorthand form
                    component.is_tag = true;
                    let mut field = FieldDecl {
                        name: "value".to_string(),
                        ty: ty.clone(),
                        default: None,
                        span: elements[2].span(),
                    };

                    // Check for :default
                    let mut i = 3;
                    while i < elements.len() {
                        if let Ast::Keyword(k, _) = &elements[i] {
                            if k == "default" {
                                i += 1;
                                if i >= elements.len() {
                                    return Err(Error::new(ErrorKind::ParseError {
                                        message: "missing value for :default".to_string(),
                                        line: span.line,
                                        column: span.column,
                                        context: String::new(),
                                    }));
                                }
                                field.default = Some(elements[i].clone());
                                i += 1;
                            } else {
                                return Err(Error::new(ErrorKind::ParseError {
                                    message: format!("unexpected keyword :{k} in tag component"),
                                    line: elements[i].span().line,
                                    column: elements[i].span().column,
                                    context: String::new(),
                                }));
                            }
                        } else {
                            return Err(Error::new(ErrorKind::ParseError {
                                message: format!(
                                    "expected keyword, got {}",
                                    elements[i].type_name()
                                ),
                                line: elements[i].span().line,
                                column: elements[i].span().column,
                                context: String::new(),
                            }));
                        }
                    }

                    component.fields.push(field);
                    return Ok(Some(component));
                }
            }
        }

        // Full form: (component: health :current :int :max :int :default 100)
        // Parse as: :field-name :type [:default value] ...
        let mut i = 2;
        while i < elements.len() {
            // Field name
            let field_name = match &elements[i] {
                Ast::Keyword(k, _) => k.clone(),
                other => {
                    return Err(Error::new(ErrorKind::ParseError {
                        message: format!("expected field name keyword, got {}", other.type_name()),
                        line: other.span().line,
                        column: other.span().column,
                        context: String::new(),
                    }));
                }
            };
            let field_span = elements[i].span();
            i += 1;

            if i >= elements.len() {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!("missing type for field :{field_name}"),
                    line: span.line,
                    column: span.column,
                    context: String::new(),
                }));
            }

            // Field type
            let field_type = match &elements[i] {
                Ast::Keyword(k, _) => k.clone(),
                other => {
                    return Err(Error::new(ErrorKind::ParseError {
                        message: format!("expected type keyword, got {}", other.type_name()),
                        line: other.span().line,
                        column: other.span().column,
                        context: String::new(),
                    }));
                }
            };
            i += 1;

            let mut field = FieldDecl {
                name: field_name,
                ty: field_type,
                default: None,
                span: field_span,
            };

            // Check for :default
            if i < elements.len() {
                if let Ast::Keyword(k, _) = &elements[i] {
                    if k == "default" {
                        i += 1;
                        if i >= elements.len() {
                            return Err(Error::new(ErrorKind::ParseError {
                                message: "missing value for :default".to_string(),
                                line: span.line,
                                column: span.column,
                                context: String::new(),
                            }));
                        }
                        field.default = Some(elements[i].clone());
                        i += 1;
                    }
                }
            }

            component.fields.push(field);
        }

        Ok(Some(component))
    }

    /// Check if a keyword is a type name.
    fn is_type_keyword(k: &str) -> bool {
        matches!(
            k,
            "bool"
                | "int"
                | "float"
                | "string"
                | "keyword"
                | "symbol"
                | "entity-ref"
                | "map"
                | "vec"
                | "set"
                | "any"
        ) || k.starts_with("option<")
    }

    // =========================================================================
    // Relationship Declaration Analysis
    // =========================================================================

    /// Analyze a top-level form and return a relationship if it's a relationship declaration.
    #[allow(clippy::too_many_lines)]
    pub fn analyze_relationship(ast: &Ast) -> Result<Option<RelationshipDecl>> {
        let list = match ast {
            Ast::List(elements, span) => (elements, *span),
            _ => return Ok(None),
        };

        let (elements, span) = list;
        if elements.is_empty() {
            return Ok(None);
        }

        // Check for (relationship: name ...) form
        match &elements[0] {
            Ast::Symbol(s, _) if s == "relationship:" => {}
            _ => return Ok(None),
        }

        if elements.len() < 2 {
            return Err(Error::new(ErrorKind::ParseError {
                message: "relationship: requires a name".to_string(),
                line: span.line,
                column: span.column,
                context: String::new(),
            }));
        }

        // Get relationship name
        let name = match &elements[1] {
            Ast::Symbol(s, _) => s.clone(),
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!(
                        "relationship name must be a symbol, got {}",
                        other.type_name()
                    ),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        let mut rel = RelationshipDecl::new(name, span);

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
                "storage" => {
                    rel.storage = match value {
                        Ast::Keyword(k, _) => match k.as_str() {
                            "field" => StorageKind::Field,
                            "entity" => StorageKind::Entity,
                            other => {
                                return Err(Error::new(ErrorKind::ParseError {
                                    message: format!(
                                        ":storage must be :field or :entity, got :{other}"
                                    ),
                                    line: value.span().line,
                                    column: value.span().column,
                                    context: String::new(),
                                }));
                            }
                        },
                        other => {
                            return Err(Error::new(ErrorKind::ParseError {
                                message: format!(
                                    ":storage must be a keyword, got {}",
                                    other.type_name()
                                ),
                                line: other.span().line,
                                column: other.span().column,
                                context: String::new(),
                            }));
                        }
                    };
                }
                "cardinality" => {
                    rel.cardinality = match value {
                        Ast::Keyword(k, _) => match k.as_str() {
                            "one-to-one" => Cardinality::OneToOne,
                            "one-to-many" => Cardinality::OneToMany,
                            "many-to-one" => Cardinality::ManyToOne,
                            "many-to-many" => Cardinality::ManyToMany,
                            other => {
                                return Err(Error::new(ErrorKind::ParseError {
                                    message: format!("invalid cardinality :{other}"),
                                    line: value.span().line,
                                    column: value.span().column,
                                    context: String::new(),
                                }));
                            }
                        },
                        other => {
                            return Err(Error::new(ErrorKind::ParseError {
                                message: format!(
                                    ":cardinality must be a keyword, got {}",
                                    other.type_name()
                                ),
                                line: other.span().line,
                                column: other.span().column,
                                context: String::new(),
                            }));
                        }
                    };
                }
                "on-target-delete" => {
                    rel.on_target_delete = match value {
                        Ast::Keyword(k, _) => match k.as_str() {
                            "remove" => OnTargetDelete::Remove,
                            "cascade" => OnTargetDelete::Cascade,
                            "nullify" => OnTargetDelete::Nullify,
                            other => {
                                return Err(Error::new(ErrorKind::ParseError {
                                    message: format!("invalid on-target-delete :{other}"),
                                    line: value.span().line,
                                    column: value.span().column,
                                    context: String::new(),
                                }));
                            }
                        },
                        other => {
                            return Err(Error::new(ErrorKind::ParseError {
                                message: format!(
                                    ":on-target-delete must be a keyword, got {}",
                                    other.type_name()
                                ),
                                line: other.span().line,
                                column: other.span().column,
                                context: String::new(),
                            }));
                        }
                    };
                }
                "on-violation" => {
                    rel.on_violation = match value {
                        Ast::Keyword(k, _) => match k.as_str() {
                            "error" => OnViolation::Error,
                            "replace" => OnViolation::Replace,
                            other => {
                                return Err(Error::new(ErrorKind::ParseError {
                                    message: format!("invalid on-violation :{other}"),
                                    line: value.span().line,
                                    column: value.span().column,
                                    context: String::new(),
                                }));
                            }
                        },
                        other => {
                            return Err(Error::new(ErrorKind::ParseError {
                                message: format!(
                                    ":on-violation must be a keyword, got {}",
                                    other.type_name()
                                ),
                                line: other.span().line,
                                column: other.span().column,
                                context: String::new(),
                            }));
                        }
                    };
                }
                "required" => {
                    rel.required = match value {
                        Ast::Bool(b, _) => *b,
                        other => {
                            return Err(Error::new(ErrorKind::ParseError {
                                message: format!(
                                    ":required must be a boolean, got {}",
                                    other.type_name()
                                ),
                                line: other.span().line,
                                column: other.span().column,
                                context: String::new(),
                            }));
                        }
                    };
                }
                "attributes" => {
                    rel.attributes = Self::analyze_attribute_list(value)?;
                }
                other => {
                    return Err(Error::new(ErrorKind::ParseError {
                        message: format!("unknown relationship option :{other}"),
                        line: value.span().line,
                        column: value.span().column,
                        context: String::new(),
                    }));
                }
            }
        }

        Ok(Some(rel))
    }

    /// Analyze an :attributes list for entity-storage relationships.
    fn analyze_attribute_list(ast: &Ast) -> Result<Vec<FieldDecl>> {
        let attrs = match ast {
            Ast::Vector(elements, _) => elements,
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!(":attributes must be a vector, got {}", other.type_name()),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        if attrs.len() % 2 != 0 {
            return Err(Error::new(ErrorKind::ParseError {
                message: ":attributes must be pairs of name and type".to_string(),
                line: ast.span().line,
                column: ast.span().column,
                context: String::new(),
            }));
        }

        let mut result = Vec::new();
        for chunk in attrs.chunks(2) {
            let name = match &chunk[0] {
                Ast::Keyword(k, _) => k.clone(),
                other => {
                    return Err(Error::new(ErrorKind::ParseError {
                        message: format!(
                            "attribute name must be a keyword, got {}",
                            other.type_name()
                        ),
                        line: other.span().line,
                        column: other.span().column,
                        context: String::new(),
                    }));
                }
            };
            let ty = match &chunk[1] {
                Ast::Keyword(k, _) => k.clone(),
                other => {
                    return Err(Error::new(ErrorKind::ParseError {
                        message: format!(
                            "attribute type must be a keyword, got {}",
                            other.type_name()
                        ),
                        line: other.span().line,
                        column: other.span().column,
                        context: String::new(),
                    }));
                }
            };
            result.push(FieldDecl {
                name,
                ty,
                default: None,
                span: chunk[0].span(),
            });
        }

        Ok(result)
    }

    // =========================================================================
    // Derived Component Declaration Analysis
    // =========================================================================

    /// Analyze a top-level form and return a derived component if it's a derived declaration.
    #[allow(clippy::too_many_lines)]
    pub fn analyze_derived(ast: &Ast) -> Result<Option<DerivedDecl>> {
        let list = match ast {
            Ast::List(elements, span) => (elements, *span),
            _ => return Ok(None),
        };

        let (elements, span) = list;
        if elements.is_empty() {
            return Ok(None);
        }

        // Check for (derived: name ...) form
        match &elements[0] {
            Ast::Symbol(s, _) if s == "derived:" => {}
            _ => return Ok(None),
        }

        if elements.len() < 2 {
            return Err(Error::new(ErrorKind::ParseError {
                message: "derived: requires a name".to_string(),
                line: span.line,
                column: span.column,
                context: String::new(),
            }));
        }

        // Get derived name
        let name = match &elements[1] {
            Ast::Symbol(s, _) => s.clone(),
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!("derived name must be a symbol, got {}", other.type_name()),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        // We need :for and :value at minimum
        let mut for_var: Option<String> = None;
        let mut pattern = Pattern::new();
        let mut bindings = Vec::new();
        let mut aggregates = Vec::new();
        let mut value_expr: Option<Ast> = None;

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
                "for" => {
                    for_var = Some(match value {
                        Ast::Symbol(s, _) if s.starts_with('?') => s[1..].to_string(),
                        other => {
                            return Err(Error::new(ErrorKind::ParseError {
                                message: format!(
                                    ":for must be a ?variable, got {}",
                                    other.type_name()
                                ),
                                line: other.span().line,
                                column: other.span().column,
                                context: String::new(),
                            }));
                        }
                    });
                }
                "where" => {
                    pattern = Self::analyze_where_clause(value)?;
                }
                "let" => {
                    bindings = Self::analyze_let_bindings(value)?;
                }
                "aggregate" => {
                    aggregates = Self::analyze_aggregate_clause(value)?;
                }
                "value" => {
                    value_expr = Some(value.clone());
                }
                other => {
                    return Err(Error::new(ErrorKind::ParseError {
                        message: format!("unknown derived clause :{other}"),
                        line: value.span().line,
                        column: value.span().column,
                        context: String::new(),
                    }));
                }
            }
        }

        // Validate required fields
        let for_var = for_var.ok_or_else(|| {
            Error::new(ErrorKind::ParseError {
                message: "derived: requires :for clause".to_string(),
                line: span.line,
                column: span.column,
                context: String::new(),
            })
        })?;

        let value_expr = value_expr.ok_or_else(|| {
            Error::new(ErrorKind::ParseError {
                message: "derived: requires :value clause".to_string(),
                line: span.line,
                column: span.column,
                context: String::new(),
            })
        })?;

        let mut derived = DerivedDecl::new(name, for_var, value_expr, span);
        derived.pattern = pattern;
        derived.bindings = bindings;
        derived.aggregates = aggregates;

        Ok(Some(derived))
    }

    /// Analyze an :aggregate clause into a list of (name, expression) pairs.
    fn analyze_aggregate_clause(ast: &Ast) -> Result<Vec<(String, Ast)>> {
        let map = match ast {
            Ast::Map(entries, _) => entries,
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!(":aggregate must be a map, got {}", other.type_name()),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        let mut result = Vec::new();
        for (key, value) in map {
            let name = match key {
                Ast::Keyword(k, _) => k.clone(),
                other => {
                    return Err(Error::new(ErrorKind::ParseError {
                        message: format!(
                            "aggregate key must be a keyword, got {}",
                            other.type_name()
                        ),
                        line: other.span().line,
                        column: other.span().column,
                        context: String::new(),
                    }));
                }
            };
            result.push((name, value.clone()));
        }

        Ok(result)
    }

    // =========================================================================
    // Constraint Declaration Analysis
    // =========================================================================

    /// Analyze a top-level form and return a constraint if it's a constraint declaration.
    #[allow(clippy::too_many_lines)]
    pub fn analyze_constraint(ast: &Ast) -> Result<Option<ConstraintDecl>> {
        let list = match ast {
            Ast::List(elements, span) => (elements, *span),
            _ => return Ok(None),
        };

        let (elements, span) = list;
        if elements.is_empty() {
            return Ok(None);
        }

        // Check for (constraint: name ...) form
        match &elements[0] {
            Ast::Symbol(s, _) if s == "constraint:" => {}
            _ => return Ok(None),
        }

        if elements.len() < 2 {
            return Err(Error::new(ErrorKind::ParseError {
                message: "constraint: requires a name".to_string(),
                line: span.line,
                column: span.column,
                context: String::new(),
            }));
        }

        // Get constraint name
        let name = match &elements[1] {
            Ast::Symbol(s, _) => s.clone(),
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!(
                        "constraint name must be a symbol, got {}",
                        other.type_name()
                    ),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        let mut constraint = ConstraintDecl::new(name, span);

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
                "where" => {
                    constraint.pattern = Self::analyze_where_clause(value)?;
                }
                "let" => {
                    constraint.bindings = Self::analyze_let_bindings(value)?;
                }
                "aggregate" => {
                    constraint.aggregates = Self::analyze_aggregate_clause(value)?;
                }
                "guard" => {
                    constraint.guards = Self::analyze_guard_clause(value)?;
                }
                "check" => {
                    constraint.checks = Self::analyze_check_clause(value)?;
                }
                "on-violation" => {
                    constraint.on_violation = match value {
                        Ast::Keyword(k, _) => match k.as_str() {
                            "rollback" => ConstraintViolation::Rollback,
                            "warn" => ConstraintViolation::Warn,
                            other => {
                                return Err(Error::new(ErrorKind::ParseError {
                                    message: format!("invalid on-violation :{other}"),
                                    line: value.span().line,
                                    column: value.span().column,
                                    context: String::new(),
                                }));
                            }
                        },
                        other => {
                            return Err(Error::new(ErrorKind::ParseError {
                                message: format!(
                                    ":on-violation must be a keyword, got {}",
                                    other.type_name()
                                ),
                                line: other.span().line,
                                column: other.span().column,
                                context: String::new(),
                            }));
                        }
                    };
                }
                other => {
                    return Err(Error::new(ErrorKind::ParseError {
                        message: format!("unknown constraint clause :{other}"),
                        line: value.span().line,
                        column: value.span().column,
                        context: String::new(),
                    }));
                }
            }
        }

        Ok(Some(constraint))
    }

    /// Analyze a :check clause.
    fn analyze_check_clause(ast: &Ast) -> Result<Vec<Ast>> {
        match ast {
            Ast::Vector(elements, _) => Ok(elements.clone()),
            other => Err(Error::new(ErrorKind::ParseError {
                message: format!(":check must be a vector, got {}", other.type_name()),
                line: other.span().line,
                column: other.span().column,
                context: String::new(),
            })),
        }
    }

    // =========================================================================
    // Query Declaration Analysis
    // =========================================================================

    /// Analyze a top-level form and return a query if it's a query expression.
    #[allow(clippy::too_many_lines)]
    pub fn analyze_query(ast: &Ast) -> Result<Option<QueryDecl>> {
        let list = match ast {
            Ast::List(elements, span) => (elements, *span),
            _ => return Ok(None),
        };

        let (elements, span) = list;
        if elements.is_empty() {
            return Ok(None);
        }

        // Check for (query ...) form
        match &elements[0] {
            Ast::Symbol(s, _) if s == "query" => {}
            _ => return Ok(None),
        }

        let mut query = QueryDecl::new(span);

        // Parse keyword arguments
        let mut i = 1;
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
                "where" => {
                    query.pattern = Self::analyze_where_clause(value)?;
                }
                "let" => {
                    query.bindings = Self::analyze_let_bindings(value)?;
                }
                "aggregate" => {
                    query.aggregates = Self::analyze_aggregate_clause(value)?;
                }
                "group-by" => {
                    query.group_by = Self::analyze_group_by_clause(value)?;
                }
                "guard" => {
                    query.guards = Self::analyze_guard_clause(value)?;
                }
                "order-by" => {
                    query.order_by = Self::analyze_order_by_clause(value)?;
                }
                "limit" => {
                    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                    {
                        query.limit = Some(match value {
                            Ast::Int(n, _) if *n > 0 => *n as usize,
                            other => {
                                return Err(Error::new(ErrorKind::ParseError {
                                    message: format!(
                                        ":limit must be a positive integer, got {}",
                                        other.type_name()
                                    ),
                                    line: other.span().line,
                                    column: other.span().column,
                                    context: String::new(),
                                }));
                            }
                        });
                    }
                }
                "return" => {
                    query.return_expr = Some(value.clone());
                }
                other => {
                    return Err(Error::new(ErrorKind::ParseError {
                        message: format!("unknown query clause :{other}"),
                        line: value.span().line,
                        column: value.span().column,
                        context: String::new(),
                    }));
                }
            }
        }

        Ok(Some(query))
    }

    /// Analyze a :group-by clause.
    fn analyze_group_by_clause(ast: &Ast) -> Result<Vec<String>> {
        let vars = match ast {
            Ast::Vector(elements, _) => elements,
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!(":group-by must be a vector, got {}", other.type_name()),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        let mut result = Vec::new();
        for v in vars {
            match v {
                Ast::Symbol(s, _) if s.starts_with('?') => {
                    result.push(s[1..].to_string());
                }
                other => {
                    return Err(Error::new(ErrorKind::ParseError {
                        message: format!(
                            "group-by variable must be a ?variable, got {}",
                            other.type_name()
                        ),
                        line: other.span().line,
                        column: other.span().column,
                        context: String::new(),
                    }));
                }
            }
        }

        Ok(result)
    }

    /// Analyze an :order-by clause.
    fn analyze_order_by_clause(ast: &Ast) -> Result<Vec<(String, OrderDirection)>> {
        let orders = match ast {
            Ast::Vector(elements, _) => elements,
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!(":order-by must be a vector, got {}", other.type_name()),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        let mut result = Vec::new();
        for order in orders {
            match order {
                Ast::Vector(pair, span) => {
                    if pair.len() != 2 {
                        return Err(Error::new(ErrorKind::ParseError {
                            message: "order-by entry must be [?var :asc/:desc]".to_string(),
                            line: span.line,
                            column: span.column,
                            context: String::new(),
                        }));
                    }
                    let var = match &pair[0] {
                        Ast::Symbol(s, _) if s.starts_with('?') => s[1..].to_string(),
                        other => {
                            return Err(Error::new(ErrorKind::ParseError {
                                message: format!(
                                    "order-by variable must be a ?variable, got {}",
                                    other.type_name()
                                ),
                                line: other.span().line,
                                column: other.span().column,
                                context: String::new(),
                            }));
                        }
                    };
                    let dir = match &pair[1] {
                        Ast::Keyword(k, _) => match k.as_str() {
                            "asc" => OrderDirection::Asc,
                            "desc" => OrderDirection::Desc,
                            other => {
                                return Err(Error::new(ErrorKind::ParseError {
                                    message: format!(
                                        "order direction must be :asc or :desc, got :{other}"
                                    ),
                                    line: pair[1].span().line,
                                    column: pair[1].span().column,
                                    context: String::new(),
                                }));
                            }
                        },
                        other => {
                            return Err(Error::new(ErrorKind::ParseError {
                                message: format!(
                                    "order direction must be a keyword, got {}",
                                    other.type_name()
                                ),
                                line: other.span().line,
                                column: other.span().column,
                                context: String::new(),
                            }));
                        }
                    };
                    result.push((var, dir));
                }
                other => {
                    return Err(Error::new(ErrorKind::ParseError {
                        message: format!(
                            "order-by entry must be a vector, got {}",
                            other.type_name()
                        ),
                        line: other.span().line,
                        column: other.span().column,
                        context: String::new(),
                    }));
                }
            }
        }

        Ok(result)
    }

    // =========================================================================
    // Unified Analysis
    // =========================================================================

    /// Analyze any top-level declaration.
    pub fn analyze(ast: &Ast) -> Result<Option<Declaration>> {
        // Try each declaration type
        if let Some(comp) = Self::analyze_component(ast)? {
            return Ok(Some(Declaration::Component(comp)));
        }
        if let Some(rel) = Self::analyze_relationship(ast)? {
            return Ok(Some(Declaration::Relationship(rel)));
        }
        if let Some(rule) = Self::analyze_rule(ast)? {
            return Ok(Some(Declaration::Rule(rule)));
        }
        if let Some(derived) = Self::analyze_derived(ast)? {
            return Ok(Some(Declaration::Derived(derived)));
        }
        if let Some(constraint) = Self::analyze_constraint(ast)? {
            return Ok(Some(Declaration::Constraint(constraint)));
        }
        if let Some(query) = Self::analyze_query(ast)? {
            return Ok(Some(Declaration::Query(query)));
        }

        Ok(None)
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
            r"(rule: my-rule
                 :where [[?e :health ?hp]]
                 :then [(print! ?hp)])",
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
            r"(rule: priority-rule
                 :salience 100
                 :once true
                 :where [[?e :tag/player true]]
                 :then [])",
        );

        let rule = DeclarationAnalyzer::analyze_rule(&ast).unwrap().unwrap();

        assert_eq!(rule.name, "priority-rule");
        assert_eq!(rule.salience, 100);
        assert!(rule.once);
        // Check that value is a literal Bool(true) regardless of span
        match &rule.pattern.clauses[0].value {
            PatternValue::Literal(Ast::Bool(true, _)) => {}
            other => panic!("expected Literal(Bool(true, _)), got {other:?}"),
        }
    }

    #[test]
    fn analyze_rule_with_negation() {
        let ast = parse(
            r"(rule: no-velocity
                 :where [[?e :position _]
                         (not [?e :velocity])]
                 :then [])",
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
            r"(rule: wildcard
                 :where [[?e :position _]]
                 :then [])",
        );

        let rule = DeclarationAnalyzer::analyze_rule(&ast).unwrap().unwrap();

        assert_eq!(rule.pattern.clauses[0].value, PatternValue::Wildcard);
    }

    #[test]
    fn analyze_multiple_patterns() {
        let ast = parse(
            r"(rule: multi
                 :where [[?e :position ?pos]
                         [?e :velocity ?vel]]
                 :then [])",
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

    // =========================================================================
    // Component Tests
    // =========================================================================

    #[test]
    fn analyze_simple_component() {
        let ast = parse(
            r"(component: health
                 :current :int
                 :max :int)",
        );

        let comp = DeclarationAnalyzer::analyze_component(&ast)
            .unwrap()
            .unwrap();

        assert_eq!(comp.name, "health");
        assert!(!comp.is_tag);
        assert_eq!(comp.fields.len(), 2);
        assert_eq!(comp.fields[0].name, "current");
        assert_eq!(comp.fields[0].ty, "int");
        assert!(comp.fields[0].default.is_none());
        assert_eq!(comp.fields[1].name, "max");
        assert_eq!(comp.fields[1].ty, "int");
    }

    #[test]
    fn analyze_component_with_defaults() {
        let ast = parse(
            r"(component: health
                 :current :int
                 :max :int :default 100
                 :regen-rate :float :default 0.5)",
        );

        let comp = DeclarationAnalyzer::analyze_component(&ast)
            .unwrap()
            .unwrap();

        assert_eq!(comp.fields.len(), 3);
        assert!(comp.fields[0].default.is_none());
        assert!(comp.fields[1].default.is_some());
        assert_eq!(comp.fields[1].default.as_ref().unwrap().as_int(), Some(100));
        assert!(comp.fields[2].default.is_some());
    }

    #[test]
    fn analyze_tag_component() {
        let ast = parse("(component: tag/player :bool :default true)");

        let comp = DeclarationAnalyzer::analyze_component(&ast)
            .unwrap()
            .unwrap();

        assert_eq!(comp.name, "tag/player");
        assert!(comp.is_tag);
        assert_eq!(comp.fields.len(), 1);
        assert_eq!(comp.fields[0].name, "value");
        assert_eq!(comp.fields[0].ty, "bool");
        match &comp.fields[0].default {
            Some(Ast::Bool(true, _)) => {}
            other => panic!("expected Bool(true), got {other:?}"),
        }
    }

    #[test]
    fn analyze_tag_without_default() {
        let ast = parse("(component: tag/enemy :bool)");

        let comp = DeclarationAnalyzer::analyze_component(&ast)
            .unwrap()
            .unwrap();

        assert!(comp.is_tag);
        assert!(comp.fields[0].default.is_none());
    }

    // =========================================================================
    // Relationship Tests
    // =========================================================================

    #[test]
    fn analyze_simple_relationship() {
        let ast = parse(
            r"(relationship: in-room
                 :storage :field
                 :cardinality :many-to-one)",
        );

        let rel = DeclarationAnalyzer::analyze_relationship(&ast)
            .unwrap()
            .unwrap();

        assert_eq!(rel.name, "in-room");
        assert_eq!(rel.storage, StorageKind::Field);
        assert_eq!(rel.cardinality, Cardinality::ManyToOne);
        assert_eq!(rel.on_target_delete, OnTargetDelete::Remove);
        assert!(rel.required);
    }

    #[test]
    fn analyze_full_relationship() {
        let ast = parse(
            r"(relationship: employment
                 :storage :entity
                 :cardinality :many-to-many
                 :on-target-delete :cascade
                 :on-violation :replace
                 :required false
                 :attributes [:start-date :int :salary :int])",
        );

        let rel = DeclarationAnalyzer::analyze_relationship(&ast)
            .unwrap()
            .unwrap();

        assert_eq!(rel.name, "employment");
        assert_eq!(rel.storage, StorageKind::Entity);
        assert_eq!(rel.cardinality, Cardinality::ManyToMany);
        assert_eq!(rel.on_target_delete, OnTargetDelete::Cascade);
        assert_eq!(rel.on_violation, OnViolation::Replace);
        assert!(!rel.required);
        assert_eq!(rel.attributes.len(), 2);
        assert_eq!(rel.attributes[0].name, "start-date");
        assert_eq!(rel.attributes[1].name, "salary");
    }

    #[test]
    fn analyze_relationship_all_cardinalities() {
        for (src, expected) in [
            (":one-to-one", Cardinality::OneToOne),
            (":one-to-many", Cardinality::OneToMany),
            (":many-to-one", Cardinality::ManyToOne),
            (":many-to-many", Cardinality::ManyToMany),
        ] {
            let ast = parse(&format!("(relationship: test :cardinality {src})"));
            let rel = DeclarationAnalyzer::analyze_relationship(&ast)
                .unwrap()
                .unwrap();
            assert_eq!(rel.cardinality, expected);
        }
    }

    // =========================================================================
    // Derived Tests
    // =========================================================================

    #[test]
    fn analyze_simple_derived() {
        let ast = parse(
            r"(derived: health/percent
                 :for ?self
                 :where [[?self :health/current ?curr]
                         [?self :health/max ?max]]
                 :value (/ (* ?curr 100) ?max))",
        );

        let derived = DeclarationAnalyzer::analyze_derived(&ast).unwrap().unwrap();

        assert_eq!(derived.name, "health/percent");
        assert_eq!(derived.for_var, "self");
        assert_eq!(derived.pattern.clauses.len(), 2);
        assert!(derived.value.is_list());
    }

    #[test]
    fn analyze_derived_with_aggregation() {
        let ast = parse(
            r"(derived: faction/total-power
                 :for ?faction
                 :where [[?faction :tag/faction]
                         [?member :faction ?faction]
                         [?member :power ?p]]
                 :aggregate {:total (sum ?p)}
                 :value ?total)",
        );

        let derived = DeclarationAnalyzer::analyze_derived(&ast).unwrap().unwrap();

        assert_eq!(derived.name, "faction/total-power");
        assert_eq!(derived.aggregates.len(), 1);
        assert_eq!(derived.aggregates[0].0, "total");
    }

    #[test]
    fn analyze_derived_missing_for() {
        let ast = parse(
            r"(derived: bad
                 :where [[?e :health ?hp]]
                 :value ?hp)",
        );

        let result = DeclarationAnalyzer::analyze_derived(&ast);
        assert!(result.is_err());
    }

    #[test]
    fn analyze_derived_missing_value() {
        let ast = parse(
            r"(derived: bad
                 :for ?self
                 :where [[?self :health ?hp]])",
        );

        let result = DeclarationAnalyzer::analyze_derived(&ast);
        assert!(result.is_err());
    }

    // =========================================================================
    // Constraint Tests
    // =========================================================================

    #[test]
    fn analyze_simple_constraint() {
        let ast = parse(
            r"(constraint: health-bounds
                 :where [[?e :health/current ?hp]
                         [?e :health/max ?max]]
                 :check [(>= ?hp 0) (<= ?hp ?max)])",
        );

        let constraint = DeclarationAnalyzer::analyze_constraint(&ast)
            .unwrap()
            .unwrap();

        assert_eq!(constraint.name, "health-bounds");
        assert_eq!(constraint.pattern.clauses.len(), 2);
        assert_eq!(constraint.checks.len(), 2);
        assert_eq!(constraint.on_violation, ConstraintViolation::Rollback);
    }

    #[test]
    fn analyze_constraint_with_warn() {
        let ast = parse(
            r"(constraint: warn-on-damage
                 :where [[?e :damage ?d]]
                 :check [(< ?d 1000)]
                 :on-violation :warn)",
        );

        let constraint = DeclarationAnalyzer::analyze_constraint(&ast)
            .unwrap()
            .unwrap();

        assert_eq!(constraint.on_violation, ConstraintViolation::Warn);
    }

    #[test]
    fn analyze_constraint_with_guard() {
        let ast = parse(
            r"(constraint: guarded
                 :where [[?e :tag/player]]
                 :guard [(active? ?e)]
                 :check [(valid? ?e)])",
        );

        let constraint = DeclarationAnalyzer::analyze_constraint(&ast)
            .unwrap()
            .unwrap();

        assert_eq!(constraint.guards.len(), 1);
        assert_eq!(constraint.checks.len(), 1);
    }

    // =========================================================================
    // Query Tests
    // =========================================================================

    #[test]
    fn analyze_simple_query() {
        let ast = parse(
            r"(query
                 :where [[?e :health/current ?hp]]
                 :return ?e)",
        );

        let query = DeclarationAnalyzer::analyze_query(&ast).unwrap().unwrap();

        assert_eq!(query.pattern.clauses.len(), 1);
        assert!(query.return_expr.is_some());
    }

    #[test]
    fn analyze_full_query() {
        let ast = parse(
            r"(query
                 :where [[?e :health/current ?hp]
                         [?e :name ?name]]
                 :let [threshold 50]
                 :guard [(< ?hp threshold)]
                 :group-by [?name]
                 :order-by [[?hp :desc]]
                 :limit 10
                 :return {:entity ?e :hp ?hp})",
        );

        let query = DeclarationAnalyzer::analyze_query(&ast).unwrap().unwrap();

        assert_eq!(query.pattern.clauses.len(), 2);
        assert_eq!(query.bindings.len(), 1);
        assert_eq!(query.guards.len(), 1);
        assert_eq!(query.group_by.len(), 1);
        assert_eq!(query.group_by[0], "name");
        assert_eq!(query.order_by.len(), 1);
        assert_eq!(query.order_by[0].0, "hp");
        assert_eq!(query.order_by[0].1, OrderDirection::Desc);
        assert_eq!(query.limit, Some(10));
        assert!(query.return_expr.is_some());
    }

    #[test]
    fn analyze_query_order_by_asc() {
        let ast = parse(
            r"(query
                 :where [[?e :score ?s]]
                 :order-by [[?s :asc]]
                 :return ?e)",
        );

        let query = DeclarationAnalyzer::analyze_query(&ast).unwrap().unwrap();

        assert_eq!(query.order_by[0].1, OrderDirection::Asc);
    }

    #[test]
    fn analyze_query_with_aggregates() {
        let ast = parse(
            r"(query
                 :where [[?e :faction ?f]
                         [?e :power ?p]]
                 :aggregate {:total (sum ?p) :count (count ?e)}
                 :return {:faction ?f :total ?total :count ?count})",
        );

        let query = DeclarationAnalyzer::analyze_query(&ast).unwrap().unwrap();

        assert_eq!(query.aggregates.len(), 2);
    }

    // =========================================================================
    // Unified Analysis Tests
    // =========================================================================

    #[test]
    fn unified_analyze_component() {
        let ast = parse("(component: test :value :int)");
        let decl = DeclarationAnalyzer::analyze(&ast).unwrap().unwrap();
        assert!(matches!(decl, Declaration::Component(_)));
    }

    #[test]
    fn unified_analyze_relationship() {
        let ast = parse("(relationship: test :storage :field)");
        let decl = DeclarationAnalyzer::analyze(&ast).unwrap().unwrap();
        assert!(matches!(decl, Declaration::Relationship(_)));
    }

    #[test]
    fn unified_analyze_rule() {
        let ast = parse("(rule: test :where [[?e :tag]] :then [])");
        let decl = DeclarationAnalyzer::analyze(&ast).unwrap().unwrap();
        assert!(matches!(decl, Declaration::Rule(_)));
    }

    #[test]
    fn unified_analyze_derived() {
        let ast = parse("(derived: test :for ?e :value 42)");
        let decl = DeclarationAnalyzer::analyze(&ast).unwrap().unwrap();
        assert!(matches!(decl, Declaration::Derived(_)));
    }

    #[test]
    fn unified_analyze_constraint() {
        let ast = parse("(constraint: test :check [true])");
        let decl = DeclarationAnalyzer::analyze(&ast).unwrap().unwrap();
        assert!(matches!(decl, Declaration::Constraint(_)));
    }

    #[test]
    fn unified_analyze_query() {
        let ast = parse("(query :where [[?e :tag]] :return ?e)");
        let decl = DeclarationAnalyzer::analyze(&ast).unwrap().unwrap();
        assert!(matches!(decl, Declaration::Query(_)));
    }

    #[test]
    fn unified_analyze_non_declaration() {
        let ast = parse("(+ 1 2)");
        let decl = DeclarationAnalyzer::analyze(&ast).unwrap();
        assert!(decl.is_none());
    }
}
