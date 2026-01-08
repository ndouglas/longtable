//! Action registry and execution.
//!
//! Defines and executes game actions triggered by commands.

use std::collections::HashMap;

use longtable_foundation::{EntityId, KeywordId, Value};
use longtable_storage::World;

use crate::command::CommandKeywords;

/// A compiled action definition.
#[derive(Clone, Debug)]
pub struct CompiledAction {
    /// Action name
    pub name: KeywordId,
    /// Parameter variable names
    pub params: Vec<String>,
    /// Salience for rule ordering (higher = fires first)
    pub salience: i32,
    /// Preconditions to check before executing
    pub preconditions: Vec<CompiledPrecondition>,
    /// Effect handlers to execute
    pub handlers: Vec<CompiledHandler>,
}

impl CompiledAction {
    /// Creates a new action with the given name.
    #[must_use]
    pub fn new(name: KeywordId) -> Self {
        Self {
            name,
            params: Vec::new(),
            salience: 0,
            preconditions: Vec::new(),
            handlers: Vec::new(),
        }
    }

    /// Sets the parameter names.
    #[must_use]
    pub fn with_params(mut self, params: Vec<String>) -> Self {
        self.params = params;
        self
    }

    /// Sets the salience (priority).
    #[must_use]
    pub fn with_salience(mut self, salience: i32) -> Self {
        self.salience = salience;
        self
    }

    /// Adds a precondition.
    #[must_use]
    pub fn with_precondition(mut self, precondition: CompiledPrecondition) -> Self {
        self.preconditions.push(precondition);
        self
    }

    /// Adds an effect handler.
    #[must_use]
    pub fn with_handler(mut self, handler: CompiledHandler) -> Self {
        self.handlers.push(handler);
        self
    }
}

/// A compiled precondition check.
#[derive(Clone, Debug)]
pub enum CompiledPrecondition {
    /// Entity must have a component: [?target :component]
    HasComponent {
        /// Variable name for the entity
        entity_var: String,
        /// Component to check
        component: KeywordId,
    },
    /// Entity must NOT have a component: (not [?target :component])
    NotHasComponent {
        /// Variable name for the entity
        entity_var: String,
        /// Component to check
        component: KeywordId,
    },
    /// Entity must have field with specific value: [?target :component/field value]
    FieldEquals {
        /// Variable name for the entity
        entity_var: String,
        /// Component
        component: KeywordId,
        /// Field
        field: KeywordId,
        /// Expected value
        value: Value,
    },
    /// Custom check with failure message
    Custom {
        /// Description of the check (for debugging)
        description: String,
        /// Failure message to display
        failure_message: String,
        /// Check function (evaluated at runtime)
        check: CustomCheck,
    },
}

/// A custom precondition check function.
#[derive(Clone, Debug)]
pub enum CustomCheck {
    /// Actor must be able to reach the target
    CanReach {
        actor_var: String,
        target_var: String,
    },
    /// Target must be takeable
    IsTakeable { target_var: String },
    /// Target must be visible
    IsVisible { target_var: String },
    /// Container must be open
    IsOpen { container_var: String },
    /// Actor must be holding the item
    IsHolding { actor_var: String, item_var: String },
}

/// A compiled effect handler.
#[derive(Clone, Debug)]
pub enum CompiledHandler {
    /// Set a component field: (set! ?entity :component/field value)
    SetField {
        /// Entity variable
        entity_var: String,
        /// Component
        component: KeywordId,
        /// Field
        field: KeywordId,
        /// Value to set
        value: HandlerValue,
    },
    /// Add a tag component: (tag! ?entity :tag)
    AddTag {
        /// Entity variable
        entity_var: String,
        /// Tag component
        tag: KeywordId,
    },
    /// Remove a tag component: (untag! ?entity :tag)
    RemoveTag {
        /// Entity variable
        entity_var: String,
        /// Tag component
        tag: KeywordId,
    },
    /// Create a relationship: (link! ?source :relationship ?target)
    Link {
        /// Source entity variable
        source_var: String,
        /// Relationship type
        relationship: KeywordId,
        /// Target entity variable
        target_var: String,
    },
    /// Remove a relationship: (unlink! ?source :relationship ?target)
    Unlink {
        /// Source entity variable
        source_var: String,
        /// Relationship type
        relationship: KeywordId,
        /// Target entity variable
        target_var: String,
    },
    /// Print a message: (say! "message")
    Say {
        /// Message template (can include {var} placeholders)
        message: String,
    },
}

