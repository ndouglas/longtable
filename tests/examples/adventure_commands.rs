//! Full Adventure Game with Parser Integration.
//!
//! Demonstrates the complete parser pipeline with a working text adventure:
//! - Natural language command parsing
//! - Movement between rooms
//! - Item manipulation (take, drop, put)
//! - Container interaction (open, close)
//! - Combat system
//! - Inventory management

use longtable_foundation::{EntityId, KeywordId, LtMap, LtVec, Type, Value};
use longtable_parser::action::{
    ActionBindings, ActionExecutor, ActionRegistry, CompiledAction, CompiledHandler,
    CompiledPrecondition,
};
use longtable_parser::noun_phrase::NounResolver;
use longtable_parser::stdlib::{StdlibKeywords, create_syntaxes, register_all};
use longtable_parser::syntax::CompiledSyntax;
use longtable_parser::vocabulary::VocabularyRegistry;
use longtable_storage::{Cardinality, ComponentSchema, FieldSchema, RelationshipSchema, World};

// =============================================================================
// Adventure Keywords - Game-specific components beyond stdlib
// =============================================================================

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct AdventureKeywords {
    // Standard library keywords
    stdlib: StdlibKeywords,

    // Room components
    room: KeywordId,
    description: KeywordId,

    // Entity components
    name: KeywordId,
    aliases: KeywordId,
    adjectives: KeywordId,
    value: KeywordId, // Field keyword for component values

    // Item properties
    takeable: KeywordId,
    weapon: KeywordId,
    damage: KeywordId,

    // Container properties
    container: KeywordId,
    capacity: KeywordId,
    openable: KeywordId,
    open: KeywordId,
    locked: KeywordId,
    key_id: KeywordId,

    // Living entity properties
    health: KeywordId,
    current: KeywordId,
    max: KeywordId,
    hostile: KeywordId,
    dead: KeywordId,

    // Player
    player: KeywordId,

    // Relationships
    in_room: KeywordId,
    in_inventory: KeywordId,
    in_container: KeywordId,
    exit_north: KeywordId,
    exit_south: KeywordId,
    exit_east: KeywordId,
    exit_west: KeywordId,
    exit_up: KeywordId,
    exit_down: KeywordId,

    // Command components
    command: KeywordId,
    verb: KeywordId,
    target: KeywordId,
    instrument: KeywordId,
    destination: KeywordId,
    direction: KeywordId,
    processed: KeywordId,
}

impl AdventureKeywords {
    fn intern(world: &mut World) -> Self {
        let stdlib = StdlibKeywords::intern(world);
        let interner = world.interner_mut();

        Self {
            stdlib,
            // Room
            room: interner.intern_keyword("room"),
            description: interner.intern_keyword("description"),

            // Entity
            name: interner.intern_keyword("name"),
            aliases: interner.intern_keyword("aliases"),
            adjectives: interner.intern_keyword("adjectives"),
            value: interner.intern_keyword("value"),

            // Items
            takeable: interner.intern_keyword("takeable"),
            weapon: interner.intern_keyword("weapon"),
            damage: interner.intern_keyword("damage"),

            // Containers
            container: interner.intern_keyword("container"),
            capacity: interner.intern_keyword("capacity"),
            openable: interner.intern_keyword("openable"),
            open: interner.intern_keyword("open"),
            locked: interner.intern_keyword("locked"),
            key_id: interner.intern_keyword("key-id"),

            // Living
            health: interner.intern_keyword("health"),
            current: interner.intern_keyword("current"),
            max: interner.intern_keyword("max"),
            hostile: interner.intern_keyword("hostile"),
            dead: interner.intern_keyword("dead"),

            // Player
            player: interner.intern_keyword("player"),

            // Relationships
            in_room: interner.intern_keyword("in-room"),
            in_inventory: interner.intern_keyword("in-inventory"),
            in_container: interner.intern_keyword("in-container"),
            exit_north: interner.intern_keyword("exit/north"),
            exit_south: interner.intern_keyword("exit/south"),
            exit_east: interner.intern_keyword("exit/east"),
            exit_west: interner.intern_keyword("exit/west"),
            exit_up: interner.intern_keyword("exit/up"),
            exit_down: interner.intern_keyword("exit/down"),

            // Commands
            command: interner.intern_keyword("command"),
            verb: interner.intern_keyword("verb"),
            target: interner.intern_keyword("target"),
            instrument: interner.intern_keyword("instrument"),
            destination: interner.intern_keyword("destination"),
            direction: interner.intern_keyword("direction"),
            processed: interner.intern_keyword("processed"),
        }
    }
}

// =============================================================================
// World Setup - Component and Relationship Schemas
// =============================================================================

