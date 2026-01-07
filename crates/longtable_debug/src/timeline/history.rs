//! History buffer for storing tick snapshots.
//!
//! Implements a configurable ring buffer of world snapshots for time travel.

use longtable_storage::World;
use std::collections::VecDeque;
use std::sync::Arc;

// =============================================================================
// Tick Snapshot
// =============================================================================

/// A snapshot of the world state at a particular tick.
#[derive(Clone, Debug)]
pub struct TickSnapshot {
    /// The tick number.
    tick: u64,
    /// The world state (shared via Arc for efficiency).
    world: Arc<World>,
    /// Summary of what happened in this tick.
    summary: TickSummary,
}

impl TickSnapshot {
    /// Creates a new tick snapshot.
    #[must_use]
    pub fn new(tick: u64, world: World, summary: TickSummary) -> Self {
        Self {
            tick,
            world: Arc::new(world),
            summary,
        }
    }

    /// Returns the tick number.
    #[must_use]
    pub const fn tick(&self) -> u64 {
        self.tick
    }

    /// Returns a reference to the world state.
    #[must_use]
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Returns a clone of the Arc-wrapped world.
    #[must_use]
    pub fn world_arc(&self) -> Arc<World> {
        Arc::clone(&self.world)
    }

    /// Returns the tick summary.
    #[must_use]
    pub fn summary(&self) -> &TickSummary {
        &self.summary
    }
}

// =============================================================================
// Tick Summary
// =============================================================================

/// Summary of what happened during a tick.
#[derive(Clone, Debug, Default)]
pub struct TickSummary {
    /// Number of entities spawned.
    pub entities_spawned: usize,
    /// Number of entities destroyed.
    pub entities_destroyed: usize,
    /// Number of component writes.
    pub component_writes: usize,
    /// Number of rules that fired.
    pub rules_fired: usize,
    /// Whether the tick completed successfully.
    pub success: bool,
}

impl TickSummary {
    /// Creates a new tick summary.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a successful tick summary.
    #[must_use]
    pub fn success() -> Self {
        Self {
            success: true,
            ..Self::default()
        }
    }

    /// Builder method to set entities spawned.
    #[must_use]
    pub const fn with_spawned(mut self, count: usize) -> Self {
        self.entities_spawned = count;
        self
    }

    /// Builder method to set entities destroyed.
    #[must_use]
    pub const fn with_destroyed(mut self, count: usize) -> Self {
        self.entities_destroyed = count;
        self
    }

    /// Builder method to set component writes.
    #[must_use]
    pub const fn with_writes(mut self, count: usize) -> Self {
        self.component_writes = count;
        self
    }

    /// Builder method to set rules fired.
    #[must_use]
    pub const fn with_rules(mut self, count: usize) -> Self {
        self.rules_fired = count;
        self
    }
}

impl std::fmt::Display for TickSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = if self.success { "OK" } else { "FAILED" };
        write!(
            f,
            "{}: {} spawned, {} destroyed, {} writes, {} rules",
            status,
            self.entities_spawned,
            self.entities_destroyed,
            self.component_writes,
            self.rules_fired
        )
    }
}

// =============================================================================
// History Buffer
// =============================================================================

/// Ring buffer of tick snapshots for time travel.
#[derive(Clone, Debug)]
pub struct HistoryBuffer {
    /// The snapshots in chronological order.
    snapshots: VecDeque<TickSnapshot>,
    /// Maximum number of snapshots to retain.
    capacity: usize,
}

