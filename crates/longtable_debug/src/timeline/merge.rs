//! Merge strategies for combining branch histories.
//!
//! Provides different strategies for merging branches back together.

use super::diff::WorldDiff;
use longtable_storage::World;

// =============================================================================
// Merge Strategy
// =============================================================================

/// Strategy for merging branches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MergeStrategy {
    /// Replace current tip with branch tip (fast-forward style).
    #[default]
    Replace,
    /// Generate a diff report without actually merging.
    Compare,
}

// =============================================================================
// Merge Result
// =============================================================================

/// Result of a merge operation.
#[derive(Clone, Debug)]
pub enum MergeResult {
    /// Merge completed successfully.
    Success {
        /// The resulting world state.
        world: World,
        /// Summary of what changed.
        summary: String,
    },
    /// Merge produced only a comparison (no actual merge).
    Comparison {
        /// The diff between the worlds.
        diff: WorldDiff,
    },
    /// Merge failed.
    Failed {
        /// Reason for failure.
        reason: String,
    },
}

impl MergeResult {
    /// Creates a successful merge result.
    #[must_use]
    pub fn success(world: World, summary: String) -> Self {
        Self::Success { world, summary }
    }

    /// Creates a comparison result.
    #[must_use]
    pub fn comparison(diff: WorldDiff) -> Self {
        Self::Comparison { diff }
    }

    /// Creates a failed merge result.
    #[must_use]
    pub fn failed(reason: impl Into<String>) -> Self {
        Self::Failed {
            reason: reason.into(),
        }
    }

    /// Returns true if the merge was successful.
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// Returns true if this is just a comparison.
    #[must_use]
    pub fn is_comparison(&self) -> bool {
        matches!(self, Self::Comparison { .. })
    }

    /// Returns true if the merge failed.
    #[must_use]
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Extracts the world from a successful merge.
    #[must_use]
    pub fn into_world(self) -> Option<World> {
        match self {
            Self::Success { world, .. } => Some(world),
            _ => None,
        }
    }
}

// =============================================================================
// Merge Operations
// =============================================================================

/// Performs a merge using the given strategy.
#[must_use]
pub fn merge(
    _base: &World,
    current: &World,
    incoming: &World,
    strategy: MergeStrategy,
) -> MergeResult {
    match strategy {
        MergeStrategy::Replace => {
            // Simple replace: just use the incoming world
            MergeResult::success(
                incoming.clone(),
                "Replaced current state with branch tip".to_string(),
            )
        }
        MergeStrategy::Compare => {
            // Just compute diff, don't actually merge
            let diff =
                super::diff::diff_worlds(current, incoming, super::diff::DiffGranularity::Field);
            MergeResult::comparison(diff)
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use longtable_foundation::{LtMap, Type, Value};
    use longtable_storage::schema::{ComponentSchema, FieldSchema};

    fn setup() -> (
        World,
        longtable_foundation::KeywordId,
        longtable_foundation::KeywordId,
    ) {
        let mut world = World::new(0);

        let health = world.interner_mut().intern_keyword("health");
        let value_field = world.interner_mut().intern_keyword("value");

        world = world
            .register_component(
                ComponentSchema::new(health)
                    .with_field(FieldSchema::required(value_field, Type::Int)),
            )
            .unwrap();

        (world, health, value_field)
    }

    fn make_value_map(value_field: longtable_foundation::KeywordId, value: Value) -> Value {
        let mut map = longtable_foundation::LtMap::new();
        map = map.insert(Value::Keyword(value_field), value);
        Value::Map(map)
    }

    #[test]
    fn merge_replace_strategy() {
        let (world1, health, value_field) = setup();

        let (world1, e1) = world1.spawn(&LtMap::new()).unwrap();
        let world1 = world1
            .set(e1, health, make_value_map(value_field, Value::Int(100)))
            .unwrap();

        let world2 = world1
            .clone()
            .set(e1, health, make_value_map(value_field, Value::Int(50)))
            .unwrap();

        let result = merge(&world1, &world1, &world2, MergeStrategy::Replace);

        assert!(result.is_success());

        let merged = result.into_world().unwrap();
        assert_eq!(
            merged.get(e1, health).unwrap(),
            Some(make_value_map(value_field, Value::Int(50)))
        );
    }

    #[test]
    fn merge_compare_strategy() {
        let (world1, health, value_field) = setup();

        let (world1, e1) = world1.spawn(&LtMap::new()).unwrap();
        let world1 = world1
            .set(e1, health, make_value_map(value_field, Value::Int(100)))
            .unwrap();

        let world2 = world1
            .clone()
            .set(e1, health, make_value_map(value_field, Value::Int(50)))
            .unwrap();

        let result = merge(&world1, &world1, &world2, MergeStrategy::Compare);

        assert!(result.is_comparison());

        match result {
            MergeResult::Comparison { diff } => {
                assert!(!diff.is_empty());
                assert_eq!(diff.modified.len(), 1);
            }
            _ => panic!("expected comparison result"),
        }
    }

    #[test]
    fn merge_result_predicates() {
        let world = World::new(0);

        let success = MergeResult::success(world.clone(), "test".to_string());
        assert!(success.is_success());
        assert!(!success.is_comparison());
        assert!(!success.is_failed());

        let comparison = MergeResult::comparison(super::super::diff::WorldDiff::default());
        assert!(!comparison.is_success());
        assert!(comparison.is_comparison());
        assert!(!comparison.is_failed());

        let failed = MergeResult::failed("test error");
        assert!(!failed.is_success());
        assert!(!failed.is_comparison());
        assert!(failed.is_failed());
    }
}