fn make_vec(values: Vec<Value>) -> LtVec<Value> {
    values.into_iter().collect()
}

fn register_schemas(world: World, kw: &AdventureKeywords) -> World {
    // Room component
    let world = world
        .register_component(
            ComponentSchema::new(kw.room)
                .with_field(FieldSchema::required(kw.name, Type::String))
                .with_field(FieldSchema::optional_nil(kw.description, Type::String)),
        )
        .unwrap();

    // Named entity component (uses :value field like the real game)
    let world = world
        .register_component(
            ComponentSchema::new(kw.name).with_field(FieldSchema::required(kw.value, Type::String)),
        )
        .unwrap();

    // Aliases component (uses :value field like the real game)
    let world = world
        .register_component(
            ComponentSchema::new(kw.aliases).with_field(FieldSchema::required(
                kw.value,
                Type::Vec(Box::new(Type::String)),
            )),
        )
        .unwrap();

    // Adjectives component (uses :value field like the real game)
    let world =
        world
            .register_component(ComponentSchema::new(kw.adjectives).with_field(
                FieldSchema::required(kw.value, Type::Vec(Box::new(Type::String))),
            ))
            .unwrap();

    // Takeable tag
    let world = world
        .register_component(ComponentSchema::tag(kw.takeable))
        .unwrap();

    // Weapon component
    let world = world
        .register_component(
            ComponentSchema::new(kw.weapon).with_field(FieldSchema::required(kw.damage, Type::Int)),
        )
        .unwrap();

    // Container component
    let world = world
        .register_component(
            ComponentSchema::new(kw.container)
                .with_field(FieldSchema::required(kw.capacity, Type::Int)),
        )
        .unwrap();

    // Openable tag
    let world = world
        .register_component(ComponentSchema::tag(kw.openable))
        .unwrap();

    // Open tag
    let world = world
        .register_component(ComponentSchema::tag(kw.open))
        .unwrap();

    // Locked tag
    let world = world
        .register_component(ComponentSchema::tag(kw.locked))
        .unwrap();

    // Health component
    let world = world
        .register_component(
            ComponentSchema::new(kw.health)
                .with_field(FieldSchema::required(kw.current, Type::Int))
                .with_field(FieldSchema::required(kw.max, Type::Int)),
        )
        .unwrap();

    // Hostile tag
    let world = world
        .register_component(ComponentSchema::tag(kw.hostile))
        .unwrap();

    // Dead tag
    let world = world
        .register_component(ComponentSchema::tag(kw.dead))
        .unwrap();

    // Player component
    let world = world
        .register_component(ComponentSchema::tag(kw.player))
        .unwrap();

    // Processed tag (for commands)
    let world = world
        .register_component(ComponentSchema::tag(kw.processed))
        .unwrap();

    // Relationships
    let world = world
        .register_relationship(
            RelationshipSchema::new(kw.in_room).with_cardinality(Cardinality::ManyToOne),
        )
        .unwrap();

    let world = world
        .register_relationship(
            RelationshipSchema::new(kw.in_inventory).with_cardinality(Cardinality::ManyToOne),
        )
        .unwrap();

    let world = world
        .register_relationship(
            RelationshipSchema::new(kw.in_container).with_cardinality(Cardinality::ManyToOne),
        )
        .unwrap();

    // Exit relationships (one exit per direction per room)
    let world = world
        .register_relationship(
            RelationshipSchema::new(kw.exit_north).with_cardinality(Cardinality::OneToOne),
        )
        .unwrap();

    let world = world
        .register_relationship(
            RelationshipSchema::new(kw.exit_south).with_cardinality(Cardinality::OneToOne),
        )
        .unwrap();

    let world = world
        .register_relationship(
            RelationshipSchema::new(kw.exit_east).with_cardinality(Cardinality::OneToOne),
        )
        .unwrap();

    let world = world
        .register_relationship(
            RelationshipSchema::new(kw.exit_west).with_cardinality(Cardinality::OneToOne),
        )
        .unwrap();

    let world = world
        .register_relationship(
            RelationshipSchema::new(kw.exit_up).with_cardinality(Cardinality::OneToOne),
        )
        .unwrap();

    world
        .register_relationship(
            RelationshipSchema::new(kw.exit_down).with_cardinality(Cardinality::OneToOne),
        )
        .unwrap()
}

// =============================================================================
// Entity Creation Helpers
// =============================================================================

fn create_room(
    world: World,
    kw: &AdventureKeywords,
    name: &str,
    description: &str,
) -> (World, EntityId) {
    let (world, room) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(room, kw.room, kw.name, Value::from(name))
        .unwrap();
    let world = world
        .set_field(room, kw.room, kw.description, Value::from(description))
        .unwrap();
    (world, room)
}

