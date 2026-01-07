//! Time travel integration tests
//!
//! Tests history buffer, world diffing, and timeline navigation.

use longtable_debug::{
    DiffGranularity, HistoryBuffer, MergeResult, MergeStrategy, TickSummary, diff_summary,
    diff_worlds, format_diff, merge,
};
use longtable_foundation::{LtMap, Value};
use longtable_storage::{ComponentSchema, World};

// =============================================================================
// History Buffer
// =============================================================================

#[test]
fn history_buffer_stores_snapshots() {
    let mut buffer = HistoryBuffer::new(10);

    buffer.push_tick(1, World::new(1), TickSummary::success());
    buffer.push_tick(2, World::new(2), TickSummary::success());
    buffer.push_tick(3, World::new(3), TickSummary::success());

    assert_eq!(buffer.len(), 3);
}

#[test]
fn history_buffer_retrieves_by_tick() {
    let mut buffer = HistoryBuffer::new(10);

    buffer.push_tick(5, World::new(5), TickSummary::success());
    buffer.push_tick(6, World::new(6), TickSummary::success());
    buffer.push_tick(7, World::new(7), TickSummary::success());

    let snapshot = buffer.get(6).unwrap();
    assert_eq!(snapshot.tick(), 6);
}

#[test]
fn history_buffer_evicts_oldest() {
    let mut buffer = HistoryBuffer::new(3);

    for i in 1..=5 {
        buffer.push_tick(i, World::new(i), TickSummary::success());
    }

    // Only ticks 3, 4, 5 should remain
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

    buffer.push_tick(10, World::new(10), TickSummary::success());
    buffer.push_tick(11, World::new(11), TickSummary::success());
    buffer.push_tick(12, World::new(12), TickSummary::success());

    assert_eq!(buffer.tick_range(), Some((10, 12)));
}

#[test]
fn history_buffer_truncate_for_rollback() {
    let mut buffer = HistoryBuffer::new(10);

    for i in 1..=5 {
        buffer.push_tick(i, World::new(i), TickSummary::success());
    }

    // Rollback to tick 3
    buffer.truncate_after(3);

    assert_eq!(buffer.len(), 3);
    assert_eq!(buffer.latest().unwrap().tick(), 3);
}

#[test]
fn history_buffer_latest_and_oldest() {
    let mut buffer = HistoryBuffer::new(10);

    buffer.push_tick(1, World::new(1), TickSummary::success());
    buffer.push_tick(2, World::new(2), TickSummary::success());
    buffer.push_tick(3, World::new(3), TickSummary::success());

    assert_eq!(buffer.oldest().unwrap().tick(), 1);
    assert_eq!(buffer.latest().unwrap().tick(), 3);
}

// =============================================================================
// World State Diffing
// =============================================================================

#[test]
fn diff_identical_worlds() {
    let world1 = World::new(42);
    let world2 = World::new(42);

    let diff = diff_worlds(&world1, &world2, DiffGranularity::Component);

    // Identical worlds should have empty diff
    assert!(diff.is_empty());
}

#[test]
fn diff_detects_spawned_entity() {
    let mut world1 = World::new(42);
    let player_kw = world1.interner_mut().intern_keyword("tag/player");
    let world1 = world1
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();

    // world2 has an additional entity
    let (world2, entity) = world1.clone().spawn(&LtMap::new()).unwrap();
    let world2 = world2.set(entity, player_kw, Value::Bool(true)).unwrap();

    let diff = diff_worlds(&world1, &world2, DiffGranularity::Component);

    // Should detect the spawned entity in right_only
    assert!(!diff.right_only.is_empty());
}

#[test]
fn diff_detects_destroyed_entity() {
    let mut world1 = World::new(42);
    let player_kw = world1.interner_mut().intern_keyword("tag/player");
    let world1 = world1
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();
    let (world1, entity) = world1.spawn(&LtMap::new()).unwrap();
    let world1 = world1.set(entity, player_kw, Value::Bool(true)).unwrap();

    // world2 has the entity destroyed
    let world2 = world1.clone().destroy(entity).unwrap();

    let diff = diff_worlds(&world1, &world2, DiffGranularity::Component);

    // Should detect the destroyed entity in left_only
    assert!(!diff.left_only.is_empty());
}

