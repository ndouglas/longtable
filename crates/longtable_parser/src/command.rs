//! Command entity creation.
//!
//! Spawns command entities for the rule engine to process.

use std::collections::HashMap;

use longtable_foundation::{EntityId, KeywordId, LtMap, Type, Value};
use longtable_storage::World;
use longtable_storage::schema::{ComponentSchema, FieldSchema};

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
    /// Direction binding (variable name -> direction keyword), if any
    pub direction: Option<(String, KeywordId)>,
    /// Adverb modifier, if any
    pub adverb: Option<KeywordId>,
}

/// Configuration for command entity component keywords.
#[derive(Clone, Debug)]
pub struct CommandKeywords {
    /// Keyword for `:command/verb`
    pub verb: KeywordId,
    /// Keyword for `:command/action`
    pub action: KeywordId,
    /// Keyword for `:command/actor`
    pub actor: KeywordId,
    /// Keyword for `:command/target`
    pub target: KeywordId,
    /// Keyword for `:command/instrument`
    pub instrument: KeywordId,
    /// Keyword for `:command/destination`
    pub destination: KeywordId,
    /// Keyword for `:command/adverb`
    pub adverb: KeywordId,
    /// Keyword for the `:value` field in component maps
    pub value_field: KeywordId,
}

/// Spawns command entities in the world.
#[derive(Clone, Debug)]
pub struct CommandSpawner {
    /// Keyword configuration
    keywords: CommandKeywords,
}

impl CommandSpawner {
    /// Creates a new command spawner with the given keyword configuration.
    #[must_use]
    pub fn new(keywords: CommandKeywords) -> Self {
        Self { keywords }
    }

    /// Registers the required component schemas on the world.
    ///
    /// This must be called before spawning commands.
    #[allow(clippy::result_large_err)]
    pub fn register_schemas(&self, world: &World) -> longtable_foundation::Result<World> {
        let value_kw = self.keywords.value_field;

        // Register command verb schema (keyword value)
        let verb_schema = ComponentSchema::new(self.keywords.verb)
            .with_field(FieldSchema::required(value_kw, Type::Keyword));
        let world = world.register_component(verb_schema)?;

        // Register command action schema (keyword value)
        let action_schema = ComponentSchema::new(self.keywords.action)
            .with_field(FieldSchema::required(value_kw, Type::Keyword));
        let world = world.register_component(action_schema)?;

        // Register command actor schema (entity ref value)
        let actor_schema = ComponentSchema::new(self.keywords.actor)
            .with_field(FieldSchema::required(value_kw, Type::EntityRef));
        let world = world.register_component(actor_schema)?;

        // Register optional noun slot schemas
        let target_schema = ComponentSchema::new(self.keywords.target)
            .with_field(FieldSchema::required(value_kw, Type::EntityRef));
        let world = world.register_component(target_schema)?;

        let instrument_schema = ComponentSchema::new(self.keywords.instrument)
            .with_field(FieldSchema::required(value_kw, Type::EntityRef));
        let world = world.register_component(instrument_schema)?;

        let destination_schema = ComponentSchema::new(self.keywords.destination)
            .with_field(FieldSchema::required(value_kw, Type::EntityRef));
        let world = world.register_component(destination_schema)?;

        // Register adverb schema (keyword value)
        let adverb_schema = ComponentSchema::new(self.keywords.adverb)
            .with_field(FieldSchema::required(value_kw, Type::Keyword));
        let world = world.register_component(adverb_schema)?;

        Ok(world)
    }