fn create_item(
    world: World,
    kw: &AdventureKeywords,
    name: &str,
    adjectives: Vec<&str>,
    aliases: Vec<&str>,
    takeable: bool,
) -> (World, EntityId) {
    let (world, item) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(item, kw.name, kw.value, Value::from(name))
        .unwrap();
    let world = world
        .set_field(
            item,
            kw.adjectives,
            kw.value,
            Value::Vec(make_vec(adjectives.into_iter().map(Value::from).collect())),
        )
        .unwrap();
    let world = world
        .set_field(
            item,
            kw.aliases,
            kw.value,
            Value::Vec(make_vec(aliases.into_iter().map(Value::from).collect())),
        )
        .unwrap();
    let world = if takeable {
        world.set(item, kw.takeable, Value::Bool(true)).unwrap()
    } else {
        world
    };
    (world, item)
}

fn create_weapon(
    world: World,
    kw: &AdventureKeywords,
    name: &str,
    adjectives: Vec<&str>,
    damage: i64,
) -> (World, EntityId) {
    let (world, weapon) = create_item(world, kw, name, adjectives, vec![], true);
    let world = world
        .set_field(weapon, kw.weapon, kw.damage, Value::Int(damage))
        .unwrap();
    (world, weapon)
}

fn create_container(
    world: World,
    kw: &AdventureKeywords,
    name: &str,
    adjectives: Vec<&str>,
    capacity: i64,
    is_open: bool,
) -> (World, EntityId) {
    let (world, container) = create_item(world, kw, name, adjectives, vec![], false);
    let world = world
        .set_field(container, kw.container, kw.capacity, Value::Int(capacity))
        .unwrap();
    let world = world
        .set(container, kw.openable, Value::Bool(true))
        .unwrap();
    let world = if is_open {
        world.set(container, kw.open, Value::Bool(true)).unwrap()
    } else {
        world
    };
    (world, container)
}

fn create_enemy(
    world: World,
    kw: &AdventureKeywords,
    name: &str,
    adjectives: Vec<&str>,
    hp: i64,
) -> (World, EntityId) {
    let (world, enemy) = world.spawn(&LtMap::new()).unwrap();
    let world = world
        .set_field(enemy, kw.name, kw.value, Value::from(name))
        .unwrap();
    let world = world
        .set_field(
            enemy,
            kw.adjectives,
            kw.value,
            Value::Vec(make_vec(adjectives.into_iter().map(Value::from).collect())),
        )
        .unwrap();
    let world = world
        .set_field(
            enemy,
            kw.aliases,
            kw.value,
            Value::Vec(make_vec(Vec::new())),
        )
        .unwrap();
    let world = world
        .set_field(enemy, kw.health, kw.current, Value::Int(hp))
        .unwrap();
    let world = world
        .set_field(enemy, kw.health, kw.max, Value::Int(hp))
        .unwrap();
    let world = world.set(enemy, kw.hostile, Value::Bool(true)).unwrap();
    (world, enemy)
}

fn create_player(world: World, kw: &AdventureKeywords, hp: i64) -> (World, EntityId) {
    let (world, player) = world.spawn(&LtMap::new()).unwrap();
    let world = world.set(player, kw.player, Value::Bool(true)).unwrap();
    let world = world
        .set_field(player, kw.health, kw.current, Value::Int(hp))
        .unwrap();
    let world = world
        .set_field(player, kw.health, kw.max, Value::Int(hp))
        .unwrap();
    (world, player)
}

// =============================================================================
// Action Definitions
// =============================================================================

