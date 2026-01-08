//! Action system tests.
//!
//! Tests for action registration, precondition checking, and handler execution.

use longtable_foundation::{LtMap, Type, Value};
use longtable_parser::action::{
    ActionBindings, ActionExecutor, ActionRegistry, CompiledAction, CompiledHandler,
    CompiledPrecondition, HandlerValue, PreconditionResult,
};
use longtable_storage::{ComponentSchema, FieldSchema, World};

fn setup_action_world() -> (World, ActionSetup) {
    let mut world = World::new(42);

    // Intern keywords
    let health_kw = world.interner_mut().intern_keyword("health");
    let current_kw = world.interner_mut().intern_keyword("current");
    let max_kw = world.interner_mut().intern_keyword("max");
    let takeable_kw = world.interner_mut().intern_keyword("takeable");
    let alive_kw = world.interner_mut().intern_keyword("alive");
    let inventory_kw = world.interner_mut().intern_keyword("inventory");
    let take_action_kw = world.interner_mut().intern_keyword("action/take");
    let heal_action_kw = world.interner_mut().intern_keyword("action/heal");

    // Register component schemas
    let world = world
        .register_component(
            ComponentSchema::new(health_kw)
                .with_field(FieldSchema::required(current_kw, Type::Int))
                .with_field(FieldSchema::required(max_kw, Type::Int)),
        )
        .unwrap();

    let world = world
        .register_component(ComponentSchema::tag(takeable_kw))
        .unwrap();

    let world = world
        .register_component(ComponentSchema::tag(alive_kw))
        .unwrap();

    // Create player
    let (world, player) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(player, health_kw, current_kw, Value::Int(50))
        .unwrap();
    let world = world
        .set_field(player, health_kw, max_kw, Value::Int(100))
        .unwrap();
    let world = world.set(player, alive_kw, Value::Bool(true)).unwrap();

    // Create item (sword)
    let (world, sword) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(sword, takeable_kw, Value::Bool(true)).unwrap();

    // Create non-takeable item
    let (world, statue) = world.spawn(&LtMap::new()).unwrap();

    let setup = ActionSetup {
        player,
        sword,
        statue,
        health_kw,
        current_kw,
        max_kw,
        takeable_kw,
        alive_kw,
        inventory_kw,
        take_action_kw,
        heal_action_kw,
    };

    (world, setup)
}

#[allow(dead_code)]
struct ActionSetup {
    player: longtable_foundation::EntityId,
    sword: longtable_foundation::EntityId,
    statue: longtable_foundation::EntityId,
    health_kw: longtable_foundation::KeywordId,
    current_kw: longtable_foundation::KeywordId,
    max_kw: longtable_foundation::KeywordId,
    takeable_kw: longtable_foundation::KeywordId,
    alive_kw: longtable_foundation::KeywordId,
    inventory_kw: longtable_foundation::KeywordId,
    take_action_kw: longtable_foundation::KeywordId,
    heal_action_kw: longtable_foundation::KeywordId,
}

// =============================================================================
// Precondition Tests via Action Execution
// =============================================================================

#[test]
fn precondition_has_component_passes() {
    let (world, setup) = setup_action_world();
    let executor = ActionExecutor::new();

    let action = CompiledAction::new(setup.take_action_kw).with_precondition(
        CompiledPrecondition::HasComponent {
            entity_var: "target".to_string(),
            component: setup.takeable_kw,
        },
    );

    let mut bindings = ActionBindings::new();
    bindings.bind_entity("target", setup.sword);

    let result = executor.check_preconditions(&action, &bindings, &world);

    assert!(matches!(result, PreconditionResult::Pass));
}

#[test]
fn precondition_has_component_fails() {
    let (world, setup) = setup_action_world();
    let executor = ActionExecutor::new();

    let action = CompiledAction::new(setup.take_action_kw).with_precondition(
        CompiledPrecondition::HasComponent {
            entity_var: "target".to_string(),
            component: setup.takeable_kw,
        },
    );

    let mut bindings = ActionBindings::new();
    bindings.bind_entity("target", setup.statue); // Statue is not takeable

    let result = executor.check_preconditions(&action, &bindings, &world);

    if let PreconditionResult::Fail { message } = result {
        assert!(!message.is_empty());
    } else {
        panic!("Expected Fail, got {result:?}");
    }
}

#[test]
fn precondition_not_has_component_passes() {
    let (world, setup) = setup_action_world();
    let executor = ActionExecutor::new();

    let action = CompiledAction::new(setup.take_action_kw).with_precondition(
        CompiledPrecondition::NotHasComponent {
            entity_var: "target".to_string(),
            component: setup.takeable_kw,
        },
    );

    let mut bindings = ActionBindings::new();
    bindings.bind_entity("target", setup.statue); // Statue is not takeable

    let result = executor.check_preconditions(&action, &bindings, &world);

    assert!(matches!(result, PreconditionResult::Pass));
}

