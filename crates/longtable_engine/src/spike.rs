//! Semantic spike for rule engine validation.
//!
//! This module implements a minimal end-to-end rule execution to validate
//! core semantics before committing to full implementation.
//!
//! **Intentionally ugly.** This is a spike, not production code.
//! - Uses simplified pattern representation
//! - No optimization
//! - Can be thrown away once semantics are validated

// Spike code - allow pedantic warnings for clarity over polish
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::used_underscore_binding)]
#![allow(clippy::needless_pass_by_value)]

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use longtable_foundation::{
    EntityId, Error, ErrorKind, KeywordId, LtSet, LtVec, Result, SemanticLimit, Type, Value,
};
use longtable_language::VmEffect;
use longtable_storage::World;

// =============================================================================
// Pattern Types
// =============================================================================

/// A pattern clause: [?entity :component/field ?value] or [?e :component]
#[derive(Clone, Debug)]
pub struct PatternClause {
    /// Variable name for the entity (e.g., "e" for ?e)
    pub entity_var: String,
    /// Component to match
    pub component: KeywordId,
    /// Optional field within component
    pub field: Option<KeywordId>,
    /// What to bind/match for the value
    pub binding: PatternBinding,
    /// Whether this is a negated pattern (not [?e :component])
    pub negated: bool,
}

/// What the value part of a pattern binds to.
#[derive(Clone, Debug)]
pub enum PatternBinding {
    /// Bind to a new variable: [?e :health/current ?hp]
    Variable(String),
    /// Match against a literal value: [?e :tag/player true]
    Literal(Value),
    /// Ignore the value (wildcard): [?e :health _]
    Wildcard,
}

/// A complete pattern (conjunction of clauses).
#[derive(Clone, Debug, Default)]
pub struct Pattern {
    /// Pattern clauses to match
    pub clauses: Vec<PatternClause>,
}

impl Pattern {
    /// Create a new empty pattern.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a clause to match entity having component with field bound to variable.
    #[must_use]
    pub fn with_clause(
        mut self,
        entity_var: &str,
        component: KeywordId,
        field: Option<KeywordId>,
        binding: PatternBinding,
    ) -> Self {
        self.clauses.push(PatternClause {
            entity_var: entity_var.to_string(),
            component,
            field,
            binding,
            negated: false,
        });
        self
    }