impl HistoryBuffer {
    /// Creates a new history buffer with the given capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            snapshots: VecDeque::with_capacity(capacity.min(1024)),
            capacity,
        }
    }

    /// Returns the capacity of the buffer.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the number of snapshots in the buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    /// Returns true if the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    /// Pushes a new snapshot, evicting the oldest if at capacity.
    pub fn push(&mut self, snapshot: TickSnapshot) {
        if self.snapshots.len() >= self.capacity {
            self.snapshots.pop_front();
        }
        self.snapshots.push_back(snapshot);
    }

    /// Pushes a new snapshot from its components.
    pub fn push_tick(&mut self, tick: u64, world: World, summary: TickSummary) {
        self.push(TickSnapshot::new(tick, world, summary));
    }

    /// Gets the snapshot for a specific tick.
    #[must_use]
    pub fn get(&self, tick: u64) -> Option<&TickSnapshot> {
        self.snapshots.iter().find(|s| s.tick == tick)
    }

    /// Gets the most recent snapshot.
    #[must_use]
    pub fn latest(&self) -> Option<&TickSnapshot> {
        self.snapshots.back()
    }

    /// Gets the oldest snapshot.
    #[must_use]
    pub fn oldest(&self) -> Option<&TickSnapshot> {
        self.snapshots.front()
    }

    /// Returns an iterator over snapshots from oldest to newest.
    pub fn iter(&self) -> impl Iterator<Item = &TickSnapshot> {
        self.snapshots.iter()
    }

    /// Returns an iterator over snapshots from newest to oldest.
    pub fn iter_rev(&self) -> impl Iterator<Item = &TickSnapshot> {
        self.snapshots.iter().rev()
    }

    /// Returns the range of ticks available.
    #[must_use]
    pub fn tick_range(&self) -> Option<(u64, u64)> {
        match (self.snapshots.front(), self.snapshots.back()) {
            (Some(first), Some(last)) => Some((first.tick, last.tick)),
            _ => None,
        }
    }

    /// Gets N most recent snapshots.
    pub fn recent(&self, count: usize) -> impl Iterator<Item = &TickSnapshot> {
        let skip = self.snapshots.len().saturating_sub(count);
        self.snapshots.iter().skip(skip)
    }

    /// Clears all snapshots.
    pub fn clear(&mut self) {
        self.snapshots.clear();
    }

    /// Removes all snapshots after the given tick (for rollback).
    pub fn truncate_after(&mut self, tick: u64) {
        while let Some(last) = self.snapshots.back() {
            if last.tick > tick {
                self.snapshots.pop_back();
            } else {
                break;
            }
        }
    }

    /// Returns summaries of all snapshots.
    #[must_use]
    pub fn summaries(&self) -> Vec<(u64, &TickSummary)> {
        self.snapshots
            .iter()
            .map(|s| (s.tick, &s.summary))
            .collect()
    }
}

impl Default for HistoryBuffer {
    fn default() -> Self {
        Self::new(100)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_world(tick: u64) -> World {
        World::new(tick)
    }

    #[test]
    fn snapshot_accessors() {
        let world = make_world(5);
        let summary = TickSummary::success().with_spawned(3);
        let snapshot = TickSnapshot::new(5, world, summary);

        assert_eq!(snapshot.tick(), 5);
        assert_eq!(snapshot.summary().entities_spawned, 3);
        assert!(snapshot.summary().success);
    }

    #[test]
    fn history_buffer_push_and_get() {
        let mut buffer = HistoryBuffer::new(10);

        buffer.push_tick(1, make_world(1), TickSummary::success());
        buffer.push_tick(2, make_world(2), TickSummary::success());
        buffer.push_tick(3, make_world(3), TickSummary::success());

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.get(1).unwrap().tick(), 1);
        assert_eq!(buffer.get(2).unwrap().tick(), 2);
        assert_eq!(buffer.get(3).unwrap().tick(), 3);
        assert!(buffer.get(4).is_none());
    }

    #[test]
    fn history_buffer_eviction() {
        let mut buffer = HistoryBuffer::new(3);

        for i in 1..=5 {
            buffer.push_tick(i, make_world(i), TickSummary::success());
        }

        assert_eq!(buffer.len(), 3);
        assert!(buffer.get(1).is_none());
        assert!(buffer.get(2).is_none());
        assert!(buffer.get(3).is_some());
        assert!(buffer.get(4).is_some());
        assert!(buffer.get(5).is_some());
    }

    #[test]
    fn history_buffer_tick_range() {
        let mut buffer = HistoryBuffer::new(10);

        assert!(buffer.tick_range().is_none());

        buffer.push_tick(5, make_world(5), TickSummary::success());
        buffer.push_tick(6, make_world(6), TickSummary::success());
        buffer.push_tick(7, make_world(7), TickSummary::success());

        assert_eq!(buffer.tick_range(), Some((5, 7)));
    }

    #[test]
    fn history_buffer_truncate_after() {
        let mut buffer = HistoryBuffer::new(10);

        for i in 1..=5 {
            buffer.push_tick(i, make_world(i), TickSummary::success());
        }

        buffer.truncate_after(3);

        assert_eq!(buffer.len(), 3);
        assert!(buffer.get(4).is_none());
        assert!(buffer.get(5).is_none());
        assert_eq!(buffer.latest().unwrap().tick(), 3);
    }

    #[test]
    fn history_buffer_recent() {
        let mut buffer = HistoryBuffer::new(10);

        for i in 1..=5 {
            buffer.push_tick(i, make_world(i), TickSummary::success());
        }

        let recent: Vec<_> = buffer.recent(2).collect();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].tick(), 4);
        assert_eq!(recent[1].tick(), 5);
    }

    #[test]
    fn tick_summary_display() {
        let summary = TickSummary::success()
            .with_spawned(5)
            .with_destroyed(2)
            .with_writes(10)
            .with_rules(3);

        let display = summary.to_string();
        assert!(display.contains("OK"));
        assert!(display.contains("5 spawned"));
        assert!(display.contains("2 destroyed"));
        assert!(display.contains("10 writes"));
        assert!(display.contains("3 rules"));
    }
}
