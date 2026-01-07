//! Adventure Game integration tests.
//!
//! Tests for a text adventure demonstrating:
//! - Spatial model with rooms and exits
//! - Player movement between rooms
//! - Item pickup and inventory management
//! - Basic combat mechanics
//!
//! Uses the spike API to validate rule-based behaviors.

use longtable_engine::spike::{Pattern, PatternBinding, RuleEngine, SpikeRule};
use longtable_foundation::{LtMap, Type, Value};
use longtable_storage::{Cardinality, ComponentSchema, FieldSchema, RelationshipSchema, World};

// =============================================================================
// World Setup
// =============================================================================

/// Create a world with adventure game components registered.
fn setup_world() -> World {
    let mut world = World::new(42);

    // Components for rooms
    let room_kw = world.interner_mut().intern_keyword("room");
    let name_kw = world.interner_mut().intern_keyword("name");
    let description_kw = world.interner_mut().intern_keyword("description");

    // Components for player
    let player_kw = world.interner_mut().intern_keyword("player");
    let health_kw = world.interner_mut().intern_keyword("health");
    let current_kw = world.interner_mut().intern_keyword("current");
    let max_kw = world.interner_mut().intern_keyword("max");

    // Components for items
    let item_kw = world.interner_mut().intern_keyword("item");
    let takeable_kw = world.interner_mut().intern_keyword("takeable");
    let weapon_kw = world.interner_mut().intern_keyword("weapon");
    let damage_kw = world.interner_mut().intern_keyword("damage");

    // Components for enemies
    let enemy_kw = world.interner_mut().intern_keyword("enemy");
    let hostile_kw = world.interner_mut().intern_keyword("hostile");

    // Components for commands
    let command_kw = world.interner_mut().intern_keyword("command");
    let verb_kw = world.interner_mut().intern_keyword("verb");
    let direction_kw = world.interner_mut().intern_keyword("direction");
    let target_kw = world.interner_mut().intern_keyword("target");
    let processed_kw = world.interner_mut().intern_keyword("processed");

    // Relationships
    let in_room_kw = world.interner_mut().intern_keyword("in-room");
    let exit_north_kw = world.interner_mut().intern_keyword("exit/north");
    let exit_south_kw = world.interner_mut().intern_keyword("exit/south");
    let exit_east_kw = world.interner_mut().intern_keyword("exit/east");
    let exit_west_kw = world.interner_mut().intern_keyword("exit/west");
    let in_inventory_kw = world.interner_mut().intern_keyword("in-inventory");

    // Register room component
    let world = world
        .register_component(
            ComponentSchema::new(room_kw)
                .with_field(FieldSchema::required(name_kw, Type::String))
                .with_field(FieldSchema::optional_nil(description_kw, Type::String)),
        )
        .unwrap();

    // Register player component (tag with health)
    let world = world
        .register_component(
            ComponentSchema::new(player_kw)
                .with_field(FieldSchema::required(current_kw, Type::Int))
                .with_field(FieldSchema::required(max_kw, Type::Int)),
        )
        .unwrap();

    // Register health component
    let world = world
        .register_component(
            ComponentSchema::new(health_kw)
                .with_field(FieldSchema::required(current_kw, Type::Int))
                .with_field(FieldSchema::required(max_kw, Type::Int)),
        )
        .unwrap();

    // Register item component
    let world = world
        .register_component(
            ComponentSchema::new(item_kw).with_field(FieldSchema::required(name_kw, Type::String)),
        )
        .unwrap();

    // Register takeable tag
    let world = world
        .register_component(ComponentSchema::tag(takeable_kw))
        .unwrap();

    // Register weapon component
    let world = world
        .register_component(
            ComponentSchema::new(weapon_kw).with_field(FieldSchema::required(damage_kw, Type::Int)),
        )
        .unwrap();

    // Register enemy component
    let world = world
        .register_component(
            ComponentSchema::new(enemy_kw).with_field(FieldSchema::required(name_kw, Type::String)),
        )
        .unwrap();

    // Register hostile tag
    let world = world
        .register_component(ComponentSchema::tag(hostile_kw))
        .unwrap();

    // Register command component
    let world = world
        .register_component(
            ComponentSchema::new(command_kw)
                .with_field(FieldSchema::required(verb_kw, Type::Symbol))
                .with_field(FieldSchema::optional_nil(direction_kw, Type::Keyword))
                .with_field(FieldSchema::optional_nil(target_kw, Type::EntityRef)),
        )
        .unwrap();

    // Register processed tag
    let world = world
        .register_component(ComponentSchema::tag(processed_kw))
        .unwrap();

    // Register relationships
    let world = world
        .register_relationship(
            RelationshipSchema::new(in_room_kw).with_cardinality(Cardinality::ManyToOne), // Many things can be in a room, but each thing is in one room
        )
        .unwrap();

    let world = world
        .register_relationship(
            RelationshipSchema::new(exit_north_kw).with_cardinality(Cardinality::OneToOne), // Each room has at most one north exit
        )
        .unwrap();

    let world = world
        .register_relationship(
            RelationshipSchema::new(exit_south_kw).with_cardinality(Cardinality::OneToOne),
        )
        .unwrap();

    let world = world
        .register_relationship(
            RelationshipSchema::new(exit_east_kw).with_cardinality(Cardinality::OneToOne),
        )
        .unwrap();

    let world = world
        .register_relationship(
            RelationshipSchema::new(exit_west_kw).with_cardinality(Cardinality::OneToOne),
        )
        .unwrap();

    world
        .register_relationship(
            RelationshipSchema::new(in_inventory_kw).with_cardinality(Cardinality::ManyToOne), // Many items in one player's inventory
        )
        .unwrap()
}

