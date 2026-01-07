//! Large-scale benchmarks for Longtable storage layer.
//!
//! Run with: `cargo bench --package longtable_storage --bench scale_benchmarks`
//!
//! WARNING: These benchmarks can take significant time.
//! Use `cargo bench --package longtable_storage --bench scale_benchmarks -- <filter>` to run specific tests.

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};

use longtable_foundation::{LtMap, Type, Value};
use longtable_storage::World;
use longtable_storage::schema::{
    Cardinality, ComponentSchema, FieldSchema, OnDelete, RelationshipSchema,
};

// =============================================================================
// Helper Functions
// =============================================================================

/// Creates a world with the given number of entities, each with health and position.
fn create_world_with_entities(count: usize) -> World {
    let mut world = World::new(42);

    // Register components
    let health = world.interner_mut().intern_keyword("health");
    let current = world.interner_mut().intern_keyword("current");
    let max = world.interner_mut().intern_keyword("max");
    let position = world.interner_mut().intern_keyword("position");
    let x = world.interner_mut().intern_keyword("x");
    let y = world.interner_mut().intern_keyword("y");
    let tag_active = world.interner_mut().intern_keyword("tag/active");

    let health_schema = ComponentSchema::new(health)
        .with_field(FieldSchema::required(current, Type::Int))
        .with_field(FieldSchema::optional(max, Type::Int, Value::Int(100)));
    world = world.register_component(health_schema).unwrap();

    let position_schema = ComponentSchema::new(position)
        .with_field(FieldSchema::required(x, Type::Int))
        .with_field(FieldSchema::required(y, Type::Int));
    world = world.register_component(position_schema).unwrap();

    world = world
        .register_component(ComponentSchema::tag(tag_active))
        .unwrap();

    // Create entities
    for i in 0..count {
        let mut components = LtMap::new();

        // Health component
        let mut health_data = LtMap::new();
        health_data = health_data.insert(Value::Keyword(current), Value::Int((i % 100) as i64));
        health_data = health_data.insert(Value::Keyword(max), Value::Int(100));
        components = components.insert(Value::Keyword(health), Value::Map(health_data));

        // Position component
        let mut pos_data = LtMap::new();
        pos_data = pos_data.insert(Value::Keyword(x), Value::Int((i % 1000) as i64));
        pos_data = pos_data.insert(Value::Keyword(y), Value::Int((i / 1000) as i64));
        components = components.insert(Value::Keyword(position), Value::Map(pos_data));

        // Only 10% are active
        if i % 10 == 0 {
            components = components.insert(Value::Keyword(tag_active), Value::Bool(true));
        }

        let (w, _) = world.spawn(&components).unwrap();
        world = w;
    }

    world
}

/// Creates a world with a chain of relationships.
fn create_world_with_relationship_chain(chain_length: usize) -> World {
    let mut world = World::new(42);

    let tag = world.interner_mut().intern_keyword("tag/node");
    world = world.register_component(ComponentSchema::tag(tag)).unwrap();

    let next = world.interner_mut().intern_keyword("next");
    world = world
        .register_relationship(
            RelationshipSchema::new(next)
                .with_cardinality(Cardinality::OneToOne)
                .with_on_delete(OnDelete::Remove),
        )
        .unwrap();

    // Create chain of entities
    let mut entities = Vec::new();
    for _ in 0..chain_length {
        let mut components = LtMap::new();
        components = components.insert(Value::Keyword(tag), Value::Bool(true));
        let (w, e) = world.spawn(&components).unwrap();
        world = w;
        entities.push(e);
    }

    // Link them in a chain
    for i in 0..chain_length - 1 {
        world = world.link(entities[i], next, entities[i + 1]).unwrap();
    }

    world
}

/// Creates a world with a graph of relationships.
fn create_world_with_relationship_graph(nodes: usize, edges_per_node: usize) -> World {
    let mut world = World::new(42);

    let tag = world.interner_mut().intern_keyword("tag/node");
    world = world.register_component(ComponentSchema::tag(tag)).unwrap();

    let connected = world.interner_mut().intern_keyword("connected");
    world = world
        .register_relationship(
            RelationshipSchema::new(connected)
                .with_cardinality(Cardinality::ManyToMany)
                .with_on_delete(OnDelete::Remove),
        )
        .unwrap();

    // Create nodes
    let mut entities = Vec::new();
    for _ in 0..nodes {
        let mut components = LtMap::new();
        components = components.insert(Value::Keyword(tag), Value::Bool(true));
        let (w, e) = world.spawn(&components).unwrap();
        world = w;
        entities.push(e);
    }

    // Create edges (deterministic pattern)
    for i in 0..nodes {
        for j in 0..edges_per_node {
            let target = (i + j + 1) % nodes;
            if target != i {
                world = world
                    .link(entities[i], connected, entities[target])
                    .unwrap();
            }
        }
    }

    world
}

