//! Multi-hop "why" queries for causal chain tracing.
//!
//! This module provides the data types and logic for answering questions like:
//! - "Why does this entity have this component value?"
//! - "What rule set this value, and why did that rule fire?"
//!
//! # Example
//!
//! ```text
//! (why player :health/current)
//! ;; => {:rule :apply-damage, :tick 5, :bindings {?e Entity(3), ?dmg 25}}
//!
//! (why player :health/current :depth 3)
//! ;; => {:chain [{rule: apply-damage, ...}, {rule: attack, ...}, ...]}
//! ```

use longtable_engine::provenance::{ProvenanceTracker, WriteRecord};
use longtable_foundation::{EntityId, KeywordId, Value};

// =============================================================================
// Causal Link
// =============================================================================

/// A single link in a causal chain.
///
/// Represents one write operation that contributed to the current state.
#[derive(Clone, Debug)]
pub struct CausalLink {
    /// The entity that was written to.
    pub entity: EntityId,

    /// The component that was written.
    pub component: KeywordId,

    /// The value that was written.
    pub value: Option<Value>,

    /// Which rule performed the write.
    pub rule: KeywordId,

    /// Tick number when the write occurred.
    pub tick: u64,

    /// Entity binding context from the rule.
    pub context: Vec<(String, EntityId)>,

    /// Full variable bindings at effect time (if captured).
    pub bindings: Option<Vec<(String, Value)>>,

    /// Previous value before this write (if captured).
    pub previous_value: Option<Value>,
}

impl CausalLink {
    /// Creates a causal link from a write record.
    #[must_use]
    pub fn from_write_record(
        entity: EntityId,
        component: KeywordId,
        record: &WriteRecord,
        current_value: Option<Value>,
    ) -> Self {
        Self {
            entity,
            component,
            value: current_value,
            rule: record.rule,
            tick: record.tick,
            context: record.context.clone(),
            bindings: record.bindings_snapshot.clone(),
            previous_value: record.previous_value.clone(),
        }
    }
}

// =============================================================================
// Causal Chain
// =============================================================================

/// A chain of causally-related writes.
///
/// Links are ordered from most recent (index 0) to oldest.
#[derive(Clone, Debug, Default)]
pub struct CausalChain {
    /// The links in the chain, most recent first.
    pub links: Vec<CausalLink>,

    /// True if the chain was truncated due to depth limit.
    pub truncated: bool,
}

impl CausalChain {
    /// Creates an empty causal chain.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a causal chain with a single link.
    #[must_use]
    pub fn single(link: CausalLink) -> Self {
        Self {
            links: vec![link],
            truncated: false,
        }
    }

    /// Returns the number of links in the chain.
    #[must_use]
    pub fn len(&self) -> usize {
        self.links.len()
    }

    /// Returns true if the chain is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }

    /// Returns the most recent link (the immediate cause).
    #[must_use]
    pub fn immediate_cause(&self) -> Option<&CausalLink> {
        self.links.first()
    }

    /// Returns the oldest link (the root cause within the traced depth).
    #[must_use]
    pub fn root_cause(&self) -> Option<&CausalLink> {
        self.links.last()
    }
}

// =============================================================================
// Why Result
// =============================================================================

/// Result of a "why" query.
#[derive(Clone, Debug)]
pub enum WhyResult {
    /// Single-hop result (depth 0 or 1).
    Single(Option<CausalLink>),

    /// Multi-hop result with full causal chain.
    Chain(CausalChain),

    /// No provenance information available.
    Unknown,
}

impl WhyResult {
    /// Returns true if provenance information was found.
    #[must_use]
    pub fn found(&self) -> bool {
        match self {
            Self::Single(Some(_)) => true,
            Self::Chain(chain) => !chain.is_empty(),
            Self::Single(None) | Self::Unknown => false,
        }
    }

    /// Returns the immediate cause (most recent write) if available.
    #[must_use]
    pub fn immediate_cause(&self) -> Option<&CausalLink> {
        match self {
            Self::Single(link) => link.as_ref(),
            Self::Chain(chain) => chain.immediate_cause(),
            Self::Unknown => None,
        }
    }