    /// Add a negated clause (entity must NOT have this component).
    #[must_use]
    pub fn with_negated(mut self, entity_var: &str, component: KeywordId) -> Self {
        self.clauses.push(PatternClause {
            entity_var: entity_var.to_string(),
            component,
            field: None,
            binding: PatternBinding::Wildcard,
            negated: true,
        });
        self
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

/// Find all binding sets that satisfy a pattern against a world.
pub fn match_pattern(pattern: &Pattern, world: &World) -> Vec<Bindings> {
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
        if let Some(bound) = try_bind_clause(first, entity, world, &bindings) {
            // Try to match remaining clauses
            if let Some(final_bindings) = match_remaining(&pattern.clauses[1..], world, bound) {
                results.push(final_bindings);
            }
        }
    }

    results
}

fn try_bind_clause(
    clause: &PatternClause,
    entity: EntityId,
    world: &World,
    bindings: &Bindings,
) -> Option<Bindings> {
    // Handle negated patterns
    if clause.negated {
        // Entity must NOT have this component
        if world.has(entity, clause.component) {
            return None;
        }
        return Some(bindings.clone());
    }

    // Entity must have this component
    if !world.has(entity, clause.component) {
        return None;
    }

    // Get the value to bind/match
    let value = if let Some(field) = clause.field {
        world.get_field(entity, clause.component, field).ok()??
    } else {
        world.get(entity, clause.component).ok()??
    };

    // Apply the binding
    let mut new_bindings = bindings.clone();
    match &clause.binding {
        PatternBinding::Variable(var) => {
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
        PatternBinding::Literal(lit) => {
            // Must match literal
            if &value != lit {
                return None;
            }
        }
        PatternBinding::Wildcard => {
            // Always matches
        }
    }

    Some(new_bindings)
}

fn match_remaining(
    clauses: &[PatternClause],
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
        if let Some(new_bindings) = try_bind_clause(clause, entity, world, &bindings) {
            return match_remaining(&clauses[1..], world, new_bindings);
        }
        return None;
    }

    // Need to find matching entities for this clause
    for entity in world.with_component(clause.component) {
        let mut new_bindings = bindings.clone();
        new_bindings.set(clause.entity_var.clone(), Value::EntityRef(entity));

        if let Some(bound) = try_bind_clause(clause, entity, world, &new_bindings) {
            if let Some(final_bindings) = match_remaining(&clauses[1..], world, bound) {
                return Some(final_bindings);
            }
        }
    }

    None
}

// =============================================================================
// Rule and Activation
// =============================================================================

/// A spike rule (simplified for semantic validation).
#[derive(Clone)]
pub struct SpikeRule {
    /// Rule name/id
    pub name: KeywordId,
    /// Priority (higher fires first)
    pub salience: i32,
    /// Pattern to match
    pub pattern: Pattern,
    /// Rule body as source code (we'll eval it with bindings substituted)
    /// For the spike, we use simple DSL expressions.
    pub body: String,
    /// Fire only once per tick
    pub once: bool,
}

/// A rule activation ready to fire.
#[derive(Clone, Debug)]
pub struct Activation {
    /// Which rule
    pub rule_name: KeywordId,
    /// Variable bindings
    pub bindings: Bindings,
    /// Rule salience
    pub salience: i32,
    /// Pattern specificity (number of clauses)
    pub specificity: usize,
}

impl Activation {
    /// Compute refraction key (rule + entity bindings).
    #[must_use]
    pub fn refraction_key(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.rule_name.hash(&mut hasher);
        self.bindings.refraction_key().hash(&mut hasher);
        hasher.finish()
    }
}

// =============================================================================
// Rule Engine
// =============================================================================

/// Manages rule execution within a tick.
pub struct RuleEngine {
    /// Refracted activations (already fired this tick)
    refracted: HashSet<u64>,
    /// Rules that fired with :once flag
    once_fired: HashSet<KeywordId>,
    /// Effect log for this tick
    effects: Vec<EffectRecord>,
    /// Number of activations this tick (for kill switch)
    activation_count: usize,
    /// Maximum activations per tick
    max_activations: usize,
}

/// Record of an effect for debugging.
#[derive(Clone, Debug)]
pub struct EffectRecord {
    /// Which rule caused this
    pub rule: KeywordId,
    /// The effect
    pub effect: VmEffect,
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleEngine {
    /// Create a new rule engine.
    #[must_use]
    pub fn new() -> Self {
        Self {
            refracted: HashSet::new(),
            once_fired: HashSet::new(),
            effects: Vec::new(),
            activation_count: 0,
            max_activations: 10_000, // Kill switch
        }
    }

    /// Set the maximum activations before the kill switch triggers.
    #[must_use]
    pub fn with_max_activations(mut self, limit: usize) -> Self {
        self.max_activations = limit;
        self
    }

    /// Reset for a new tick.
    pub fn begin_tick(&mut self) {
        self.refracted.clear();
        self.once_fired.clear();
        self.effects.clear();
        self.activation_count = 0;
    }

    /// Find all current activations, respecting refraction.
    pub fn find_activations(&self, rules: &[SpikeRule], world: &World) -> Vec<Activation> {
        let mut activations = Vec::new();

        for rule in rules {
            // Skip :once rules that already fired
            if rule.once && self.once_fired.contains(&rule.name) {
                continue;
            }

            // Find pattern matches
            let matches = match_pattern(&rule.pattern, world);

            for bindings in matches {
                let activation = Activation {
                    rule_name: rule.name,
                    bindings,
                    salience: rule.salience,
                    specificity: rule.pattern.clauses.len(),
                };

                // Skip if refracted
                if self.refracted.contains(&activation.refraction_key()) {
                    continue;
                }

                activations.push(activation);
            }
        }

        // Sort by salience (descending), then specificity (descending)
        activations.sort_by(|a, b| {
            b.salience
                .cmp(&a.salience)
                .then_with(|| b.specificity.cmp(&a.specificity))
        });

        activations
    }

    /// Execute until quiescence.
    pub fn run_to_quiescence(
        &mut self,
        rules: &[SpikeRule],
        mut world: World,
    ) -> Result<(World, Vec<EffectRecord>)> {
        loop {
            let activations = self.find_activations(rules, &world);

            if activations.is_empty() {
                // Quiescence reached
                break;
            }

            // Fire the highest-priority activation
            let activation = &activations[0];

            // Kill switch
            self.activation_count += 1;
            if self.activation_count > self.max_activations {
                return Err(Error::limit_exceeded(SemanticLimit::MaxActivations {
                    limit: self.max_activations as u32,
                    context: Some(format!("rule {:?}", activation.rule_name)),
                }));
            }

            // Find the rule
            let rule = rules
                .iter()
                .find(|r| r.name == activation.rule_name)
                .expect("Rule not found");

            // Fire the rule
            world = self.fire_rule(rule, activation, world)?;
        }

        Ok((world, std::mem::take(&mut self.effects)))
    }

    /// Fire a single rule.
    fn fire_rule(
        &mut self,
        rule: &SpikeRule,
        activation: &Activation,
        world: World,
    ) -> Result<World> {
        // Mark as refracted
        self.refracted.insert(activation.refraction_key());

        // Mark :once rules
        if rule.once {
            self.once_fired.insert(rule.name);
        }

        // For the spike, we execute simple effects directly
        // The rule body is a simplified expression that we interpret
        let new_world = execute_spike_body(rule, activation, world)?;

        Ok(new_world)
    }
}

/// Execute a spike rule body (simplified interpreter for testing).
///
/// For the spike, rule bodies are simple effect descriptors, not full DSL.
/// We support:
/// - "set ?e :component/field value" - set a field
/// - "increment ?e :counter/value" - increment a counter
/// - "noop" - do nothing
fn execute_spike_body(
    rule: &SpikeRule,
    activation: &Activation,
    mut world: World,
) -> Result<World> {
    let body = rule.body.trim();

    if body == "noop" || body.is_empty() {
        return Ok(world);
    }

    // Parse simple "increment ?var :component/field" command
    if body.starts_with("increment ") {
        let parts: Vec<&str> = body.split_whitespace().collect();
        if parts.len() >= 3 {
            let var = parts[1].trim_start_matches('?');
            let comp_field = parts[2];

            if let Some(entity) = activation.bindings.get_entity(var) {
                // Parse component/field
                if let Some((comp_str, field_str)) = comp_field.split_once('/') {
                    let comp = world.interner_mut().intern_keyword(comp_str);
                    let field = world.interner_mut().intern_keyword(field_str);

                    // Get current value
                    if let Some(Value::Int(current)) = world.get_field(entity, comp, field)? {
                        world = world.set_field(entity, comp, field, Value::Int(current + 1))?;
                    }
                }
            }
        }
        return Ok(world);
    }

    // Parse "set ?var :component/field value" command
    if body.starts_with("set ") {
        let parts: Vec<&str> = body.split_whitespace().collect();
        if parts.len() >= 4 {
            let var = parts[1].trim_start_matches('?');
            let comp_field = parts[2];
            let value_str = parts[3];

            if let Some(entity) = activation.bindings.get_entity(var) {
                if let Some((comp_str, field_str)) = comp_field.split_once('/') {
                    let comp = world.interner_mut().intern_keyword(comp_str);
                    let field = world.interner_mut().intern_keyword(field_str);

                    // Parse value (simple int/bool for spike)
                    let value = if let Ok(n) = value_str.parse::<i64>() {
                        Value::Int(n)
                    } else if value_str == "true" {
                        Value::Bool(true)
                    } else if value_str == "false" {
                        Value::Bool(false)
                    } else {
                        Value::Nil
                    };

                    world = world.set_field(entity, comp, field, value)?;
                }
            }
        }
        return Ok(world);
    }

    // Parse "tag ?var :component" command (add tag component)
    if body.starts_with("tag ") {
        let parts: Vec<&str> = body.split_whitespace().collect();
        if parts.len() >= 3 {
            let var = parts[1].trim_start_matches('?');
            let comp_str = parts[2].trim_start_matches(':');

            if let Some(entity) = activation.bindings.get_entity(var) {
                let comp = world.interner_mut().intern_keyword(comp_str);
                world = world.set(entity, comp, Value::Bool(true))?;
            }
        }
        return Ok(world);
    }

    Ok(world)
}

// =============================================================================
// Apply VmEffect to World
// =============================================================================

/// Apply an effect to the world.
#[allow(dead_code, clippy::too_many_lines)]
pub fn apply_effect(world: World, effect: &VmEffect) -> Result<World> {
    match effect {
        VmEffect::Spawn { components } => {
            let (new_world, _id) = world.spawn(components)?;
            Ok(new_world)
        }
        VmEffect::Destroy { entity } => world.destroy(*entity),
        VmEffect::SetComponent {
            entity,
            component,
            value,
        } => world.set(*entity, *component, value.clone()),
        VmEffect::SetField {
            entity,
            component,
            field,
            value,
        } => world.set_field(*entity, *component, *field, value.clone()),
        VmEffect::Link {
            source,
            relationship,
            target,
        } => world.link(*source, *relationship, *target),
        VmEffect::Unlink {
            source,
            relationship,
            target,
        } => world.unlink(*source, *relationship, *target),
        VmEffect::Retract { entity, component } => world.retract(*entity, *component),
        VmEffect::VecRemove {
            entity,
            component,
            field,
            value,
        } => {
            // Get current field value and remove the value from the vector
            let current = world.get_field(*entity, *component, *field)?;
            let elements: LtVec<Value> = match current {
                Some(Value::Vec(v)) => v,
                Some(Value::Nil) | None => LtVec::new(),
                Some(other) => {
                    return Err(Error::new(ErrorKind::TypeMismatch {
                        expected: Type::vec(Type::Any),
                        actual: other.value_type(),
                    }));
                }
            };
            // Convert to Vec, remove, and convert back
            let mut vec_elements: Vec<Value> = elements.iter().cloned().collect();
            if let Some(pos) = vec_elements.iter().position(|e| e == value) {
                vec_elements.remove(pos);
            }
            let new_vec = Value::Vec(vec_elements.into_iter().collect());
            world.set_field(*entity, *component, *field, new_vec)
        }
        VmEffect::VecAdd {
            entity,
            component,
            field,
            value,
        } => {
            // Get current field value and add the value to the vector
            let current = world.get_field(*entity, *component, *field)?;
            let elements: LtVec<Value> = match current {
                Some(Value::Vec(v)) => v,
                Some(Value::Nil) | None => LtVec::new(),
                Some(other) => {
                    return Err(Error::new(ErrorKind::TypeMismatch {
                        expected: Type::vec(Type::Any),
                        actual: other.value_type(),
                    }));
                }
            };
            let new_vec = Value::Vec(elements.push_back(value.clone()));
            world.set_field(*entity, *component, *field, new_vec)
        }
        VmEffect::SetRemove {
            entity,
            component,
            field,
            value,
        } => {
            // Get current field value and remove the value from the set
            let current = world.get_field(*entity, *component, *field)?;
            let elements: LtSet<Value> = match current {
                Some(Value::Set(s)) => s,
                Some(Value::Nil) | None => LtSet::new(),
                Some(other) => {
                    return Err(Error::new(ErrorKind::TypeMismatch {
                        expected: Type::set(Type::Any),
                        actual: other.value_type(),
                    }));
                }
            };
            let new_set = Value::Set(elements.remove(value));
            world.set_field(*entity, *component, *field, new_set)
        }
        VmEffect::SetAdd {
            entity,
            component,
            field,
            value,
        } => {
            // Get current field value and add the value to the set
            let current = world.get_field(*entity, *component, *field)?;
            let elements: LtSet<Value> = match current {
                Some(Value::Set(s)) => s,
                Some(Value::Nil) | None => LtSet::new(),
                Some(other) => {
                    return Err(Error::new(ErrorKind::TypeMismatch {
                        expected: Type::set(Type::Any),
                        actual: other.value_type(),
                    }));
                }
            };
            let new_set = Value::Set(elements.insert(value.clone()));
            world.set_field(*entity, *component, *field, new_set)
        }
        // State management effects are handled at the REPL level, not here.
        // This function only handles effects that modify the World directly.
        VmEffect::SaveState { .. } | VmEffect::RestoreState { .. } => Ok(world),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use longtable_foundation::{ErrorKind, LtMap};
    use longtable_storage::{ComponentSchema, FieldSchema};

    fn setup_world_with_counters() -> (World, KeywordId, KeywordId, EntityId, EntityId) {
        let mut world = World::new(42);

        // Intern keywords
        let counter = world.interner_mut().intern_keyword("counter");
        let value = world.interner_mut().intern_keyword("value");

        // Register schema
        let schema = ComponentSchema::new(counter).with_field(FieldSchema::required(
            value,
            longtable_foundation::Type::Int,
        ));
        world = world.register_component(schema).unwrap();

        // Spawn two entities with counters
        let (world, e1) = world.spawn(&LtMap::new()).unwrap();
        let (world, e2) = world.spawn(&LtMap::new()).unwrap();

        let mut comp1 = LtMap::new();
        comp1 = comp1.insert(Value::Keyword(value), Value::Int(5));
        let world = world.set(e1, counter, Value::Map(comp1)).unwrap();

        let mut comp2 = LtMap::new();
        comp2 = comp2.insert(Value::Keyword(value), Value::Int(10));
        let world = world.set(e2, counter, Value::Map(comp2)).unwrap();

        (world, counter, value, e1, e2)
    }

    #[test]
    fn pattern_matches_entities() {
        let (world, counter, value, e1, e2) = setup_world_with_counters();

        // Pattern: [?e :counter/value ?v]
        let pattern = Pattern::new().with_clause(
            "e",
            counter,
            Some(value),
            PatternBinding::Variable("v".to_string()),
        );

        let matches = match_pattern(&pattern, &world);

        assert_eq!(matches.len(), 2);

        // Check that we got both entities
        let entities: HashSet<EntityId> =
            matches.iter().filter_map(|b| b.get_entity("e")).collect();
        assert!(entities.contains(&e1));
        assert!(entities.contains(&e2));
    }

    #[test]
    fn pattern_binds_values() {
        let (world, counter, value, e1, _e2) = setup_world_with_counters();

        let pattern = Pattern::new().with_clause(
            "e",
            counter,
            Some(value),
            PatternBinding::Variable("v".to_string()),
        );

        let matches = match_pattern(&pattern, &world);

        // Find the binding for e1
        let e1_binding = matches
            .iter()
            .find(|b| b.get_entity("e") == Some(e1))
            .unwrap();
        assert_eq!(e1_binding.get("v"), Some(&Value::Int(5)));
    }

    #[test]
    fn literal_pattern_filters() {
        let (world, counter, value, e1, _e2) = setup_world_with_counters();

        // Pattern: [?e :counter/value 5] - only matches e1
        let pattern = Pattern::new().with_clause(
            "e",
            counter,
            Some(value),
            PatternBinding::Literal(Value::Int(5)),
        );

        let matches = match_pattern(&pattern, &world);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].get_entity("e"), Some(e1));
    }

