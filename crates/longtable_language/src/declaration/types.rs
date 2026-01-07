//! Declaration type definitions.
//!
//! Contains all the typed declaration structures extracted from AST.

use crate::ast::Ast;
use crate::span::Span;

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
// Spawn Declaration
// =============================================================================

/// A spawn declaration for creating entities.
///
/// Corresponds to:
/// ```clojure
/// (spawn: player
///   :tag/player true
///   :name {:value "Adventurer"}
///   :health {:current 100 :max 100})
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnDecl {
    /// Entity symbolic name (used for referencing in links)
    pub name: String,
    /// Component values (component keyword -> value AST)
    pub components: Vec<(String, Ast)>,
    /// Source span
    pub span: Span,
}

impl SpawnDecl {
    /// Creates a new spawn declaration.
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            components: Vec::new(),
            span,
        }
    }
}

// =============================================================================
// Link Declaration
// =============================================================================

/// A link declaration for creating relationships between entities.
///
/// Corresponds to:
/// ```clojure
/// (link: player :in-room cave-entrance)
/// (link: cave-entrance :exit/south main-hall)
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct LinkDecl {
    /// Source entity name
    pub source: String,
    /// Relationship keyword (e.g., "in-room", "exit/south")
    pub relationship: String,
    /// Target entity name
    pub target: String,
    /// Source span
    pub span: Span,
}

impl LinkDecl {
    /// Creates a new link declaration.
    pub fn new(
        source: impl Into<String>,
        relationship: impl Into<String>,
        target: impl Into<String>,
        span: Span,
    ) -> Self {
        Self {
            source: source.into(),
            relationship: relationship.into(),
            target: target.into(),
            span,
        }
    }
}