    /// Returns the rule that last wrote the value.
    #[must_use]
    pub fn last_writer(&self) -> Option<KeywordId> {
        self.immediate_cause().map(|link| link.rule)
    }

    /// Returns the tick when the value was last written.
    #[must_use]
    pub fn last_write_tick(&self) -> Option<u64> {
        self.immediate_cause().map(|link| link.tick)
    }
}

// =============================================================================
// Why Query
// =============================================================================

/// Performs "why" queries against a provenance tracker.
pub struct WhyQuery<'a> {
    tracker: &'a ProvenanceTracker,
}

impl<'a> WhyQuery<'a> {
    /// Creates a new why query against the given tracker.
    #[must_use]
    pub fn new(tracker: &'a ProvenanceTracker) -> Self {
        Self { tracker }
    }

    /// Answers "why does this entity have this component value?"
    ///
    /// Returns the single most recent write (depth 1).
    #[must_use]
    pub fn why(&self, entity: EntityId, component: KeywordId) -> WhyResult {
        self.why_depth(entity, component, 1, None)
    }

    /// Answers "why" with a configurable depth for causal chain tracing.
    ///
    /// - `depth`: Maximum number of links to trace (1 = immediate cause only)
    /// - `current_value`: Optional current value for the first link
    #[must_use]
    pub fn why_depth(
        &self,
        entity: EntityId,
        component: KeywordId,
        depth: usize,
        current_value: Option<Value>,
    ) -> WhyResult {
        if depth == 0 {
            return WhyResult::Unknown;
        }

        // Get the last writer
        let Some(record) = self.tracker.last_writer(entity, component) else {
            return WhyResult::Unknown;
        };

        let link = CausalLink::from_write_record(entity, component, record, current_value);

        if depth == 1 {
            return WhyResult::Single(Some(link));
        }

        // Multi-hop tracing
        let mut chain = CausalChain::new();
        chain.links.push(link);

        // Try to trace back through entity references in the binding context
        self.trace_chain(&mut chain, depth - 1);

        WhyResult::Chain(chain)
    }

