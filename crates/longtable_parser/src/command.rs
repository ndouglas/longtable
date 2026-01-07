//! Command entity creation.
//!
//! Spawns command entities for the rule engine to process.

use std::collections::HashMap;

use longtable_foundation::{EntityId, KeywordId};

/// A parsed command ready to be spawned as an entity.
#[derive(Clone, Debug)]
pub struct CommandEntity {
    /// The canonical verb
    pub verb: KeywordId,
    /// The action to invoke
    pub action: KeywordId,
    /// Who issued the command
    pub actor: EntityId,
    /// Noun bindings (slot name -> entity)
    pub noun_bindings: HashMap<String, EntityId>,
    /// Adverb modifier, if any
    pub adverb: Option<KeywordId>,
}

/// Spawns command entities in the world.
pub struct CommandSpawner;

impl CommandSpawner {
    /// Spawns a command entity in the world.
    ///
    /// Command entity components:
    /// - `:command/verb` - the canonical verb
    /// - `:command/action` - action to invoke
    /// - `:command/actor` - who issued command
    /// - `:command/target` - direct object (if any)
    /// - `:command/instrument` - "with X" object (if any)
    /// - `:command/destination` - "to X" object (if any)
    /// - `:command/adverb` - adverb modifier (if any)
    #[allow(clippy::result_large_err)]
    pub fn spawn(
        _cmd: &CommandEntity,
        // world: World,
    ) -> longtable_foundation::Result<EntityId> {
        // TODO: Implement command spawning
        Ok(EntityId::new(0, 0))
    }
}
