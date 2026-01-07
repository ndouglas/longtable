//! Tests for declaration analysis.

use super::*;
use crate::ast::Ast;
use crate::namespace::RequireSpec;
use crate::parser;

fn parse(src: &str) -> Ast {
    parser::parse(src).unwrap().remove(0)
}

// =========================================================================
// Rule Tests
// =========================================================================

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

// =========================================================================
// Namespace Declaration Tests
// =========================================================================

#[test]
fn analyze_simple_namespace() {
    let ast = parse("(namespace game.core)");
    let ns = DeclarationAnalyzer::analyze_namespace(&ast)
        .unwrap()
        .unwrap();

    assert_eq!(ns.name.full_name(), "game.core");
    assert!(ns.requires.is_empty());
}

#[test]
fn analyze_namespace_with_alias_require() {
    let ast = parse(
        r"(namespace game.combat
               (:require [game.core :as core]))",
    );
    let ns = DeclarationAnalyzer::analyze_namespace(&ast)
        .unwrap()
        .unwrap();

    assert_eq!(ns.name.full_name(), "game.combat");
    assert_eq!(ns.requires.len(), 1);
    match &ns.requires[0] {
        RequireSpec::Alias { namespace, alias } => {
            assert_eq!(namespace.full_name(), "game.core");
            assert_eq!(alias, "core");
        }
        other => panic!("expected Alias, got {other:?}"),
    }
}

#[test]
fn analyze_namespace_with_refer_require() {
    let ast = parse(
        r"(namespace game.combat
               (:require [game.utils :refer [distance clamp]]))",
    );
    let ns = DeclarationAnalyzer::analyze_namespace(&ast)
        .unwrap()
        .unwrap();

    assert_eq!(ns.name.full_name(), "game.combat");
    assert_eq!(ns.requires.len(), 1);
    match &ns.requires[0] {
        RequireSpec::Refer { namespace, symbols } => {
            assert_eq!(namespace.full_name(), "game.utils");
            assert_eq!(symbols, &["distance".to_string(), "clamp".to_string()]);
        }
        other => panic!("expected Refer, got {other:?}"),
    }
}

#[test]
fn analyze_namespace_with_use_require() {
    let ast = parse(
        r"(namespace game.combat
               (:require [game.items]))",
    );
    let ns = DeclarationAnalyzer::analyze_namespace(&ast)
        .unwrap()
        .unwrap();

    assert_eq!(ns.name.full_name(), "game.combat");
    assert_eq!(ns.requires.len(), 1);
    match &ns.requires[0] {
        RequireSpec::Use { namespace } => {
            assert_eq!(namespace.full_name(), "game.items");
        }
        other => panic!("expected Use, got {other:?}"),
    }
}

#[test]
fn analyze_namespace_with_multiple_requires() {
    let ast = parse(
        r"(namespace game.combat
               (:require [game.core :as core]
                         [game.utils :refer [distance]]
                         [game.items]))",
    );
    let ns = DeclarationAnalyzer::analyze_namespace(&ast)
        .unwrap()
        .unwrap();

    assert_eq!(ns.name.full_name(), "game.combat");
    assert_eq!(ns.requires.len(), 3);
    assert!(matches!(&ns.requires[0], RequireSpec::Alias { .. }));
    assert!(matches!(&ns.requires[1], RequireSpec::Refer { .. }));
    assert!(matches!(&ns.requires[2], RequireSpec::Use { .. }));
}

#[test]
fn analyze_namespace_non_namespace_returns_none() {
    let ast = parse("(def foo 42)");
    let result = DeclarationAnalyzer::analyze_namespace(&ast).unwrap();
    assert!(result.is_none());
}

#[test]
fn unified_analyze_namespace() {
    let ast = parse("(namespace game.core)");
    let decl = DeclarationAnalyzer::analyze(&ast).unwrap().unwrap();
    assert!(matches!(decl, Declaration::Namespace(_)));
}

// =========================================================================
// Load Declaration Tests
// =========================================================================

#[test]
fn analyze_simple_load() {
    let ast = parse(r#"(load "game/core.lt")"#);
    let load = DeclarationAnalyzer::analyze_load(&ast).unwrap().unwrap();

    assert_eq!(load.path, "game/core.lt");
}

#[test]
fn analyze_load_non_load_returns_none() {
    let ast = parse("(def foo 42)");
    let result = DeclarationAnalyzer::analyze_load(&ast).unwrap();
    assert!(result.is_none());
}

#[test]
fn analyze_load_missing_path_error() {
    let ast = parse("(load)");
    let result = DeclarationAnalyzer::analyze_load(&ast);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("exactly one path"));
}

