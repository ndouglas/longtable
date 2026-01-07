//! World diff functionality.
//!
//! Compares world states to identify changes between ticks or branches.

use longtable_foundation::{EntityId, KeywordId, Value};
use longtable_storage::World;
use std::collections::HashSet;

// =============================================================================
// Diff Granularity
// =============================================================================

/// Controls the level of detail in diffs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DiffGranularity {
    /// Only show which entities changed.
    Entity,
    /// Show which components changed on each entity.
    #[default]
    Component,
    /// Show the actual value changes.
    Field,
}

// =============================================================================
// Diff Types
// =============================================================================

/// A change to a component value.
#[derive(Clone, Debug, PartialEq)]
pub struct ValueChange {
    /// The component that changed.
    pub component: KeywordId,
    /// The old value (None if newly added).
    pub old: Option<Value>,
    /// The new value (None if removed).
    pub new: Option<Value>,
}

impl ValueChange {
    /// Creates a new value change.
    #[must_use]
    pub fn new(component: KeywordId, old: Option<Value>, new: Option<Value>) -> Self {
        Self {
            component,
            old,
            new,
        }
    }

    /// Returns true if this is an addition.
    #[must_use]
    pub fn is_added(&self) -> bool {
        self.old.is_none() && self.new.is_some()
    }

    /// Returns true if this is a removal.
    #[must_use]
    pub fn is_removed(&self) -> bool {
        self.old.is_some() && self.new.is_none()
    }

    /// Returns true if this is a modification.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.old.is_some() && self.new.is_some()
    }
}

/// Changes to a single entity.
#[derive(Clone, Debug)]
pub struct EntityDiff {
    /// The entity that changed.
    pub entity: EntityId,
    /// Components that were added, modified, or removed.
    pub changes: Vec<ValueChange>,
}

impl EntityDiff {
    /// Creates a new entity diff.
    #[must_use]
    pub fn new(entity: EntityId) -> Self {
        Self {
            entity,
            changes: Vec::new(),
        }
    }

    /// Creates an entity diff with changes.
    #[must_use]
    pub fn with_changes(entity: EntityId, changes: Vec<ValueChange>) -> Self {
        Self { entity, changes }
    }

    /// Returns true if there are no changes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

/// Differences between two world states.
#[derive(Clone, Debug, Default)]
pub struct WorldDiff {
    /// Entities that only exist in the left (old) world.
    pub left_only: Vec<EntityId>,
    /// Entities that only exist in the right (new) world.
    pub right_only: Vec<EntityId>,
    /// Entities that exist in both but have different components.
    pub modified: Vec<EntityDiff>,
}

impl WorldDiff {
    /// Returns true if the worlds are identical (at the diffed level).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.left_only.is_empty() && self.right_only.is_empty() && self.modified.is_empty()
    }

    /// Returns the total number of entities affected.
    #[must_use]
    pub fn affected_count(&self) -> usize {
        self.left_only.len() + self.right_only.len() + self.modified.len()
    }
}

// =============================================================================
// Diff Functions
// =============================================================================

/// Computes the diff between two worlds at the specified granularity.
#[must_use]
pub fn diff_worlds(left: &World, right: &World, granularity: DiffGranularity) -> WorldDiff {
    let left_entities: HashSet<_> = left.entities().collect();
    let right_entities: HashSet<_> = right.entities().collect();

    let left_only: Vec<_> = left_entities.difference(&right_entities).copied().collect();
    let right_only: Vec<_> = right_entities.difference(&left_entities).copied().collect();

    if granularity == DiffGranularity::Entity {
        // For entity-level, mark entities as modified if they have different components
        let modified: Vec<_> = left_entities
            .intersection(&right_entities)
            .filter(|&&entity| !entities_equal(left, right, entity))
            .map(|&entity| EntityDiff::new(entity))
            .collect();

        return WorldDiff {
            left_only,
            right_only,
            modified,
        };
    }

    // For component or field granularity, compute detailed diffs
    let modified: Vec<_> = left_entities
        .intersection(&right_entities)
        .filter_map(|&entity| {
            let diff = diff_entity(left, right, entity, granularity);
            if diff.is_empty() { None } else { Some(diff) }
        })
        .collect();

    WorldDiff {
        left_only,
        right_only,
        modified,
    }
}

/// Checks if an entity has identical components in both worlds.
fn entities_equal(left: &World, right: &World, entity: EntityId) -> bool {
    let left_comps: HashSet<_> = left.entity_components(entity).iter().copied().collect();
    let right_comps: HashSet<_> = right.entity_components(entity).iter().copied().collect();

    if left_comps != right_comps {
        return false;
    }

    for comp in left_comps {
        let left_val = left.get(entity, comp).ok().flatten();
        let right_val = right.get(entity, comp).ok().flatten();
        if left_val != right_val {
            return false;
        }
    }

    true
}