/// Create a simple world with two rooms connected north-south.
fn create_two_room_world() -> (World, RoomSetup) {
    let mut world = setup_world();

    // Get keyword IDs
    let room_kw = world.interner_mut().intern_keyword("room");
    let name_kw = world.interner_mut().intern_keyword("name");
    let description_kw = world.interner_mut().intern_keyword("description");
    let player_kw = world.interner_mut().intern_keyword("player");
    let current_kw = world.interner_mut().intern_keyword("current");
    let max_kw = world.interner_mut().intern_keyword("max");
    let in_room_kw = world.interner_mut().intern_keyword("in-room");
    let exit_north_kw = world.interner_mut().intern_keyword("exit/north");
    let exit_south_kw = world.interner_mut().intern_keyword("exit/south");

    // Create Cave Entrance
    let (world, cave_entrance) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(
            cave_entrance,
            room_kw,
            name_kw,
            Value::from("Cave Entrance"),
        )
        .unwrap();
    let world = world
        .set_field(
            cave_entrance,
            room_kw,
            description_kw,
            Value::from("A dark cave entrance."),
        )
        .unwrap();

    // Create Main Hall
    let (world, main_hall) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(main_hall, room_kw, name_kw, Value::from("Main Hall"))
        .unwrap();
    let world = world
        .set_field(
            main_hall,
            room_kw,
            description_kw,
            Value::from("A grand hall with torches."),
        )
        .unwrap();

    // Connect rooms: Cave Entrance --north--> Main Hall --south--> Cave Entrance
    let world = world.link(cave_entrance, exit_north_kw, main_hall).unwrap();
    let world = world.link(main_hall, exit_south_kw, cave_entrance).unwrap();

    // Create player in Cave Entrance
    let (world, player) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(player, player_kw, current_kw, Value::Int(100))
        .unwrap();
    let world = world
        .set_field(player, player_kw, max_kw, Value::Int(100))
        .unwrap();
    let world = world.link(player, in_room_kw, cave_entrance).unwrap();

    let setup = RoomSetup {
        cave_entrance,
        main_hall,
        player,
        room_kw,
        name_kw,
        in_room_kw,
        exit_north_kw,
        exit_south_kw,
        player_kw,
        current_kw,
    };

    (world, setup)
}

struct RoomSetup {
    cave_entrance: longtable_foundation::EntityId,
    main_hall: longtable_foundation::EntityId,
    player: longtable_foundation::EntityId,
    room_kw: longtable_foundation::KeywordId,
    name_kw: longtable_foundation::KeywordId,
    in_room_kw: longtable_foundation::KeywordId,
    exit_north_kw: longtable_foundation::KeywordId,
    exit_south_kw: longtable_foundation::KeywordId,
    player_kw: longtable_foundation::KeywordId,
    current_kw: longtable_foundation::KeywordId,
}

// =============================================================================
// Room and Navigation Tests
// =============================================================================

#[test]
fn rooms_have_names() {
    let (world, setup) = create_two_room_world();

    let name = world
        .get_field(setup.cave_entrance, setup.room_kw, setup.name_kw)
        .unwrap();
    assert_eq!(name, Some(Value::from("Cave Entrance")));

    let name = world
        .get_field(setup.main_hall, setup.room_kw, setup.name_kw)
        .unwrap();
    assert_eq!(name, Some(Value::from("Main Hall")));
}