fn create_action_registry(kw: &AdventureKeywords) -> ActionRegistry {
    let mut registry = ActionRegistry::new();

    // ----- LOOK ACTION -----
    registry.register(
        CompiledAction::new(kw.stdlib.action_look)
            .with_salience(100)
            .with_handler(CompiledHandler::Say {
                message: "You look around the room.".to_string(),
            }),
    );

    // ----- EXAMINE ACTION -----
    registry.register(
        CompiledAction::new(kw.stdlib.action_examine)
            .with_params(vec!["target".to_string()])
            .with_salience(100)
            .with_handler(CompiledHandler::Say {
                message: "You examine it closely.".to_string(),
            }),
    );

    // ----- TAKE ACTION -----
    registry.register(
        CompiledAction::new(kw.stdlib.action_take)
            .with_params(vec!["target".to_string()])
            .with_salience(100)
            .with_precondition(CompiledPrecondition::HasComponent {
                entity_var: "target".to_string(),
                component: kw.takeable,
            })
            .with_handler(CompiledHandler::Say {
                message: "Taken.".to_string(),
            }),
    );

    // ----- DROP ACTION -----
    registry.register(
        CompiledAction::new(kw.stdlib.action_drop)
            .with_params(vec!["target".to_string()])
            .with_salience(100)
            .with_handler(CompiledHandler::Say {
                message: "Dropped.".to_string(),
            }),
    );

    // ----- OPEN ACTION -----
    registry.register(
        CompiledAction::new(kw.stdlib.action_open)
            .with_params(vec!["target".to_string()])
            .with_salience(100)
            .with_precondition(CompiledPrecondition::HasComponent {
                entity_var: "target".to_string(),
                component: kw.openable,
            })
            .with_precondition(CompiledPrecondition::NotHasComponent {
                entity_var: "target".to_string(),
                component: kw.open,
            })
            .with_handler(CompiledHandler::AddTag {
                entity_var: "target".to_string(),
                tag: kw.open,
            })
            .with_handler(CompiledHandler::Say {
                message: "Opened.".to_string(),
            }),
    );

    // ----- CLOSE ACTION -----
    registry.register(
        CompiledAction::new(kw.stdlib.action_close)
            .with_params(vec!["target".to_string()])
            .with_salience(100)
            .with_precondition(CompiledPrecondition::HasComponent {
                entity_var: "target".to_string(),
                component: kw.openable,
            })
            .with_precondition(CompiledPrecondition::HasComponent {
                entity_var: "target".to_string(),
                component: kw.open,
            })
            .with_handler(CompiledHandler::Say {
                message: "Closed.".to_string(),
            }),
    );

    // ----- ATTACK ACTION -----
    registry.register(
        CompiledAction::new(kw.stdlib.action_attack)
            .with_params(vec!["target".to_string()])
            .with_salience(100)
            .with_precondition(CompiledPrecondition::HasComponent {
                entity_var: "target".to_string(),
                component: kw.health,
            })
            .with_precondition(CompiledPrecondition::NotHasComponent {
                entity_var: "target".to_string(),
                component: kw.dead,
            })
            .with_handler(CompiledHandler::Say {
                message: "You attack!".to_string(),
            }),
    );

    // ----- INVENTORY ACTION -----
    registry.register(
        CompiledAction::new(kw.stdlib.action_inventory)
            .with_salience(100)
            .with_handler(CompiledHandler::Say {
                message: "You check your inventory.".to_string(),
            }),
    );

    // ----- WAIT ACTION -----
    registry.register(
        CompiledAction::new(kw.stdlib.action_wait)
            .with_salience(100)
            .with_handler(CompiledHandler::Say {
                message: "Time passes.".to_string(),
            }),
    );

    // ----- GO ACTION -----
    registry.register(
        CompiledAction::new(kw.stdlib.action_go)
            .with_params(vec!["direction".to_string()])
            .with_salience(100)
            .with_handler(CompiledHandler::Say {
                message: "You go that way.".to_string(),
            }),
    );

    registry
}

// =============================================================================
// Test World Setup
// =============================================================================

struct TestWorld {
    world: World,
    kw: AdventureKeywords,
    player: EntityId,
    cave_entrance: EntityId,
    main_hall: EntityId,
    armory: EntityId,
    sword: EntityId,
    chest: EntityId,
    key: EntityId,
    goblin: EntityId,
    vocab: VocabularyRegistry,
    resolver: NounResolver,
    actions: ActionRegistry,
    syntaxes: Vec<CompiledSyntax>,
}

