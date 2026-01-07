//! Time travel system for Longtable.
//!
//! Provides history tracking, branching, and timeline manipulation.
//!
//! # Features
//!
//! - **History**: Ring buffer of past world states
//! - **Branching**: Git-like branching for exploring alternatives
//! - **Diff**: Compare world states between ticks
//! - **Rollback**: Return to previous states
//!
//! # Example
//!
//! ```text
//! (rollback! 5)                ;; Go back 5 ticks
//! (goto-tick! 42)              ;; Jump to tick 42
//! (branch! "experiment")       ;; Create branch
//! (checkout! "main")           ;; Switch to main
//! (diff 40 42)                 ;; Compare ticks
//! ```

pub mod branch;
pub mod diff;
pub mod history;
pub mod merge;

pub use branch::{Branch, BranchId, BranchRegistry};
pub use diff::{
    DiffGranularity, EntityDiff, ValueChange, WorldDiff, diff_summary, diff_worlds, format_diff,
};
pub use history::{HistoryBuffer, TickSnapshot, TickSummary};
pub use merge::{MergeResult, MergeStrategy, merge};

use longtable_storage::World;

// =============================================================================
// Timeline Configuration
// =============================================================================

/// Configuration for the timeline system.
#[derive(Clone, Debug)]
pub struct TimelineConfig {
    /// Maximum history size per branch.
    pub history_size: usize,
    /// Default diff granularity.
    pub diff_granularity: DiffGranularity,
    /// Whether to automatically capture snapshots.
    pub auto_capture: bool,
}

impl Default for TimelineConfig {
    fn default() -> Self {
        Self {
            history_size: 100,
            diff_granularity: DiffGranularity::Component,
            auto_capture: true,
        }
    }
}

impl TimelineConfig {
    /// Creates a new configuration with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder method to set history size.
    #[must_use]
    pub const fn with_history_size(mut self, size: usize) -> Self {
        self.history_size = size;
        self
    }

    /// Builder method to set diff granularity.
    #[must_use]
    pub const fn with_granularity(mut self, granularity: DiffGranularity) -> Self {
        self.diff_granularity = granularity;
        self
    }

    /// Builder method to enable/disable auto capture.
    #[must_use]
    pub const fn with_auto_capture(mut self, enabled: bool) -> Self {
        self.auto_capture = enabled;
        self
    }
}

// =============================================================================
// Timeline
// =============================================================================

/// Main timeline manager.
///
/// Coordinates history tracking and branching for time travel debugging.
#[derive(Clone, Debug)]
pub struct Timeline {
    /// Configuration.
    config: TimelineConfig,
    /// Branch registry.
    branches: BranchRegistry,
    /// Current branch ID.
    current_branch: BranchId,
    /// Whether the timeline is enabled.
    enabled: bool,
}

impl Timeline {
    /// Creates a new timeline with default configuration.
    #[must_use]
    pub fn new() -> Self {
        let branches = BranchRegistry::new();
        let main_id = branches.main_id();

        Self {
            config: TimelineConfig::default(),
            branches,
            current_branch: main_id,
            enabled: true,
        }
    }

    /// Creates a new timeline with custom configuration.
    #[must_use]
    pub fn with_config(config: TimelineConfig) -> Self {
        let branches = BranchRegistry::new();
        let main_id = branches.main_id();

        Self {
            config,
            branches,
            current_branch: main_id,
            enabled: true,
        }
    }

    /// Returns whether the timeline is enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enables the timeline.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disables the timeline.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Returns the configuration.
    #[must_use]
    pub fn config(&self) -> &TimelineConfig {
        &self.config
    }

    /// Returns the current branch ID.
    #[must_use]
    pub const fn current_branch_id(&self) -> BranchId {
        self.current_branch
    }

    /// Returns the current branch.
    ///
    /// # Panics
    ///
    /// Panics if the current branch doesn't exist (should never happen).
    #[must_use]
    pub fn current_branch(&self) -> &Branch {
        self.branches
            .get(self.current_branch)
            .expect("current branch exists")
    }

    /// Returns a mutable reference to the current branch.
    ///
    /// # Panics
    ///
    /// Panics if the current branch doesn't exist (should never happen).
    pub fn current_branch_mut(&mut self) -> &mut Branch {
        self.branches
            .get_mut(self.current_branch)
            .expect("current branch exists")
    }

