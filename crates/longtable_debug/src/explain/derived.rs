//! Explanation for derived component values.
//!
//! Derived components are computed values that depend on other components.
//! This module provides types for explaining how a derived value was computed.
//!
//! # Example
//!
//! ```text
//! (why player :derived/health-percent)
//! ;; => {:dependencies [{:entity Entity(1), :component :health, :value 75}
//! ;;                    {:entity Entity(1), :component :max-health, :value 100}]
//! ;;     :result 75
//! ;;     :pattern "[?self :health ?hp] [?self :max-health ?max]"
//! ;;     :bindings {?self Entity(1), ?hp 75, ?max 100}}
//! ```

use longtable_foundation::{EntityId, KeywordId, Value};

// =============================================================================
// Derived Dependency
// =============================================================================

/// A component value that was read during derived evaluation.
#[derive(Clone, Debug)]
pub struct DerivedDependency {
    /// Entity that was read.
    pub entity: EntityId,

    /// Component that was read.
    pub component: KeywordId,

    /// Value that was read.
    pub value: Value,
}

impl DerivedDependency {
    /// Creates a new dependency record.
    #[must_use]
    pub fn new(entity: EntityId, component: KeywordId, value: Value) -> Self {
        Self {
            entity,
            component,
            value,
        }
    }
}

// =============================================================================
// Derived Explanation
// =============================================================================

/// Explanation of how a derived component value was computed.
#[derive(Clone, Debug)]
pub struct DerivedExplanation {
    /// The derived component that was computed.
    pub derived: KeywordId,

    /// The entity for which it was computed.
    pub entity: EntityId,

    /// The computed result value.
    pub result: Value,

    /// All dependencies (component values) that were read during computation.
    pub dependencies: Vec<DerivedDependency>,

    /// Human-readable representation of the matched pattern.
    pub matched_pattern: Option<String>,

    /// Variable bindings from pattern matching.
    pub pattern_bindings: Vec<(String, Value)>,

    /// Whether the result was served from cache.
    pub from_cache: bool,

    /// Cache version when computed (for debugging).
    pub cache_version: Option<u64>,
}

impl DerivedExplanation {
    /// Creates a new derived explanation.
    #[must_use]
    pub fn new(derived: KeywordId, entity: EntityId, result: Value) -> Self {
        Self {
            derived,
            entity,
            result,
            dependencies: Vec::new(),
            matched_pattern: None,
            pattern_bindings: Vec::new(),
            from_cache: false,
            cache_version: None,
        }
    }

    /// Builder method to add dependencies.
    #[must_use]
    pub fn with_dependencies(mut self, deps: Vec<DerivedDependency>) -> Self {
        self.dependencies = deps;
        self
    }

    /// Builder method to add a single dependency.
    #[must_use]
    pub fn with_dependency(mut self, dep: DerivedDependency) -> Self {
        self.dependencies.push(dep);
        self
    }

    /// Builder method to set the matched pattern.
    #[must_use]
    pub fn with_matched_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.matched_pattern = Some(pattern.into());
        self
    }

    /// Builder method to set pattern bindings.
    #[must_use]
    pub fn with_bindings(mut self, bindings: Vec<(String, Value)>) -> Self {
        self.pattern_bindings = bindings;
        self
    }

    /// Builder method to mark as from cache.
    #[must_use]
    pub fn from_cache(mut self, version: u64) -> Self {
        self.from_cache = true;
        self.cache_version = Some(version);
        self
    }

    /// Returns the number of dependencies.
    #[must_use]
    pub fn dependency_count(&self) -> usize {
        self.dependencies.len()
    }

    /// Returns true if the value came from cache.
    #[must_use]
    pub fn was_cached(&self) -> bool {
        self.from_cache
    }

    /// Returns all unique entities that were read.
    #[must_use]
    pub fn entities_read(&self) -> Vec<EntityId> {
        use std::collections::HashSet;
        let entities: HashSet<_> = self.dependencies.iter().map(|d| d.entity).collect();
        entities.into_iter().collect()
    }

    /// Returns all unique components that were read.
    #[must_use]
    pub fn components_read(&self) -> Vec<KeywordId> {
        use std::collections::HashSet;
        let components: HashSet<_> = self.dependencies.iter().map(|d| d.component).collect();
        components.into_iter().collect()
    }
}

// =============================================================================
// Derived Explanation Builder
// =============================================================================

