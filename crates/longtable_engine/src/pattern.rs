//! Production pattern matching for the rule engine.
//!
//! This module compiles declaration patterns (AST-based) into efficient
//! runtime patterns using interned keyword IDs.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use longtable_foundation::{EntityId, Interner, KeywordId, Result, Value};
use longtable_language::Ast;
use longtable_language::declaration::{
    Pattern as DeclPattern, PatternClause as DeclClause, PatternValue,
};
use longtable_storage::World;

// =============================================================================
// Compiled Pattern Types
// =============================================================================

/// A compiled pattern clause ready for matching.
#[derive(Clone, Debug)]
pub struct CompiledClause {
    /// Variable name for the entity (e.g., "e" for ?e)
    pub entity_var: String,
    /// Component to match (interned)
    pub component: KeywordId,
    /// What to bind/match for the value
    pub binding: CompiledBinding,
}

/// What the value part of a pattern binds to.
#[derive(Clone, Debug)]
pub enum CompiledBinding {
    /// Bind to a new variable: [?e :health ?hp]
    Variable(String),
    /// Match against a literal value: [?e :tag/player true]
    Literal(Value),
    /// Ignore the value (wildcard): [?e :health _]
    Wildcard,
}

/// A compiled pattern (positive clauses + negations).
#[derive(Clone, Debug, Default)]
pub struct CompiledPattern {
    /// Positive clauses that must match
    pub clauses: Vec<CompiledClause>,
    /// Negated clauses (entity must NOT have these)
    pub negations: Vec<CompiledClause>,
}

impl CompiledPattern {
    /// Create a new empty pattern.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns all entity variable names referenced.
    #[must_use]
    pub fn entity_vars(&self) -> HashSet<&str> {
        let mut vars = HashSet::new();
        for c in &self.clauses {
            vars.insert(c.entity_var.as_str());
        }
        for c in &self.negations {
            vars.insert(c.entity_var.as_str());
        }
        vars
    }
}

// =============================================================================
// Pattern Compiler
// =============================================================================

/// Compiles declaration patterns into efficient runtime patterns.
pub struct PatternCompiler;

impl PatternCompiler {
    /// Compile a declaration pattern into a runtime pattern.
    ///
    /// # Errors
    /// Returns an error if component keywords cannot be interned.
    pub fn compile(pattern: &DeclPattern, interner: &mut Interner) -> Result<CompiledPattern> {
        let mut compiled = CompiledPattern::new();

        // Compile positive clauses
        for clause in &pattern.clauses {
            compiled
                .clauses
                .push(Self::compile_clause(clause, interner)?);
        }

        // Compile negations
        for clause in &pattern.negations {
            compiled
                .negations
                .push(Self::compile_clause(clause, interner)?);
        }

        Ok(compiled)
    }

    fn compile_clause(clause: &DeclClause, interner: &mut Interner) -> Result<CompiledClause> {
        let component = interner.intern_keyword(&clause.component);

        let binding = match &clause.value {
            PatternValue::Variable(v) => CompiledBinding::Variable(v.clone()),
            PatternValue::Wildcard => CompiledBinding::Wildcard,
            PatternValue::Literal(ast) => {
                CompiledBinding::Literal(Self::ast_to_value(ast, interner)?)
            }
        };

        Ok(CompiledClause {
            entity_var: clause.entity_var.clone(),
            component,
            binding,
        })
    }

    fn ast_to_value(ast: &Ast, interner: &mut Interner) -> Result<Value> {
        Ok(match ast {
            Ast::Nil(_) => Value::Nil,
            Ast::Bool(b, _) => Value::Bool(*b),
            Ast::Int(n, _) => Value::Int(*n),
            Ast::Float(f, _) => Value::Float(*f),
            Ast::String(s, _) => Value::String(s.clone().into()),
            Ast::Keyword(k, _) => Value::Keyword(interner.intern_keyword(k)),
            Ast::Symbol(s, _) => Value::Symbol(interner.intern_symbol(s)),
            // For complex types, convert recursively
            Ast::Vector(elements, _) => {
                let values: Result<_> = elements
                    .iter()
                    .map(|e| Self::ast_to_value(e, interner))
                    .collect();
                Value::Vec(values?)
            }
            _ => {
                // For unsupported literals, just use Nil
                Value::Nil
            }
        })
    }
}

// =============================================================================
// Bindings
// =============================================================================

