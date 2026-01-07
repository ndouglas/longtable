//! Provenance tracking for Longtable.
//!
//! This module tracks who wrote what and when, supporting:
//! - Last-writer per (entity, component) field
//! - Multi-hop "why did this value change" queries
//! - Configurable verbosity levels (Minimal/Standard/Full)
//! - Optional full history tracking for time travel debugging

use std::collections::HashMap;

use longtable_foundation::{EntityId, KeywordId, Value};

// =============================================================================
// Verbosity Levels
// =============================================================================

/// Verbosity level for provenance tracking.
///
/// Higher verbosity captures more information but uses more memory.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProvenanceVerbosity {
    /// Minimal: Last-writer only, no value or binding snapshots.
    /// This is the most memory-efficient mode with minimal overhead.
    #[default]
    Minimal,

    /// Standard: Captures previous values and binding snapshots.
    /// Enables meaningful "why" queries with change context.
    Standard,

    /// Full: Captures expression IDs for source location tracking.
    /// Enables step-through debugging and source attribution.
    Full,
}

// =============================================================================
// Write Record
// =============================================================================

/// Record of who wrote a value and when.
///
/// The fields captured depend on the verbosity level:
/// - Minimal: rule, tick, context only
/// - Standard: + `previous_value`, `bindings_snapshot`
/// - Full: + `expr_id`
#[derive(Clone, Debug)]
pub struct WriteRecord {
    /// Which rule performed the write
    pub rule: KeywordId,
    /// Tick number when the write occurred
    pub tick: u64,
    /// Optional binding context (entity variables involved)
    pub context: Vec<(String, EntityId)>,

    // --- Standard+ verbosity fields ---
    /// The value that was overwritten (None if new component or Minimal verbosity)
    pub previous_value: Option<Value>,
    /// Full variable bindings at effect time (None if Minimal verbosity)
    pub bindings_snapshot: Option<Vec<(String, Value)>>,

    // --- Full verbosity fields ---
    /// Expression ID for source location tracking (None if < Full verbosity)
    pub expr_id: Option<u32>,
}

impl WriteRecord {
    /// Creates a new write record with minimal information.
    #[must_use]
    pub fn new(rule: KeywordId, tick: u64) -> Self {
        Self {
            rule,
            tick,
            context: Vec::new(),
            previous_value: None,
            bindings_snapshot: None,
            expr_id: None,
        }
    }

    /// Adds binding context (entity variables).
    #[must_use]
    pub fn with_context(mut self, var: impl Into<String>, entity: EntityId) -> Self {
        self.context.push((var.into(), entity));
        self
    }

    /// Sets the previous value (Standard+ verbosity).
    #[must_use]
    pub fn with_previous_value(mut self, value: Value) -> Self {
        self.previous_value = Some(value);
        self
    }

    /// Sets the full bindings snapshot (Standard+ verbosity).
    #[must_use]
    pub fn with_bindings(mut self, bindings: Vec<(String, Value)>) -> Self {
        self.bindings_snapshot = Some(bindings);
        self
    }

    /// Sets the expression ID (Full verbosity).
    #[must_use]
    pub fn with_expr_id(mut self, expr_id: u32) -> Self {
        self.expr_id = Some(expr_id);
        self
    }
}

// =============================================================================
// Write History
// =============================================================================

/// History of writes for a single (entity, component) pair.
///
/// Only populated when verbosity > Minimal.
#[derive(Clone, Debug, Default)]
pub struct WriteHistory {
    /// All writes in chronological order (oldest first).
    pub writes: Vec<WriteRecord>,
}

// =============================================================================
// Provenance Tracker
// =============================================================================

/// Maximum history entries per (entity, component) to prevent unbounded growth.
const DEFAULT_MAX_HISTORY_PER_KEY: usize = 100;