    /// Returns the branch registry.
    #[must_use]
    pub fn branches(&self) -> &BranchRegistry {
        &self.branches
    }

    /// Captures a tick snapshot on the current branch.
    pub fn capture(&mut self, tick: u64, world: World, summary: TickSummary) {
        if !self.enabled || !self.config.auto_capture {
            return;
        }

        self.current_branch_mut()
            .push_snapshot(tick, world, summary);
    }

    /// Gets a snapshot from the current branch.
    #[must_use]
    pub fn get_snapshot(&self, tick: u64) -> Option<&TickSnapshot> {
        self.current_branch().get(tick)
    }

    /// Gets the latest snapshot from the current branch.
    #[must_use]
    pub fn latest_snapshot(&self) -> Option<&TickSnapshot> {
        self.current_branch().latest()
    }

    /// Returns the available tick range on the current branch.
    #[must_use]
    pub fn tick_range(&self) -> Option<(u64, u64)> {
        self.current_branch().history().tick_range()
    }

    /// Rolls back to N ticks ago on the current branch.
    ///
    /// Returns the world state at that tick, or None if unavailable.
    #[must_use]
    pub fn rollback(&self, ticks_back: usize) -> Option<&TickSnapshot> {
        let history = self.current_branch().history();
        let snapshots: Vec<_> = history.iter().collect();

        if ticks_back >= snapshots.len() {
            return None;
        }

        let index = snapshots.len() - 1 - ticks_back;
        Some(snapshots[index])
    }

    /// Goes to a specific tick on the current branch.
    ///
    /// Returns the world state at that tick, or None if unavailable.
    #[must_use]
    pub fn goto_tick(&self, tick: u64) -> Option<&TickSnapshot> {
        self.get_snapshot(tick)
    }

    /// Creates a new branch from the current branch at a specific tick.
    ///
    /// Returns the new branch ID, or None if creation failed.
    pub fn create_branch(&mut self, name: String, fork_tick: u64) -> Option<BranchId> {
        self.branches
            .create_branch(name, self.current_branch, fork_tick)
    }

    /// Switches to a different branch by name.
    ///
    /// Returns true if successful.
    pub fn checkout(&mut self, name: &str) -> bool {
        if let Some(id) = self.branches.id_by_name(name) {
            self.current_branch = id;
            true
        } else {
            false
        }
    }

    /// Switches to a branch by ID.
    pub fn checkout_id(&mut self, id: BranchId) -> bool {
        if self.branches.get(id).is_some() {
            self.current_branch = id;
            true
        } else {
            false
        }
    }

    /// Deletes a branch by name.
    ///
    /// Cannot delete the current branch or main branch.
    pub fn delete_branch(&mut self, name: &str) -> bool {
        if let Some(id) = self.branches.id_by_name(name) {
            if id == self.current_branch {
                return false;
            }
            self.branches.delete(id)
        } else {
            false
        }
    }

    /// Compares two ticks on the current branch.
    #[must_use]
    pub fn diff_ticks(&self, tick1: u64, tick2: u64) -> Option<WorldDiff> {
        let snap1 = self.get_snapshot(tick1)?;
        let snap2 = self.get_snapshot(tick2)?;

        Some(diff_worlds(
            snap1.world(),
            snap2.world(),
            self.config.diff_granularity,
        ))
    }

    /// Compares two branches at their tips.
    #[must_use]
    pub fn diff_branches(&self, name1: &str, name2: &str) -> Option<WorldDiff> {
        let branch1 = self.branches.get_by_name(name1)?;
        let branch2 = self.branches.get_by_name(name2)?;

        let snap1 = branch1.latest()?;
        let snap2 = branch2.latest()?;

        Some(diff_worlds(
            snap1.world(),
            snap2.world(),
            self.config.diff_granularity,
        ))
    }

    /// Returns a list of branch names.
    #[must_use]
    pub fn branch_names(&self) -> Vec<&str> {
        self.branches.names().collect()
    }

    /// Returns recent history on the current branch.
    #[must_use]
    pub fn recent_history(&self, count: usize) -> Vec<(u64, &TickSummary)> {
        self.current_branch()
            .history()
            .recent(count)
            .map(|s| (s.tick(), s.summary()))
            .collect()
    }

    /// Clears history on the current branch.
    pub fn clear_history(&mut self) {
        self.current_branch_mut().history_mut().clear();
    }