fn create_test_world() -> TestWorld {
    let mut world = World::new(42);
    let kw = AdventureKeywords::intern(&mut world);
    let world = register_schemas(world, &kw);

    // Create rooms
    let (world, cave_entrance) = create_room(
        world,
        &kw,
        "Cave Entrance",
        "A dark cave entrance. Cold air flows from the north.",
    );

    let (world, main_hall) = create_room(
        world,
        &kw,
        "Main Hall",
        "A grand hall lit by flickering torches. Passages lead in several directions.",
    );

    let (world, armory) = create_room(
        world,
        &kw,
        "Armory",
        "Racks of weapons line the walls. Most are rusted beyond use.",
    );

    // Connect rooms
    let world = world.link(cave_entrance, kw.exit_north, main_hall).unwrap();
    let world = world.link(main_hall, kw.exit_south, cave_entrance).unwrap();
    let world = world.link(main_hall, kw.exit_east, armory).unwrap();
    let world = world.link(armory, kw.exit_west, main_hall).unwrap();

    // Create player in cave entrance
    let (world, player) = create_player(world, &kw, 100);
    let world = world.link(player, kw.in_room, cave_entrance).unwrap();

    // Create items
    let (world, sword) = create_weapon(world, &kw, "sword", vec!["iron", "rusty"], 15);
    let world = world.link(sword, kw.in_room, cave_entrance).unwrap();

    let (world, key) = create_item(world, &kw, "key", vec!["brass", "small"], vec![], true);
    let world = world.link(key, kw.in_room, main_hall).unwrap();

    // Create chest in armory
    let (world, chest) = create_container(world, &kw, "chest", vec!["wooden", "old"], 10, false);
    let world = world.link(chest, kw.in_room, armory).unwrap();

    // Create enemy
    let (world, goblin) = create_enemy(world, &kw, "goblin", vec!["ugly", "green"], 30);
    let world = world.link(goblin, kw.in_room, main_hall).unwrap();

    // Set up vocabulary
    let mut vocab = VocabularyRegistry::new();
    register_all(&mut vocab, &kw.stdlib);

    // Set up noun resolver (using :value as the field keyword, like the real game)
    let resolver = NounResolver::new(kw.name, kw.value, kw.aliases, kw.adjectives);

    // Set up actions
    let actions = create_action_registry(&kw);

    // Set up syntaxes
    let syntaxes = create_syntaxes(&kw.stdlib);

    TestWorld {
        world,
        kw,
        player,
        cave_entrance,
        main_hall,
        armory,
        sword,
        chest,
        key,
        goblin,
        vocab,
        resolver,
        actions,
        syntaxes,
    }
}

// =============================================================================
// Tests - World Setup
// =============================================================================

#[test]
fn world_has_rooms() {
    let tw = create_test_world();

    // Check room names
    let name = tw
        .world
        .get_field(tw.cave_entrance, tw.kw.room, tw.kw.name)
        .unwrap();
    assert_eq!(name, Some(Value::from("Cave Entrance")));

    let name = tw
        .world
        .get_field(tw.main_hall, tw.kw.room, tw.kw.name)
        .unwrap();
    assert_eq!(name, Some(Value::from("Main Hall")));
}

#[test]
fn rooms_are_connected() {
    let tw = create_test_world();

    // Cave entrance -> north -> Main Hall
    let targets: Vec<_> = tw
        .world
        .targets(tw.cave_entrance, tw.kw.exit_north)
        .collect();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0], tw.main_hall);

    // Main Hall -> south -> Cave entrance
    let targets: Vec<_> = tw.world.targets(tw.main_hall, tw.kw.exit_south).collect();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0], tw.cave_entrance);

    // Main Hall -> east -> Armory
    let targets: Vec<_> = tw.world.targets(tw.main_hall, tw.kw.exit_east).collect();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0], tw.armory);
}

#[test]
fn player_starts_in_cave() {
    let tw = create_test_world();

    let rooms: Vec<_> = tw.world.targets(tw.player, tw.kw.in_room).collect();
    assert_eq!(rooms.len(), 1);
    assert_eq!(rooms[0], tw.cave_entrance);
}

#[test]
fn items_are_in_rooms() {
    let tw = create_test_world();

    // Sword in cave entrance
    let rooms: Vec<_> = tw.world.targets(tw.sword, tw.kw.in_room).collect();
    assert_eq!(rooms.len(), 1);
    assert_eq!(rooms[0], tw.cave_entrance);

    // Key in main hall
    let rooms: Vec<_> = tw.world.targets(tw.key, tw.kw.in_room).collect();
    assert_eq!(rooms.len(), 1);
    assert_eq!(rooms[0], tw.main_hall);
}

#[test]
fn sword_is_weapon() {
    let tw = create_test_world();

    let damage = tw
        .world
        .get_field(tw.sword, tw.kw.weapon, tw.kw.damage)
        .unwrap();
    assert_eq!(damage, Some(Value::Int(15)));
}

#[test]
fn chest_is_container() {
    let tw = create_test_world();

    // Chest is openable
    assert!(tw.world.has(tw.chest, tw.kw.openable));

    // Chest starts closed
    assert!(!tw.world.has(tw.chest, tw.kw.open));

    // Chest has capacity
    let cap = tw
        .world
        .get_field(tw.chest, tw.kw.container, tw.kw.capacity)
        .unwrap();
    assert_eq!(cap, Some(Value::Int(10)));
}

#[test]
fn goblin_is_hostile() {
    let tw = create_test_world();

    assert!(tw.world.has(tw.goblin, tw.kw.hostile));

    let hp = tw
        .world
        .get_field(tw.goblin, tw.kw.health, tw.kw.current)
        .unwrap();
    assert_eq!(hp, Some(Value::Int(30)));
}

// =============================================================================
// Tests - Noun Resolution
// =============================================================================