    #[test]
    fn negated_pattern_excludes() {
        let (mut world, counter, value, e1, e2) = setup_world_with_counters();

        // Add a "done" tag to e1
        let done = world.interner_mut().intern_keyword("done");
        world = world
            .register_component(ComponentSchema::tag(done))
            .unwrap();
        world = world.set(e1, done, Value::Bool(true)).unwrap();

        // Pattern: [?e :counter/value ?v] (not [?e :done])
        let pattern = Pattern::new()
            .with_clause(
                "e",
                counter,
                Some(value),
                PatternBinding::Variable("v".to_string()),
            )
            .with_negated("e", done);

        let matches = match_pattern(&pattern, &world);

        // Only e2 should match (e1 has :done)
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].get_entity("e"), Some(e2));
    }

    #[test]
    fn refraction_prevents_refire() {
        let (world, counter, value, _e1, _e2) = setup_world_with_counters();

        let pattern = Pattern::new().with_clause(
            "e",
            counter,
            Some(value),
            PatternBinding::Variable("v".to_string()),
        );

        // Create a rule (body is noop for this test)
        let mut world = world;
        let rule = SpikeRule {
            name: world.interner_mut().intern_keyword("test-rule"),
            salience: 0,
            pattern,
            body: "noop".to_string(),
            once: false,
        };

        let mut engine = RuleEngine::new();
        engine.begin_tick();

        // First find should return 2 activations
        let activations = engine.find_activations(&[rule.clone()], &world);
        assert_eq!(activations.len(), 2);

        // Mark first activation as refracted
        engine.refracted.insert(activations[0].refraction_key());

        // Second find should return only 1 activation
        let activations = engine.find_activations(&[rule], &world);
        assert_eq!(activations.len(), 1);
    }

    #[test]
    fn bindings_refraction_key_is_stable() {
        let (_world, _counter, _value, e1, _e2) = setup_world_with_counters();

        let mut b1 = Bindings::new();
        b1.set("e".to_string(), Value::EntityRef(e1));
        b1.set("v".to_string(), Value::Int(5));

        let mut b2 = Bindings::new();
        b2.set("e".to_string(), Value::EntityRef(e1));
        b2.set("v".to_string(), Value::Int(999)); // Different value

        // Same entity binding should give same refraction key
        // (value bindings don't affect refraction - only entity identity matters)
        assert_eq!(b1.refraction_key(), b2.refraction_key());
    }

    #[test]
    fn activation_sorting() {
        let (mut world, counter, value, _e1, _e2) = setup_world_with_counters();

        let low_salience = SpikeRule {
            name: world.interner_mut().intern_keyword("low"),
            salience: 0,
            pattern: Pattern::new().with_clause(
                "e",
                counter,
                Some(value),
                PatternBinding::Wildcard,
            ),
            body: "noop".to_string(),
            once: false,
        };

        let high_salience = SpikeRule {
            name: world.interner_mut().intern_keyword("high"),
            salience: 100,
            pattern: Pattern::new().with_clause(
                "e",
                counter,
                Some(value),
                PatternBinding::Wildcard,
            ),
            body: "noop".to_string(),
            once: false,
        };

        let high_keyword = world.interner_mut().intern_keyword("high");

        let mut engine = RuleEngine::new();
        engine.begin_tick();

        let activations = engine.find_activations(&[low_salience, high_salience], &world);

        // High salience should come first
        assert_eq!(activations[0].rule_name, high_keyword);
    }

    #[test]
    fn spike_rule_fires_once_per_match() {
        let (mut world, counter, value, _e1, _e2) = setup_world_with_counters();

        // Rule: when entity has :counter, increment it
        let pattern = Pattern::new().with_clause(
            "e",
            counter,
            Some(value),
            PatternBinding::Variable("v".to_string()),
        );

        let rule = SpikeRule {
            name: world.interner_mut().intern_keyword("increment"),
            salience: 0,
            pattern,
            body: "increment ?e counter/value".to_string(),
            once: false,
        };

        let mut engine = RuleEngine::new();
        engine.begin_tick();

        let (final_world, _effects) = engine.run_to_quiescence(&[rule], world).unwrap();

        // Both counters should have been incremented exactly once
        // e1: 5 -> 6, e2: 10 -> 11
        // Refraction prevents re-firing on the same entity

        // Check by querying the final values
        let mut found_6 = false;
        let mut found_11 = false;
        for entity in final_world.with_component(counter) {
            if let Some(Value::Int(v)) = final_world.get_field(entity, counter, value).unwrap() {
                if v == 6 {
                    found_6 = true;
                }
                if v == 11 {
                    found_11 = true;
                }
            }
        }
        assert!(found_6, "e1 should have been incremented to 6");
        assert!(found_11, "e2 should have been incremented to 11");
    }

    #[test]
    fn changes_visible_to_later_rules() {
        let (mut world, counter, value, e1, _e2) = setup_world_with_counters();

        // Add a "flagged" component schema
        let flagged = world.interner_mut().intern_keyword("flagged");
        world = world
            .register_component(ComponentSchema::tag(flagged))
            .unwrap();

        // Rule A: if counter/value == 5, set flagged
        let pattern_a = Pattern::new().with_clause(
            "e",
            counter,
            Some(value),
            PatternBinding::Literal(Value::Int(5)),
        );
        let rule_a = SpikeRule {
            name: world.interner_mut().intern_keyword("flag-five"),
            salience: 100, // Higher priority
            pattern: pattern_a,
            body: "tag ?e flagged".to_string(),
            once: false,
        };

        // Rule B: if entity is flagged, increment counter
        let pattern_b = Pattern::new().with_clause("e", flagged, None, PatternBinding::Wildcard);
        let rule_b = SpikeRule {
            name: world.interner_mut().intern_keyword("increment-flagged"),
            salience: 0,
            pattern: pattern_b,
            body: "increment ?e counter/value".to_string(),
            once: false,
        };

        let mut engine = RuleEngine::new();
        engine.begin_tick();

        let (final_world, _effects) = engine.run_to_quiescence(&[rule_a, rule_b], world).unwrap();

        // e1 should have been flagged by rule_a, then incremented by rule_b
        // So counter/value should be 6
        let final_value = final_world.get_field(e1, counter, value).unwrap();
        assert_eq!(final_value, Some(Value::Int(6)));

        // And e1 should be flagged
        assert!(final_world.has(e1, flagged));
    }

    #[test]
    fn refraction_uses_binding_identity() {
        let (mut world, counter, value, e1, _e2) = setup_world_with_counters();

        // Add done component
        let done = world.interner_mut().intern_keyword("done");
        world = world
            .register_component(ComponentSchema::tag(done))
            .unwrap();

        // Rule: [?e :counter/value ?v] -> tag ?e done
        // After firing, e1 still matches the pattern (counter still exists)
        // but refraction should prevent re-firing because ?e is the same
        let pattern = Pattern::new().with_clause(
            "e",
            counter,
            Some(value),
            PatternBinding::Variable("v".to_string()),
        );

        let rule = SpikeRule {
            name: world.interner_mut().intern_keyword("mark-done"),
            salience: 0,
            pattern,
            body: "tag ?e done".to_string(),
            once: false,
        };

        let mut engine = RuleEngine::new();
        engine.begin_tick();

        let (final_world, _) = engine.run_to_quiescence(&[rule], world).unwrap();

        // Both entities should be tagged (rule fires once per entity)
        assert!(final_world.has(e1, done));

        // Engine should have refracted 2 activations (one per entity)
        assert_eq!(engine.refracted.len(), 2);
    }

    #[test]
    fn kill_switch_triggers() {
        let (mut world, counter, value, _e1, _e2) = setup_world_with_counters();

        // A badly written rule that would loop forever without refraction
        // But we'll make it NOT use entity var in body so it doesn't get refracted properly
        // Actually, let's make it so the pattern keeps matching

        // This is hard to test without infinite loop, so let's just test
        // that the kill switch is armed by setting max_activations very low
        let pattern = Pattern::new().with_clause(
            "e",
            counter,
            Some(value),
            PatternBinding::Variable("v".to_string()),
        );

        let rule = SpikeRule {
            name: world.interner_mut().intern_keyword("loop"),
            salience: 0,
            pattern,
            body: "noop".to_string(), // Doesn't matter
            once: false,
        };

        let mut engine = RuleEngine::new();
        engine.max_activations = 1; // Very low limit
        engine.begin_tick();

        let result = engine.run_to_quiescence(&[rule], world);

        // Should fail with limit exceeded (we have 2 entities matching,
        // but limit is 1)
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(matches!(e.kind, ErrorKind::LimitExceeded(_)));
        }
    }

    #[test]
    fn once_flag_prevents_refire() {
        let (mut world, counter, value, _e1, _e2) = setup_world_with_counters();

        let pattern = Pattern::new().with_clause(
            "e",
            counter,
            Some(value),
            PatternBinding::Variable("v".to_string()),
        );

        // Rule with :once flag
        let rule = SpikeRule {
            name: world.interner_mut().intern_keyword("once-rule"),
            salience: 0,
            pattern,
            body: "increment ?e counter/value".to_string(),
            once: true, // Only fire once per tick, regardless of matches
        };

        let mut engine = RuleEngine::new();
        engine.begin_tick();

        let (final_world, _) = engine.run_to_quiescence(&[rule], world).unwrap();

        // With :once, only one entity should be incremented
        let mut incremented_count = 0;
        for entity in final_world.with_component(counter) {
            if let Some(Value::Int(v)) = final_world.get_field(entity, counter, value).unwrap() {
                if v == 6 || v == 11 {
                    incremented_count += 1;
                }
            }
        }
        assert_eq!(
            incremented_count, 1,
            ":once should only fire for first match"
        );
    }
}