/// Computes the diff for a single entity.
fn diff_entity(
    left: &World,
    right: &World,
    entity: EntityId,
    granularity: DiffGranularity,
) -> EntityDiff {
    let mut diff = EntityDiff::new(entity);

    let left_comps: HashSet<_> = left.entity_components(entity).iter().copied().collect();
    let right_comps: HashSet<_> = right.entity_components(entity).iter().copied().collect();

    // Components only in left (removed)
    for comp in left_comps.difference(&right_comps) {
        let old = if granularity == DiffGranularity::Field {
            left.get(entity, *comp).ok().flatten()
        } else {
            None
        };
        diff.changes.push(ValueChange::new(*comp, old, None));
    }

    // Components only in right (added)
    for comp in right_comps.difference(&left_comps) {
        let new = if granularity == DiffGranularity::Field {
            right.get(entity, *comp).ok().flatten()
        } else {
            None
        };
        diff.changes.push(ValueChange::new(*comp, None, new));
    }

    // Components in both (check for modifications)
    for comp in left_comps.intersection(&right_comps) {
        let left_val = left.get(entity, *comp).ok().flatten();
        let right_val = right.get(entity, *comp).ok().flatten();

        if left_val != right_val {
            let (old, new) = if granularity == DiffGranularity::Field {
                (left_val, right_val)
            } else {
                (None, None)
            };
            diff.changes.push(ValueChange::new(*comp, old, new));
        }
    }

    diff
}

/// Generates a summary of the diff.
#[must_use]
pub fn diff_summary(diff: &WorldDiff) -> String {
    use std::fmt::Write;
    let mut summary = String::new();

    if diff.is_empty() {
        return "No differences".to_string();
    }

    if !diff.left_only.is_empty() {
        let _ = writeln!(summary, "Removed entities: {}", diff.left_only.len());
    }

    if !diff.right_only.is_empty() {
        let _ = writeln!(summary, "Added entities: {}", diff.right_only.len());
    }

    if !diff.modified.is_empty() {
        let _ = writeln!(summary, "Modified entities: {}", diff.modified.len());

        let total_changes: usize = diff.modified.iter().map(|d| d.changes.len()).sum();
        if total_changes > 0 {
            let _ = writeln!(summary, "Total component changes: {total_changes}");
        }
    }

    summary.trim_end().to_string()
}