#[test]
fn resolver_finds_sword_by_name() {
    let tw = create_test_world();
    use longtable_parser::noun_phrase::{NounPhrase, NounResolution};

    let np = NounPhrase::new("sword");
    let scope = vec![tw.sword, tw.chest, tw.key];

    let result = tw.resolver.resolve(&np, None, &scope, &tw.world, &tw.vocab);

    assert!(matches!(result, NounResolution::Unique(id) if id == tw.sword));
}

#[test]
fn resolver_finds_sword_by_adjective() {
    let tw = create_test_world();
    use longtable_parser::noun_phrase::{NounPhrase, NounResolution};

    let np = NounPhrase::new("sword").with_adjective("rusty");
    let scope = vec![tw.sword, tw.chest, tw.key];

    let result = tw.resolver.resolve(&np, None, &scope, &tw.world, &tw.vocab);

    assert!(matches!(result, NounResolution::Unique(id) if id == tw.sword));
}

#[test]
fn resolver_finds_chest_by_adjective() {
    let tw = create_test_world();
    use longtable_parser::noun_phrase::{NounPhrase, NounResolution};

    let np = NounPhrase::new("chest").with_adjective("wooden");
    let scope = vec![tw.sword, tw.chest, tw.key];

    let result = tw.resolver.resolve(&np, None, &scope, &tw.world, &tw.vocab);

    assert!(matches!(result, NounResolution::Unique(id) if id == tw.chest));
}

#[test]
fn resolver_describes_entities() {
    let tw = create_test_world();

    let desc = tw.resolver.describe(tw.sword, &tw.world);
    assert!(desc.contains("sword"));
    assert!(desc.contains("rusty") || desc.contains("iron"));

    let desc = tw.resolver.describe(tw.chest, &tw.world);
    assert!(desc.contains("chest"));
}

// =============================================================================
// Tests - Action Execution
// =============================================================================

#[test]
fn look_action_produces_message() {
    let tw = create_test_world();
    let mut executor = ActionExecutor::new();

    let action = tw.actions.get(tw.kw.stdlib.action_look).unwrap();
    let bindings = ActionBindings::new();

    let _world = executor.execute(action, &bindings, tw.world).unwrap();

    assert!(!executor.messages().is_empty());
    assert!(executor.messages()[0].contains("look around"));
}

#[test]
fn take_action_checks_takeable() {
    let tw = create_test_world();
    let executor = ActionExecutor::new();

    let action = tw.actions.get(tw.kw.stdlib.action_take).unwrap();

    // Sword is takeable
    let mut bindings = ActionBindings::new();
    bindings.bind_entity("target", tw.sword);
    let result = executor.check_preconditions(action, &bindings, &tw.world);
    assert!(matches!(
        result,
        longtable_parser::action::PreconditionResult::Pass
    ));

    // Chest is not takeable
    let mut bindings = ActionBindings::new();
    bindings.bind_entity("target", tw.chest);
    let result = executor.check_preconditions(action, &bindings, &tw.world);
    assert!(matches!(
        result,
        longtable_parser::action::PreconditionResult::Fail { .. }
    ));
}

#[test]
fn open_action_opens_container() {
    let tw = create_test_world();
    let mut executor = ActionExecutor::new();

    let action = tw.actions.get(tw.kw.stdlib.action_open).unwrap();

    // Chest starts closed
    assert!(!tw.world.has(tw.chest, tw.kw.open));

    let mut bindings = ActionBindings::new();
    bindings.bind_entity("target", tw.chest);

    let world = executor.execute(action, &bindings, tw.world).unwrap();

    // Chest is now open
    assert!(world.has(tw.chest, tw.kw.open));
    assert!(executor.messages().iter().any(|m| m.contains("Opened")));
}

#[test]
fn attack_action_checks_health() {
    let tw = create_test_world();
    let executor = ActionExecutor::new();

    let action = tw.actions.get(tw.kw.stdlib.action_attack).unwrap();

    // Goblin has health
    let mut bindings = ActionBindings::new();
    bindings.bind_entity("target", tw.goblin);
    let result = executor.check_preconditions(action, &bindings, &tw.world);
    assert!(matches!(
        result,
        longtable_parser::action::PreconditionResult::Pass
    ));

    // Sword doesn't have health (can't attack)
    let mut bindings = ActionBindings::new();
    bindings.bind_entity("target", tw.sword);
    let result = executor.check_preconditions(action, &bindings, &tw.world);
    assert!(matches!(
        result,
        longtable_parser::action::PreconditionResult::Fail { .. }
    ));
}

