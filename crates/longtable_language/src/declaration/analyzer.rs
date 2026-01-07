//! Declaration analyzer implementation.
//!
//! Contains the `DeclarationAnalyzer` struct and all analysis methods.

use crate::ast::Ast;
use crate::namespace::{LoadDecl, NamespaceDecl, NamespaceName, RequireSpec};
use crate::span::Span;
use longtable_foundation::{Error, ErrorKind, Result};

use super::Declaration;
use super::types::{
    Cardinality, ComponentDecl, ConstraintDecl, ConstraintViolation, DerivedDecl, FieldDecl,
    LinkDecl, OnTargetDelete, OnViolation, OrderDirection, Pattern, PatternClause, PatternValue,
    QueryDecl, RelationshipDecl, RuleDecl, SpawnDecl, StorageKind,
};

/// Analyzes AST and extracts typed declarations.
pub struct DeclarationAnalyzer;

impl DeclarationAnalyzer {
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
        if let Some(ns) = Self::analyze_namespace(ast)? {
            return Ok(Some(Declaration::Namespace(ns)));
        }
        if let Some(load) = Self::analyze_load(ast)? {
            return Ok(Some(Declaration::Load(load)));
        }
        if let Some(spawn) = Self::analyze_spawn(ast)? {
            return Ok(Some(Declaration::Spawn(spawn)));
        }
        if let Some(link) = Self::analyze_link(ast)? {
            return Ok(Some(Declaration::Link(link)));
        }