    /// Returns a summary of the timeline state.
    #[must_use]
    pub fn status(&self) -> String {
        use std::fmt::Write;
        let mut summary = String::new();

        let _ = writeln!(
            summary,
            "Timeline: {}",
            if self.enabled { "enabled" } else { "disabled" }
        );
        let _ = writeln!(summary, "Current branch: {}", self.current_branch().name());

        if let Some((min, max)) = self.tick_range() {
            let _ = writeln!(summary, "Tick range: {min} - {max}");
        } else {
            let _ = writeln!(summary, "Tick range: (empty)");
        }

        let _ = writeln!(
            summary,
            "History size: {}",
            self.current_branch().history().len()
        );
        let _ = writeln!(summary, "Branches: {}", self.branches.len());

        summary.trim_end().to_string()
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
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
    fn timeline_capture_and_retrieve() {
        let mut timeline = Timeline::new();

        timeline.capture(1, make_world(1), TickSummary::success());
        timeline.capture(2, make_world(2), TickSummary::success());
        timeline.capture(3, make_world(3), TickSummary::success());

        assert_eq!(timeline.get_snapshot(1).unwrap().tick(), 1);
        assert_eq!(timeline.get_snapshot(2).unwrap().tick(), 2);
        assert_eq!(timeline.get_snapshot(3).unwrap().tick(), 3);
        assert!(timeline.get_snapshot(4).is_none());
    }

    #[test]
    fn timeline_rollback() {
        let mut timeline = Timeline::new();

        for i in 1..=5 {
            timeline.capture(i, make_world(i), TickSummary::success());
        }

        // Rollback 0 = latest (tick 5)
        assert_eq!(timeline.rollback(0).unwrap().tick(), 5);
        // Rollback 2 = 2 ticks ago (tick 3)
        assert_eq!(timeline.rollback(2).unwrap().tick(), 3);
        // Rollback 4 = 4 ticks ago (tick 1)
        assert_eq!(timeline.rollback(4).unwrap().tick(), 1);
        // Rollback too far = None
        assert!(timeline.rollback(10).is_none());
    }

    #[test]
    fn timeline_branching() {
        let mut timeline = Timeline::new();

        timeline.capture(1, make_world(1), TickSummary::success());
        timeline.capture(2, make_world(2), TickSummary::success());

        // Create branch
        let branch_id = timeline.create_branch("experiment".to_string(), 2);
        assert!(branch_id.is_some());

        // Checkout branch
        assert!(timeline.checkout("experiment"));
        assert_eq!(timeline.current_branch().name(), "experiment");

        // Checkout back to main
        assert!(timeline.checkout("main"));
        assert_eq!(timeline.current_branch().name(), "main");
    }

    #[test]
    fn timeline_delete_branch() {
        let mut timeline = Timeline::new();

        timeline.create_branch("test".to_string(), 0);

        // Can delete test branch
        assert!(timeline.delete_branch("test"));

        // Can't delete non-existent branch
        assert!(!timeline.delete_branch("test"));

        // Can't delete main branch
        assert!(!timeline.delete_branch("main"));
    }

    #[test]
    fn timeline_cannot_delete_current_branch() {
        let mut timeline = Timeline::new();

        timeline.create_branch("test".to_string(), 0);
        timeline.checkout("test");

        // Can't delete current branch
        assert!(!timeline.delete_branch("test"));
    }

    #[test]
    fn timeline_disabled_skips_capture() {
        let mut timeline = Timeline::new();
        timeline.disable();

        timeline.capture(1, make_world(1), TickSummary::success());

        assert!(timeline.get_snapshot(1).is_none());
    }

    #[test]
    fn timeline_status() {
        let mut timeline = Timeline::new();

        timeline.capture(1, make_world(1), TickSummary::success());
        timeline.capture(2, make_world(2), TickSummary::success());

        let status = timeline.status();
        assert!(status.contains("enabled"));
        assert!(status.contains("main"));
        assert!(status.contains("1 - 2"));
    }

    #[test]
    fn timeline_branch_names() {
        let mut timeline = Timeline::new();

        timeline.create_branch("alpha".to_string(), 0);
        timeline.create_branch("beta".to_string(), 0);

        let names = timeline.branch_names();
        assert!(names.contains(&"main"));
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
    }
}
