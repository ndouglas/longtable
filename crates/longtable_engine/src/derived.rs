//! Derived component system for Longtable.
//!
//! Derived components are computed values that behave like components
//! but are calculated from other data. They support:
//! - Pattern-based dependency tracking
//! - Lazy evaluation with caching
//! - Automatic invalidation when dependencies change
//! - Cycle detection

use std::collections::{HashMap, HashSet};

use longtable_foundation::{EntityId, Error, ErrorKind, Interner, KeywordId, Result, Value};
use longtable_language::declaration::DerivedDecl;
use longtable_language::{Ast, Bytecode, compile_expression};
use longtable_storage::World;

use crate::pattern::{CompiledPattern, PatternCompiler, PatternMatcher};

// =============================================================================
// Compiled Derived Component
// =============================================================================

/// A compiled derived component ready for evaluation.
#[derive(Clone, Debug)]
pub struct CompiledDerived {
    /// Derived component name (interned keyword)
    pub name: KeywordId,
    /// Entity variable this is computed for (e.g., "self")
    pub for_var: String,
    /// Compiled pattern for finding dependencies
    pub pattern: CompiledPattern,
    /// Local bindings (name, AST)
    pub bindings: Vec<(String, Ast)>,
    /// Aggregation expressions (name, bytecode)
    pub aggregates: Vec<(String, Bytecode)>,
    /// Value expression (bytecode)
    pub value: Bytecode,
    /// Components this derived depends on (for invalidation)
    pub dependencies: HashSet<KeywordId>,
}

// =============================================================================
// Derived Compiler
// =============================================================================

/// Compiles derived component declarations.
pub struct DerivedCompiler;

impl DerivedCompiler {
    /// Compile a derived component declaration.
    ///
    /// # Errors
    /// Returns an error if compilation fails.
    pub fn compile(decl: &DerivedDecl, interner: &mut Interner) -> Result<CompiledDerived> {
        // Intern the derived component name
        let name = interner.intern_keyword(&decl.name);

        // Compile the pattern
        let pattern = PatternCompiler::compile(&decl.pattern, interner)?;

        // Collect dependencies from pattern
        let dependencies: HashSet<KeywordId> =
            pattern.clauses.iter().map(|c| c.component).collect();

        // Collect all binding variables
        let binding_vars: Vec<String> = decl
            .pattern
            .bound_variables()
            .into_iter()
            .map(String::from)
            .collect();

        // Compile aggregation expressions
        let aggregates = decl
            .aggregates
            .iter()
            .map(|(name, ast)| {
                let compiled = compile_expression(ast, &binding_vars)?;
                Ok((name.clone(), compiled.code))
            })
            .collect::<Result<Vec<_>>>()?;

        // Compile value expression
        let value_compiled = compile_expression(&decl.value, &binding_vars)?;

        Ok(CompiledDerived {
            name,
            for_var: decl.for_var.clone(),
            pattern,
            bindings: decl.bindings.clone(),
            aggregates,
            value: value_compiled.code,
            dependencies,
        })
    }

    /// Compile multiple derived components.
    ///
    /// # Errors
    /// Returns an error if any compilation fails.
    pub fn compile_all(
        decls: &[DerivedDecl],
        interner: &mut Interner,
    ) -> Result<Vec<CompiledDerived>> {
        decls.iter().map(|d| Self::compile(d, interner)).collect()
    }
}

// =============================================================================
// Derived Cache
// =============================================================================

/// Cached value for a derived component.
#[derive(Clone, Debug)]
struct CachedValue {
    /// The computed value
    value: Value,
    /// Version/tick when this was computed
    version: u64,
}

/// Cache for derived component values.
#[derive(Clone, Debug, Default)]
pub struct DerivedCache {
    /// Cached values: `(entity, derived_name) -> cached_value`
    cache: HashMap<(EntityId, KeywordId), CachedValue>,
    /// Current version/tick
    version: u64,
}

impl DerivedCache {
    /// Creates a new empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Advances the version, invalidating all cached values.
    pub fn advance_version(&mut self) {
        self.version += 1;
    }