        Ok(None)
    }

    // =========================================================================
    // Rule Declaration Analysis
    // =========================================================================

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
    pub(crate) fn analyze_where_clause(ast: &Ast) -> Result<Pattern> {
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
                                message: "only (not [...]) is allowed in :where clause lists"
                                    .to_string(),
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
                            "pattern must be a vector or (not [...]), got {}",
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

    /// Analyze a single pattern clause like [?e :component ?value].
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
                    message: format!("entity must be a ?variable, got {}", other.type_name()),
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
                    message: format!("component must be a keyword, got {}", other.type_name()),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        // Value (optional)
        let value = if elements.len() == 3 {
            match &elements[2] {
                Ast::Symbol(s, _) if s.starts_with('?') => {
                    PatternValue::Variable(s[1..].to_string())
                }
                Ast::Symbol(s, _) if s == "_" => PatternValue::Wildcard,
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
    pub(crate) fn analyze_let_bindings(ast: &Ast) -> Result<Vec<(String, Ast)>> {
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
                message: ":let bindings must be pairs of [name value ...]".to_string(),
                line: ast.span().line,
                column: ast.span().column,
                context: String::new(),
            }));
        }

        let mut result = Vec::new();
        let mut i = 0;
        while i < bindings.len() {
            let name = match &bindings[i] {
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
            let value = bindings[i + 1].clone();
            result.push((name, value));
            i += 2;
        }

        Ok(result)
    }

    /// Analyze a :guard clause.
    pub(crate) fn analyze_guard_clause(ast: &Ast) -> Result<Vec<Ast>> {
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
                        message: format!("unknown relationship clause :{other}"),
                        line: value.span().line,
                        column: value.span().column,
                        context: String::new(),
                    }));
                }
            }
        }

        Ok(Some(rel))
    }

    /// Analyze an :attributes list into field declarations.
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
                message: ":attributes must be pairs of [:name :type ...]".to_string(),
                line: ast.span().line,
                column: ast.span().column,
                context: String::new(),
            }));
        }

        let mut result = Vec::new();
        let mut i = 0;
        while i < attrs.len() {
            let name = match &attrs[i] {
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
            let ty = match &attrs[i + 1] {
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
                span: attrs[i].span(),
            });
            i += 2;
        }

        Ok(result)
    }

    // =========================================================================
    // Derived Declaration Analysis
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
    pub(crate) fn analyze_aggregate_clause(ast: &Ast) -> Result<Vec<(String, Ast)>> {
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
    // Namespace Declaration Analysis
    // =========================================================================

    /// Analyze a top-level form and return a namespace if it's a namespace declaration.
    #[allow(clippy::too_many_lines)]
    pub fn analyze_namespace(ast: &Ast) -> Result<Option<NamespaceDecl>> {
        let list = match ast {
            Ast::List(elements, span) => (elements, *span),
            _ => return Ok(None),
        };

        let (elements, span) = list;
        if elements.is_empty() {
            return Ok(None);
        }

        // Check for (namespace name ...) form
        match &elements[0] {
            Ast::Symbol(s, _) if s == "namespace" => {}
            _ => return Ok(None),
        }

        if elements.len() < 2 {
            return Err(Error::new(ErrorKind::ParseError {
                message: "namespace requires a name".to_string(),
                line: span.line,
                column: span.column,
                context: String::new(),
            }));
        }

        // Get namespace name (can be dotted like game.combat)
        let name = match &elements[1] {
            Ast::Symbol(s, _) => NamespaceName::parse(s),
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!("namespace name must be a symbol, got {}", other.type_name()),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        let mut ns_decl = NamespaceDecl::new(name, span);

        // Parse optional clauses like (:require [...])
        for element in &elements[2..] {
            if let Ast::List(clause, clause_span) = element {
                if clause.is_empty() {
                    continue;
                }

                // Check for (:require [...]) form
                if let Ast::Keyword(kw, _) = &clause[0] {
                    if kw == "require" {
                        ns_decl.requires =
                            Self::analyze_require_clause(&clause[1..], *clause_span)?;
                    } else {
                        return Err(Error::new(ErrorKind::ParseError {
                            message: format!("unknown namespace clause :{kw}"),
                            line: clause_span.line,
                            column: clause_span.column,
                            context: String::new(),
                        }));
                    }
                }
            }
        }

        Ok(Some(ns_decl))
    }

    /// Analyze a (:require [...]) clause into `RequireSpec` items.
    fn analyze_require_clause(items: &[Ast], _span: Span) -> Result<Vec<RequireSpec>> {
        let mut requires = Vec::new();

        for item in items {
            let req = Self::analyze_require_spec(item)?;
            requires.push(req);
        }

        Ok(requires)
    }

    /// Analyze a single require spec like [game.core :as core] or [game.utils :refer [foo bar]].
    fn analyze_require_spec(ast: &Ast) -> Result<RequireSpec> {
        let elements = match ast {
            Ast::Vector(elements, _) => elements,
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!("require spec must be a vector, got {}", other.type_name()),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        if elements.is_empty() {
            return Err(Error::new(ErrorKind::ParseError {
                message: "require spec vector cannot be empty".to_string(),
                line: ast.span().line,
                column: ast.span().column,
                context: String::new(),
            }));
        }

        // First element is the namespace name
        let ns_name = match &elements[0] {
            Ast::Symbol(s, _) => NamespaceName::parse(s),
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!(
                        "namespace in require must be a symbol, got {}",
                        other.type_name()
                    ),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        // If only namespace, it's a Use
        if elements.len() == 1 {
            return Ok(RequireSpec::Use { namespace: ns_name });
        }

        // Parse :as or :refer
        let kw = match &elements[1] {
            Ast::Keyword(k, _) => k.as_str(),
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!(
                        "expected keyword in require spec, got {}",
                        other.type_name()
                    ),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        if elements.len() < 3 {
            return Err(Error::new(ErrorKind::ParseError {
                message: format!("missing value for :{kw} in require spec"),
                line: ast.span().line,
                column: ast.span().column,
                context: String::new(),
            }));
        }

        match kw {
            "as" => {
                let alias = match &elements[2] {
                    Ast::Symbol(s, _) => s.clone(),
                    other => {
                        return Err(Error::new(ErrorKind::ParseError {
                            message: format!(
                                ":as alias must be a symbol, got {}",
                                other.type_name()
                            ),
                            line: other.span().line,
                            column: other.span().column,
                            context: String::new(),
                        }));
                    }
                };
                Ok(RequireSpec::Alias {
                    namespace: ns_name,
                    alias,
                })
            }
            "refer" => {
                let symbols = Self::analyze_refer_symbols(&elements[2])?;
                Ok(RequireSpec::Refer {
                    namespace: ns_name,
                    symbols,
                })
            }
            other => Err(Error::new(ErrorKind::ParseError {
                message: format!("unknown require option :{other}"),
                line: elements[1].span().line,
                column: elements[1].span().column,
                context: String::new(),
            })),
        }
    }

    /// Analyze the symbols in a :refer clause like [foo bar baz].
    fn analyze_refer_symbols(ast: &Ast) -> Result<Vec<String>> {
        let elements = match ast {
            Ast::Vector(elements, _) => elements,
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!(":refer value must be a vector, got {}", other.type_name()),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        let mut symbols = Vec::new();
        for elem in elements {
            match elem {
                Ast::Symbol(s, _) => symbols.push(s.clone()),
                other => {
                    return Err(Error::new(ErrorKind::ParseError {
                        message: format!(
                            "symbol in :refer must be a symbol, got {}",
                            other.type_name()
                        ),
                        line: other.span().line,
                        column: other.span().column,
                        context: String::new(),
                    }));
                }
            }
        }

        Ok(symbols)
    }

    // =========================================================================
    // Load Directive Analysis
    // =========================================================================

    /// Analyze a top-level form and return a load directive if it's a load form.
    pub fn analyze_load(ast: &Ast) -> Result<Option<LoadDecl>> {
        let list = match ast {
            Ast::List(elements, span) => (elements, *span),
            _ => return Ok(None),
        };

        let (elements, span) = list;
        if elements.is_empty() {
            return Ok(None);
        }

        // Check for (load "path") form
        match &elements[0] {
            Ast::Symbol(s, _) if s == "load" => {}
            _ => return Ok(None),
        }

        if elements.len() != 2 {
            return Err(Error::new(ErrorKind::ParseError {
                message: "load requires exactly one path argument".to_string(),
                line: span.line,
                column: span.column,
                context: String::new(),
            }));
        }

        let path = match &elements[1] {
            Ast::String(s, _) => s.clone(),
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!("load path must be a string, got {}", other.type_name()),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        Ok(Some(LoadDecl::new(path, span)))
    }

    // =========================================================================
    // Spawn Declaration Analysis
    // =========================================================================

    /// Analyze a top-level form and return a spawn declaration if it's a spawn form.
    ///
    /// Spawn form: `(spawn: name :component value ...)`
    pub fn analyze_spawn(ast: &Ast) -> Result<Option<SpawnDecl>> {
        let list = match ast {
            Ast::List(elements, span) => (elements, *span),
            _ => return Ok(None),
        };

        let (elements, span) = list;
        if elements.is_empty() {
            return Ok(None);
        }

        // Check for (spawn: name ...) form
        match &elements[0] {
            Ast::Symbol(s, _) if s == "spawn:" => {}
            _ => return Ok(None),
        }

        if elements.len() < 2 {
            return Err(Error::new(ErrorKind::ParseError {
                message: "spawn: requires a name".to_string(),
                line: span.line,
                column: span.column,
                context: String::new(),
            }));
        }

        // Get entity name
        let name = match &elements[1] {
            Ast::Symbol(s, _) => s.clone(),
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!("spawn name must be a symbol, got {}", other.type_name()),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        let mut decl = SpawnDecl::new(name, span);

        // Parse component keyword-value pairs
        let mut i = 2;
        while i < elements.len() {
            let component = match &elements[i] {
                Ast::Keyword(k, _) => k.clone(),
                other => {
                    return Err(Error::new(ErrorKind::ParseError {
                        message: format!("expected component keyword, got {}", other.type_name()),
                        line: other.span().line,
                        column: other.span().column,
                        context: String::new(),
                    }));
                }
            };

            i += 1;
            if i >= elements.len() {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!("missing value for :{component}"),
                    line: span.line,
                    column: span.column,
                    context: String::new(),
                }));
            }

            let value = elements[i].clone();
            i += 1;

            decl.components.push((component, value));
        }

        Ok(Some(decl))
    }

    // =========================================================================
    // Link Declaration Analysis
    // =========================================================================

    /// Analyze a top-level form and return a link declaration if it's a link form.
    ///
    /// Link form: `(link: source :relationship target)`
    pub fn analyze_link(ast: &Ast) -> Result<Option<LinkDecl>> {
        let list = match ast {
            Ast::List(elements, span) => (elements, *span),
            _ => return Ok(None),
        };

        let (elements, span) = list;
        if elements.is_empty() {
            return Ok(None);
        }

        // Check for (link: source :rel target) form
        match &elements[0] {
            Ast::Symbol(s, _) if s == "link:" => {}
            _ => return Ok(None),
        }

        if elements.len() != 4 {
            return Err(Error::new(ErrorKind::ParseError {
                message: "link: requires source, relationship, and target".to_string(),
                line: span.line,
                column: span.column,
                context: String::new(),
            }));
        }

        // Get source entity name
        let source = match &elements[1] {
            Ast::Symbol(s, _) => s.clone(),
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!("link source must be a symbol, got {}", other.type_name()),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        // Get relationship keyword
        let relationship = match &elements[2] {
            Ast::Keyword(k, _) => k.clone(),
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!(
                        "link relationship must be a keyword, got {}",
                        other.type_name()
                    ),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        // Get target entity name
        let target = match &elements[3] {
            Ast::Symbol(s, _) => s.clone(),
            other => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: format!("link target must be a symbol, got {}", other.type_name()),
                    line: other.span().line,
                    column: other.span().column,
                    context: String::new(),
                }));
            }
        };

        Ok(Some(LinkDecl::new(source, relationship, target, span)))
    }
}