#[test]
fn wait_action_passes_time() {
    let tw = create_test_world();
    let mut executor = ActionExecutor::new();

    let action = tw.actions.get(tw.kw.stdlib.action_wait).unwrap();
    let bindings = ActionBindings::new();

    let _world = executor.execute(action, &bindings, tw.world).unwrap();

    assert!(
        executor
            .messages()
            .iter()
            .any(|m| m.contains("Time passes"))
    );
}

// =============================================================================
// Tests - Vocabulary
// =============================================================================

#[test]
fn vocabulary_has_directions() {
    let tw = create_test_world();

    assert!(tw.vocab.lookup_direction(tw.kw.stdlib.north).is_some());
    assert!(tw.vocab.lookup_direction(tw.kw.stdlib.south).is_some());
    assert!(tw.vocab.lookup_direction(tw.kw.stdlib.east).is_some());
    assert!(tw.vocab.lookup_direction(tw.kw.stdlib.west).is_some());
}

#[test]
fn vocabulary_has_verbs() {
    let tw = create_test_world();

    assert!(tw.vocab.lookup_verb(tw.kw.stdlib.look).is_some());
    assert!(tw.vocab.lookup_verb(tw.kw.stdlib.take).is_some());
    assert!(tw.vocab.lookup_verb(tw.kw.stdlib.drop).is_some());
    assert!(tw.vocab.lookup_verb(tw.kw.stdlib.attack).is_some());
}

#[test]
fn vocabulary_has_prepositions() {
    let tw = create_test_world();

    assert!(tw.vocab.lookup_preposition(tw.kw.stdlib.prep_in).is_some());
    assert!(tw.vocab.lookup_preposition(tw.kw.stdlib.prep_on).is_some());
    assert!(
        tw.vocab
            .lookup_preposition(tw.kw.stdlib.prep_with)
            .is_some()
    );
}

// =============================================================================
// Tests - Syntaxes
// =============================================================================

#[test]
fn syntaxes_exist_for_commands() {
    let tw = create_test_world();

    // Find look syntax
    let look_syntax = tw.syntaxes.iter().find(|s| s.command == tw.kw.stdlib.look);
    assert!(look_syntax.is_some());

    // Find take syntax
    let take_syntax = tw.syntaxes.iter().find(|s| s.command == tw.kw.stdlib.take);
    assert!(take_syntax.is_some());

    // Find go syntax
    let go_syntax = tw.syntaxes.iter().find(|s| s.command == tw.kw.stdlib.go);
    assert!(go_syntax.is_some());
}

// =============================================================================
// Tests - Movement
// =============================================================================

#[test]
fn player_can_move_north() {
    let tw = create_test_world();

    // Get destination
    let destinations: Vec<_> = tw
        .world
        .targets(tw.cave_entrance, tw.kw.exit_north)
        .collect();
    assert_eq!(destinations.len(), 1);
    let destination = destinations[0];

    // Move player
    let world = tw
        .world
        .unlink(tw.player, tw.kw.in_room, tw.cave_entrance)
        .unwrap();
    let world = world.link(tw.player, tw.kw.in_room, destination).unwrap();

    // Verify
    let rooms: Vec<_> = world.targets(tw.player, tw.kw.in_room).collect();
    assert_eq!(rooms.len(), 1);
    assert_eq!(rooms[0], tw.main_hall);
}

#[test]
fn player_can_navigate_full_path() {
    let tw = create_test_world();
    let mut world = tw.world;

    // Start: Cave Entrance
    // Go north to Main Hall
    world = world
        .unlink(tw.player, tw.kw.in_room, tw.cave_entrance)
        .unwrap();
    world = world.link(tw.player, tw.kw.in_room, tw.main_hall).unwrap();

    // Go east to Armory
    world = world
        .unlink(tw.player, tw.kw.in_room, tw.main_hall)
        .unwrap();
    world = world.link(tw.player, tw.kw.in_room, tw.armory).unwrap();

    // Verify in Armory
    let rooms: Vec<_> = world.targets(tw.player, tw.kw.in_room).collect();
    assert_eq!(rooms[0], tw.armory);

    // Go west back to Main Hall
    world = world.unlink(tw.player, tw.kw.in_room, tw.armory).unwrap();
    world = world.link(tw.player, tw.kw.in_room, tw.main_hall).unwrap();

    // Go south to Cave Entrance
    world = world
        .unlink(tw.player, tw.kw.in_room, tw.main_hall)
        .unwrap();
    world = world
        .link(tw.player, tw.kw.in_room, tw.cave_entrance)
        .unwrap();

    // Back at start
    let rooms: Vec<_> = world.targets(tw.player, tw.kw.in_room).collect();
    assert_eq!(rooms[0], tw.cave_entrance);
}