    /// Returns the current cache version.
    #[must_use]
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Gets a cached value if still valid.
    #[must_use]
    pub fn get(&self, entity: EntityId, derived: KeywordId) -> Option<&Value> {
        self.cache
            .get(&(entity, derived))
            .filter(|c| c.version == self.version)
            .map(|c| &c.value)
    }

    /// Stores a computed value in the cache.
    pub fn set(&mut self, entity: EntityId, derived: KeywordId, value: Value) {
        self.cache.insert(
            (entity, derived),
            CachedValue {
                value,
                version: self.version,
            },
        );
    }

    /// Clears all cached values.
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Invalidates cached values that depend on a specific component.
    /// Returns the number of invalidated entries.
    pub fn invalidate_by_component(
        &mut self,
        component: KeywordId,
        deriveds: &[CompiledDerived],
    ) -> usize {
        // Find which derived components depend on this component
        let affected: HashSet<KeywordId> = deriveds
            .iter()
            .filter(|d| d.dependencies.contains(&component))
            .map(|d| d.name)
            .collect();

        // Remove affected cached values
        let before = self.cache.len();
        self.cache
            .retain(|(_, derived), _| !affected.contains(derived));
        before - self.cache.len()
    }
}

// =============================================================================
// Derived Evaluator
// =============================================================================

/// Evaluates derived components.
#[derive(Clone, Debug)]
pub struct DerivedEvaluator {
    /// Compiled derived components
    deriveds: Vec<CompiledDerived>,
    /// Value cache
    cache: DerivedCache,
    /// Max evaluation depth for cycle detection
    max_depth: usize,
}