/// Formats a detailed diff report.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn format_diff(
    diff: &WorldDiff,
    interner: &longtable_foundation::Interner,
    max_entities: usize,
) -> String {
    use std::fmt::Write;
    let mut output = String::new();

    if diff.is_empty() {
        return "No differences".to_string();
    }

    // Removed entities
    if !diff.left_only.is_empty() {
        let _ = writeln!(output, "--- Removed ({}) ---", diff.left_only.len());
        for (i, entity) in diff.left_only.iter().take(max_entities).enumerate() {
            let _ = writeln!(output, "  {entity}");
            if i >= max_entities - 1 && diff.left_only.len() > max_entities {
                let _ = writeln!(
                    output,
                    "  ... and {} more",
                    diff.left_only.len() - max_entities
                );
                break;
            }
        }
        let _ = writeln!(output);
    }

    // Added entities
    if !diff.right_only.is_empty() {
        let _ = writeln!(output, "+++ Added ({}) +++", diff.right_only.len());
        for (i, entity) in diff.right_only.iter().take(max_entities).enumerate() {
            let _ = writeln!(output, "  {entity}");
            if i >= max_entities - 1 && diff.right_only.len() > max_entities {
                let _ = writeln!(
                    output,
                    "  ... and {} more",
                    diff.right_only.len() - max_entities
                );
                break;
            }
        }
        let _ = writeln!(output);
    }

    // Modified entities
    if !diff.modified.is_empty() {
        let _ = writeln!(output, "~~~ Modified ({}) ~~~", diff.modified.len());
        for (i, entity_diff) in diff.modified.iter().take(max_entities).enumerate() {
            let _ = writeln!(output, "  {}", entity_diff.entity);

            for change in &entity_diff.changes {
                let comp_name = interner.get_keyword(change.component).unwrap_or("?");

                match (&change.old, &change.new) {
                    (None, Some(new)) => {
                        let _ = writeln!(output, "    + :{comp_name} = {new}");
                    }
                    (Some(old), None) => {
                        let _ = writeln!(output, "    - :{comp_name} = {old}");
                    }
                    (Some(old), Some(new)) => {
                        let _ = writeln!(output, "    ~ :{comp_name}: {old} -> {new}");
                    }
                    (None, None) => {
                        let _ = writeln!(output, "    ~ :{comp_name} (changed)");
                    }
                }
            }

            if i >= max_entities - 1 && diff.modified.len() > max_entities {
                let _ = writeln!(
                    output,
                    "  ... and {} more",
                    diff.modified.len() - max_entities
                );
                break;
            }
        }
    }

    output.trim_end().to_string()
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use longtable_foundation::LtMap;
    use longtable_foundation::Type;
    use longtable_storage::schema::{ComponentSchema, FieldSchema};

    fn setup() -> (World, KeywordId, KeywordId, KeywordId) {
        let mut world = World::new(0);

        // Intern keywords
        let health = world.interner_mut().intern_keyword("health");
        let name = world.interner_mut().intern_keyword("name");
        let value_field = world.interner_mut().intern_keyword("value");

        // Register component schemas with value fields
        world = world
            .register_component(
                ComponentSchema::new(health)
                    .with_field(FieldSchema::required(value_field, Type::Int)),
            )
            .unwrap()
            .register_component(
                ComponentSchema::new(name)
                    .with_field(FieldSchema::required(value_field, Type::String)),
            )
            .unwrap();

        (world, health, name, value_field)
    }

    fn make_value_map(value_field: KeywordId, value: Value) -> Value {
        let mut map = longtable_foundation::LtMap::new();
        map = map.insert(Value::Keyword(value_field), value);
        Value::Map(map)
    }

    #[test]
    fn diff_identical_worlds() {
        let world1 = World::new(0);
        let world2 = World::new(0);

        let diff = diff_worlds(&world1, &world2, DiffGranularity::Field);
        assert!(diff.is_empty());
    }

    #[test]
    fn diff_entity_added() {
        let world1 = World::new(0);
        let (world2, _e1) = world1.spawn(&LtMap::new()).unwrap();

        let diff = diff_worlds(&world1, &world2, DiffGranularity::Entity);

        assert!(diff.left_only.is_empty());
        assert_eq!(diff.right_only.len(), 1);
        assert!(diff.modified.is_empty());
    }

    #[test]
    fn diff_entity_removed() {
        let world1 = World::new(0);
        let (world1, _e1) = world1.spawn(&LtMap::new()).unwrap();
        let world2 = World::new(0);

        let diff = diff_worlds(&world1, &world2, DiffGranularity::Entity);

        assert_eq!(diff.left_only.len(), 1);
        assert!(diff.right_only.is_empty());
        assert!(diff.modified.is_empty());
    }

    #[test]
    fn diff_component_changed() {
        let (world1, health, _name, value_field) = setup();

        let (world1, e1) = world1.spawn(&LtMap::new()).unwrap();
        let world1 = world1
            .set(e1, health, make_value_map(value_field, Value::Int(100)))
            .unwrap();

        let world2 = world1
            .clone()
            .set(e1, health, make_value_map(value_field, Value::Int(75)))
            .unwrap();

        let diff = diff_worlds(&world1, &world2, DiffGranularity::Field);

        assert!(diff.left_only.is_empty());
        assert!(diff.right_only.is_empty());
        assert_eq!(diff.modified.len(), 1);

        let entity_diff = &diff.modified[0];
        assert_eq!(entity_diff.entity, e1);
        assert_eq!(entity_diff.changes.len(), 1);
    }

    #[test]
    fn diff_component_added_to_entity() {
        let (world1, health, name, value_field) = setup();

        let (world1, e1) = world1.spawn(&LtMap::new()).unwrap();
        let world1 = world1
            .set(e1, health, make_value_map(value_field, Value::Int(100)))
            .unwrap();

        let world2 = world1
            .clone()
            .set(
                e1,
                name,
                make_value_map(value_field, Value::String("Player".into())),
            )
            .unwrap();

        let diff = diff_worlds(&world1, &world2, DiffGranularity::Field);

        assert_eq!(diff.modified.len(), 1);
        let entity_diff = &diff.modified[0];

        let added: Vec<_> = entity_diff
            .changes
            .iter()
            .filter(|c| c.is_added())
            .collect();
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].component, name);
    }

    #[test]
    fn value_change_predicates() {
        let (_world, health, _name, _value_field) = setup();

        let added = ValueChange::new(health, None, Some(Value::Int(100)));
        assert!(added.is_added());
        assert!(!added.is_removed());
        assert!(!added.is_modified());

        let removed = ValueChange::new(health, Some(Value::Int(100)), None);
        assert!(!removed.is_added());
        assert!(removed.is_removed());
        assert!(!removed.is_modified());

        let modified = ValueChange::new(health, Some(Value::Int(100)), Some(Value::Int(75)));
        assert!(!modified.is_added());
        assert!(!modified.is_removed());
        assert!(modified.is_modified());
    }

    #[test]
    fn diff_summary_output() {
        let (world1, health, _name, value_field) = setup();

        let (world1, e1) = world1.spawn(&LtMap::new()).unwrap();
        let world1 = world1
            .set(e1, health, make_value_map(value_field, Value::Int(100)))
            .unwrap();

        let world2 = world1
            .clone()
            .set(e1, health, make_value_map(value_field, Value::Int(75)))
            .unwrap();

        let diff = diff_worlds(&world1, &world2, DiffGranularity::Field);
        let summary = diff_summary(&diff);

        assert!(summary.contains("Modified entities: 1"));
        assert!(summary.contains("Total component changes: 1"));
    }
}