// =============================================================================
// Entity Scale Benchmarks
// =============================================================================

fn bench_entity_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("entity_scale");
    group.sample_size(20);

    // World creation at scale
    for count in [500, 1_000, 2_500] {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("world_create", count),
            &count,
            |b, &count| b.iter(|| black_box(create_world_with_entities(count))),
        );
    }

    // World clone at scale (important for immutable operations)
    for count in [500, 1_000, 2_500] {
        let world = create_world_with_entities(count);

        group.bench_with_input(BenchmarkId::new("world_clone", count), &world, |b, w| {
            b.iter(|| black_box(w.clone()))
        });
    }

    // Entity spawn into large world
    for count in [500, 1_000, 2_500] {
        let mut world = create_world_with_entities(count);
        let health = world.interner_mut().intern_keyword("health");
        let current = world.interner_mut().intern_keyword("current");

        let mut components = LtMap::new();
        let mut health_data = LtMap::new();
        health_data = health_data.insert(Value::Keyword(current), Value::Int(100));
        components = components.insert(Value::Keyword(health), Value::Map(health_data));

        group.bench_with_input(
            BenchmarkId::new("spawn_into", count),
            &(world, components),
            |b, (w, c)| {
                b.iter_batched(
                    || w.clone(),
                    |world| black_box(world.spawn(c)),
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }

    // Component iteration at scale
    for count in [500, 1_000, 2_500] {
        let mut world = create_world_with_entities(count);
        let health = world.interner_mut().intern_keyword("health");

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("iterate_component", count),
            &world,
            |b, w| {
                b.iter(|| {
                    let count = w.with_component(health).count();
                    black_box(count)
                })
            },
        );
    }

    // Component get at scale (random access pattern)
    for count in [500, 1_000, 2_500] {
        let mut world = create_world_with_entities(count);
        let health = world.interner_mut().intern_keyword("health");
        let entities: Vec<_> = world.with_component(health).collect();

        // Get first, middle, and last entities
        let sample = vec![
            entities[0],
            entities[entities.len() / 2],
            entities[entities.len() - 1],
        ];

        group.bench_with_input(
            BenchmarkId::new("get_component_sample", count),
            &(world, sample),
            |b, (w, sample)| {
                b.iter(|| {
                    for &e in sample {
                        let _ = black_box(w.get(e, health));
                    }
                })
            },
        );
    }

    // Component set at scale
    for count in [500, 1_000] {
        let mut world = create_world_with_entities(count);
        let health = world.interner_mut().intern_keyword("health");
        let current = world.interner_mut().intern_keyword("current");
        let entities: Vec<_> = world.with_component(health).take(100).collect();

        let mut new_health = LtMap::new();
        new_health = new_health.insert(Value::Keyword(current), Value::Int(50));
        let new_value = Value::Map(new_health);

        group.bench_with_input(
            BenchmarkId::new("set_100_components", count),
            &(world, entities, new_value),
            |b, (w, ents, val)| {
                b.iter_batched(
                    || w.clone(),
                    |mut world| {
                        for &e in ents {
                            world = world.set(e, health, val.clone()).unwrap();
                        }
                        black_box(world)
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

// =============================================================================
// Relationship Scale Benchmarks
// =============================================================================

fn bench_relationship_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("relationship_scale");
    group.sample_size(20);

    // Relationship chain creation
    for length in [100, 500, 1_000] {
        group.throughput(Throughput::Elements(length as u64));
        group.bench_with_input(
            BenchmarkId::new("chain_create", length),
            &length,
            |b, &length| b.iter(|| black_box(create_world_with_relationship_chain(length))),
        );
    }

    // Chain traversal (single hop)
    for length in [500, 1_000] {
        let mut world = create_world_with_relationship_chain(length);
        let next = world.interner_mut().intern_keyword("next");
        let tag = world.interner_mut().intern_keyword("tag/node");

        let start = world.with_component(tag).next().unwrap();

        group.bench_with_input(
            BenchmarkId::new("chain_single_hop", length),
            &(world, start),
            |b, (w, start)| {
                b.iter(|| {
                    let target = w.targets(*start, next).next();
                    black_box(target)
                })
            },
        );
    }

    // Chain full traversal
    for length in [100, 500, 1_000] {
        let mut world = create_world_with_relationship_chain(length);
        let next = world.interner_mut().intern_keyword("next");
        let tag = world.interner_mut().intern_keyword("tag/node");

        let start = world.with_component(tag).next().unwrap();

        group.throughput(Throughput::Elements(length as u64));
        group.bench_with_input(
            BenchmarkId::new("chain_full_traverse", length),
            &(world, start),
            |b, (w, start)| {
                b.iter(|| {
                    let mut count = 0;
                    let mut current = *start;
                    while let Some(n) = w.targets(current, next).next() {
                        current = n;
                        count += 1;
                    }
                    black_box(count)
                })
            },
        );
    }

    // Graph creation with edges
    for (nodes, edges_per_node) in [(100, 5), (1_000, 5), (1_000, 10)] {
        let total_edges = nodes * edges_per_node;
        group.throughput(Throughput::Elements(total_edges as u64));
        group.bench_with_input(
            BenchmarkId::new("graph_create", format!("{nodes}n_{edges_per_node}e")),
            &(nodes, edges_per_node),
            |b, &(nodes, edges)| {
                b.iter(|| black_box(create_world_with_relationship_graph(nodes, edges)))
            },
        );
    }

    // Graph edge iteration
    for (nodes, edges_per_node) in [(500, 5), (1_000, 5), (1_000, 10)] {
        let world = create_world_with_relationship_graph(nodes, edges_per_node);
        let mut world_mut = world.clone();
        let connected = world_mut.interner_mut().intern_keyword("connected");
        let tag = world_mut.interner_mut().intern_keyword("tag/node");

        let sample: Vec<_> = world_mut.with_component(tag).take(100).collect();

        let total_edges = 100 * edges_per_node;
        group.throughput(Throughput::Elements(total_edges as u64));
        group.bench_with_input(
            BenchmarkId::new("graph_iterate_edges", format!("{nodes}n_{edges_per_node}e")),
            &(world_mut, sample),
            |b, (w, sample)| {
                b.iter(|| {
                    let mut count = 0;
                    for &node in sample {
                        count += w.targets(node, connected).count();
                    }
                    black_box(count)
                })
            },
        );
    }

    group.finish();
}

// =============================================================================
// Sparse World Benchmarks
// =============================================================================

fn bench_sparse_world(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_world");
    group.sample_size(20);

    // Create world where only 10% of entities are "active"
    // Tests how well we can filter/query sparse data

    for count in [1_000, 2_500, 5_000] {
        let mut world = create_world_with_entities(count);
        let tag_active = world.interner_mut().intern_keyword("tag/active");

        // 10% have tag/active
        let active_count = count / 10;
        group.throughput(Throughput::Elements(active_count as u64));

        group.bench_with_input(
            BenchmarkId::new("iterate_sparse_10pct", count),
            &world,
            |b, w| {
                b.iter(|| {
                    let count = w.with_component(tag_active).count();
                    black_box(count)
                })
            },
        );
    }

    // Iteration over all entities but only processing active ones
    for count in [2_500, 5_000] {
        let mut world = create_world_with_entities(count);
        let health = world.interner_mut().intern_keyword("health");
        let tag_active = world.interner_mut().intern_keyword("tag/active");

        group.bench_with_input(
            BenchmarkId::new("filter_sparse_10pct", count),
            &world,
            |b, w| {
                b.iter(|| {
                    let count = w
                        .with_component(health)
                        .filter(|&e| w.get(e, tag_active).is_ok())
                        .count();
                    black_box(count)
                })
            },
        );
    }

    group.finish();
}

// =============================================================================
// Batch Operations Benchmarks
// =============================================================================

fn bench_batch_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_operations");
    group.sample_size(20);

    // Batch spawn
    for batch_size in [100, 500, 1_000] {
        let mut template_world = World::new(42);
        let health = template_world.interner_mut().intern_keyword("health");
        let current = template_world.interner_mut().intern_keyword("current");

        let health_schema =
            ComponentSchema::new(health).with_field(FieldSchema::required(current, Type::Int));
        template_world = template_world.register_component(health_schema).unwrap();

        let mut components = LtMap::new();
        let mut health_data = LtMap::new();
        health_data = health_data.insert(Value::Keyword(current), Value::Int(100));
        components = components.insert(Value::Keyword(health), Value::Map(health_data));

        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("batch_spawn", batch_size),
            &(template_world, components, batch_size),
            |b, (w, c, size)| {
                b.iter_batched(
                    || w.clone(),
                    |mut world| {
                        for _ in 0..*size {
                            let (w, _) = world.spawn(c).unwrap();
                            world = w;
                        }
                        black_box(world)
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }

    // Batch destroy
    for batch_size in [100, 500] {
        let mut world = create_world_with_entities(1000);
        let health = world.interner_mut().intern_keyword("health");
        let to_destroy: Vec<_> = world.with_component(health).take(batch_size).collect();

        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("batch_destroy", batch_size),
            &(world, to_destroy),
            |b, (w, targets)| {
                b.iter_batched(
                    || w.clone(),
                    |mut world| {
                        for &e in targets {
                            world = world.destroy(e).unwrap();
                        }
                        black_box(world)
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

// =============================================================================
// Criterion Groups
// =============================================================================

criterion_group!(
    benches,
    bench_entity_scale,
    bench_relationship_scale,
    bench_sparse_world,
    bench_batch_operations,
);

criterion_main!(benches);
