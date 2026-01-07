//! Speculation integration tests
//!
//! Tests branching and speculative execution using the timeline/branch system.

use longtable_debug::{Branch, BranchId, BranchRegistry, TickSummary, Timeline, TimelineConfig};
use longtable_foundation::{LtMap, Value};
use longtable_storage::{ComponentSchema, World};

// =============================================================================
// Branch Registry
// =============================================================================

#[test]
fn branch_registry_starts_with_main() {
    let registry = BranchRegistry::new();

    assert_eq!(registry.len(), 1);
    assert!(registry.get_by_name("main").is_some());
}

#[test]
fn branch_registry_create_branch() {
    let mut registry = BranchRegistry::new();
    let main_id = registry.main_id();

    let branch_id = registry.create_branch("speculation".to_string(), main_id, 0);

    assert!(branch_id.is_some());
    assert_eq!(registry.len(), 2);
}

#[test]
fn branch_registry_get_branch() {
    let mut registry = BranchRegistry::new();
    let main_id = registry.main_id();

    let branch_id = registry
        .create_branch("test".to_string(), main_id, 5)
        .unwrap();
    let branch = registry.get(branch_id).unwrap();

    assert_eq!(branch.name(), "test");
    assert_eq!(branch.fork_tick(), 5);
}

#[test]
fn branch_registry_list_branches() {
    let mut registry = BranchRegistry::new();
    let main_id = registry.main_id();

    registry.create_branch("branch1".to_string(), main_id, 0);
    registry.create_branch("branch2".to_string(), main_id, 0);
    registry.create_branch("branch3".to_string(), main_id, 0);

    let names: Vec<_> = registry.names().collect();
    assert_eq!(names.len(), 4); // main + 3 branches
    assert!(names.contains(&"main"));
    assert!(names.contains(&"branch1"));
    assert!(names.contains(&"branch2"));
    assert!(names.contains(&"branch3"));
}

#[test]
fn branch_registry_delete_branch() {
    let mut registry = BranchRegistry::new();
    let main_id = registry.main_id();

    let branch_id = registry
        .create_branch("temp".to_string(), main_id, 0)
        .unwrap();
    assert!(registry.get(branch_id).is_some());

    assert!(registry.delete(branch_id));
    assert!(registry.get(branch_id).is_none());
}

#[test]
fn branch_registry_cannot_delete_main() {
    let mut registry = BranchRegistry::new();
    let main_id = registry.main_id();

    assert!(!registry.delete(main_id));
    assert!(registry.get(main_id).is_some());
}

// =============================================================================
// Branch State
// =============================================================================

#[test]
fn branch_stores_world_snapshots() {
    let mut world = World::new(42);
    let health_kw = world.interner_mut().intern_keyword("health");

    let world = world
        .register_component(ComponentSchema::tag(health_kw))
        .unwrap();
    let (world, entity) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(entity, health_kw, Value::Bool(true)).unwrap();

    let id = BranchId::new(1);
    let mut branch = Branch::new(id, "speculation".to_string(), 0, None);
    branch.push_snapshot(1, world.clone(), TickSummary::success());

    // Branch should have the snapshot
    let snapshot = branch.latest().unwrap();
    assert!(snapshot.world().has(entity, health_kw));
}

#[test]
fn branch_can_accumulate_snapshots() {
    let id = BranchId::new(1);
    let mut branch = Branch::new(id, "test".to_string(), 0, None);

    branch.push_snapshot(1, World::new(1), TickSummary::success());
    branch.push_snapshot(2, World::new(2), TickSummary::success());
    branch.push_snapshot(3, World::new(3), TickSummary::success());

    assert_eq!(branch.tip_tick(), Some(3));
    assert!(branch.get(1).is_some());
    assert!(branch.get(2).is_some());
    assert!(branch.get(3).is_some());
}

// =============================================================================
// Timeline Branching
// =============================================================================

#[test]
fn timeline_creates_branch() {
    let mut timeline = Timeline::new();

    timeline.capture(1, World::new(1), TickSummary::success());

    let branch_id = timeline.create_branch("experiment".to_string(), 1);
    assert!(branch_id.is_some());

    assert!(timeline.checkout("experiment"));
    assert_eq!(timeline.current_branch().name(), "experiment");
}

