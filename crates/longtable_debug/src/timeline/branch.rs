//! Branch management for time travel.
//!
//! Provides git-like branching for exploring alternative timelines.

use super::history::{HistoryBuffer, TickSnapshot, TickSummary};
use longtable_storage::World;
use std::collections::HashMap;
use std::fmt;

// =============================================================================
// Branch ID
// =============================================================================

/// Unique identifier for a branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BranchId(u64);

impl BranchId {
    /// Creates a new branch ID.
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns the raw ID value.
    #[must_use]
    pub const fn value(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for BranchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "branch-{}", self.0)
    }
}

// =============================================================================
// Branch
// =============================================================================

/// A timeline branch.
#[derive(Clone, Debug)]
pub struct Branch {
    /// Unique identifier.
    id: BranchId,
    /// Human-readable name.
    name: String,
    /// Tick where this branch forked from its parent.
    fork_tick: u64,
    /// Parent branch (None for main).
    parent: Option<BranchId>,
    /// History of snapshots on this branch.
    history: HistoryBuffer,
}

impl Branch {
    /// Creates a new branch.
    #[must_use]
    pub fn new(id: BranchId, name: String, fork_tick: u64, parent: Option<BranchId>) -> Self {
        Self {
            id,
            name,
            fork_tick,
            parent,
            history: HistoryBuffer::default(),
        }
    }

    /// Creates a new branch with custom history capacity.
    #[must_use]
    pub fn with_capacity(
        id: BranchId,
        name: String,
        fork_tick: u64,
        parent: Option<BranchId>,
        capacity: usize,
    ) -> Self {
        Self {
            id,
            name,
            fork_tick,
            parent,
            history: HistoryBuffer::new(capacity),
        }
    }

    /// Returns the branch ID.
    #[must_use]
    pub const fn id(&self) -> BranchId {
        self.id
    }

    /// Returns the branch name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the fork tick.
    #[must_use]
    pub const fn fork_tick(&self) -> u64 {
        self.fork_tick
    }

    /// Returns the parent branch ID.
    #[must_use]
    pub const fn parent(&self) -> Option<BranchId> {
        self.parent
    }

    /// Returns a reference to the history buffer.
    #[must_use]
    pub fn history(&self) -> &HistoryBuffer {
        &self.history
    }

    /// Returns a mutable reference to the history buffer.
    pub fn history_mut(&mut self) -> &mut HistoryBuffer {
        &mut self.history
    }

    /// Pushes a snapshot to this branch.
    pub fn push_snapshot(&mut self, tick: u64, world: World, summary: TickSummary) {
        self.history.push_tick(tick, world, summary);
    }

    /// Gets the latest snapshot on this branch.
    #[must_use]
    pub fn latest(&self) -> Option<&TickSnapshot> {
        self.history.latest()
    }

    /// Gets a snapshot by tick.
    #[must_use]
    pub fn get(&self, tick: u64) -> Option<&TickSnapshot> {
        self.history.get(tick)
    }

    /// Returns the current tip tick.
    #[must_use]
    pub fn tip_tick(&self) -> Option<u64> {
        self.history.latest().map(TickSnapshot::tick)
    }
}

// =============================================================================
// Branch Registry
// =============================================================================

/// Registry of all branches in a timeline.
#[derive(Clone, Debug)]
pub struct BranchRegistry {
    /// All branches by ID.
    branches: HashMap<BranchId, Branch>,
    /// Branch lookup by name.
    by_name: HashMap<String, BranchId>,
    /// Next branch ID to assign.
    next_id: u64,
    /// The main branch ID.
    main_id: BranchId,
}

impl BranchRegistry {
    /// Creates a new branch registry with the main branch.
    #[must_use]
    pub fn new() -> Self {
        let main_id = BranchId::new(0);
        let main = Branch::new(main_id, "main".to_string(), 0, None);

        let mut branches = HashMap::new();
        branches.insert(main_id, main);

        let mut by_name = HashMap::new();
        by_name.insert("main".to_string(), main_id);

        Self {
            branches,
            by_name,
            next_id: 1,
            main_id,
        }
    }

    /// Returns the main branch ID.
    #[must_use]
    pub const fn main_id(&self) -> BranchId {
        self.main_id
    }

    /// Returns the main branch.
    ///
    /// # Panics
    ///
    /// Panics if the main branch doesn't exist (should never happen).
    #[must_use]
    pub fn main(&self) -> &Branch {
        self.branches
            .get(&self.main_id)
            .expect("main branch exists")
    }

    /// Returns a mutable reference to the main branch.
    ///
    /// # Panics
    ///
    /// Panics if the main branch doesn't exist (should never happen).
    pub fn main_mut(&mut self) -> &mut Branch {
        self.branches
            .get_mut(&self.main_id)
            .expect("main branch exists")
    }

    /// Gets a branch by ID.
    #[must_use]
    pub fn get(&self, id: BranchId) -> Option<&Branch> {
        self.branches.get(&id)
    }

    /// Gets a mutable reference to a branch by ID.
    pub fn get_mut(&mut self, id: BranchId) -> Option<&mut Branch> {
        self.branches.get_mut(&id)
    }