/// A value for a handler effect.
#[derive(Clone, Debug)]
pub enum HandlerValue {
    /// A literal value
    Literal(Value),
    /// A reference to a bound variable
    Variable(String),
    /// Current value plus delta
    Increment { var: String, delta: i64 },
    /// Current value minus delta
    Decrement { var: String, delta: i64 },
}

/// Result of checking an action's preconditions.
#[derive(Clone, Debug)]
pub enum PreconditionResult {
    /// All preconditions passed
    Pass,
    /// A precondition failed with a message
    Fail {
        /// The failure message to display
        message: String,
    },
}

/// Registry of all defined actions.
#[derive(Clone, Debug, Default)]
pub struct ActionRegistry {
    actions: HashMap<KeywordId, CompiledAction>,
}

impl ActionRegistry {
    /// Creates a new empty action registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an action.
    pub fn register(&mut self, action: CompiledAction) {
        self.actions.insert(action.name, action);
    }

    /// Looks up an action by name.
    #[must_use]
    pub fn get(&self, name: KeywordId) -> Option<&CompiledAction> {
        self.actions.get(&name)
    }

    /// Returns all registered actions.
    pub fn actions(&self) -> impl Iterator<Item = &CompiledAction> {
        self.actions.values()
    }
}

/// Bindings for action execution.
#[derive(Clone, Debug, Default)]
pub struct ActionBindings {
    /// Variable name -> EntityId
    entities: HashMap<String, EntityId>,
    /// Variable name -> Value
    values: HashMap<String, Value>,
}

impl ActionBindings {
    /// Creates empty bindings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds an entity to a variable.
    pub fn bind_entity(&mut self, var: impl Into<String>, entity: EntityId) {
        self.entities.insert(var.into(), entity);
    }

    /// Binds a value to a variable.
    pub fn bind_value(&mut self, var: impl Into<String>, value: Value) {
        self.values.insert(var.into(), value);
    }

    /// Gets the entity bound to a variable.
    #[must_use]
    pub fn get_entity(&self, var: &str) -> Option<EntityId> {
        self.entities.get(var).copied()
    }

    /// Gets the value bound to a variable.
    #[must_use]
    pub fn get_value(&self, var: &str) -> Option<&Value> {
        self.values.get(var)
    }

    /// Creates bindings from a command entity.
    #[must_use]
    pub fn from_command(
        command_entity: EntityId,
        world: &World,
        keywords: &CommandKeywords,
    ) -> Option<Self> {
        let mut bindings = Self::new();

        // Bind the command entity itself
        bindings.bind_entity("command", command_entity);

        // Get actor from command
        if let Ok(Some(Value::Map(map))) = world.get(command_entity, keywords.actor) {
            if let Some(Value::EntityRef(actor)) = map.get(&Value::Keyword(keywords.value_field)) {
                bindings.bind_entity("actor", *actor);
            }
        }

        // Get target from command (if present)
        if let Ok(Some(Value::Map(map))) = world.get(command_entity, keywords.target) {
            if let Some(Value::EntityRef(target)) = map.get(&Value::Keyword(keywords.value_field)) {
                bindings.bind_entity("target", *target);
            }
        }

        // Get instrument from command (if present)
        if let Ok(Some(Value::Map(map))) = world.get(command_entity, keywords.instrument) {
            if let Some(Value::EntityRef(instrument)) =
                map.get(&Value::Keyword(keywords.value_field))
            {
                bindings.bind_entity("instrument", *instrument);
            }
        }

        // Get destination from command (if present)
        if let Ok(Some(Value::Map(map))) = world.get(command_entity, keywords.destination) {
            if let Some(Value::EntityRef(destination)) =
                map.get(&Value::Keyword(keywords.value_field))
            {
                bindings.bind_entity("destination", *destination);
            }
        }

        Some(bindings)
    }
}

/// Executes actions against the world.
#[derive(Clone, Debug)]
pub struct ActionExecutor {
    /// Messages produced during execution
    messages: Vec<String>,
}

impl Default for ActionExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionExecutor {
    /// Creates a new action executor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    /// Gets messages produced during execution.
    #[must_use]
    pub fn messages(&self) -> &[String] {
        &self.messages
    }

    /// Takes the messages, leaving an empty vec.
    pub fn take_messages(&mut self) -> Vec<String> {
        std::mem::take(&mut self.messages)
    }