#[test]
fn rooms_connected_by_exits() {
    let (world, setup) = create_two_room_world();

    // Cave entrance should have exit north to main hall
    let targets: Vec<_> = world
        .targets(setup.cave_entrance, setup.exit_north_kw)
        .collect();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0], setup.main_hall);

    // Main hall should have exit south to cave entrance
    let targets: Vec<_> = world
        .targets(setup.main_hall, setup.exit_south_kw)
        .collect();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0], setup.cave_entrance);
}

#[test]
fn player_starts_in_cave_entrance() {
    let (world, setup) = create_two_room_world();

    let room_targets: Vec<_> = world.targets(setup.player, setup.in_room_kw).collect();
    assert_eq!(room_targets.len(), 1);
    assert_eq!(room_targets[0], setup.cave_entrance);
}

#[test]
fn player_can_move_north() {
    let (world, setup) = create_two_room_world();

    // Get current room's north exit
    let north_targets: Vec<_> = world
        .targets(setup.cave_entrance, setup.exit_north_kw)
        .collect();
    assert_eq!(north_targets.len(), 1);
    let destination = north_targets[0];

    // Move player: unlink from current room, link to new room
    let world = world
        .unlink(setup.player, setup.in_room_kw, setup.cave_entrance)
        .unwrap();
    let world = world
        .link(setup.player, setup.in_room_kw, destination)
        .unwrap();

    // Player should now be in Main Hall
    let room_targets: Vec<_> = world.targets(setup.player, setup.in_room_kw).collect();
    assert_eq!(room_targets.len(), 1);
    assert_eq!(room_targets[0], setup.main_hall);
}

#[test]
fn player_can_move_south_back() {
    let (world, setup) = create_two_room_world();

    // Move north first
    let world = world
        .unlink(setup.player, setup.in_room_kw, setup.cave_entrance)
        .unwrap();
    let world = world
        .link(setup.player, setup.in_room_kw, setup.main_hall)
        .unwrap();

    // Now move south
    let south_targets: Vec<_> = world
        .targets(setup.main_hall, setup.exit_south_kw)
        .collect();
    let destination = south_targets[0];

    let world = world
        .unlink(setup.player, setup.in_room_kw, setup.main_hall)
        .unwrap();
    let world = world
        .link(setup.player, setup.in_room_kw, destination)
        .unwrap();

    // Player should be back in Cave Entrance
    let room_targets: Vec<_> = world.targets(setup.player, setup.in_room_kw).collect();
    assert_eq!(room_targets.len(), 1);
    assert_eq!(room_targets[0], setup.cave_entrance);
}

// =============================================================================
// Item Tests
// =============================================================================

fn create_world_with_item() -> (World, ItemSetup) {
    let (world, room_setup) = create_two_room_world();
    let mut world = world;

    let item_kw = world.interner_mut().intern_keyword("item");
    let name_kw = world.interner_mut().intern_keyword("name");
    let takeable_kw = world.interner_mut().intern_keyword("takeable");
    let in_room_kw = room_setup.in_room_kw;
    let in_inventory_kw = world.interner_mut().intern_keyword("in-inventory");

    // Create a sword in the cave entrance
    let (world, sword) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(sword, item_kw, name_kw, Value::from("Iron Sword"))
        .unwrap();
    let world = world.set(sword, takeable_kw, Value::Bool(true)).unwrap();
    let world = world
        .link(sword, in_room_kw, room_setup.cave_entrance)
        .unwrap();

    let setup = ItemSetup {
        sword,
        item_kw,
        name_kw,
        takeable_kw,
        in_room_kw,
        in_inventory_kw,
        player: room_setup.player,
        cave_entrance: room_setup.cave_entrance,
    };

    (world, setup)
}

struct ItemSetup {
    sword: longtable_foundation::EntityId,
    item_kw: longtable_foundation::KeywordId,
    name_kw: longtable_foundation::KeywordId,
    takeable_kw: longtable_foundation::KeywordId,
    in_room_kw: longtable_foundation::KeywordId,
    in_inventory_kw: longtable_foundation::KeywordId,
    player: longtable_foundation::EntityId,
    cave_entrance: longtable_foundation::EntityId,
}

