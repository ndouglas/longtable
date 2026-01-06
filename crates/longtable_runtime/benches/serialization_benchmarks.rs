//! Benchmarks for Longtable serialization (MessagePack).
//!
//! Run with: `cargo bench --package longtable_runtime`

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};

use longtable_foundation::{LtMap, Type, Value};
use longtable_runtime::{from_bytes, to_bytes};
use longtable_storage::World;
use longtable_storage::schema::{
    Cardinality, ComponentSchema, FieldSchema, OnDelete, RelationshipSchema,
};

// =============================================================================
// Helper Functions
// =============================================================================

/// Creates a minimal world with just components registered.
fn create_minimal_world() -> World {
    let mut world = World::new(42);

    let health = world.interner_mut().intern_keyword("health");
    let current = world.interner_mut().intern_keyword("current");
    let max = world.interner_mut().intern_keyword("max");

    let schema = ComponentSchema::new(health)
        .with_field(FieldSchema::required(current, Type::Int))
        .with_field(FieldSchema::required(max, Type::Int));
    world = world.register_component(schema).unwrap();

    world
}

/// Creates a world with the given number of entities.
fn create_world_with_entities(count: usize) -> World {
    let mut world = World::new(42);

    // Register components
    let health = world.interner_mut().intern_keyword("health");
    let current = world.interner_mut().intern_keyword("current");
    let max = world.interner_mut().intern_keyword("max");
    let position = world.interner_mut().intern_keyword("position");
    let x = world.interner_mut().intern_keyword("x");
    let y = world.interner_mut().intern_keyword("y");
    let name = world.interner_mut().intern_keyword("name");
    let value = world.interner_mut().intern_keyword("value");
    let tag_player = world.interner_mut().intern_keyword("tag/player");
    let tag_enemy = world.interner_mut().intern_keyword("tag/enemy");

    let health_schema = ComponentSchema::new(health)
        .with_field(FieldSchema::required(current, Type::Int))
        .with_field(FieldSchema::required(max, Type::Int));
    world = world.register_component(health_schema).unwrap();

    let position_schema = ComponentSchema::new(position)
        .with_field(FieldSchema::required(x, Type::Int))
        .with_field(FieldSchema::required(y, Type::Int));
    world = world.register_component(position_schema).unwrap();

    let name_schema =
        ComponentSchema::new(name).with_field(FieldSchema::required(value, Type::String));
    world = world.register_component(name_schema).unwrap();

    world = world
        .register_component(ComponentSchema::tag(tag_player))
        .unwrap();
    world = world
        .register_component(ComponentSchema::tag(tag_enemy))
        .unwrap();

    // Create entities
    for i in 0..count {
        let mut components = LtMap::new();

        // Health
        let mut health_data = LtMap::new();
        health_data = health_data.insert(Value::Keyword(current), Value::Int((i % 100) as i64));
        health_data = health_data.insert(Value::Keyword(max), Value::Int(100));
        components = components.insert(Value::Keyword(health), Value::Map(health_data));

        // Position
        let mut pos_data = LtMap::new();
        pos_data = pos_data.insert(Value::Keyword(x), Value::Int((i % 100) as i64));
        pos_data = pos_data.insert(Value::Keyword(y), Value::Int((i / 100) as i64));
        components = components.insert(Value::Keyword(position), Value::Map(pos_data));

        // Name
        let mut name_data = LtMap::new();
        name_data = name_data.insert(
            Value::Keyword(value),
            Value::String(format!("Entity{i}").into()),
        );
        components = components.insert(Value::Keyword(name), Value::Map(name_data));

        // Tag
        if i % 10 == 0 {
            components = components.insert(Value::Keyword(tag_player), Value::Bool(true));
        } else {
            components = components.insert(Value::Keyword(tag_enemy), Value::Bool(true));
        }

        let (w, _) = world.spawn(&components).unwrap();
        world = w;
    }

    world
}