/// Tracks who wrote what and when.
///
/// The tracker supports three verbosity levels:
/// - Minimal: Only tracks last writer (low overhead)
/// - Standard: Tracks last writer + full history with value snapshots
/// - Full: Standard + expression IDs for source attribution
#[derive(Clone, Debug)]
pub struct ProvenanceTracker {
    /// Last writer for each (entity, component) pair (always maintained)
    last_writer: HashMap<(EntityId, KeywordId), WriteRecord>,

    /// Full history per (entity, component) - only when verbosity > Minimal
    history: Option<HashMap<(EntityId, KeywordId), WriteHistory>>,

    /// Current verbosity level
    verbosity: ProvenanceVerbosity,

    /// Current tick number
    tick: u64,

    /// Maximum history entries per key
    max_history_per_key: usize,
}

impl Default for ProvenanceTracker {
    fn default() -> Self {
        Self {
            last_writer: HashMap::new(),
            history: None,
            verbosity: ProvenanceVerbosity::Minimal,
            tick: 0,
            max_history_per_key: DEFAULT_MAX_HISTORY_PER_KEY,
        }
    }
}

impl ProvenanceTracker {
    /// Creates a new provenance tracker with Minimal verbosity.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a provenance tracker with the specified verbosity.
    #[must_use]
    pub fn with_verbosity(verbosity: ProvenanceVerbosity) -> Self {
        let history = if verbosity == ProvenanceVerbosity::Minimal {
            None
        } else {
            Some(HashMap::new())
        };

        Self {
            last_writer: HashMap::new(),
            history,
            verbosity,
            tick: 0,
            max_history_per_key: DEFAULT_MAX_HISTORY_PER_KEY,
        }
    }

    /// Returns the current verbosity level.
    #[must_use]
    pub fn verbosity(&self) -> ProvenanceVerbosity {
        self.verbosity
    }

    /// Sets the verbosity level.
    ///
    /// Note: Downgrading from Standard/Full to Minimal will clear history.
    pub fn set_verbosity(&mut self, verbosity: ProvenanceVerbosity) {
        if verbosity == ProvenanceVerbosity::Minimal {
            self.history = None;
        } else if self.history.is_none() {
            self.history = Some(HashMap::new());
        }
        self.verbosity = verbosity;
    }