    /// Checks all preconditions for an action.
    #[must_use]
    pub fn check_preconditions(
        &self,
        action: &CompiledAction,
        bindings: &ActionBindings,
        world: &World,
    ) -> PreconditionResult {
        for precondition in &action.preconditions {
            match self.check_single_precondition(precondition, bindings, world) {
                PreconditionResult::Pass => continue,
                fail => return fail,
            }
        }
        PreconditionResult::Pass
    }

    /// Checks a single precondition.
    fn check_single_precondition(
        &self,
        precondition: &CompiledPrecondition,
        bindings: &ActionBindings,
        world: &World,
    ) -> PreconditionResult {
        match precondition {
            CompiledPrecondition::HasComponent {
                entity_var,
                component,
            } => {
                if let Some(entity) = bindings.get_entity(entity_var) {
                    if world.has(entity, *component) {
                        return PreconditionResult::Pass;
                    }
                }
                PreconditionResult::Fail {
                    message: "Entity doesn't have required component".to_string(),
                }
            }
            CompiledPrecondition::NotHasComponent {
                entity_var,
                component,
            } => {
                if let Some(entity) = bindings.get_entity(entity_var) {
                    if !world.has(entity, *component) {
                        return PreconditionResult::Pass;
                    }
                }
                PreconditionResult::Fail {
                    message: "Entity shouldn't have this component".to_string(),
                }
            }
            CompiledPrecondition::FieldEquals {
                entity_var,
                component,
                field,
                value,
            } => {
                if let Some(entity) = bindings.get_entity(entity_var) {
                    if let Ok(Some(actual)) = world.get_field(entity, *component, *field) {
                        if &actual == value {
                            return PreconditionResult::Pass;
                        }
                    }
                }
                PreconditionResult::Fail {
                    message: "Field doesn't match expected value".to_string(),
                }
            }
            CompiledPrecondition::Custom {
                failure_message, ..
            } => {
                // Custom checks would need runtime evaluation
                // For now, just pass
                PreconditionResult::Fail {
                    message: failure_message.clone(),
                }
            }
        }
    }

    /// Executes all handlers for an action.
    #[allow(clippy::result_large_err)]
    pub fn execute(
        &mut self,
        action: &CompiledAction,
        bindings: &ActionBindings,
        world: World,
    ) -> longtable_foundation::Result<World> {
        let mut current_world = world;

        for handler in &action.handlers {
            current_world = self.execute_handler(handler, bindings, current_world)?;
        }

        Ok(current_world)
    }

    /// Executes a single handler.
    #[allow(clippy::result_large_err)]
    fn execute_handler(
        &mut self,
        handler: &CompiledHandler,
        bindings: &ActionBindings,
        world: World,
    ) -> longtable_foundation::Result<World> {
        match handler {
            CompiledHandler::SetField {
                entity_var,
                component,
                field,
                value,
            } => {
                if let Some(entity) = bindings.get_entity(entity_var) {
                    let resolved_value = self.resolve_value(value, bindings, &world)?;
                    return world.set_field(entity, *component, *field, resolved_value);
                }
                Ok(world)
            }
            CompiledHandler::AddTag { entity_var, tag } => {
                if let Some(entity) = bindings.get_entity(entity_var) {
                    return world.set(entity, *tag, Value::Bool(true));
                }
                Ok(world)
            }
            CompiledHandler::RemoveTag { entity_var, tag } => {
                if let Some(entity) = bindings.get_entity(entity_var) {
                    // "Remove" by setting to false (World has no component removal API)
                    return world.set(entity, *tag, Value::Bool(false));
                }
                Ok(world)
            }
            CompiledHandler::Link {
                source_var,
                relationship,
                target_var,
            } => {
                if let (Some(source), Some(target)) = (
                    bindings.get_entity(source_var),
                    bindings.get_entity(target_var),
                ) {
                    return world.link(source, *relationship, target);
                }
                Ok(world)
            }
            CompiledHandler::Unlink {
                source_var,
                relationship,
                target_var,
            } => {
                if let (Some(source), Some(target)) = (
                    bindings.get_entity(source_var),
                    bindings.get_entity(target_var),
                ) {
                    return world.unlink(source, *relationship, target);
                }
                Ok(world)
            }
            CompiledHandler::Say { message } => {
                // Expand message template with bindings
                let expanded = self.expand_message(message, bindings);
                self.messages.push(expanded);
                Ok(world)
            }
        }
    }