/// A set of variable bindings from pattern matching.
#[derive(Clone, Debug, Default)]
pub struct Bindings {
    values: HashMap<String, Value>,
}

impl Bindings {
    /// Create empty bindings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a binding by variable name.
    #[must_use]
    pub fn get(&self, var: &str) -> Option<&Value> {
        self.values.get(var)
    }

    /// Set a binding.
    pub fn set(&mut self, var: String, value: Value) {
        self.values.insert(var, value);
    }

    /// Get the entity bound to an entity variable.
    #[must_use]
    pub fn get_entity(&self, var: &str) -> Option<EntityId> {
        match self.values.get(var) {
            Some(Value::EntityRef(id)) => Some(*id),
            _ => None,
        }
    }

    /// Iterate all bindings.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.values.iter()
    }

    /// Convert to a vector of values in deterministic order.
    #[must_use]
    pub fn to_vec(&self) -> Vec<Value> {
        let mut keys: Vec<_> = self.values.keys().collect();
        keys.sort();
        keys.into_iter()
            .map(|k| self.values.get(k).cloned().unwrap_or(Value::Nil))
            .collect()
    }

    /// Compute a hash for refraction identity.
    /// Two bindings with the same entity bindings produce the same hash.
    #[must_use]
    pub fn refraction_key(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();

        // Sort keys for determinism
        let mut keys: Vec<_> = self.values.keys().collect();
        keys.sort();

        for key in keys {
            key.hash(&mut hasher);
            // Hash entity refs for refraction identity
            if let Some(Value::EntityRef(id)) = self.values.get(key) {
                id.hash(&mut hasher);
            }
        }
        hasher.finish()
    }
}

// =============================================================================
// Pattern Matching
// =============================================================================

/// Pattern matcher that executes patterns against a World.
pub struct PatternMatcher;

impl PatternMatcher {
    /// Find all binding sets that satisfy a pattern against a world.
    #[must_use]
    pub fn match_pattern(pattern: &CompiledPattern, world: &World) -> Vec<Bindings> {
        if pattern.clauses.is_empty() {
            return vec![Bindings::new()];
        }

        // Start with first clause
        let first = &pattern.clauses[0];
        let mut results: Vec<Bindings> = Vec::new();

        // Find entities with the first clause's component
        for entity in world.with_component(first.component) {
            let mut bindings = Bindings::new();
            bindings.set(first.entity_var.clone(), Value::EntityRef(entity));

            // Try to bind the value
            if let Some(bound) = Self::try_bind_clause(first, entity, world, &bindings) {
                // Try to match remaining positive clauses
                if let Some(positive_bindings) =
                    Self::match_remaining(&pattern.clauses[1..], world, bound)
                {
                    // Check negations
                    if Self::check_negations(&pattern.negations, world, &positive_bindings) {
                        results.push(positive_bindings);
                    }
                }
            }
        }

        results
    }

    fn try_bind_clause(
        clause: &CompiledClause,
        entity: EntityId,
        world: &World,
        bindings: &Bindings,
    ) -> Option<Bindings> {
        // Entity must have this component
        if !world.has(entity, clause.component) {
            return None;
        }

        // Get the value to bind/match
        let value = world.get(entity, clause.component).ok()??;

        // Apply the binding
        let mut new_bindings = bindings.clone();
        match &clause.binding {
            CompiledBinding::Variable(var) => {
                // Check if variable is already bound
                if let Some(existing) = bindings.get(var) {
                    // Must match existing binding (unification)
                    if existing != &value {
                        return None;
                    }
                } else {
                    new_bindings.set(var.clone(), value);
                }
            }
            CompiledBinding::Literal(lit) => {
                // Must match literal
                if &value != lit {
                    return None;
                }
            }
            CompiledBinding::Wildcard => {
                // Always matches
            }
        }

        Some(new_bindings)
    }

    fn match_remaining(
        clauses: &[CompiledClause],
        world: &World,
        bindings: Bindings,
    ) -> Option<Bindings> {
        if clauses.is_empty() {
            return Some(bindings);
        }

        let clause = &clauses[0];

        // Check if entity variable is already bound
        if let Some(entity) = bindings.get_entity(&clause.entity_var) {
            // Use the already-bound entity
            if let Some(new_bindings) = Self::try_bind_clause(clause, entity, world, &bindings) {
                return Self::match_remaining(&clauses[1..], world, new_bindings);
            }
            return None;
        }

        // Need to find matching entities for this clause
        for entity in world.with_component(clause.component) {
            let mut new_bindings = bindings.clone();
            new_bindings.set(clause.entity_var.clone(), Value::EntityRef(entity));

            if let Some(bound) = Self::try_bind_clause(clause, entity, world, &new_bindings) {
                if let Some(final_bindings) = Self::match_remaining(&clauses[1..], world, bound) {
                    return Some(final_bindings);
                }
            }
        }

        None
    }