    /// Sets the maximum history entries per (entity, component) key.
    pub fn set_max_history(&mut self, max: usize) {
        self.max_history_per_key = max;
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

    /// Records a write to an entity's component (minimal information).
    pub fn record_write(&mut self, entity: EntityId, component: KeywordId, rule: KeywordId) {
        let record = WriteRecord::new(rule, self.tick);
        self.insert_record(entity, component, record);
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
        self.insert_record(entity, component, record);
    }

    /// Records a write with full information based on verbosity level.
    ///
    /// This is the primary recording method for Phase 6+ that captures
    /// all available provenance information.
    #[allow(clippy::too_many_arguments)]
    pub fn record_write_full(
        &mut self,
        entity: EntityId,
        component: KeywordId,
        rule: KeywordId,
        context: Vec<(String, EntityId)>,
        previous_value: Option<Value>,
        bindings: Option<Vec<(String, Value)>>,
        expr_id: Option<u32>,
    ) {
        let mut record = WriteRecord::new(rule, self.tick);
        record.context = context;

        // Only capture additional fields based on verbosity
        if self.verbosity != ProvenanceVerbosity::Minimal {
            record.previous_value = previous_value;
            record.bindings_snapshot = bindings;

            if self.verbosity == ProvenanceVerbosity::Full {
                record.expr_id = expr_id;
            }
        }

        self.insert_record(entity, component, record);
    }

    /// Internal helper to insert a record and update history.
    fn insert_record(&mut self, entity: EntityId, component: KeywordId, record: WriteRecord) {
        let key = (entity, component);

        // Update history if enabled
        if let Some(history) = &mut self.history {
            let write_history = history.entry(key).or_default();
            write_history.writes.push(record.clone());

            // Enforce max history limit
            while write_history.writes.len() > self.max_history_per_key {
                write_history.writes.remove(0);
            }
        }

        // Always update last writer
        self.last_writer.insert(key, record);
    }

    /// Gets the last writer for an entity's component.
    #[must_use]
    pub fn last_writer(&self, entity: EntityId, component: KeywordId) -> Option<&WriteRecord> {
        self.last_writer.get(&(entity, component))
    }

    /// Gets the full write history for an entity's component.
    ///
    /// Returns None if verbosity is Minimal or no history exists.
    #[must_use]
    pub fn history(&self, entity: EntityId, component: KeywordId) -> Option<&[WriteRecord]> {
        self.history
            .as_ref()
            .and_then(|h| h.get(&(entity, component)))
            .map(|wh| wh.writes.as_slice())
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
        if let Some(history) = &mut self.history {
            history.clear();
        }
    }

    /// Clears provenance for a specific entity.
    pub fn clear_entity(&mut self, entity: EntityId) {
        self.last_writer.retain(|(e, _), _| *e != entity);
        if let Some(history) = &mut self.history {
            history.retain(|(e, _), _| *e != entity);
        }
    }

    /// Prunes history entries older than the specified tick.
    pub fn prune_before_tick(&mut self, tick: u64) {
        if let Some(history) = &mut self.history {
            for write_history in history.values_mut() {
                write_history.writes.retain(|r| r.tick >= tick);
            }
            // Remove empty histories
            history.retain(|_, wh| !wh.writes.is_empty());
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

    #[test]
    fn verbosity_levels() {
        // Test Minimal (default)
        let tracker = ProvenanceTracker::new();
        assert_eq!(tracker.verbosity(), ProvenanceVerbosity::Minimal);

        // Test with_verbosity constructor
        let tracker = ProvenanceTracker::with_verbosity(ProvenanceVerbosity::Standard);
        assert_eq!(tracker.verbosity(), ProvenanceVerbosity::Standard);

        let tracker = ProvenanceTracker::with_verbosity(ProvenanceVerbosity::Full);
        assert_eq!(tracker.verbosity(), ProvenanceVerbosity::Full);
    }

    #[test]
    fn history_tracking_minimal() {
        let (_interner, health, _mana, rule1) = setup();
        let mut tracker = ProvenanceTracker::new(); // Minimal verbosity

        let entity = EntityId::new(1, 0);

        tracker.record_write(entity, health, rule1);

        // History should not be available in Minimal mode
        assert!(tracker.history(entity, health).is_none());

        // But last_writer should still work
        assert!(tracker.last_writer(entity, health).is_some());
    }

    #[test]
    fn history_tracking_standard() {
        let (_interner, health, _mana, rule1) = setup();
        let mut tracker = ProvenanceTracker::with_verbosity(ProvenanceVerbosity::Standard);

        let entity = EntityId::new(1, 0);

        // Record multiple writes
        tracker.record_write(entity, health, rule1);
        tracker.begin_tick();
        tracker.record_write(entity, health, rule1);
        tracker.begin_tick();
        tracker.record_write(entity, health, rule1);

        // History should be available
        let history = tracker.history(entity, health);
        assert!(history.is_some());
        assert_eq!(history.unwrap().len(), 3);

        // Check tick ordering
        let writes = history.unwrap();
        assert_eq!(writes[0].tick, 0);
        assert_eq!(writes[1].tick, 1);
        assert_eq!(writes[2].tick, 2);
    }

    #[test]
    fn record_write_full() {
        use longtable_foundation::Value;

        let (_interner, health, _mana, rule1) = setup();
        let mut tracker = ProvenanceTracker::with_verbosity(ProvenanceVerbosity::Full);

        let entity = EntityId::new(1, 0);
        let prev_value = Value::Int(100);
        let bindings = vec![("?e".to_string(), Value::EntityRef(entity))];

        tracker.record_write_full(
            entity,
            health,
            rule1,
            vec![("?e".to_string(), entity)],
            Some(prev_value.clone()),
            Some(bindings.clone()),
            Some(42),
        );

        let record = tracker.last_writer(entity, health).unwrap();
        assert_eq!(record.rule, rule1);
        assert_eq!(record.previous_value, Some(prev_value));
        assert!(record.bindings_snapshot.is_some());
        assert_eq!(record.expr_id, Some(42));
    }

    #[test]
    fn record_write_full_respects_verbosity() {
        use longtable_foundation::Value;

        let (_interner, health, _mana, rule1) = setup();

        // Minimal: should not capture extra fields
        let mut tracker = ProvenanceTracker::with_verbosity(ProvenanceVerbosity::Minimal);
        let entity = EntityId::new(1, 0);

        tracker.record_write_full(
            entity,
            health,
            rule1,
            vec![],
            Some(Value::Int(100)),
            Some(vec![]),
            Some(42),
        );

        let record = tracker.last_writer(entity, health).unwrap();
        assert!(record.previous_value.is_none());
        assert!(record.bindings_snapshot.is_none());
        assert!(record.expr_id.is_none());

        // Standard: should capture value and bindings but not expr_id
        let mut tracker = ProvenanceTracker::with_verbosity(ProvenanceVerbosity::Standard);
        tracker.record_write_full(
            entity,
            health,
            rule1,
            vec![],
            Some(Value::Int(100)),
            Some(vec![]),
            Some(42),
        );

        let record = tracker.last_writer(entity, health).unwrap();
        assert!(record.previous_value.is_some());
        assert!(record.bindings_snapshot.is_some());
        assert!(record.expr_id.is_none()); // Not captured in Standard
    }

    #[test]
    fn history_max_limit() {
        let (_interner, health, _mana, rule1) = setup();
        let mut tracker = ProvenanceTracker::with_verbosity(ProvenanceVerbosity::Standard);
        tracker.set_max_history(3);

        let entity = EntityId::new(1, 0);

        // Record 5 writes
        for _ in 0..5 {
            tracker.record_write(entity, health, rule1);
            tracker.begin_tick();
        }

        // Should only keep last 3
        let history = tracker.history(entity, health).unwrap();
        assert_eq!(history.len(), 3);

        // Should be the most recent writes (ticks 2, 3, 4)
        assert_eq!(history[0].tick, 2);
        assert_eq!(history[1].tick, 3);
        assert_eq!(history[2].tick, 4);
    }

    #[test]
    fn prune_before_tick() {
        let (_interner, health, mana, rule1) = setup();
        let mut tracker = ProvenanceTracker::with_verbosity(ProvenanceVerbosity::Standard);

        let entity = EntityId::new(1, 0);

        // Record writes at different ticks
        tracker.record_write(entity, health, rule1); // tick 0
        tracker.begin_tick();
        tracker.record_write(entity, health, rule1); // tick 1
        tracker.record_write(entity, mana, rule1); // tick 1
        tracker.begin_tick();
        tracker.record_write(entity, health, rule1); // tick 2

        // Prune before tick 2
        tracker.prune_before_tick(2);

        // Health should have 1 entry (tick 2)
        let health_history = tracker.history(entity, health).unwrap();
        assert_eq!(health_history.len(), 1);
        assert_eq!(health_history[0].tick, 2);

        // Mana history should be gone entirely (only had tick 1)
        assert!(tracker.history(entity, mana).is_none());
    }

    #[test]
    fn set_verbosity_clears_history_on_downgrade() {
        let (_interner, health, _mana, rule1) = setup();
        let mut tracker = ProvenanceTracker::with_verbosity(ProvenanceVerbosity::Standard);

        let entity = EntityId::new(1, 0);

        tracker.record_write(entity, health, rule1);
        assert!(tracker.history(entity, health).is_some());

        // Downgrade to Minimal
        tracker.set_verbosity(ProvenanceVerbosity::Minimal);
        assert!(tracker.history(entity, health).is_none());

        // last_writer should still work
        assert!(tracker.last_writer(entity, health).is_some());
    }
}