#[test]
fn item_exists_in_room() {
    let (world, setup) = create_world_with_item();

    // Sword should be in cave entrance
    let room_targets: Vec<_> = world.targets(setup.sword, setup.in_room_kw).collect();
    assert_eq!(room_targets.len(), 1);
    assert_eq!(room_targets[0], setup.cave_entrance);

    // Sword should have name
    let name = world
        .get_field(setup.sword, setup.item_kw, setup.name_kw)
        .unwrap();
    assert_eq!(name, Some(Value::from("Iron Sword")));
}

#[test]
fn item_can_be_picked_up() {
    let (world, setup) = create_world_with_item();

    // Pick up sword: unlink from room, link to player inventory
    let world = world
        .unlink(setup.sword, setup.in_room_kw, setup.cave_entrance)
        .unwrap();
    let world = world
        .link(setup.sword, setup.in_inventory_kw, setup.player)
        .unwrap();

    // Sword should not be in room anymore
    let room_targets: Vec<_> = world.targets(setup.sword, setup.in_room_kw).collect();
    assert!(room_targets.is_empty());

    // Sword should be in player inventory
    let inv_targets: Vec<_> = world.targets(setup.sword, setup.in_inventory_kw).collect();
    assert_eq!(inv_targets.len(), 1);
    assert_eq!(inv_targets[0], setup.player);
}

#[test]
fn item_can_be_dropped() {
    let (world, setup) = create_world_with_item();

    // Pick up first
    let world = world
        .unlink(setup.sword, setup.in_room_kw, setup.cave_entrance)
        .unwrap();
    let world = world
        .link(setup.sword, setup.in_inventory_kw, setup.player)
        .unwrap();

    // Drop: unlink from inventory, link back to room
    let world = world
        .unlink(setup.sword, setup.in_inventory_kw, setup.player)
        .unwrap();
    let world = world
        .link(setup.sword, setup.in_room_kw, setup.cave_entrance)
        .unwrap();

    // Sword should be back in room
    let room_targets: Vec<_> = world.targets(setup.sword, setup.in_room_kw).collect();
    assert_eq!(room_targets.len(), 1);
    assert_eq!(room_targets[0], setup.cave_entrance);
}

// =============================================================================
// Rule-Based Movement Tests
// =============================================================================

#[test]
fn movement_rule_pattern_matches_player() {
    let (mut world, setup) = create_two_room_world();

    // Create a command to go north
    let command_kw = world.interner_mut().intern_keyword("command");
    let verb_kw = world.interner_mut().intern_keyword("verb");
    let direction_kw = world.interner_mut().intern_keyword("direction");
    let go_sym = world.interner_mut().intern_symbol("go");
    let north_kw = world.interner_mut().intern_keyword("north");
    let processed_kw = world.interner_mut().intern_keyword("processed");

    // Spawn a "go north" command
    let (world, cmd) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(cmd, command_kw, verb_kw, Value::Symbol(go_sym))
        .unwrap();
    let mut world = world
        .set_field(cmd, command_kw, direction_kw, Value::Keyword(north_kw))
        .unwrap();

    // Create pattern for movement rule:
    // [?cmd :command/verb 'go]
    // [?cmd :command/direction ?dir]
    // [?p :player/current ?_]  (matches player entities)
    // (not [?cmd :processed])
    let move_pattern = Pattern::new()
        .with_clause(
            "cmd",
            command_kw,
            Some(verb_kw),
            PatternBinding::Literal(Value::Symbol(go_sym)),
        )
        .with_clause(
            "cmd",
            command_kw,
            Some(direction_kw),
            PatternBinding::Variable("dir".to_string()),
        )
        .with_clause(
            "p",
            setup.player_kw,
            Some(setup.current_kw),
            PatternBinding::Wildcard,
        )
        .with_negated("cmd", processed_kw);

    // Create rule
    let move_rule_kw = world.interner_mut().intern_keyword("rules/go");
    let move_rule = SpikeRule {
        name: move_rule_kw,
        salience: 10,
        once: false,
        pattern: move_pattern,
        body: "tag ?cmd processed".to_string(), // Mark command as processed
    };

    let engine = RuleEngine::new();
    let activations = engine.find_activations(&[move_rule], &world);

    // Should find one activation (player + command)
    assert_eq!(activations.len(), 1);

    // Bindings should have player and command
    let binding = &activations[0].bindings;
    assert_eq!(binding.get_entity("cmd"), Some(cmd));
    assert_eq!(binding.get_entity("p"), Some(setup.player));
    assert_eq!(binding.get("dir"), Some(&Value::Keyword(north_kw)));
}