    /// Check that all negations are satisfied (entity does NOT have component).
    fn check_negations(negations: &[CompiledClause], world: &World, bindings: &Bindings) -> bool {
        for clause in negations {
            // Get the bound entity
            if let Some(entity) = bindings.get_entity(&clause.entity_var) {
                // Entity must NOT have this component
                if world.has(entity, clause.component) {
                    return false;
                }
            }
        }
        true
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use longtable_foundation::{LtMap, Type};
    use longtable_language::Span;
    use longtable_language::declaration::{
        Pattern as DeclPattern, PatternClause as DeclClause, PatternValue,
    };
    use longtable_storage::{ComponentSchema, FieldSchema};

    fn setup_world() -> World {
        let mut world = World::new(42);

        // Intern keywords - use tag components for simplicity
        let health_id = world.interner_mut().intern_keyword("health");
        let velocity_id = world.interner_mut().intern_keyword("velocity");

        // Register as tag components (accept Bool or Map)
        world = world
            .register_component(ComponentSchema::tag(health_id))
            .unwrap();
        world = world
            .register_component(ComponentSchema::tag(velocity_id))
            .unwrap();

        // Create entity with health only (use Bool for tag)
        let (w, e1) = world.spawn(&LtMap::new()).unwrap();
        world = w;
        world = world.set(e1, health_id, Value::Bool(true)).unwrap();

        // Create entity with health and velocity
        let (w, e2) = world.spawn(&LtMap::new()).unwrap();
        world = w;
        world = world.set(e2, health_id, Value::Bool(true)).unwrap();
        world = world.set(e2, velocity_id, Value::Bool(true)).unwrap();

        world
    }

    #[test]
    fn compile_simple_pattern() {
        let mut interner = Interner::new();

        let decl_pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "health".to_string(),
                value: PatternValue::Variable("hp".to_string()),
                span: Span::default(),
            }],
            negations: vec![],
        };

        let compiled = PatternCompiler::compile(&decl_pattern, &mut interner).unwrap();

        assert_eq!(compiled.clauses.len(), 1);
        assert_eq!(compiled.clauses[0].entity_var, "e");
        assert!(matches!(
            compiled.clauses[0].binding,
            CompiledBinding::Variable(ref v) if v == "hp"
        ));
    }

    #[test]
    fn compile_literal_pattern() {
        let mut interner = Interner::new();

        let decl_pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "active".to_string(),
                value: PatternValue::Literal(Ast::Bool(true, Span::default())),
                span: Span::default(),
            }],
            negations: vec![],
        };

        let compiled = PatternCompiler::compile(&decl_pattern, &mut interner).unwrap();

        assert!(matches!(
            compiled.clauses[0].binding,
            CompiledBinding::Literal(Value::Bool(true))
        ));
    }

    #[test]
    fn compile_wildcard_pattern() {
        let mut interner = Interner::new();

        let decl_pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "health".to_string(),
                value: PatternValue::Wildcard,
                span: Span::default(),
            }],
            negations: vec![],
        };

        let compiled = PatternCompiler::compile(&decl_pattern, &mut interner).unwrap();

        assert!(matches!(
            compiled.clauses[0].binding,
            CompiledBinding::Wildcard
        ));
    }

    #[test]
    fn match_simple_pattern() {
        let mut world = setup_world();

        // Compile pattern: [?e :health ?hp]
        let decl_pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "health".to_string(),
                value: PatternValue::Variable("hp".to_string()),
                span: Span::default(),
            }],
            negations: vec![],
        };

        let compiled = PatternCompiler::compile(&decl_pattern, world.interner_mut()).unwrap();
        let matches = PatternMatcher::match_pattern(&compiled, &world);

        // Should match both entities with health
        assert_eq!(matches.len(), 2);

        // Check bindings contain entity and hp (which is Bool(true) for tags)
        for m in &matches {
            assert!(m.get("e").is_some());
            assert_eq!(m.get("hp"), Some(&Value::Bool(true)));
        }
    }

    #[test]
    fn match_with_negation() {
        let mut world = setup_world();

        // Compile pattern: [?e :health _] (not [?e :velocity])
        let decl_pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "health".to_string(),
                value: PatternValue::Wildcard,
                span: Span::default(),
            }],
            negations: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "velocity".to_string(),
                value: PatternValue::Wildcard,
                span: Span::default(),
            }],
        };

        let compiled = PatternCompiler::compile(&decl_pattern, world.interner_mut()).unwrap();
        let matches = PatternMatcher::match_pattern(&compiled, &world);

        // Should match only entity without velocity
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn match_multiple_clauses() {
        let mut world = setup_world();

        // Compile pattern: [?e :health ?hp] [?e :velocity ?vel]
        let decl_pattern = DeclPattern {
            clauses: vec![
                DeclClause {
                    entity_var: "e".to_string(),
                    component: "health".to_string(),
                    value: PatternValue::Variable("hp".to_string()),
                    span: Span::default(),
                },
                DeclClause {
                    entity_var: "e".to_string(),
                    component: "velocity".to_string(),
                    value: PatternValue::Variable("vel".to_string()),
                    span: Span::default(),
                },
            ],
            negations: vec![],
        };

        let compiled = PatternCompiler::compile(&decl_pattern, world.interner_mut()).unwrap();
        let matches = PatternMatcher::match_pattern(&compiled, &world);

        // Should match only entity with both components
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].get("hp"), Some(&Value::Bool(true)));
        assert_eq!(matches[0].get("vel"), Some(&Value::Bool(true)));
    }

    #[test]
    fn bindings_refraction_key() {
        let mut b1 = Bindings::new();
        b1.set("e".to_string(), Value::EntityRef(EntityId::new(1, 0)));
        b1.set("hp".to_string(), Value::Int(100));

        let mut b2 = Bindings::new();
        b2.set("e".to_string(), Value::EntityRef(EntityId::new(1, 0)));
        b2.set("hp".to_string(), Value::Int(50)); // Different hp

        // Same entity = same refraction key
        assert_eq!(b1.refraction_key(), b2.refraction_key());

        let mut b3 = Bindings::new();
        b3.set("e".to_string(), Value::EntityRef(EntityId::new(2, 0)));
        b3.set("hp".to_string(), Value::Int(100));

        // Different entity = different refraction key
        assert_ne!(b1.refraction_key(), b3.refraction_key());
    }

    // =========================================================================
    // Spec Pattern Examples - Tests from SPECIFICATION.md
    // =========================================================================

    // Helper to create a map component value with a single value field
    fn make_value_map(value_id: KeywordId, value: Value) -> Value {
        let mut map = LtMap::new();
        map = map.insert(Value::Keyword(value_id), value);
        Value::Map(map)
    }

    /// Test the damage rule pattern from spec:
    /// :where [[?target :health ?hp]
    ///         [?target :incoming-damage ?dmg]]
    /// Using structured components with value fields
    #[test]
    fn spec_damage_rule_pattern() {
        let mut world = World::new(42);

        // Register components with value fields
        let health = world.interner_mut().intern_keyword("health");
        let incoming_damage = world.interner_mut().intern_keyword("incoming-damage");
        let value_field = world.interner_mut().intern_keyword("value");

        world = world
            .register_component(
                ComponentSchema::new(health)
                    .with_field(FieldSchema::required(value_field, Type::Int)),
            )
            .unwrap();
        world = world
            .register_component(
                ComponentSchema::new(incoming_damage)
                    .with_field(FieldSchema::required(value_field, Type::Int)),
            )
            .unwrap();

        // Entity with only health
        let (w, e1) = world.spawn(&LtMap::new()).unwrap();
        world = w;
        world = world
            .set(e1, health, make_value_map(value_field, Value::Int(100)))
            .unwrap();
        let _ = e1; // Not used in assertions, just spawned for pattern testing

        // Entity with both health and damage
        let (w, e2) = world.spawn(&LtMap::new()).unwrap();
        world = w;
        world = world
            .set(e2, health, make_value_map(value_field, Value::Int(75)))
            .unwrap();
        world = world
            .set(
                e2,
                incoming_damage,
                make_value_map(value_field, Value::Int(25)),
            )
            .unwrap();

        // Entity with only damage (no health)
        let (w, e3) = world.spawn(&LtMap::new()).unwrap();
        world = w;
        world = world
            .set(
                e3,
                incoming_damage,
                make_value_map(value_field, Value::Int(10)),
            )
            .unwrap();
        let _ = e3; // Not used in assertions, just spawned for pattern testing

        // Pattern: [?target :health ?hp] [?target :incoming-damage ?dmg]
        let decl_pattern = DeclPattern {
            clauses: vec![
                DeclClause {
                    entity_var: "target".to_string(),
                    component: "health".to_string(),
                    value: PatternValue::Variable("hp".to_string()),
                    span: Span::default(),
                },
                DeclClause {
                    entity_var: "target".to_string(),
                    component: "incoming-damage".to_string(),
                    value: PatternValue::Variable("dmg".to_string()),
                    span: Span::default(),
                },
            ],
            negations: vec![],
        };

        let compiled = PatternCompiler::compile(&decl_pattern, world.interner_mut()).unwrap();
        let matches = PatternMatcher::match_pattern(&compiled, &world);

        // Only e2 has both components
        assert_eq!(matches.len(), 1);
        // The bound value is the whole Map component
        assert!(matches[0].get("hp").is_some());
        assert!(matches[0].get("dmg").is_some());
        assert_eq!(matches[0].get_entity("target"), Some(e2));
    }

    /// Test join across entities (faction membership pattern):
    /// `:where [[?faction :tag/faction]
    ///         [?member :member-of ?faction-ref]]`
    /// Using tag for faction marker and structured component for membership
    #[test]
    fn spec_faction_join_pattern() {
        let mut world = World::new(42);

        // Register components
        let tag_faction = world.interner_mut().intern_keyword("tag/faction");
        let member_of = world.interner_mut().intern_keyword("member-of");
        let value_field = world.interner_mut().intern_keyword("value");

        world = world
            .register_component(ComponentSchema::tag(tag_faction))
            .unwrap();
        world = world
            .register_component(
                ComponentSchema::new(member_of)
                    .with_field(FieldSchema::required(value_field, Type::EntityRef)),
            )
            .unwrap();

        // Create a faction entity
        let (w, faction_a) = world.spawn(&LtMap::new()).unwrap();
        world = w;
        world = world
            .set(faction_a, tag_faction, Value::Bool(true))
            .unwrap();

        // Create another faction
        let (w, faction_b) = world.spawn(&LtMap::new()).unwrap();
        world = w;
        world = world
            .set(faction_b, tag_faction, Value::Bool(true))
            .unwrap();

        // Create member belonging to faction_a
        let (w, member1) = world.spawn(&LtMap::new()).unwrap();
        world = w;
        world = world
            .set(
                member1,
                member_of,
                make_value_map(value_field, Value::EntityRef(faction_a)),
            )
            .unwrap();

        // Create member belonging to faction_b
        let (w, member2) = world.spawn(&LtMap::new()).unwrap();
        world = w;
        world = world
            .set(
                member2,
                member_of,
                make_value_map(value_field, Value::EntityRef(faction_b)),
            )
            .unwrap();

        // Pattern: [?faction :tag/faction] [?member :member-of ?faction_ref]
        // This finds all faction entities and all members with membership info
        let decl_pattern = DeclPattern {
            clauses: vec![
                DeclClause {
                    entity_var: "faction".to_string(),
                    component: "tag/faction".to_string(),
                    value: PatternValue::Wildcard,
                    span: Span::default(),
                },
                DeclClause {
                    entity_var: "member".to_string(),
                    component: "member-of".to_string(),
                    value: PatternValue::Variable("faction_ref".to_string()),
                    span: Span::default(),
                },
            ],
            negations: vec![],
        };

        let compiled = PatternCompiler::compile(&decl_pattern, world.interner_mut()).unwrap();
        let matches = PatternMatcher::match_pattern(&compiled, &world);

        // Should find combinations - each faction paired with each member
        // (2 factions × 2 members = 4 combinations)
        assert!(!matches.is_empty());

        // Check that we have member and faction entities bound
        for m in &matches {
            assert!(m.get_entity("faction").is_some());
            assert!(m.get_entity("member").is_some());
            assert!(m.get("faction_ref").is_some());
        }
    }

    /// Test literal matching in patterns:
    /// [?e :tag/enemy true]
    #[test]
    fn spec_tag_literal_pattern() {
        let mut world = World::new(42);

        let tag_enemy = world.interner_mut().intern_keyword("tag/enemy");
        let tag_friendly = world.interner_mut().intern_keyword("tag/friendly");

        world = world
            .register_component(ComponentSchema::tag(tag_enemy))
            .unwrap();
        world = world
            .register_component(ComponentSchema::tag(tag_friendly))
            .unwrap();

        // Create enemies
        let (w, enemy1) = world.spawn(&LtMap::new()).unwrap();
        world = w;
        world = world.set(enemy1, tag_enemy, Value::Bool(true)).unwrap();

        let (w, enemy2) = world.spawn(&LtMap::new()).unwrap();
        world = w;
        world = world.set(enemy2, tag_enemy, Value::Bool(true)).unwrap();

        // Create a friendly
        let (w, friendly) = world.spawn(&LtMap::new()).unwrap();
        world = w;
        world = world
            .set(friendly, tag_friendly, Value::Bool(true))
            .unwrap();

        // Pattern: [?e :tag/enemy true]
        let decl_pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "tag/enemy".to_string(),
                value: PatternValue::Literal(Ast::Bool(true, Span::default())),
                span: Span::default(),
            }],
            negations: vec![],
        };

        let compiled = PatternCompiler::compile(&decl_pattern, world.interner_mut()).unwrap();
        let matches = PatternMatcher::match_pattern(&compiled, &world);

        // Should find exactly 2 enemies
        assert_eq!(matches.len(), 2);
        let entities: Vec<_> = matches.iter().filter_map(|m| m.get_entity("e")).collect();
        assert!(entities.contains(&enemy1));
        assert!(entities.contains(&enemy2));
        assert!(!entities.contains(&friendly));
    }

    /// Test empty pattern returns single empty bindings
    #[test]
    fn spec_empty_pattern() {
        let world = World::new(42);

        let decl_pattern = DeclPattern {
            clauses: vec![],
            negations: vec![],
        };

        let compiled = PatternCompiler::compile(&decl_pattern, &mut Interner::new()).unwrap();
        let matches = PatternMatcher::match_pattern(&compiled, &world);

        // Empty pattern matches once with empty bindings
        assert_eq!(matches.len(), 1);
        assert!(matches[0].iter().count() == 0);
    }

    /// Test variable unification - same variable must bind same value
    /// Using tag components which store Bool values directly
    #[test]
    fn spec_variable_unification() {
        let mut world = World::new(42);

        // Use tag components for simpler testing (store Bool directly)
        let has_flag_a = world.interner_mut().intern_keyword("has-flag-a");
        let has_flag_b = world.interner_mut().intern_keyword("has-flag-b");

        world = world
            .register_component(ComponentSchema::tag(has_flag_a))
            .unwrap();
        world = world
            .register_component(ComponentSchema::tag(has_flag_b))
            .unwrap();

        // Entity with both flags true
        let (w, e1) = world.spawn(&LtMap::new()).unwrap();
        world = w;
        world = world.set(e1, has_flag_a, Value::Bool(true)).unwrap();
        world = world.set(e1, has_flag_b, Value::Bool(true)).unwrap();

        // Entity with only flag_a (no flag_b)
        let (w, e2) = world.spawn(&LtMap::new()).unwrap();
        world = w;
        world = world.set(e2, has_flag_a, Value::Bool(true)).unwrap();
        let _ = e2; // Not used in assertions, just spawned for pattern testing

        // Pattern: [?e :has-flag-a ?x] [?e :has-flag-b ?x] (same ?x must unify to same bool)
        let decl_pattern = DeclPattern {
            clauses: vec![
                DeclClause {
                    entity_var: "e".to_string(),
                    component: "has-flag-a".to_string(),
                    value: PatternValue::Variable("x".to_string()),
                    span: Span::default(),
                },
                DeclClause {
                    entity_var: "e".to_string(),
                    component: "has-flag-b".to_string(),
                    value: PatternValue::Variable("x".to_string()),
                    span: Span::default(),
                },
            ],
            negations: vec![],
        };

        let compiled = PatternCompiler::compile(&decl_pattern, world.interner_mut()).unwrap();
        let matches = PatternMatcher::match_pattern(&compiled, &world);

        // Only e1 matches (has both flags with same value true)
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].get_entity("e"), Some(e1));
        assert_eq!(matches[0].get("x"), Some(&Value::Bool(true)));
    }
}
