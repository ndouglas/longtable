//! Basic provenance tracking for Longtable.
//!
//! Phase 4 implements minimal effect logging—enough for basic `why` queries
//! and error context. Full tracing remains in Phase 6.
//!
//! This module tracks:
//! - Last-writer per (entity, component) field
//! - Basic "why did this value change" queries

use std::collections::HashMap;

use longtable_foundation::{EntityId, KeywordId};

// =============================================================================
// Write Record
// =============================================================================

/// Record of who wrote a value and when.
#[derive(Clone, Debug)]
pub struct WriteRecord {
    /// Which rule performed the write
    pub rule: KeywordId,
    /// Tick number when the write occurred
    pub tick: u64,
    /// Optional binding context (entity variables involved)
    pub context: Vec<(String, EntityId)>,
}

impl WriteRecord {
    /// Creates a new write record.
    #[must_use]
    pub fn new(rule: KeywordId, tick: u64) -> Self {
        Self {
            rule,
            tick,
            context: Vec::new(),
        }
    }

    /// Adds binding context.
    #[must_use]
    pub fn with_context(mut self, var: impl Into<String>, entity: EntityId) -> Self {
        self.context.push((var.into(), entity));
        self
    }
}

// =============================================================================
// Provenance Tracker
// =============================================================================

/// Tracks who wrote what and when.
#[derive(Clone, Debug, Default)]
pub struct ProvenanceTracker {
    /// Last writer for each (entity, component) pair
    last_writer: HashMap<(EntityId, KeywordId), WriteRecord>,
    /// Current tick number
    tick: u64,
}

impl ProvenanceTracker {
    /// Creates a new provenance tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Advances to the next tick.
    pub fn begin_tick(&mut self) {
        self.tick += 1;
    }

    /// Returns the current tick number.
    #[must_use]
    pub fn current_tick(&self) -> u64 {
        self.tick
    }

    /// Records a write to an entity's component.
    pub fn record_write(&mut self, entity: EntityId, component: KeywordId, rule: KeywordId) {
        self.last_writer
            .insert((entity, component), WriteRecord::new(rule, self.tick));
    }

    /// Records a write with binding context.
    pub fn record_write_with_context(
        &mut self,
        entity: EntityId,
        component: KeywordId,
        rule: KeywordId,
        context: Vec<(String, EntityId)>,
    ) {
        let mut record = WriteRecord::new(rule, self.tick);
        record.context = context;
        self.last_writer.insert((entity, component), record);
    }

    /// Gets the last writer for an entity's component.
    #[must_use]
    pub fn last_writer(&self, entity: EntityId, component: KeywordId) -> Option<&WriteRecord> {
        self.last_writer.get(&(entity, component))
    }

    /// Answers "why does this entity have this value?"
    ///
    /// Returns the rule that last wrote to this entity/component pair.
    #[must_use]
    pub fn why(&self, entity: EntityId, component: KeywordId) -> Option<KeywordId> {
        self.last_writer.get(&(entity, component)).map(|r| r.rule)
    }

    /// Returns all writes for a given entity.
    #[must_use]
    pub fn writes_for_entity(&self, entity: EntityId) -> Vec<(KeywordId, &WriteRecord)> {
        self.last_writer
            .iter()
            .filter_map(
                |((e, c), r)| {
                    if *e == entity { Some((*c, r)) } else { None }
                },
            )
            .collect()
    }