/// Creates a world with rooms and items in relationships.
fn create_world_with_relationships(rooms: usize, items_per_room: usize) -> World {
    let mut world = World::new(42);

    // Register components
    let tag_room = world.interner_mut().intern_keyword("tag/room");
    let tag_item = world.interner_mut().intern_keyword("tag/item");
    let name = world.interner_mut().intern_keyword("name");
    let value = world.interner_mut().intern_keyword("value");

    world = world
        .register_component(ComponentSchema::tag(tag_room))
        .unwrap();
    world = world
        .register_component(ComponentSchema::tag(tag_item))
        .unwrap();

    let name_schema =
        ComponentSchema::new(name).with_field(FieldSchema::required(value, Type::String));
    world = world.register_component(name_schema).unwrap();

    // Register relationship
    let in_room = world.interner_mut().intern_keyword("in-room");
    world = world
        .register_relationship(
            RelationshipSchema::new(in_room)
                .with_cardinality(Cardinality::ManyToOne)
                .with_on_delete(OnDelete::Remove),
        )
        .unwrap();

    // Create rooms
    let mut room_entities = Vec::new();
    for i in 0..rooms {
        let mut components = LtMap::new();
        components = components.insert(Value::Keyword(tag_room), Value::Bool(true));

        let mut name_data = LtMap::new();
        name_data = name_data.insert(
            Value::Keyword(value),
            Value::String(format!("Room{i}").into()),
        );
        components = components.insert(Value::Keyword(name), Value::Map(name_data));

        let (w, room) = world.spawn(&components).unwrap();
        world = w;
        room_entities.push(room);
    }

    // Create items in rooms
    for (room_idx, room) in room_entities.iter().enumerate() {
        for j in 0..items_per_room {
            let mut components = LtMap::new();
            components = components.insert(Value::Keyword(tag_item), Value::Bool(true));

            let mut name_data = LtMap::new();
            name_data = name_data.insert(
                Value::Keyword(value),
                Value::String(format!("Item{room_idx}_{j}").into()),
            );
            components = components.insert(Value::Keyword(name), Value::Map(name_data));

            let (w, item) = world.spawn(&components).unwrap();
            world = w;
            world = world.link(item, in_room, *room).unwrap();
        }
    }

    world
}

// =============================================================================
// Serialization Benchmarks
// =============================================================================

fn bench_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialize");

    // Empty world
    let world = create_minimal_world();
    let size = to_bytes(&world).unwrap().len();
    group.throughput(Throughput::Bytes(size as u64));
    group.bench_function("empty_world", |b| {
        b.iter(|| black_box(to_bytes(&world).unwrap()))
    });

    // World with entities at different scales
    for entity_count in [10, 100, 1_000, 10_000] {
        let world = create_world_with_entities(entity_count);
        let bytes = to_bytes(&world).unwrap();
        let size = bytes.len();

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new("entities", entity_count),
            &world,
            |b, w| b.iter(|| black_box(to_bytes(w).unwrap())),
        );
    }

    // World with relationships
    let world = create_world_with_relationships(20, 50); // 20 rooms, 50 items each = 1020 entities
    let size = to_bytes(&world).unwrap().len();
    group.throughput(Throughput::Bytes(size as u64));
    group.bench_function("with_relationships", |b| {
        b.iter(|| black_box(to_bytes(&world).unwrap()))
    });

    group.finish();
}

fn bench_deserialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("deserialize");

    // Empty world
    let world = create_minimal_world();
    let bytes = to_bytes(&world).unwrap();
    let size = bytes.len();
    group.throughput(Throughput::Bytes(size as u64));
    group.bench_function("empty_world", |b| {
        b.iter(|| black_box(from_bytes(&bytes).unwrap()))
    });

    // World with entities at different scales
    for entity_count in [10, 100, 1_000, 10_000] {
        let world = create_world_with_entities(entity_count);
        let bytes = to_bytes(&world).unwrap();
        let size = bytes.len();

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new("entities", entity_count),
            &bytes,
            |b, data| b.iter(|| black_box(from_bytes(data).unwrap())),
        );
    }

    // World with relationships
    let world = create_world_with_relationships(20, 50);
    let bytes = to_bytes(&world).unwrap();
    let size = bytes.len();
    group.throughput(Throughput::Bytes(size as u64));
    group.bench_function("with_relationships", |b| {
        b.iter(|| black_box(from_bytes(&bytes).unwrap()))
    });

    group.finish();
}

fn bench_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("roundtrip");

    for entity_count in [100, 1_000, 10_000] {
        let original = create_world_with_entities(entity_count);
        let bytes = to_bytes(&original).unwrap();
        let size = bytes.len();

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new("entities", entity_count),
            &original,
            |b, world| {
                b.iter(|| {
                    let serialized = to_bytes(world).unwrap();
                    let deserialized: World = from_bytes(&serialized).unwrap();
                    black_box(deserialized)
                })
            },
        );
    }

    group.finish();
}

fn bench_serialized_size(c: &mut Criterion) {
    // This benchmark group just reports sizes, useful for tracking size regressions
    let mut group = c.benchmark_group("size_check");
    group.sample_size(10); // Just a few samples since we're measuring size consistency

    for entity_count in [100, 1_000, 10_000] {
        let world = create_world_with_entities(entity_count);
        let bytes = to_bytes(&world).unwrap();
        let expected_size = bytes.len();

        // Use entity count as throughput to see bytes per entity
        group.throughput(Throughput::Elements(entity_count as u64));
        group.bench_with_input(
            BenchmarkId::new("bytes_per_entity", entity_count),
            &world,
            |b, w| {
                b.iter(|| {
                    let result = to_bytes(w).unwrap();
                    assert_eq!(result.len(), expected_size);
                    black_box(result.len())
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_serialize,
    bench_deserialize,
    bench_roundtrip,
    bench_serialized_size,
);

criterion_main!(benches);