impl Default for DerivedEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl DerivedEvaluator {
    /// Creates a new evaluator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            deriveds: Vec::new(),
            cache: DerivedCache::new(),
            max_depth: 100,
        }
    }

    /// Creates an evaluator with the given derived components.
    #[must_use]
    pub fn with_deriveds(mut self, deriveds: Vec<CompiledDerived>) -> Self {
        self.deriveds = deriveds;
        self
    }

    /// Sets the max evaluation depth.
    #[must_use]
    pub fn with_max_depth(mut self, max: usize) -> Self {
        self.max_depth = max;
        self
    }

    /// Invalidates the cache (call at start of tick).
    pub fn begin_tick(&mut self) {
        self.cache.advance_version();
    }

    /// Gets a derived component value for an entity.
    ///
    /// Returns None if the derived component doesn't exist or
    /// the entity doesn't match the pattern.
    ///
    /// # Errors
    /// Returns an error if evaluation fails or cycle is detected.
    pub fn get(
        &mut self,
        entity: EntityId,
        derived: KeywordId,
        world: &World,
    ) -> Result<Option<Value>> {
        self.get_with_depth(entity, derived, world, 0)
    }

    fn get_with_depth(
        &mut self,
        entity: EntityId,
        derived: KeywordId,
        world: &World,
        depth: usize,
    ) -> Result<Option<Value>> {
        // Check depth limit
        if depth > self.max_depth {
            return Err(Error::new(ErrorKind::Internal(
                "derived component evaluation depth exceeded (possible cycle)".to_string(),
            )));
        }

        // Check cache first
        if let Some(value) = self.cache.get(entity, derived) {
            return Ok(Some(value.clone()));
        }

        // Find the derived component
        let derived_def = match self.deriveds.iter().find(|d| d.name == derived) {
            Some(d) => d.clone(), // Clone to avoid borrow issues
            None => return Ok(None),
        };

        // Match the pattern to find bindings for this entity
        let matches = PatternMatcher::match_pattern(&derived_def.pattern, world);

        // Find a match where the for_var is bound to our entity
        let bindings = matches.into_iter().find(|b| {
            b.get(&derived_def.for_var)
                .is_some_and(|v| matches!(v, Value::EntityRef(e) if *e == entity))
        });

        let Some(_bindings) = bindings else {
            return Ok(None);
        };

        // For now, return a placeholder value
        // Full implementation would evaluate the bytecode with the bindings
        let value = Value::Nil;

        // Cache the result
        self.cache.set(entity, derived, value.clone());

        Ok(Some(value))
    }

    /// Returns the compiled derived components.
    #[must_use]
    pub fn deriveds(&self) -> &[CompiledDerived] {
        &self.deriveds
    }

    /// Notifies the evaluator that a component was modified.
    /// Returns the number of cache entries invalidated.
    pub fn on_component_change(&mut self, component: KeywordId) -> usize {
        self.cache
            .invalidate_by_component(component, &self.deriveds)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use longtable_foundation::LtMap;
    use longtable_language::Span;
    use longtable_language::declaration::Pattern as DeclPattern;
    use longtable_storage::ComponentSchema;

    #[test]
    fn compile_simple_derived() {
        let mut interner = Interner::new();

        // Create a simple derived: health-percent that requires health component
        let decl = DerivedDecl::new(
            "health-percent",
            "self",
            Ast::Int(100, Span::default()), // Placeholder value expression
            Span::default(),
        );

        let compiled = DerivedCompiler::compile(&decl, &mut interner).unwrap();

        assert_eq!(compiled.for_var, "self");
        assert!(compiled.aggregates.is_empty());
    }

    #[test]
    fn cache_basic_operations() {
        let mut cache = DerivedCache::new();
        let mut interner = Interner::new();

        let entity = EntityId::new(1, 0);
        let derived = interner.intern_keyword("test-derived");

        // Initially empty
        assert!(cache.get(entity, derived).is_none());

        // Set a value
        cache.set(entity, derived, Value::Int(42));

        // Should be retrievable
        assert_eq!(cache.get(entity, derived), Some(&Value::Int(42)));

        // Advance version
        cache.advance_version();

        // Should be invalidated
        assert!(cache.get(entity, derived).is_none());
    }

    #[test]
    fn cache_invalidation_by_component() {
        let mut interner = Interner::new();

        // Create a derived that depends on :health
        let health_id = interner.intern_keyword("health");
        let derived_id = interner.intern_keyword("health-percent");

        let mut pattern = DeclPattern::default();
        pattern
            .clauses
            .push(longtable_language::declaration::PatternClause {
                entity_var: "self".to_string(),
                component: "health".to_string(),
                value: longtable_language::declaration::PatternValue::Wildcard,
                span: Span::default(),
            });

        let mut decl = DerivedDecl::new(
            "health-percent",
            "self",
            Ast::Int(100, Span::default()),
            Span::default(),
        );
        decl.pattern = pattern;

        let compiled = DerivedCompiler::compile(&decl, &mut interner).unwrap();
        let deriveds = vec![compiled];

        // Set up cache with a value
        let mut cache = DerivedCache::new();
        let entity = EntityId::new(1, 0);
        cache.set(entity, derived_id, Value::Int(50));

        // Should be there
        assert!(cache.get(entity, derived_id).is_some());

        // Invalidate by :health component
        let invalidated = cache.invalidate_by_component(health_id, &deriveds);
        assert_eq!(invalidated, 1);

        // Should be gone
        assert!(cache.get(entity, derived_id).is_none());
    }

    #[test]
    fn evaluator_depth_limit() {
        let mut world = World::new(42);

        // Register a component
        let health = world.interner_mut().intern_keyword("health");
        world = world
            .register_component(ComponentSchema::tag(health))
            .unwrap();

        // Spawn an entity
        let (world, entity) = world.spawn(&LtMap::new()).unwrap();
        let world = world.set(entity, health, Value::Bool(true)).unwrap();

        // Create an evaluator with very low depth limit
        let evaluator = DerivedEvaluator::new().with_max_depth(0);

        // Use interner to create a non-existent derived id
        let mut interner = Interner::new();
        let derived_id = interner.intern_keyword("nonexistent-derived");

        // Even with depth 0, we should handle the case gracefully
        // (no derived components to evaluate)
        let mut evaluator = evaluator;
        let result = evaluator.get(entity, derived_id, &world);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