#[test]
fn diff_detects_component_change() {
    let mut world1 = World::new(42);
    let active_kw = world1.interner_mut().intern_keyword("active");
    let inactive_kw = world1.interner_mut().intern_keyword("inactive");

    let world1 = world1
        .register_component(ComponentSchema::tag(active_kw))
        .unwrap();
    let world1 = world1
        .register_component(ComponentSchema::tag(inactive_kw))
        .unwrap();
    let (world1, entity) = world1.spawn(&LtMap::new()).unwrap();
    let world1 = world1.set(entity, active_kw, Value::Bool(true)).unwrap();

    // world2 adds another component
    let world2 = world1
        .clone()
        .set(entity, inactive_kw, Value::Bool(true))
        .unwrap();

    let diff = diff_worlds(&world1, &world2, DiffGranularity::Component);

    // Should detect the component addition in modified
    assert!(!diff.modified.is_empty());
}

// =============================================================================
// Time Travel Navigation
// =============================================================================

#[test]
fn navigate_to_past_tick() {
    let mut world = World::new(42);
    let counter_kw = world.interner_mut().intern_keyword("counter");
    let world = world
        .register_component(ComponentSchema::tag(counter_kw))
        .unwrap();

    let mut buffer = HistoryBuffer::new(10);

    // Record several ticks with different states
    let (world1, e1) = world.clone().spawn(&LtMap::new()).unwrap();
    buffer.push_tick(1, world1.clone(), TickSummary::success().with_spawned(1));

    let (world2, e2) = world1.spawn(&LtMap::new()).unwrap();
    buffer.push_tick(2, world2.clone(), TickSummary::success().with_spawned(1));

    let (world3, _e3) = world2.spawn(&LtMap::new()).unwrap();
    buffer.push_tick(3, world3, TickSummary::success().with_spawned(1));

    // Navigate to tick 1 - should only have entity e1
    let past = buffer.get(1).unwrap();
    assert!(past.world().exists(e1));
    assert!(!past.world().exists(e2)); // e2 didn't exist at tick 1
}

#[test]
fn recent_snapshots_retrieval() {
    let mut buffer = HistoryBuffer::new(10);

    for i in 1..=5 {
        buffer.push_tick(i, World::new(i), TickSummary::success());
    }

    let recent: Vec<_> = buffer.recent(3).collect();
    assert_eq!(recent.len(), 3);
    assert_eq!(recent[0].tick(), 3);
    assert_eq!(recent[1].tick(), 4);
    assert_eq!(recent[2].tick(), 5);
}

// =============================================================================
// Tick Summaries
// =============================================================================

#[test]
fn tick_summary_builder() {
    let summary = TickSummary::success()
        .with_spawned(5)
        .with_destroyed(2)
        .with_writes(10)
        .with_rules(3);

    assert!(summary.success);
    assert_eq!(summary.entities_spawned, 5);
    assert_eq!(summary.entities_destroyed, 2);
    assert_eq!(summary.component_writes, 10);
    assert_eq!(summary.rules_fired, 3);
}

#[test]
fn tick_summary_display() {
    let summary = TickSummary::success().with_spawned(3).with_rules(2);
    let display = summary.to_string();

    assert!(display.contains("OK"));
    assert!(display.contains("3 spawned"));
    assert!(display.contains("2 rules"));
}

#[test]
fn history_summaries() {
    let mut buffer = HistoryBuffer::new(10);

    buffer.push_tick(1, World::new(1), TickSummary::success().with_spawned(1));
    buffer.push_tick(2, World::new(2), TickSummary::success().with_spawned(2));
    buffer.push_tick(3, World::new(3), TickSummary::success().with_spawned(3));

    let summaries = buffer.summaries();
    assert_eq!(summaries.len(), 3);
    assert_eq!(summaries[0].1.entities_spawned, 1);
    assert_eq!(summaries[1].1.entities_spawned, 2);
    assert_eq!(summaries[2].1.entities_spawned, 3);
}