    /// Resolves a handler value to an actual Value.
    #[allow(clippy::result_large_err)]
    fn resolve_value(
        &self,
        handler_value: &HandlerValue,
        bindings: &ActionBindings,
        _world: &World,
    ) -> longtable_foundation::Result<Value> {
        match handler_value {
            HandlerValue::Literal(v) => Ok(v.clone()),
            HandlerValue::Variable(var) => {
                if let Some(entity) = bindings.get_entity(var) {
                    Ok(Value::EntityRef(entity))
                } else if let Some(value) = bindings.get_value(var) {
                    Ok(value.clone())
                } else {
                    Ok(Value::Nil)
                }
            }
            HandlerValue::Increment { var, delta } => {
                if let Some(Value::Int(n)) = bindings.get_value(var) {
                    Ok(Value::Int(n + delta))
                } else {
                    Ok(Value::Int(*delta))
                }
            }
            HandlerValue::Decrement { var, delta } => {
                if let Some(Value::Int(n)) = bindings.get_value(var) {
                    Ok(Value::Int(n - delta))
                } else {
                    Ok(Value::Int(-delta))
                }
            }
        }
    }

    /// Expands a message template with variable bindings.
    fn expand_message(&self, template: &str, bindings: &ActionBindings) -> String {
        let mut result = template.to_string();

        // Simple placeholder expansion: {var} -> value
        for (var, entity) in &bindings.entities {
            let placeholder = format!("{{{var}}}");
            let replacement = format!("entity-{}", entity.index);
            result = result.replace(&placeholder, &replacement);
        }

        for (var, value) in &bindings.values {
            let placeholder = format!("{{{var}}}");
            let replacement = format!("{value:?}");
            result = result.replace(&placeholder, &replacement);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use longtable_foundation::LtMap;
    use longtable_storage::schema::ComponentSchema;

    #[test]
    fn test_action_registry() {
        let mut registry = ActionRegistry::new();

        let action = CompiledAction::new(KeywordId::REL_TYPE)
            .with_salience(10)
            .with_params(vec!["actor".to_string(), "target".to_string()]);

        registry.register(action);

        assert!(registry.get(KeywordId::REL_TYPE).is_some());
        assert!(registry.get(KeywordId::REL_SOURCE).is_none());
    }

    #[test]
    fn test_action_bindings() {
        let mut bindings = ActionBindings::new();
        let entity = EntityId::new(1, 0);

        bindings.bind_entity("actor", entity);
        bindings.bind_value("count", Value::Int(5));

        assert_eq!(bindings.get_entity("actor"), Some(entity));
        assert_eq!(bindings.get_value("count"), Some(&Value::Int(5)));
        assert_eq!(bindings.get_entity("missing"), None);
    }

    #[test]
    fn test_precondition_has_component() {
        let mut world = World::new(42);
        let tag_kw = world.interner_mut().intern_keyword("tag/player");

        world = world
            .register_component(ComponentSchema::tag(tag_kw))
            .unwrap();

        let (world, entity) = world.spawn(&LtMap::new()).unwrap();
        let world = world.set(entity, tag_kw, Value::Bool(true)).unwrap();

        let mut bindings = ActionBindings::new();
        bindings.bind_entity("target", entity);

        let precondition = CompiledPrecondition::HasComponent {
            entity_var: "target".to_string(),
            component: tag_kw,
        };

        let executor = ActionExecutor::new();
        let result = executor.check_single_precondition(&precondition, &bindings, &world);

        assert!(matches!(result, PreconditionResult::Pass));
    }

    #[test]
    fn test_precondition_not_has_component() {
        let mut world = World::new(42);
        let tag_kw = world.interner_mut().intern_keyword("tag/locked");

        world = world
            .register_component(ComponentSchema::tag(tag_kw))
            .unwrap();

        let (world, entity) = world.spawn(&LtMap::new()).unwrap();
        // Entity does NOT have the tag

        let mut bindings = ActionBindings::new();
        bindings.bind_entity("target", entity);

        let precondition = CompiledPrecondition::NotHasComponent {
            entity_var: "target".to_string(),
            component: tag_kw,
        };

        let executor = ActionExecutor::new();
        let result = executor.check_single_precondition(&precondition, &bindings, &world);

        assert!(matches!(result, PreconditionResult::Pass));
    }

    #[test]
    fn test_execute_say_handler() {
        let world = World::new(42);

        let mut bindings = ActionBindings::new();
        let entity = EntityId::new(1, 0);
        bindings.bind_entity("actor", entity);

        let handler = CompiledHandler::Say {
            message: "Hello from {actor}!".to_string(),
        };

        let mut executor = ActionExecutor::new();
        let _result = executor.execute_handler(&handler, &bindings, world);

        let messages = executor.take_messages();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("entity-1"));
    }
}