    /// Gets a branch by name.
    #[must_use]
    pub fn get_by_name(&self, name: &str) -> Option<&Branch> {
        self.by_name.get(name).and_then(|id| self.branches.get(id))
    }

    /// Gets a branch ID by name.
    #[must_use]
    pub fn id_by_name(&self, name: &str) -> Option<BranchId> {
        self.by_name.get(name).copied()
    }

    /// Creates a new branch forking from an existing branch at a given tick.
    ///
    /// Returns the new branch ID, or None if the parent doesn't exist or name is taken.
    pub fn create_branch(
        &mut self,
        name: String,
        parent: BranchId,
        fork_tick: u64,
    ) -> Option<BranchId> {
        // Check name isn't taken
        if self.by_name.contains_key(&name) {
            return None;
        }

        // Check parent exists
        if !self.branches.contains_key(&parent) {
            return None;
        }

        let id = BranchId::new(self.next_id);
        self.next_id += 1;

        let branch = Branch::new(id, name.clone(), fork_tick, Some(parent));
        self.branches.insert(id, branch);
        self.by_name.insert(name, id);

        Some(id)
    }

    /// Deletes a branch by ID.
    ///
    /// Cannot delete the main branch.
    pub fn delete(&mut self, id: BranchId) -> bool {
        if id == self.main_id {
            return false;
        }

        if let Some(branch) = self.branches.remove(&id) {
            self.by_name.remove(&branch.name);
            true
        } else {
            false
        }
    }

    /// Returns all branch names.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.by_name.keys().map(String::as_str)
    }

    /// Returns all branches.
    pub fn iter(&self) -> impl Iterator<Item = &Branch> {
        self.branches.values()
    }

    /// Returns the number of branches.
    #[must_use]
    pub fn len(&self) -> usize {
        self.branches.len()
    }

    /// Returns true if there are no branches (should never be true).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.branches.is_empty()
    }

    /// Returns true if only the main branch exists.
    #[must_use]
    pub fn is_single(&self) -> bool {
        self.branches.len() == 1
    }
}

impl Default for BranchRegistry {
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

    #[test]
    fn branch_id_display() {
        let id = BranchId::new(42);
        assert_eq!(format!("{id}"), "branch-42");
    }

    #[test]
    fn branch_basics() {
        let id = BranchId::new(1);
        let branch = Branch::new(id, "test".to_string(), 10, Some(BranchId::new(0)));

        assert_eq!(branch.id(), id);
        assert_eq!(branch.name(), "test");
        assert_eq!(branch.fork_tick(), 10);
        assert_eq!(branch.parent(), Some(BranchId::new(0)));
        assert!(branch.latest().is_none());
    }

    #[test]
    fn branch_push_snapshot() {
        let id = BranchId::new(1);
        let mut branch = Branch::new(id, "test".to_string(), 10, None);

        branch.push_snapshot(11, World::new(11), TickSummary::success());
        branch.push_snapshot(12, World::new(12), TickSummary::success());

        assert_eq!(branch.tip_tick(), Some(12));
        assert!(branch.get(11).is_some());
        assert!(branch.get(12).is_some());
    }

    #[test]
    fn registry_has_main() {
        let registry = BranchRegistry::new();

        assert_eq!(registry.len(), 1);
        assert_eq!(registry.main().name(), "main");
        assert!(registry.get_by_name("main").is_some());
    }

    #[test]
    fn registry_create_branch() {
        let mut registry = BranchRegistry::new();

        let id = registry
            .create_branch("experiment".to_string(), registry.main_id(), 5)
            .unwrap();

        assert_eq!(registry.len(), 2);

        let branch = registry.get(id).unwrap();
        assert_eq!(branch.name(), "experiment");
        assert_eq!(branch.fork_tick(), 5);
        assert_eq!(branch.parent(), Some(registry.main_id()));
    }

    #[test]
    fn registry_duplicate_name_fails() {
        let mut registry = BranchRegistry::new();

        let result = registry.create_branch("main".to_string(), registry.main_id(), 0);
        assert!(result.is_none());
    }

    #[test]
    fn registry_invalid_parent_fails() {
        let mut registry = BranchRegistry::new();

        let result = registry.create_branch("test".to_string(), BranchId::new(999), 0);
        assert!(result.is_none());
    }

    #[test]
    fn registry_delete_branch() {
        let mut registry = BranchRegistry::new();

        let id = registry
            .create_branch("test".to_string(), registry.main_id(), 0)
            .unwrap();

        assert!(registry.delete(id));
        assert_eq!(registry.len(), 1);
        assert!(registry.get(id).is_none());
    }

    #[test]
    fn registry_cannot_delete_main() {
        let mut registry = BranchRegistry::new();
        assert!(!registry.delete(registry.main_id()));
    }

    #[test]
    fn registry_names() {
        let mut registry = BranchRegistry::new();
        registry.create_branch("alpha".to_string(), registry.main_id(), 0);
        registry.create_branch("beta".to_string(), registry.main_id(), 0);

        let names: Vec<_> = registry.names().collect();
        assert!(names.contains(&"main"));
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
    }
}