/// Builder for constructing derived explanations during evaluation.
///
/// This is used during derived component evaluation to track
/// all reads and build the explanation incrementally.
#[derive(Clone, Debug, Default)]
pub struct DerivedExplanationBuilder {
    dependencies: Vec<DerivedDependency>,
    pattern_bindings: Vec<(String, Value)>,
    matched_pattern: Option<String>,
}

impl DerivedExplanationBuilder {
    /// Creates a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a component read.
    pub fn record_read(&mut self, entity: EntityId, component: KeywordId, value: Value) {
        self.dependencies
            .push(DerivedDependency::new(entity, component, value));
    }

    /// Records pattern bindings.
    pub fn record_bindings(&mut self, bindings: Vec<(String, Value)>) {
        self.pattern_bindings = bindings;
    }

    /// Records the matched pattern.
    pub fn record_pattern(&mut self, pattern: impl Into<String>) {
        self.matched_pattern = Some(pattern.into());
    }

    /// Builds the final explanation.
    #[must_use]
    pub fn build(self, derived: KeywordId, entity: EntityId, result: Value) -> DerivedExplanation {
        DerivedExplanation {
            derived,
            entity,
            result,
            dependencies: self.dependencies,
            matched_pattern: self.matched_pattern,
            pattern_bindings: self.pattern_bindings,
            from_cache: false,
            cache_version: None,
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use longtable_foundation::Interner;

    fn setup() -> (Interner, KeywordId, KeywordId, KeywordId) {
        let mut interner = Interner::new();
        let health = interner.intern_keyword("health");
        let max_health = interner.intern_keyword("max-health");
        let health_percent = interner.intern_keyword("health-percent");
        (interner, health, max_health, health_percent)
    }

    #[test]
    fn basic_explanation() {
        let (_interner, health, max_health, health_percent) = setup();

        let entity = EntityId::new(1, 0);
        let explanation = DerivedExplanation::new(health_percent, entity, Value::Int(75))
            .with_dependency(DerivedDependency::new(entity, health, Value::Int(75)))
            .with_dependency(DerivedDependency::new(entity, max_health, Value::Int(100)))
            .with_matched_pattern("[?self :health ?hp] [?self :max-health ?max]")
            .with_bindings(vec![
                ("?self".to_string(), Value::EntityRef(entity)),
                ("?hp".to_string(), Value::Int(75)),
                ("?max".to_string(), Value::Int(100)),
            ]);

        assert_eq!(explanation.dependency_count(), 2);
        assert!(!explanation.was_cached());
        assert_eq!(explanation.result, Value::Int(75));
    }

    #[test]
    fn cached_explanation() {
        let (_interner, _health, _max_health, health_percent) = setup();

        let entity = EntityId::new(1, 0);
        let explanation =
            DerivedExplanation::new(health_percent, entity, Value::Int(75)).from_cache(42);

        assert!(explanation.was_cached());
        assert_eq!(explanation.cache_version, Some(42));
    }

    #[test]
    fn entities_and_components_read() {
        let (_interner, health, max_health, health_percent) = setup();

        let player = EntityId::new(1, 0);
        let armor = EntityId::new(2, 0);

        let explanation = DerivedExplanation::new(health_percent, player, Value::Int(75))
            .with_dependency(DerivedDependency::new(player, health, Value::Int(75)))
            .with_dependency(DerivedDependency::new(player, max_health, Value::Int(100)))
            .with_dependency(DerivedDependency::new(armor, health, Value::Int(50)));

        let entities = explanation.entities_read();
        assert_eq!(entities.len(), 2);

        let components = explanation.components_read();
        assert_eq!(components.len(), 2);
    }

    #[test]
    fn builder_pattern() {
        let (_interner, health, max_health, health_percent) = setup();

        let entity = EntityId::new(1, 0);

        let mut builder = DerivedExplanationBuilder::new();
        builder.record_read(entity, health, Value::Int(75));
        builder.record_read(entity, max_health, Value::Int(100));
        builder.record_pattern("[?self :health ?hp]");
        builder.record_bindings(vec![("?self".to_string(), Value::EntityRef(entity))]);

        let explanation = builder.build(health_percent, entity, Value::Int(75));

        assert_eq!(explanation.dependency_count(), 2);
        assert!(explanation.matched_pattern.is_some());
        assert_eq!(explanation.pattern_bindings.len(), 1);
    }
}