#[test]
fn timeline_checkout_switches_branches() {
    let mut timeline = Timeline::new();

    timeline.capture(1, World::new(1), TickSummary::success());
    timeline.create_branch("feature".to_string(), 1);

    // Start on main
    assert_eq!(timeline.current_branch().name(), "main");

    // Switch to feature
    timeline.checkout("feature");
    assert_eq!(timeline.current_branch().name(), "feature");

    // Switch back to main
    timeline.checkout("main");
    assert_eq!(timeline.current_branch().name(), "main");
}

#[test]
fn timeline_branches_have_independent_history() {
    let mut timeline = Timeline::new();

    // Capture on main
    timeline.capture(1, World::new(1), TickSummary::success());
    timeline.capture(2, World::new(2), TickSummary::success());

    // Create branch from tick 1
    timeline.create_branch("branch".to_string(), 1);
    timeline.checkout("branch");

    // Branch starts empty (doesn't inherit history)
    assert!(timeline.get_snapshot(1).is_none());

    // Add different snapshots to branch
    timeline.capture(10, World::new(10), TickSummary::success());
    assert!(timeline.get_snapshot(10).is_some());

    // Switch back to main - tick 10 shouldn't exist
    timeline.checkout("main");
    assert!(timeline.get_snapshot(10).is_none());
    assert!(timeline.get_snapshot(1).is_some());
    assert!(timeline.get_snapshot(2).is_some());
}

// =============================================================================
// Timeline Configuration
// =============================================================================

#[test]
fn timeline_config_history_size() {
    let config = TimelineConfig::new().with_history_size(50);
    assert_eq!(config.history_size, 50);
}

#[test]
fn timeline_can_be_disabled() {
    let mut timeline = Timeline::new();
    timeline.disable();

    timeline.capture(1, World::new(1), TickSummary::success());

    // Should not capture when disabled
    assert!(timeline.get_snapshot(1).is_none());
}

// =============================================================================
// Branch Naming
// =============================================================================

#[test]
fn duplicate_branch_names_fail() {
    let mut registry = BranchRegistry::new();
    let main_id = registry.main_id();

    let first = registry.create_branch("unique".to_string(), main_id, 0);
    assert!(first.is_some());

    let second = registry.create_branch("unique".to_string(), main_id, 0);
    assert!(second.is_none());
}

#[test]
fn cannot_create_branch_named_main() {
    let mut registry = BranchRegistry::new();
    let main_id = registry.main_id();

    let result = registry.create_branch("main".to_string(), main_id, 0);
    assert!(result.is_none());
}

// =============================================================================
// Speculation Use Case
// =============================================================================

#[test]
fn speculative_what_if_scenario() {
    let mut world = World::new(42);
    let attack_kw = world.interner_mut().intern_keyword("attack");
    let defend_kw = world.interner_mut().intern_keyword("defend");

    let world = world
        .register_component(ComponentSchema::tag(attack_kw))
        .unwrap();
    let world = world
        .register_component(ComponentSchema::tag(defend_kw))
        .unwrap();
    let (world, entity) = world.spawn(&LtMap::new()).unwrap();

    let mut timeline = Timeline::new();

    // Main timeline: entity exists with no actions
    timeline.capture(1, world.clone(), TickSummary::success());

    // What-if scenario 1: attack
    timeline.create_branch("what-if-attack".to_string(), 1);
    timeline.checkout("what-if-attack");
    let attack_world = world
        .clone()
        .set(entity, attack_kw, Value::Bool(true))
        .unwrap();
    timeline.capture(2, attack_world.clone(), TickSummary::success());

    // What-if scenario 2: defend
    timeline.checkout("main");
    timeline.create_branch("what-if-defend".to_string(), 1);
    timeline.checkout("what-if-defend");
    let defend_world = world.set(entity, defend_kw, Value::Bool(true)).unwrap();
    timeline.capture(2, defend_world.clone(), TickSummary::success());

    // Verify both branches have different outcomes
    timeline.checkout("what-if-attack");
    let attack_snap = timeline.get_snapshot(2).unwrap();
    assert!(attack_snap.world().has(entity, attack_kw));
    assert!(!attack_snap.world().has(entity, defend_kw));

    timeline.checkout("what-if-defend");
    let defend_snap = timeline.get_snapshot(2).unwrap();
    assert!(!defend_snap.world().has(entity, attack_kw));
    assert!(defend_snap.world().has(entity, defend_kw));
}