    /// Creates a component map value with a single `:value` field.
    fn make_component_value(&self, value: Value) -> Value {
        let mut map = LtMap::new();
        map = map.insert(Value::Keyword(self.keywords.value_field), value);
        Value::Map(map)
    }

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
        &self,
        cmd: &CommandEntity,
        world: &World,
    ) -> longtable_foundation::Result<(World, EntityId)> {
        // Build component map
        let mut components = LtMap::new();

        // Required components (stored as maps with :value field)
        components = components.insert(
            Value::Keyword(self.keywords.verb),
            self.make_component_value(Value::Keyword(cmd.verb)),
        );
        components = components.insert(
            Value::Keyword(self.keywords.action),
            self.make_component_value(Value::Keyword(cmd.action)),
        );
        components = components.insert(
            Value::Keyword(self.keywords.actor),
            self.make_component_value(Value::EntityRef(cmd.actor)),
        );

        // Optional noun bindings
        if let Some(&target) = cmd.noun_bindings.get("target") {
            components = components.insert(
                Value::Keyword(self.keywords.target),
                self.make_component_value(Value::EntityRef(target)),
            );
        }
        if let Some(&instrument) = cmd.noun_bindings.get("instrument") {
            components = components.insert(
                Value::Keyword(self.keywords.instrument),
                self.make_component_value(Value::EntityRef(instrument)),
            );
        }
        if let Some(&destination) = cmd.noun_bindings.get("destination") {
            components = components.insert(
                Value::Keyword(self.keywords.destination),
                self.make_component_value(Value::EntityRef(destination)),
            );
        }

        // Optional adverb
        if let Some(adverb) = cmd.adverb {
            components = components.insert(
                Value::Keyword(self.keywords.adverb),
                self.make_component_value(Value::Keyword(adverb)),
            );
        }

        // Spawn entity with components
        world.spawn(&components)
    }

    /// Spawns multiple command entities (for "all" quantifier).
    ///
    /// Returns the final world state and all spawned entity IDs.
    #[allow(clippy::result_large_err)]
    pub fn spawn_multiple(
        &self,
        cmds: &[CommandEntity],
        world: &World,
    ) -> longtable_foundation::Result<(World, Vec<EntityId>)> {
        let mut current_world = world.clone();
        let mut ids = Vec::with_capacity(cmds.len());

        for cmd in cmds {
            let (new_world, id) = self.spawn(cmd, &current_world)?;
            current_world = new_world;
            ids.push(id);
        }

        Ok((current_world, ids))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_world_with_keywords() -> (World, CommandKeywords) {
        let mut world = World::new(42);

        // Intern keywords for command components
        let verb_kw = world.interner_mut().intern_keyword("command/verb");
        let action_kw = world.interner_mut().intern_keyword("command/action");
        let actor_kw = world.interner_mut().intern_keyword("command/actor");
        let target_kw = world.interner_mut().intern_keyword("command/target");
        let instrument_kw = world.interner_mut().intern_keyword("command/instrument");
        let destination_kw = world.interner_mut().intern_keyword("command/destination");
        let adverb_kw = world.interner_mut().intern_keyword("command/adverb");
        let value_kw = world.interner_mut().intern_keyword("value");

        let keywords = CommandKeywords {
            verb: verb_kw,
            action: action_kw,
            actor: actor_kw,
            target: target_kw,
            instrument: instrument_kw,
            destination: destination_kw,
            adverb: adverb_kw,
            value_field: value_kw,
        };

        (world, keywords)
    }

    #[test]
    fn test_spawn_simple_command() {
        let (world, keywords) = setup_world_with_keywords();
        let spawner = CommandSpawner::new(keywords.clone());

        // Register schemas
        let world = spawner.register_schemas(&world).unwrap();

        // Spawn an actor entity first
        let (world, actor) = world.spawn(&LtMap::new()).unwrap();

        // Create and spawn command
        let cmd = CommandEntity {
            verb: keywords.verb, // Use a valid keyword
            action: keywords.action,
            actor,
            noun_bindings: HashMap::new(),
            direction: None,
            adverb: None,
        };

        let result = spawner.spawn(&cmd, &world);
        assert!(result.is_ok());

        let (new_world, entity_id) = result.unwrap();
        assert!(new_world.exists(entity_id));

        // Verify components were set
        assert!(new_world.has(entity_id, keywords.verb));
        assert!(new_world.has(entity_id, keywords.action));
        assert!(new_world.has(entity_id, keywords.actor));
    }

    #[test]
    fn test_spawn_command_with_target() {
        let (world, keywords) = setup_world_with_keywords();
        let spawner = CommandSpawner::new(keywords.clone());

        // Register schemas
        let world = spawner.register_schemas(&world).unwrap();

        // Spawn actor and target entities
        let (world, actor) = world.spawn(&LtMap::new()).unwrap();
        let (world, target) = world.spawn(&LtMap::new()).unwrap();

        let mut bindings = HashMap::new();
        bindings.insert("target".to_string(), target);

        let cmd = CommandEntity {
            verb: keywords.verb,
            action: keywords.action,
            actor,
            noun_bindings: bindings,
            direction: None,
            adverb: None,
        };

        let result = spawner.spawn(&cmd, &world);
        assert!(result.is_ok());

        let (new_world, entity_id) = result.unwrap();
        assert!(new_world.has(entity_id, keywords.target));
    }

    #[test]
    fn test_spawn_multiple_commands() {
        let (world, keywords) = setup_world_with_keywords();
        let spawner = CommandSpawner::new(keywords.clone());

        // Register schemas
        let world = spawner.register_schemas(&world).unwrap();

        // Spawn actor entity
        let (world, actor) = world.spawn(&LtMap::new()).unwrap();

        let cmds = vec![
            CommandEntity {
                verb: keywords.verb,
                action: keywords.action,
                actor,
                noun_bindings: HashMap::new(),
                direction: None,
                adverb: None,
            },
            CommandEntity {
                verb: keywords.verb,
                action: keywords.action,
                actor,
                noun_bindings: HashMap::new(),
                direction: None,
                adverb: None,
            },
        ];

        let result = spawner.spawn_multiple(&cmds, &world);
        assert!(result.is_ok());

        let (new_world, ids) = result.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(new_world.exists(ids[0]));
        assert!(new_world.exists(ids[1]));
    }
}