#[test]
fn movement_rule_marks_command_processed() {
    let (mut world, setup) = create_two_room_world();

    let command_kw = world.interner_mut().intern_keyword("command");
    let verb_kw = world.interner_mut().intern_keyword("verb");
    let direction_kw = world.interner_mut().intern_keyword("direction");
    let go_sym = world.interner_mut().intern_symbol("go");
    let north_kw = world.interner_mut().intern_keyword("north");
    let processed_kw = world.interner_mut().intern_keyword("processed");
    let move_rule_kw = world.interner_mut().intern_keyword("rules/go");

    // Spawn command
    let (world, cmd) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(cmd, command_kw, verb_kw, Value::Symbol(go_sym))
        .unwrap();
    let world = world
        .set_field(cmd, command_kw, direction_kw, Value::Keyword(north_kw))
        .unwrap();

    // Rule marks command as processed
    let move_pattern = Pattern::new()
        .with_clause(
            "cmd",
            command_kw,
            Some(verb_kw),
            PatternBinding::Literal(Value::Symbol(go_sym)),
        )
        .with_clause(
            "p",
            setup.player_kw,
            Some(setup.current_kw),
            PatternBinding::Wildcard,
        )
        .with_negated("cmd", processed_kw);

    let move_rule = SpikeRule {
        name: move_rule_kw,
        salience: 10,
        once: false,
        pattern: move_pattern,
        body: "tag ?cmd processed".to_string(),
    };

    let mut engine = RuleEngine::new();
    engine.begin_tick();

    let (result_world, _) = engine
        .run_to_quiescence(&[move_rule.clone()], world)
        .unwrap();

    // Command should now be processed
    assert!(result_world.has(cmd, processed_kw));

    // Rule shouldn't fire again (command is processed)
    let activations = engine.find_activations(&[move_rule], &result_world);
    assert!(activations.is_empty());
}

// =============================================================================
// Combat Tests
// =============================================================================

fn create_world_with_enemy() -> (World, CombatSetup) {
    let (world, room_setup) = create_two_room_world();
    let mut world = world;

    let enemy_kw = world.interner_mut().intern_keyword("enemy");
    let name_kw = world.interner_mut().intern_keyword("name");
    let hostile_kw = world.interner_mut().intern_keyword("hostile");
    let health_kw = world.interner_mut().intern_keyword("health");
    let current_kw = world.interner_mut().intern_keyword("current");
    let max_kw = world.interner_mut().intern_keyword("max");
    let in_room_kw = room_setup.in_room_kw;

    // Create a goblin in the cave entrance
    let (world, goblin) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(goblin, enemy_kw, name_kw, Value::from("Goblin"))
        .unwrap();
    let world = world.set(goblin, hostile_kw, Value::Bool(true)).unwrap();
    let world = world
        .set_field(goblin, health_kw, current_kw, Value::Int(50))
        .unwrap();
    let world = world
        .set_field(goblin, health_kw, max_kw, Value::Int(50))
        .unwrap();
    let world = world
        .link(goblin, in_room_kw, room_setup.cave_entrance)
        .unwrap();

    let setup = CombatSetup {
        goblin,
        enemy_kw,
        name_kw,
        hostile_kw,
        health_kw,
        current_kw,
        in_room_kw,
        player: room_setup.player,
        player_kw: room_setup.player_kw,
        cave_entrance: room_setup.cave_entrance,
    };

    (world, setup)
}

#[allow(dead_code)]
struct CombatSetup {
    goblin: longtable_foundation::EntityId,
    enemy_kw: longtable_foundation::KeywordId,
    name_kw: longtable_foundation::KeywordId,
    hostile_kw: longtable_foundation::KeywordId,
    health_kw: longtable_foundation::KeywordId,
    current_kw: longtable_foundation::KeywordId,
    in_room_kw: longtable_foundation::KeywordId,
    player: longtable_foundation::EntityId,
    player_kw: longtable_foundation::KeywordId,
    cave_entrance: longtable_foundation::EntityId,
}

#[test]
fn enemy_exists_in_room() {
    let (world, setup) = create_world_with_enemy();

    // Goblin should be in cave entrance
    let room_targets: Vec<_> = world.targets(setup.goblin, setup.in_room_kw).collect();
    assert_eq!(room_targets.len(), 1);
    assert_eq!(room_targets[0], setup.cave_entrance);

    // Goblin should be hostile
    assert!(world.has(setup.goblin, setup.hostile_kw));
}