    /// Traces a causal chain by following entity references in binding contexts.
    fn trace_chain(&self, chain: &mut CausalChain, remaining_depth: usize) {
        if remaining_depth == 0 {
            chain.truncated = true;
            return;
        }

        // Get the last link to find the next cause
        let Some(last_link) = chain.links.last() else {
            return;
        };

        // Look for entity references in the binding context that might be causes
        // We look for entities that were read to trigger this rule
        for (var_name, referenced_entity) in &last_link.context {
            // Skip self-references
            if *referenced_entity == last_link.entity {
                continue;
            }

            // Look for writes to this referenced entity
            // This is a heuristic - we look for any component that was written
            // In a full implementation, we'd track which components were read
            let writes = self.tracker.writes_for_entity(*referenced_entity);

            // Find the most recent write that happened before or at the current link's tick
            let relevant_write = writes
                .iter()
                .filter(|(_, r)| r.tick <= last_link.tick)
                .max_by_key(|(_, r)| r.tick);

            if let Some((component, record)) = relevant_write {
                let next_link = CausalLink {
                    entity: *referenced_entity,
                    component: *component,
                    value: None, // We don't have the value without world access
                    rule: record.rule,
                    tick: record.tick,
                    context: record.context.clone(),
                    bindings: record.bindings_snapshot.clone(),
                    previous_value: record.previous_value.clone(),
                };

                chain.links.push(next_link);

                // Continue tracing
                self.trace_chain(chain, remaining_depth - 1);

                // For simplicity, we only follow one path (the first entity reference)
                // A more sophisticated implementation could follow multiple paths
                return;
            }

            // Also check if there's a binding variable that references an entity
            if let Some(bindings) = &last_link.bindings {
                for (binding_var, binding_value) in bindings {
                    if binding_var == var_name {
                        if let Value::EntityRef(entity) = binding_value {
                            // We found a potential cause - try to trace it
                            let writes = self.tracker.writes_for_entity(*entity);
                            if let Some((component, record)) = writes
                                .iter()
                                .filter(|(_, r)| r.tick <= last_link.tick)
                                .max_by_key(|(_, r)| r.tick)
                            {
                                let next_link = CausalLink {
                                    entity: *entity,
                                    component: *component,
                                    value: None,
                                    rule: record.rule,
                                    tick: record.tick,
                                    context: record.context.clone(),
                                    bindings: record.bindings_snapshot.clone(),
                                    previous_value: record.previous_value.clone(),
                                };

                                chain.links.push(next_link);
                                self.trace_chain(chain, remaining_depth - 1);
                                return;
                            }
                        }
                    }
                }
            }
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use longtable_engine::provenance::ProvenanceVerbosity;
    use longtable_foundation::Interner;

    fn setup() -> (Interner, KeywordId, KeywordId, KeywordId) {
        let mut interner = Interner::new();
        let health = interner.intern_keyword("health");
        let damage = interner.intern_keyword("damage");
        let apply_damage = interner.intern_keyword("apply-damage");
        (interner, health, damage, apply_damage)
    }

    #[test]
    fn why_single_hop() {
        let (_interner, health, _damage, apply_damage) = setup();
        let mut tracker = ProvenanceTracker::with_verbosity(ProvenanceVerbosity::Standard);

        let entity = EntityId::new(1, 0);
        tracker.record_write(entity, health, apply_damage);

        let query = WhyQuery::new(&tracker);
        let result = query.why(entity, health);

        assert!(result.found());
        assert_eq!(result.last_writer(), Some(apply_damage));
    }

    #[test]
    fn why_unknown() {
        let (_interner, health, _damage, _apply_damage) = setup();
        let tracker = ProvenanceTracker::with_verbosity(ProvenanceVerbosity::Standard);

        let entity = EntityId::new(1, 0);

        let query = WhyQuery::new(&tracker);
        let result = query.why(entity, health);

        assert!(!result.found());
        assert_eq!(result.last_writer(), None);
    }

    #[test]
    fn why_with_depth() {
        let (mut interner, health, damage, apply_damage) = setup();
        let attack = interner.intern_keyword("attack");

        let mut tracker = ProvenanceTracker::with_verbosity(ProvenanceVerbosity::Standard);

        let player = EntityId::new(1, 0);
        let enemy = EntityId::new(2, 0);

        // enemy attacked, creating damage
        tracker.record_write_with_context(
            enemy,
            damage,
            attack,
            vec![("?attacker".to_string(), enemy)],
        );

        tracker.begin_tick();

        // apply-damage rule reads enemy's damage and writes to player's health
        tracker.record_write_with_context(
            player,
            health,
            apply_damage,
            vec![
                ("?target".to_string(), player),
                ("?source".to_string(), enemy),
            ],
        );

        let query = WhyQuery::new(&tracker);
        let result = query.why_depth(player, health, 3, Some(Value::Int(75)));

        match result {
            WhyResult::Chain(chain) => {
                // Should have at least 1 link (the immediate cause)
                assert!(!chain.is_empty());
                assert_eq!(chain.immediate_cause().unwrap().rule, apply_damage);
            }
            _ => panic!("Expected chain result"),
        }
    }

    #[test]
    fn causal_chain_accessors() {
        let (_interner, health, _damage, apply_damage) = setup();

        let entity = EntityId::new(1, 0);
        let link = CausalLink {
            entity,
            component: health,
            value: Some(Value::Int(75)),
            rule: apply_damage,
            tick: 5,
            context: vec![],
            bindings: None,
            previous_value: Some(Value::Int(100)),
        };

        let chain = CausalChain::single(link);

        assert_eq!(chain.len(), 1);
        assert!(!chain.is_empty());
        assert!(chain.immediate_cause().is_some());
        assert!(chain.root_cause().is_some());
        assert!(!chain.truncated);
    }

    #[test]
    fn why_result_accessors() {
        let (_interner, health, _damage, apply_damage) = setup();

        let entity = EntityId::new(1, 0);
        let link = CausalLink {
            entity,
            component: health,
            value: Some(Value::Int(75)),
            rule: apply_damage,
            tick: 5,
            context: vec![],
            bindings: None,
            previous_value: None,
        };

        let result = WhyResult::Single(Some(link));

        assert!(result.found());
        assert_eq!(result.last_writer(), Some(apply_damage));
        assert_eq!(result.last_write_tick(), Some(5));
    }
}