#[test]
fn precondition_field_equals_passes() {
    let (world, setup) = setup_action_world();
    let executor = ActionExecutor::new();

    let action = CompiledAction::new(setup.heal_action_kw).with_precondition(
        CompiledPrecondition::FieldEquals {
            entity_var: "actor".to_string(),
            component: setup.health_kw,
            field: setup.current_kw,
            value: Value::Int(50),
        },
    );

    let mut bindings = ActionBindings::new();
    bindings.bind_entity("actor", setup.player);

    let result = executor.check_preconditions(&action, &bindings, &world);

    assert!(matches!(result, PreconditionResult::Pass));
}

#[test]
fn precondition_field_equals_fails() {
    let (world, setup) = setup_action_world();
    let executor = ActionExecutor::new();

    let action = CompiledAction::new(setup.heal_action_kw).with_precondition(
        CompiledPrecondition::FieldEquals {
            entity_var: "actor".to_string(),
            component: setup.health_kw,
            field: setup.current_kw,
            value: Value::Int(100), // Player has 50, not 100
        },
    );

    let mut bindings = ActionBindings::new();
    bindings.bind_entity("actor", setup.player);

    let result = executor.check_preconditions(&action, &bindings, &world);

    assert!(matches!(result, PreconditionResult::Fail { .. }));
}

// =============================================================================
// Handler Tests via Action Execution
// =============================================================================

#[test]
fn handler_set_field() {
    let (world, setup) = setup_action_world();
    let mut executor = ActionExecutor::new();

    let action =
        CompiledAction::new(setup.heal_action_kw).with_handler(CompiledHandler::SetField {
            entity_var: "actor".to_string(),
            component: setup.health_kw,
            field: setup.current_kw,
            value: HandlerValue::Literal(Value::Int(75)),
        });

    let mut bindings = ActionBindings::new();
    bindings.bind_entity("actor", setup.player);

    let world = executor.execute(&action, &bindings, world).unwrap();

    let health = world
        .get_field(setup.player, setup.health_kw, setup.current_kw)
        .unwrap();
    assert_eq!(health, Some(Value::Int(75)));
}

#[test]
fn handler_add_tag() {
    let (world, setup) = setup_action_world();
    let mut executor = ActionExecutor::new();

    let action = CompiledAction::new(setup.take_action_kw).with_handler(CompiledHandler::AddTag {
        entity_var: "target".to_string(),
        tag: setup.alive_kw,
    });

    let mut bindings = ActionBindings::new();
    bindings.bind_entity("target", setup.sword);

    let world = executor.execute(&action, &bindings, world).unwrap();

    assert!(world.has(setup.sword, setup.alive_kw));
}

#[test]
fn handler_remove_tag_on_tag_schema_fails() {
    let (world, setup) = setup_action_world();
    let mut executor = ActionExecutor::new();

    let action =
        CompiledAction::new(setup.heal_action_kw).with_handler(CompiledHandler::RemoveTag {
            entity_var: "actor".to_string(),
            tag: setup.alive_kw,
        });

    let mut bindings = ActionBindings::new();
    bindings.bind_entity("actor", setup.player);

    // Player starts with alive tag set to true
    assert!(world.has(setup.player, setup.alive_kw));

    // RemoveTag on a tag schema fails because tags only accept Bool(true)
    // The World storage doesn't have a component removal API, so setting
    // a tag to Bool(false) produces a type mismatch error.
    let result = executor.execute(&action, &bindings, world);

    // Expected to fail: tags can only be `true` or a Map, not `false`
    assert!(result.is_err());
}

#[test]
fn handler_say_produces_message() {
    let (world, setup) = setup_action_world();
    let mut executor = ActionExecutor::new();

    let action = CompiledAction::new(setup.take_action_kw).with_handler(CompiledHandler::Say {
        message: "You pick up the item.".to_string(),
    });

    let bindings = ActionBindings::new();

    let _world = executor.execute(&action, &bindings, world).unwrap();

    let messages = executor.messages();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0], "You pick up the item.");
}

// =============================================================================
// Action Registry Tests
// =============================================================================

#[test]
fn register_and_get_action() {
    let (_, setup) = setup_action_world();
    let mut registry = ActionRegistry::new();

    let action = CompiledAction::new(setup.take_action_kw)
        .with_salience(100)
        .with_params(vec!["target".to_string()])
        .with_precondition(CompiledPrecondition::HasComponent {
            entity_var: "target".to_string(),
            component: setup.takeable_kw,
        })
        .with_handler(CompiledHandler::Say {
            message: "You take the item.".to_string(),
        });

    registry.register(action);

    let found = registry.get(setup.take_action_kw);
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, setup.take_action_kw);
    assert_eq!(found.unwrap().salience, 100);
}

#[test]
fn action_with_multiple_preconditions() {
    let (world, setup) = setup_action_world();
    let executor = ActionExecutor::new();

    let action = CompiledAction::new(setup.take_action_kw)
        .with_params(vec!["actor".to_string(), "target".to_string()])
        .with_precondition(CompiledPrecondition::HasComponent {
            entity_var: "actor".to_string(),
            component: setup.alive_kw,
        })
        .with_precondition(CompiledPrecondition::HasComponent {
            entity_var: "target".to_string(),
            component: setup.takeable_kw,
        });

    let mut bindings = ActionBindings::new();
    bindings.bind_entity("actor", setup.player);
    bindings.bind_entity("target", setup.sword);

    let result = executor.check_preconditions(&action, &bindings, &world);

    assert!(matches!(result, PreconditionResult::Pass));
}