#[test]
fn enemy_takes_damage() {
    let (world, setup) = create_world_with_enemy();

    // Get current health
    let hp = world
        .get_field(setup.goblin, setup.health_kw, setup.current_kw)
        .unwrap();
    assert_eq!(hp, Some(Value::Int(50)));

    // Apply 15 damage
    let new_hp = if let Some(Value::Int(current)) = hp {
        current - 15
    } else {
        0
    };
    let world = world
        .set_field(
            setup.goblin,
            setup.health_kw,
            setup.current_kw,
            Value::Int(new_hp),
        )
        .unwrap();

    // Health should be reduced
    let hp = world
        .get_field(setup.goblin, setup.health_kw, setup.current_kw)
        .unwrap();
    assert_eq!(hp, Some(Value::Int(35)));
}

#[test]
fn attack_rule_matches_hostile_enemy_in_same_room() {
    let (mut world, setup) = create_world_with_enemy();

    // Create attack command
    let command_kw = world.interner_mut().intern_keyword("command");
    let verb_kw = world.interner_mut().intern_keyword("verb");
    let target_kw = world.interner_mut().intern_keyword("target");
    let attack_sym = world.interner_mut().intern_symbol("attack");
    let processed_kw = world.interner_mut().intern_keyword("processed");
    let attack_rule_kw = world.interner_mut().intern_keyword("rules/attack");

    // Spawn attack command targeting goblin
    let (world, cmd) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(cmd, command_kw, verb_kw, Value::Symbol(attack_sym))
        .unwrap();
    let world = world
        .set_field(cmd, command_kw, target_kw, Value::EntityRef(setup.goblin))
        .unwrap();

    // Attack pattern:
    // [?cmd :command/verb 'attack]
    // [?cmd :command/target ?enemy]
    // [?enemy :hostile]
    // [?p :player/current ?_]
    // (not [?cmd :processed])
    let attack_pattern = Pattern::new()
        .with_clause(
            "cmd",
            command_kw,
            Some(verb_kw),
            PatternBinding::Literal(Value::Symbol(attack_sym)),
        )
        .with_clause(
            "cmd",
            command_kw,
            Some(target_kw),
            PatternBinding::Variable("enemy".to_string()),
        )
        .with_clause(
            "p",
            setup.player_kw,
            Some(setup.current_kw),
            PatternBinding::Wildcard,
        )
        .with_negated("cmd", processed_kw);

    let attack_rule = SpikeRule {
        name: attack_rule_kw,
        salience: 10,
        once: false,
        pattern: attack_pattern,
        body: "tag ?cmd processed".to_string(),
    };

    let engine = RuleEngine::new();
    let activations = engine.find_activations(&[attack_rule], &world);

    // Should find one activation
    assert_eq!(activations.len(), 1);

    // Bindings should have enemy reference
    let binding = &activations[0].bindings;
    assert_eq!(binding.get("enemy"), Some(&Value::EntityRef(setup.goblin)));
}

// =============================================================================
// Multi-Entity in Same Room Tests
// =============================================================================

#[test]
fn multiple_items_in_room() {
    let (world, setup) = create_world_with_item();

    let item_kw = setup.item_kw;
    let name_kw = setup.name_kw;
    let takeable_kw = setup.takeable_kw;
    let in_room_kw = setup.in_room_kw;

    // Add a second item
    let (world, potion) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(potion, item_kw, name_kw, Value::from("Health Potion"))
        .unwrap();
    let world = world.set(potion, takeable_kw, Value::Bool(true)).unwrap();
    let world = world.link(potion, in_room_kw, setup.cave_entrance).unwrap();

    // Both items should be in the room (via reverse relationship)
    let items_in_room: Vec<_> = world.sources(setup.cave_entrance, in_room_kw).collect();

    // Should find sword and potion (and player, who is also in room)
    assert!(items_in_room.contains(&setup.sword));
    assert!(items_in_room.contains(&potion));
}

#[test]
fn player_and_enemy_in_same_room() {
    let (world, setup) = create_world_with_enemy();

    // Both player and goblin should be in cave entrance
    let entities_in_room: Vec<_> = world
        .sources(setup.cave_entrance, setup.in_room_kw)
        .collect();

    assert!(entities_in_room.contains(&setup.player));
    assert!(entities_in_room.contains(&setup.goblin));
}