// =============================================================================
// Diff Formatting
// =============================================================================

#[test]
fn diff_summary_output() {
    let mut world1 = World::new(42);
    let player_kw = world1.interner_mut().intern_keyword("tag/player");
    let world1 = world1
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();

    let (world2, entity) = world1.clone().spawn(&LtMap::new()).unwrap();
    let world2 = world2.set(entity, player_kw, Value::Bool(true)).unwrap();

    let diff = diff_worlds(&world1, &world2, DiffGranularity::Component);
    let summary = diff_summary(&diff);

    // Should produce some output
    assert!(summary.contains("Added entities"));
}

#[test]
fn format_diff_produces_output() {
    let mut world1 = World::new(42);
    let player_kw = world1.interner_mut().intern_keyword("tag/player");
    let world1 = world1
        .register_component(ComponentSchema::tag(player_kw))
        .unwrap();

    let (world2, entity) = world1.clone().spawn(&LtMap::new()).unwrap();
    let world2 = world2.set(entity, player_kw, Value::Bool(true)).unwrap();

    let diff = diff_worlds(&world1, &world2, DiffGranularity::Component);
    let formatted = format_diff(&diff, world2.interner(), 10);

    // Should produce some output
    assert!(!formatted.is_empty());
    assert!(formatted.contains("Added"));
}

// =============================================================================
// Merge Operations
// =============================================================================

#[test]
fn merge_replace_strategy() {
    let mut world = World::new(42);
    let tag_kw = world.interner_mut().intern_keyword("tag");
    let world = world
        .register_component(ComponentSchema::tag(tag_kw))
        .unwrap();
    let (world, entity) = world.spawn(&LtMap::new()).unwrap();

    let base = world.clone();
    let current = world.clone();
    let incoming = world.set(entity, tag_kw, Value::Bool(true)).unwrap();

    let result = merge(&base, &current, &incoming, MergeStrategy::Replace);

    // With Replace, incoming should be used
    assert!(result.is_success());
    if let MergeResult::Success { world, .. } = result {
        assert!(world.has(entity, tag_kw));
    }
}

#[test]
fn merge_compare_strategy() {
    let mut world = World::new(42);
    let tag_kw = world.interner_mut().intern_keyword("tag");
    let world = world
        .register_component(ComponentSchema::tag(tag_kw))
        .unwrap();
    let (world, entity) = world.spawn(&LtMap::new()).unwrap();

    let base = world.clone();
    let current = world.clone();
    let incoming = world.set(entity, tag_kw, Value::Bool(true)).unwrap();

    let result = merge(&base, &current, &incoming, MergeStrategy::Compare);

    // With Compare, should only return diff without merging
    assert!(result.is_comparison());
    if let MergeResult::Comparison { diff } = result {
        assert!(!diff.is_empty());
    }
}

// =============================================================================
// Empty History
// =============================================================================

#[test]
fn empty_history_operations() {
    let buffer = HistoryBuffer::new(10);

    assert!(buffer.is_empty());
    assert_eq!(buffer.len(), 0);
    assert!(buffer.latest().is_none());
    assert!(buffer.oldest().is_none());
    assert!(buffer.tick_range().is_none());
}

// =============================================================================
// Diff Affected Count
// =============================================================================

#[test]
fn diff_affected_count() {
    let mut world1 = World::new(42);
    let tag_kw = world1.interner_mut().intern_keyword("tag");
    let world1 = world1
        .register_component(ComponentSchema::tag(tag_kw))
        .unwrap();

    // Spawn 3 entities
    let (world2, _e1) = world1.clone().spawn(&LtMap::new()).unwrap();
    let (world2, _e2) = world2.spawn(&LtMap::new()).unwrap();
    let (world2, _e3) = world2.spawn(&LtMap::new()).unwrap();

    let diff = diff_worlds(&world1, &world2, DiffGranularity::Entity);

    assert_eq!(diff.affected_count(), 3);
}