// =============================================================================
// Tests - Item Manipulation
// =============================================================================

#[test]
fn player_can_take_sword() {
    let tw = create_test_world();

    // Sword starts in room
    let rooms: Vec<_> = tw.world.targets(tw.sword, tw.kw.in_room).collect();
    assert_eq!(rooms.len(), 1);

    // Take sword
    let world = tw
        .world
        .unlink(tw.sword, tw.kw.in_room, tw.cave_entrance)
        .unwrap();
    let world = world.link(tw.sword, tw.kw.in_inventory, tw.player).unwrap();

    // Sword in inventory
    let inv: Vec<_> = world.targets(tw.sword, tw.kw.in_inventory).collect();
    assert_eq!(inv.len(), 1);
    assert_eq!(inv[0], tw.player);

    // Sword not in room
    let rooms: Vec<_> = world.targets(tw.sword, tw.kw.in_room).collect();
    assert!(rooms.is_empty());
}

#[test]
fn player_can_drop_item() {
    let tw = create_test_world();

    // Take sword first
    let world = tw
        .world
        .unlink(tw.sword, tw.kw.in_room, tw.cave_entrance)
        .unwrap();
    let world = world.link(tw.sword, tw.kw.in_inventory, tw.player).unwrap();

    // Drop sword
    let world = world
        .unlink(tw.sword, tw.kw.in_inventory, tw.player)
        .unwrap();
    let world = world
        .link(tw.sword, tw.kw.in_room, tw.cave_entrance)
        .unwrap();

    // Sword back in room
    let rooms: Vec<_> = world.targets(tw.sword, tw.kw.in_room).collect();
    assert_eq!(rooms.len(), 1);
    assert_eq!(rooms[0], tw.cave_entrance);
}

// =============================================================================
// Tests - Containers
// =============================================================================

#[test]
fn can_open_chest() {
    let tw = create_test_world();

    // Chest starts closed
    assert!(!tw.world.has(tw.chest, tw.kw.open));

    // Open it
    let world = tw
        .world
        .set(tw.chest, tw.kw.open, Value::Bool(true))
        .unwrap();

    // Now open
    assert!(world.has(tw.chest, tw.kw.open));
}

#[test]
fn can_put_item_in_container() {
    let tw = create_test_world();

    // Open chest first
    let world = tw
        .world
        .set(tw.chest, tw.kw.open, Value::Bool(true))
        .unwrap();

    // Take key (it's in main hall)
    let world = world.unlink(tw.key, tw.kw.in_room, tw.main_hall).unwrap();
    let world = world.link(tw.key, tw.kw.in_inventory, tw.player).unwrap();

    // Put key in chest
    let world = world.unlink(tw.key, tw.kw.in_inventory, tw.player).unwrap();
    let world = world.link(tw.key, tw.kw.in_container, tw.chest).unwrap();

    // Key is in chest
    let containers: Vec<_> = world.targets(tw.key, tw.kw.in_container).collect();
    assert_eq!(containers.len(), 1);
    assert_eq!(containers[0], tw.chest);
}

// =============================================================================
// Tests - Combat
// =============================================================================

#[test]
fn can_damage_enemy() {
    let tw = create_test_world();

    // Get goblin health
    let hp = tw
        .world
        .get_field(tw.goblin, tw.kw.health, tw.kw.current)
        .unwrap();
    assert_eq!(hp, Some(Value::Int(30)));

    // Deal 10 damage
    let new_hp = 30 - 10;
    let world = tw
        .world
        .set_field(tw.goblin, tw.kw.health, tw.kw.current, Value::Int(new_hp))
        .unwrap();

    // Health reduced
    let hp = world
        .get_field(tw.goblin, tw.kw.health, tw.kw.current)
        .unwrap();
    assert_eq!(hp, Some(Value::Int(20)));
}

#[test]
fn enemy_can_die() {
    let tw = create_test_world();

    // Kill goblin
    let world = tw
        .world
        .set_field(tw.goblin, tw.kw.health, tw.kw.current, Value::Int(0))
        .unwrap();
    let world = world.set(tw.goblin, tw.kw.dead, Value::Bool(true)).unwrap();

    // Goblin is dead
    assert!(world.has(tw.goblin, tw.kw.dead));

    // Can't attack dead things
    let executor = ActionExecutor::new();
    let action = tw.actions.get(tw.kw.stdlib.action_attack).unwrap();

    let mut bindings = ActionBindings::new();
    bindings.bind_entity("target", tw.goblin);

    let result = executor.check_preconditions(action, &bindings, &world);
    assert!(matches!(
        result,
        longtable_parser::action::PreconditionResult::Fail { .. }
    ));
}