    /// Returns all writes by a given rule.
    #[must_use]
    pub fn writes_by_rule(&self, rule: KeywordId) -> Vec<(EntityId, KeywordId, &WriteRecord)> {
        self.last_writer
            .iter()
            .filter_map(|((e, c), r)| {
                if r.rule == rule {
                    Some((*e, *c, r))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Clears all provenance data.
    pub fn clear(&mut self) {
        self.last_writer.clear();
    }

    /// Clears provenance for a specific entity.
    pub fn clear_entity(&mut self, entity: EntityId) {
        self.last_writer.retain(|(e, _), _| *e != entity);
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
        let mana = interner.intern_keyword("mana");
        let rule1 = interner.intern_keyword("apply-damage");
        (interner, health, mana, rule1)
    }

    #[test]
    fn basic_write_tracking() {
        let (_interner, health, _mana, rule1) = setup();
        let mut tracker = ProvenanceTracker::new();

        let entity = EntityId::new(1, 0);

        // Record a write
        tracker.record_write(entity, health, rule1);

        // Should be able to query who wrote
        let writer = tracker.last_writer(entity, health);
        assert!(writer.is_some());
        assert_eq!(writer.unwrap().rule, rule1);
    }

    #[test]
    fn why_query() {
        let (_interner, health, mana, rule1) = setup();
        let mut tracker = ProvenanceTracker::new();

        let entity = EntityId::new(1, 0);

        tracker.record_write(entity, health, rule1);

        // Why does entity have health?
        let why = tracker.why(entity, health);
        assert_eq!(why, Some(rule1));

        // Why does entity have mana? (never written)
        let why_mana = tracker.why(entity, mana);
        assert!(why_mana.is_none());
    }

    #[test]
    fn tick_tracking() {
        let (_interner, health, mana, rule1) = setup();
        let mut tracker = ProvenanceTracker::new();

        let entity = EntityId::new(1, 0);

        // Initial tick is 0
        assert_eq!(tracker.current_tick(), 0);

        tracker.record_write(entity, health, rule1);

        // Advance tick
        tracker.begin_tick();
        assert_eq!(tracker.current_tick(), 1);

        // Record another write
        tracker.record_write(entity, mana, rule1);

        // Health written at tick 0, mana at tick 1
        assert_eq!(tracker.last_writer(entity, health).unwrap().tick, 0);
        assert_eq!(tracker.last_writer(entity, mana).unwrap().tick, 1);
    }

    #[test]
    fn writes_by_entity() {
        let (mut interner, health, mana, rule1) = setup();
        let rule2 = interner.intern_keyword("regen");
        let mut tracker = ProvenanceTracker::new();

        let e1 = EntityId::new(1, 0);
        let e2 = EntityId::new(2, 0);

        tracker.record_write(e1, health, rule1);
        tracker.record_write(e1, mana, rule2);
        tracker.record_write(e2, health, rule1);

        // e1 has 2 writes
        let e1_writes = tracker.writes_for_entity(e1);
        assert_eq!(e1_writes.len(), 2);

        // e2 has 1 write
        let e2_writes = tracker.writes_for_entity(e2);
        assert_eq!(e2_writes.len(), 1);
    }

    #[test]
    fn writes_by_rule() {
        let (mut interner, health, mana, rule1) = setup();
        let rule2 = interner.intern_keyword("regen");
        let mut tracker = ProvenanceTracker::new();

        let e1 = EntityId::new(1, 0);
        let e2 = EntityId::new(2, 0);

        tracker.record_write(e1, health, rule1);
        tracker.record_write(e1, mana, rule2);
        tracker.record_write(e2, health, rule1);

        // rule1 has 2 writes (e1:health, e2:health)
        let rule1_writes = tracker.writes_by_rule(rule1);
        assert_eq!(rule1_writes.len(), 2);

        // rule2 has 1 write (e1:mana)
        let rule2_writes = tracker.writes_by_rule(rule2);
        assert_eq!(rule2_writes.len(), 1);
    }

    #[test]
    fn clear_entity() {
        let (_interner, health, mana, rule1) = setup();
        let mut tracker = ProvenanceTracker::new();

        let e1 = EntityId::new(1, 0);
        let e2 = EntityId::new(2, 0);

        tracker.record_write(e1, health, rule1);
        tracker.record_write(e1, mana, rule1);
        tracker.record_write(e2, health, rule1);

        // Clear e1
        tracker.clear_entity(e1);

        // e1 writes gone
        assert!(tracker.last_writer(e1, health).is_none());
        assert!(tracker.last_writer(e1, mana).is_none());

        // e2 writes still there
        assert!(tracker.last_writer(e2, health).is_some());
    }
}