#[test]
fn analyze_load_non_string_path_error() {
    let ast = parse("(load 123)");
    let result = DeclarationAnalyzer::analyze_load(&ast);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("must be a string"));
}

#[test]
fn unified_analyze_load() {
    let ast = parse(r#"(load "test.lt")"#);
    let decl = DeclarationAnalyzer::analyze(&ast).unwrap().unwrap();
    assert!(matches!(decl, Declaration::Load(_)));
}

// =========================================================================
// Spawn Declaration Tests
// =========================================================================

#[test]
fn analyze_simple_spawn() {
    let ast = parse("(spawn: player :tag/player true)");
    let spawn = DeclarationAnalyzer::analyze_spawn(&ast).unwrap().unwrap();

    assert_eq!(spawn.name, "player");
    assert_eq!(spawn.components.len(), 1);
    assert_eq!(spawn.components[0].0, "tag/player");
    assert!(matches!(spawn.components[0].1, Ast::Bool(true, _)));
}

#[test]
fn analyze_spawn_with_multiple_components() {
    let ast = parse(
        r#"(spawn: player
             :tag/player true
             :name {:value "Adventurer"}
             :health {:current 100 :max 100})"#,
    );
    let spawn = DeclarationAnalyzer::analyze_spawn(&ast).unwrap().unwrap();

    assert_eq!(spawn.name, "player");
    assert_eq!(spawn.components.len(), 3);
    assert_eq!(spawn.components[0].0, "tag/player");
    assert_eq!(spawn.components[1].0, "name");
    assert_eq!(spawn.components[2].0, "health");
}

#[test]
fn analyze_spawn_non_spawn_returns_none() {
    let ast = parse("(something-else foo)");
    let result = DeclarationAnalyzer::analyze_spawn(&ast).unwrap();
    assert!(result.is_none());
}

#[test]
fn analyze_spawn_missing_name_error() {
    let ast = parse("(spawn:)");
    let result = DeclarationAnalyzer::analyze_spawn(&ast);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("requires a name"));
}

#[test]
fn analyze_spawn_missing_value_error() {
    let ast = parse("(spawn: player :tag/player)");
    let result = DeclarationAnalyzer::analyze_spawn(&ast);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("missing value"));
}

#[test]
fn unified_analyze_spawn() {
    let ast = parse("(spawn: player :tag/player true)");
    let decl = DeclarationAnalyzer::analyze(&ast).unwrap().unwrap();
    assert!(matches!(decl, Declaration::Spawn(_)));
}

// =========================================================================
// Link Declaration Tests
// =========================================================================

#[test]
fn analyze_simple_link() {
    let ast = parse("(link: player :in-room cave-entrance)");
    let link = DeclarationAnalyzer::analyze_link(&ast).unwrap().unwrap();

    assert_eq!(link.source, "player");
    assert_eq!(link.relationship, "in-room");
    assert_eq!(link.target, "cave-entrance");
}

#[test]
fn analyze_link_with_namespaced_relationship() {
    let ast = parse("(link: cave-entrance :exit/south main-hall)");
    let link = DeclarationAnalyzer::analyze_link(&ast).unwrap().unwrap();

    assert_eq!(link.source, "cave-entrance");
    assert_eq!(link.relationship, "exit/south");
    assert_eq!(link.target, "main-hall");
}

#[test]
fn analyze_link_non_link_returns_none() {
    let ast = parse("(something-else foo)");
    let result = DeclarationAnalyzer::analyze_link(&ast).unwrap();
    assert!(result.is_none());
}

#[test]
fn analyze_link_wrong_arg_count_error() {
    let ast = parse("(link: player :in-room)");
    let result = DeclarationAnalyzer::analyze_link(&ast);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("requires source, relationship, and target")
    );
}

#[test]
fn analyze_link_non_symbol_source_error() {
    let ast = parse("(link: 123 :in-room cave)");
    let result = DeclarationAnalyzer::analyze_link(&ast);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("must be a symbol"));
}

#[test]
fn analyze_link_non_keyword_relationship_error() {
    let ast = parse("(link: player in-room cave)");
    let result = DeclarationAnalyzer::analyze_link(&ast);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("must be a keyword")
    );
}

#[test]
fn unified_analyze_link() {
    let ast = parse("(link: player :in-room cave)");
    let decl = DeclarationAnalyzer::analyze(&ast).unwrap().unwrap();
    assert!(matches!(decl, Declaration::Link(_)));
}