#[test]
fn action_precondition_fails_on_first_failure() {
    let (world, setup) = setup_action_world();
    let executor = ActionExecutor::new();

    let action = CompiledAction::new(setup.take_action_kw)
        .with_params(vec!["actor".to_string(), "target".to_string()])
        .with_precondition(CompiledPrecondition::HasComponent {
            entity_var: "actor".to_string(),
            component: setup.alive_kw,
        })
        .with_precondition(CompiledPrecondition::HasComponent {
            entity_var: "target".to_string(),
            component: setup.takeable_kw,
        });

    let mut bindings = ActionBindings::new();
    bindings.bind_entity("actor", setup.player);
    bindings.bind_entity("target", setup.statue); // Statue not takeable

    let result = executor.check_preconditions(&action, &bindings, &world);

    assert!(matches!(result, PreconditionResult::Fail { .. }));
}

// =============================================================================
// Execute Full Action Tests
// =============================================================================

#[test]
fn execute_action_success() {
    let (world, setup) = setup_action_world();
    let mut executor = ActionExecutor::new();

    let action = CompiledAction::new(setup.heal_action_kw)
        .with_params(vec!["actor".to_string()])
        .with_precondition(CompiledPrecondition::HasComponent {
            entity_var: "actor".to_string(),
            component: setup.alive_kw,
        })
        .with_handler(CompiledHandler::SetField {
            entity_var: "actor".to_string(),
            component: setup.health_kw,
            field: setup.current_kw,
            value: HandlerValue::Literal(Value::Int(100)),
        });

    let mut bindings = ActionBindings::new();
    bindings.bind_entity("actor", setup.player);

    // Check preconditions first
    let check = executor.check_preconditions(&action, &bindings, &world);
    assert!(matches!(check, PreconditionResult::Pass));

    // Execute action
    let world = executor.execute(&action, &bindings, world).unwrap();

    // Verify health was updated
    let health = world
        .get_field(setup.player, setup.health_kw, setup.current_kw)
        .unwrap();
    assert_eq!(health, Some(Value::Int(100)));
}

// =============================================================================
// Handler Value Resolution Tests
// =============================================================================

#[test]
fn handler_value_from_variable() {
    let (world, setup) = setup_action_world();
    let mut executor = ActionExecutor::new();

    let action =
        CompiledAction::new(setup.heal_action_kw).with_handler(CompiledHandler::SetField {
            entity_var: "target".to_string(),
            component: setup.health_kw,
            field: setup.current_kw,
            value: HandlerValue::Variable("health_value".to_string()),
        });

    let mut bindings = ActionBindings::new();
    bindings.bind_entity("target", setup.player);
    bindings.bind_value("health_value", Value::Int(75));

    let world = executor.execute(&action, &bindings, world).unwrap();

    let health = world
        .get_field(setup.player, setup.health_kw, setup.current_kw)
        .unwrap();
    assert_eq!(health, Some(Value::Int(75)));
}

#[test]
fn handler_value_increment() {
    let (world, setup) = setup_action_world();
    let mut executor = ActionExecutor::new();

    let action =
        CompiledAction::new(setup.heal_action_kw).with_handler(CompiledHandler::SetField {
            entity_var: "target".to_string(),
            component: setup.health_kw,
            field: setup.current_kw,
            value: HandlerValue::Increment {
                var: "base_health".to_string(),
                delta: 10,
            },
        });

    let mut bindings = ActionBindings::new();
    bindings.bind_entity("target", setup.player);
    bindings.bind_value("base_health", Value::Int(50));

    let world = executor.execute(&action, &bindings, world).unwrap();

    let health = world
        .get_field(setup.player, setup.health_kw, setup.current_kw)
        .unwrap();
    assert_eq!(health, Some(Value::Int(60)));
}

#[test]
fn multiple_handlers_execute_in_order() {
    let (world, setup) = setup_action_world();
    let mut executor = ActionExecutor::new();

    let action = CompiledAction::new(setup.heal_action_kw)
        .with_handler(CompiledHandler::Say {
            message: "Starting heal...".to_string(),
        })
        .with_handler(CompiledHandler::SetField {
            entity_var: "target".to_string(),
            component: setup.health_kw,
            field: setup.current_kw,
            value: HandlerValue::Literal(Value::Int(100)),
        })
        .with_handler(CompiledHandler::Say {
            message: "Heal complete!".to_string(),
        });

    let mut bindings = ActionBindings::new();
    bindings.bind_entity("target", setup.player);

    let world = executor.execute(&action, &bindings, world).unwrap();

    // Check messages in order
    let messages = executor.messages();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0], "Starting heal...");
    assert_eq!(messages[1], "Heal complete!");

    // Check field was updated
    let health = world
        .get_field(setup.player, setup.health_kw, setup.current_kw)
        .unwrap();
    assert_eq!(health, Some(Value::Int(100)));
}
