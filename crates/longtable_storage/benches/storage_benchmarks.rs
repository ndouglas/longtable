//! Benchmarks for the Longtable storage layer.
//!
//! Run with: `cargo bench --package longtable_storage`

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};

use longtable_foundation::{EntityId, Interner, LtMap, Type, Value};
use longtable_storage::{
    ComponentSchema, ComponentStore, EntityStore, FieldSchema, RelationshipSchema,
    RelationshipStore, World,
};

// =============================================================================
// Entity Store Benchmarks
// =============================================================================

fn bench_entity_store(c: &mut Criterion) {
    let mut group = c.benchmark_group("entity_store");

    // Spawn
    for size in [100, 1_000, 10_000] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("spawn", size), &size, |b, &size| {
            b.iter(|| {
                let mut store = EntityStore::new();
                for _ in 0..size {
                    black_box(store.spawn());
                }
                black_box(store)
            })
        });
    }

    // Exists check
    for size in [100, 1_000, 10_000] {
        let mut store = EntityStore::new();
        let entities: Vec<_> = (0..size).map(|_| store.spawn()).collect();
        let mid = &entities[size / 2];

        group.bench_with_input(BenchmarkId::new("exists", size), mid, |b, e| {
            b.iter(|| black_box(store.exists(*e)))
        });
    }

    // Validate check
    for size in [100, 1_000, 10_000] {
        let mut store = EntityStore::new();
        let entities: Vec<_> = (0..size).map(|_| store.spawn()).collect();
        let mid = &entities[size / 2];

        group.bench_with_input(BenchmarkId::new("validate", size), mid, |b, e| {
            b.iter(|| black_box(store.validate(*e)))
        });
    }

    // Iteration
    for size in [100, 1_000, 10_000] {
        let mut store = EntityStore::new();
        for _ in 0..size {
            store.spawn();
        }

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("iterate", size), &store, |b, s| {
            b.iter(|| {
                let mut count = 0;
                for e in s.iter() {
                    black_box(e);
                    count += 1;
                }
                black_box(count)
            })
        });
    }

    // Destroy and reuse
    group.bench_function("spawn_destroy_cycle", |b| {
        b.iter_batched(
            || {
                let mut store = EntityStore::new();
                let entity = store.spawn();
                (store, entity)
            },
            |(mut store, entity)| {
                store.destroy(entity).unwrap();
                black_box(store.spawn())
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

// =============================================================================
// Component Store Benchmarks
// =============================================================================

fn bench_component_store(c: &mut Criterion) {
    let mut group = c.benchmark_group("component_store");

    // Setup helper
    fn setup_store(interner: &mut Interner) -> (ComponentStore, Vec<EntityId>) {
        let mut store = ComponentStore::new();

        // Register some schemas
        let health = interner.intern_keyword("health");
        let current = interner.intern_keyword("current");
        let max = interner.intern_keyword("max");

        let schema = ComponentSchema::new(health)
            .with_field(FieldSchema::required(current, Type::Int))
            .with_field(FieldSchema::optional(max, Type::Int, Value::Int(100)));
        store.register_schema(schema).unwrap();

        let position = interner.intern_keyword("position");
        store
            .register_schema(ComponentSchema::tag(position))
            .unwrap();

        let velocity = interner.intern_keyword("velocity");
        store
            .register_schema(ComponentSchema::tag(velocity))
            .unwrap();

        let entities: Vec<_> = (0..1000).map(|i| EntityId::new(i, 1)).collect();
        (store, entities)
    }

    // Set component
    group.bench_function("set_tag", |b| {
        let mut interner = Interner::new();
        let (mut store, entities) = setup_store(&mut interner);
        let position = interner.intern_keyword("position");
        let mut idx = 0;

        b.iter(|| {
            let entity = entities[idx % entities.len()];
            idx += 1;
            black_box(store.set(entity, position, Value::Bool(true)))
        })
    });

    group.bench_function("set_structured", |b| {
        let mut interner = Interner::new();
        let (mut store, entities) = setup_store(&mut interner);
        let health = interner.intern_keyword("health");
        let current = interner.intern_keyword("current");
        let mut idx = 0;

        b.iter(|| {
            let entity = entities[idx % entities.len()];
            idx += 1;
            let mut map = LtMap::new();
            map = map.insert(Value::Keyword(current), Value::Int(100));
            black_box(store.set(entity, health, Value::Map(map)))
        })
    });

    // Get component
    group.bench_function("get", |b| {
        let mut interner = Interner::new();
        let (mut store, entities) = setup_store(&mut interner);
        let health = interner.intern_keyword("health");
        let current = interner.intern_keyword("current");

        // Populate some data
        for entity in &entities[..100] {
            let mut map = LtMap::new();
            map = map.insert(Value::Keyword(current), Value::Int(100));
            store.set(*entity, health, Value::Map(map)).unwrap();
        }

        let entity = entities[50];
        b.iter(|| black_box(store.get(entity, health)))
    });

    // Get field
    group.bench_function("get_field", |b| {
        let mut interner = Interner::new();
        let (mut store, entities) = setup_store(&mut interner);
        let health = interner.intern_keyword("health");
        let current = interner.intern_keyword("current");

        // Populate some data
        for entity in &entities[..100] {
            let mut map = LtMap::new();
            map = map.insert(Value::Keyword(current), Value::Int(100));
            store.set(*entity, health, Value::Map(map)).unwrap();
        }

        let entity = entities[50];
        b.iter(|| black_box(store.get_field(entity, health, current)))
    });

    // Has component check
    group.bench_function("has", |b| {
        let mut interner = Interner::new();
        let (mut store, entities) = setup_store(&mut interner);
        let position = interner.intern_keyword("position");

        // Set on half
        for entity in &entities[..500] {
            store.set(*entity, position, Value::Bool(true)).unwrap();
        }

        let entity = entities[250];
        b.iter(|| black_box(store.has(entity, position)))
    });

    // With component iteration
    for size in [100, 500, 1000] {
        let mut interner = Interner::new();
        let (mut store, entities) = setup_store(&mut interner);
        let position = interner.intern_keyword("position");

        for entity in &entities[..size] {
            store.set(*entity, position, Value::Bool(true)).unwrap();
        }

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::new("with_component", size),
            &(store, position),
            |b, (s, pos)| {
                b.iter(|| {
                    let mut count = 0;
                    for e in s.with_component(*pos) {
                        black_box(e);
                        count += 1;
                    }
                    black_box(count)
                })
            },
        );
    }

    group.finish();
}

// =============================================================================
// Relationship Store Benchmarks
// =============================================================================

fn bench_relationship_store(c: &mut Criterion) {
    let mut group = c.benchmark_group("relationship_store");

    // Setup helper
    fn setup_store(interner: &mut Interner) -> (RelationshipStore, Vec<EntityId>) {
        let mut store = RelationshipStore::new();

        let contains = interner.intern_keyword("contains");
        store
            .register_schema(RelationshipSchema::new(contains))
            .unwrap();

        let child_of = interner.intern_keyword("child-of");
        store
            .register_schema(RelationshipSchema::new(child_of))
            .unwrap();

        let entities: Vec<_> = (0..1000).map(|i| EntityId::new(i, 1)).collect();
        (store, entities)
    }

    // Link
    group.bench_function("link", |b| {
        let mut interner = Interner::new();
        let (store, entities) = setup_store(&mut interner);
        let contains = interner.intern_keyword("contains");

        b.iter_batched(
            || store.clone(),
            |mut s| black_box(s.link(entities[0], contains, entities[1])),
            criterion::BatchSize::SmallInput,
        )
    });

    // Build relationship graph
    for size in [100, 500, 1000] {
        let mut interner = Interner::new();
        let (store, entities) = setup_store(&mut interner);
        let contains = interner.intern_keyword("contains");

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::new("link_many", size),
            &(store, entities.clone(), contains),
            |b, (s, ents, rel)| {
                b.iter_batched(
                    || s.clone(),
                    |mut store| {
                        // Create a star topology: entity[0] contains all others
                        for i in 1..size {
                            store.link(ents[0], *rel, ents[i]).unwrap();
                        }
                        black_box(store)
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }

    // Forward traversal
    for size in [10, 100, 500] {
        let mut interner = Interner::new();
        let (mut store, entities) = setup_store(&mut interner);
        let contains = interner.intern_keyword("contains");

        // Build star topology
        for i in 1..=size {
            store.link(entities[0], contains, entities[i]).unwrap();
        }

        group.bench_with_input(
            BenchmarkId::new("targets", size),
            &(store, entities[0], contains),
            |b, (s, source, rel)| {
                b.iter(|| {
                    let mut count = 0;
                    for t in s.targets(*source, *rel) {
                        black_box(t);
                        count += 1;
                    }
                    black_box(count)
                })
            },
        );
    }

    // Reverse traversal
    for size in [10, 100, 500] {
        let mut interner = Interner::new();
        let (mut store, entities) = setup_store(&mut interner);
        let contains = interner.intern_keyword("contains");

        // Multiple sources point to entity[0]
        for i in 1..=size {
            store.link(entities[i], contains, entities[0]).unwrap();
        }

        group.bench_with_input(
            BenchmarkId::new("sources", size),
            &(store, entities[0], contains),
            |b, (s, target, rel)| {
                b.iter(|| {
                    let mut count = 0;
                    for src in s.sources(*target, *rel) {
                        black_box(src);
                        count += 1;
                    }
                    black_box(count)
                })
            },
        );
    }

    // Has edge check
    group.bench_function("has_edge", |b| {
        let mut interner = Interner::new();
        let (mut store, entities) = setup_store(&mut interner);
        let contains = interner.intern_keyword("contains");

        store.link(entities[0], contains, entities[1]).unwrap();

        b.iter(|| black_box(store.has_edge(entities[0], contains, entities[1])))
    });

    group.finish();
}

// =============================================================================
// World Benchmarks
// =============================================================================

fn bench_world(c: &mut Criterion) {
    let mut group = c.benchmark_group("world");

    // Spawn
    for size in [100, 500, 1000] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("spawn", size), &size, |b, &size| {
            b.iter(|| {
                let mut world = World::new(42);
                for _ in 0..size {
                    let (w, _) = world.spawn(&LtMap::new()).unwrap();
                    world = w;
                }
                black_box(world)
            })
        });
    }

    // Clone (structural sharing)
    for size in [100, 500, 1000] {
        let mut world = World::new(42);
        for _ in 0..size {
            let (w, _) = world.spawn(&LtMap::new()).unwrap();
            world = w;
        }

        group.bench_with_input(BenchmarkId::new("clone", size), &world, |b, w| {
            b.iter(|| black_box(w.clone()))
        });
    }

    // Spawn with components
    group.bench_function("spawn_with_components", |b| {
        let mut world = World::new(42);
        let health = world.interner_mut().intern_keyword("health");
        let current = world.interner_mut().intern_keyword("current");

        let schema =
            ComponentSchema::new(health).with_field(FieldSchema::required(current, Type::Int));
        world = world.register_component(schema).unwrap();

        let mut components = LtMap::new();
        let mut health_data = LtMap::new();
        health_data = health_data.insert(Value::Keyword(current), Value::Int(100));
        components = components.insert(Value::Keyword(health), Value::Map(health_data));

        b.iter_batched(
            || world.clone(),
            |w| black_box(w.spawn(&components)),
            criterion::BatchSize::SmallInput,
        )
    });

    // Set component
    group.bench_function("set", |b| {
        let mut world = World::new(42);
        let tag = world.interner_mut().intern_keyword("tag");
        world = world.register_component(ComponentSchema::tag(tag)).unwrap();
        let (world, entity) = world.spawn(&LtMap::new()).unwrap();

        b.iter_batched(
            || world.clone(),
            |w| black_box(w.set(entity, tag, Value::Bool(true))),
            criterion::BatchSize::SmallInput,
        )
    });

    // Get component
    group.bench_function("get", |b| {
        let mut world = World::new(42);
        let tag = world.interner_mut().intern_keyword("tag");
        world = world.register_component(ComponentSchema::tag(tag)).unwrap();
        let (world, entity) = world.spawn(&LtMap::new()).unwrap();
        let world = world.set(entity, tag, Value::Bool(true)).unwrap();

        b.iter(|| black_box(world.get(entity, tag)))
    });

    // Link
    group.bench_function("link", |b| {
        let mut world = World::new(42);
        let contains = world.interner_mut().intern_keyword("contains");
        world = world
            .register_relationship(RelationshipSchema::new(contains))
            .unwrap();

        let (world, room) = world.spawn(&LtMap::new()).unwrap();
        let (world, item) = world.spawn(&LtMap::new()).unwrap();

        b.iter_batched(
            || world.clone(),
            |w| black_box(w.link(room, contains, item)),
            criterion::BatchSize::SmallInput,
        )
    });

    // Advance tick
    group.bench_function("advance_tick", |b| {
        let mut world = World::new(42);
        for _ in 0..100 {
            let (w, _) = world.spawn(&LtMap::new()).unwrap();
            world = w;
        }

        b.iter_batched(
            || world.clone(),
            |w| black_box(w.advance_tick()),
            criterion::BatchSize::SmallInput,
        )
    });

    // Entity iteration
    for size in [100, 500, 1000] {
        let mut world = World::new(42);
        for _ in 0..size {
            let (w, _) = world.spawn(&LtMap::new()).unwrap();
            world = w;
        }

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("entities_iter", size), &world, |b, w| {
            b.iter(|| {
                let mut count = 0;
                for e in w.entities() {
                    black_box(e);
                    count += 1;
                }
                black_box(count)
            })
        });
    }

    // With component query
    for size in [100, 500, 1000] {
        let mut world = World::new(42);
        let tag = world.interner_mut().intern_keyword("tag");
        world = world.register_component(ComponentSchema::tag(tag)).unwrap();

        for _ in 0..size {
            let (w, e) = world.spawn(&LtMap::new()).unwrap();
            world = w.set(e, tag, Value::Bool(true)).unwrap();
        }

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::new("with_component", size),
            &(world, tag),
            |b, (w, t)| {
                b.iter(|| {
                    let mut count = 0;
                    for e in w.with_component(*t) {
                        black_box(e);
                        count += 1;
                    }
                    black_box(count)
                })
            },
        );
    }

    group.finish();
}

// =============================================================================
// Archetype Transition Benchmarks (Stage 4)
// =============================================================================

fn bench_archetype_transitions(c: &mut Criterion) {
    let mut group = c.benchmark_group("archetype_transitions");

    // Add component causing archetype change
    group.bench_function("add_component_archetype_change", |b| {
        let mut world = World::new(42);
        let health = world.interner_mut().intern_keyword("health");
        let current = world.interner_mut().intern_keyword("current");
        let armor = world.interner_mut().intern_keyword("armor");
        let defense = world.interner_mut().intern_keyword("defense");

        let health_schema =
            ComponentSchema::new(health).with_field(FieldSchema::required(current, Type::Int));
        let armor_schema =
            ComponentSchema::new(armor).with_field(FieldSchema::required(defense, Type::Int));

        world = world.register_component(health_schema).unwrap();
        world = world.register_component(armor_schema).unwrap();

        // Create entity with health only
        let mut health_data = LtMap::new();
        health_data = health_data.insert(Value::Keyword(current), Value::Int(100));
        let mut components = LtMap::new();
        components = components.insert(Value::Keyword(health), Value::Map(health_data));
        let (world, entity) = world.spawn(&components).unwrap();

        // Benchmark adding armor (archetype transition: {health} -> {health, armor})
        let mut armor_data = LtMap::new();
        armor_data = armor_data.insert(Value::Keyword(defense), Value::Int(50));

        b.iter_batched(
            || world.clone(),
            |w| black_box(w.set(entity, armor, Value::Map(armor_data.clone()))),
            criterion::BatchSize::SmallInput,
        )
    });

    // Add second component to entity (another archetype transition case)
    group.bench_function("add_second_component", |b| {
        let mut world = World::new(42);
        let health = world.interner_mut().intern_keyword("health");
        let current = world.interner_mut().intern_keyword("current");
        let position = world.interner_mut().intern_keyword("position");
        let x = world.interner_mut().intern_keyword("x");
        let y = world.interner_mut().intern_keyword("y");

        let health_schema =
            ComponentSchema::new(health).with_field(FieldSchema::required(current, Type::Int));
        let pos_schema = ComponentSchema::new(position)
            .with_field(FieldSchema::required(x, Type::Float))
            .with_field(FieldSchema::required(y, Type::Float));

        world = world.register_component(health_schema).unwrap();
        world = world.register_component(pos_schema).unwrap();

        // Create entity with health only
        let mut health_data = LtMap::new();
        health_data = health_data.insert(Value::Keyword(current), Value::Int(100));
        let mut components = LtMap::new();
        components = components.insert(Value::Keyword(health), Value::Map(health_data));
        let (world, entity) = world.spawn(&components).unwrap();

        // Benchmark adding position (archetype transition: {health} -> {health, position})
        let mut pos_data = LtMap::new();
        pos_data = pos_data.insert(Value::Keyword(x), Value::Float(10.0));
        pos_data = pos_data.insert(Value::Keyword(y), Value::Float(20.0));

        b.iter_batched(
            || world.clone(),
            |w| black_box(w.set(entity, position, Value::Map(pos_data.clone()))),
            criterion::BatchSize::SmallInput,
        )
    });

    // Entity destruction (removes entity from all archetypes)
    group.bench_function("destroy_entity_with_components", |b| {
        let mut world = World::new(42);
        let health = world.interner_mut().intern_keyword("health");
        let current = world.interner_mut().intern_keyword("current");
        let armor = world.interner_mut().intern_keyword("armor");
        let defense = world.interner_mut().intern_keyword("defense");

        let health_schema =
            ComponentSchema::new(health).with_field(FieldSchema::required(current, Type::Int));
        let armor_schema =
            ComponentSchema::new(armor).with_field(FieldSchema::required(defense, Type::Int));

        world = world.register_component(health_schema).unwrap();
        world = world.register_component(armor_schema).unwrap();

        // Create entity with health and armor
        let mut health_data = LtMap::new();
        health_data = health_data.insert(Value::Keyword(current), Value::Int(100));
        let mut armor_data = LtMap::new();
        armor_data = armor_data.insert(Value::Keyword(defense), Value::Int(50));
        let mut components = LtMap::new();
        components = components.insert(Value::Keyword(health), Value::Map(health_data));
        components = components.insert(Value::Keyword(armor), Value::Map(armor_data));
        let (world, entity) = world.spawn(&components).unwrap();

        // Benchmark destroying the entity
        b.iter_batched(
            || world.clone(),
            |w| black_box(w.destroy(entity)),
            criterion::BatchSize::SmallInput,
        )
    });

    // Batch archetype migration: many entities transitioning together
    for size in [10, 100, 500] {
        group.bench_with_input(
            BenchmarkId::new("batch_migration", size),
            &size,
            |b, &size| {
                let mut world = World::new(42);
                let tag_a = world.interner_mut().intern_keyword("tag-a");
                let tag_b = world.interner_mut().intern_keyword("tag-b");

                world = world
                    .register_component(ComponentSchema::tag(tag_a))
                    .unwrap();
                world = world
                    .register_component(ComponentSchema::tag(tag_b))
                    .unwrap();

                // Create entities with tag-a only
                let mut entities = Vec::new();
                for _ in 0..size {
                    let mut components = LtMap::new();
                    components = components.insert(Value::Keyword(tag_a), Value::Bool(true));
                    let (w, e) = world.spawn(&components).unwrap();
                    world = w;
                    entities.push(e);
                }

                // Benchmark adding tag-b to all entities (batch archetype migration)
                b.iter_batched(
                    || (world.clone(), entities.clone()),
                    |(mut w, ents)| {
                        for e in ents {
                            w = w.set(e, tag_b, Value::Bool(true)).unwrap();
                        }
                        black_box(w)
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

// =============================================================================
// Advanced Relationship Benchmarks (Stage 4)
// =============================================================================

fn bench_relationship_advanced(c: &mut Criterion) {
    let mut group = c.benchmark_group("relationship_advanced");

    // Multi-hop traversal: A -> B -> C -> ...
    for hops in [2, 3, 5] {
        group.bench_with_input(
            BenchmarkId::new("multi_hop_traversal", hops),
            &hops,
            |b, &hops| {
                let mut interner = Interner::new();
                let mut store = RelationshipStore::new();

                let parent_of = interner.intern_keyword("parent-of");
                store
                    .register_schema(RelationshipSchema::new(parent_of))
                    .unwrap();

                // Create a chain: e0 -> e1 -> e2 -> ... -> e_n
                let entities: Vec<_> = (0..=hops as u64).map(|i| EntityId::new(i, 1)).collect();

                for i in 0..hops {
                    store.link(entities[i], parent_of, entities[i + 1]).unwrap();
                }

                // Benchmark traversing the entire chain
                b.iter(|| {
                    let mut current = entities[0];
                    for _ in 0..hops {
                        // Get first target (single chain)
                        if let Some(next) = store.targets(current, parent_of).next() {
                            current = next;
                        }
                    }
                    black_box(current)
                })
            },
        );
    }

    // Cyclic relationship detection/traversal
    group.bench_function("cyclic_traversal", |b| {
        let mut interner = Interner::new();
        let mut store = RelationshipStore::new();

        let next = interner.intern_keyword("next");
        store
            .register_schema(RelationshipSchema::new(next))
            .unwrap();

        // Create a cycle: e0 -> e1 -> e2 -> e3 -> e0
        let entities: Vec<_> = (0..4).map(|i| EntityId::new(i, 1)).collect();
        for i in 0..4 {
            store
                .link(entities[i], next, entities[(i + 1) % 4])
                .unwrap();
        }

        // Benchmark traversing with cycle detection (stop after N steps)
        b.iter(|| {
            let mut current = entities[0];
            let mut visited = std::collections::HashSet::new();
            while visited.insert(current) {
                if let Some(n) = store.targets(current, next).next() {
                    current = n;
                } else {
                    break;
                }
            }
            black_box((current, visited.len()))
        })
    });

    // Bidirectional lookup: both sources and targets
    for fan_out in [10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("bidirectional_lookup", fan_out),
            &fan_out,
            |b, &fan_out| {
                let mut interner = Interner::new();
                let mut store = RelationshipStore::new();

                let connected = interner.intern_keyword("connected");
                store
                    .register_schema(RelationshipSchema::new(connected))
                    .unwrap();

                // Hub entity connected to many others
                let hub = EntityId::new(0, 1);
                let entities: Vec<_> = (1..=fan_out as u64).map(|i| EntityId::new(i, 1)).collect();

                // Half point to hub, half hub points to
                for i in 0..fan_out / 2 {
                    store.link(entities[i], connected, hub).unwrap();
                }
                for i in fan_out / 2..fan_out {
                    store.link(hub, connected, entities[i]).unwrap();
                }

                // Benchmark looking up both directions
                b.iter(|| {
                    let sources: Vec<_> = store.sources(hub, connected).collect();
                    let targets: Vec<_> = store.targets(hub, connected).collect();
                    black_box((sources.len(), targets.len()))
                })
            },
        );
    }

    // Deep tree traversal: find all descendants
    for depth in [3, 5, 7] {
        group.bench_with_input(
            BenchmarkId::new("tree_descendants", depth),
            &depth,
            |b, &depth| {
                let mut interner = Interner::new();
                let mut store = RelationshipStore::new();

                let child_of = interner.intern_keyword("child-of");
                store
                    .register_schema(RelationshipSchema::new(child_of))
                    .unwrap();

                // Build a binary tree: each node has 2 children
                // Total nodes = 2^(depth+1) - 1
                let total_nodes = (1 << (depth + 1)) - 1;
                let entities: Vec<_> = (0..total_nodes as u64)
                    .map(|i| EntityId::new(i, 1))
                    .collect();

                // Build tree: parent at i, children at 2i+1 and 2i+2
                for i in 0..total_nodes / 2 {
                    let left = 2 * i + 1;
                    let right = 2 * i + 2;
                    if left < total_nodes {
                        store.link(entities[left], child_of, entities[i]).unwrap();
                    }
                    if right < total_nodes {
                        store.link(entities[right], child_of, entities[i]).unwrap();
                    }
                }

                let root = entities[0];

                // Benchmark finding all descendants (BFS)
                b.iter(|| {
                    let mut descendants = Vec::new();
                    let mut queue = vec![root];

                    while let Some(current) = queue.pop() {
                        // Find children (entities that have child_of pointing to current)
                        for child in store.sources(current, child_of) {
                            descendants.push(child);
                            queue.push(child);
                        }
                    }
                    black_box(descendants.len())
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_entity_store,
    bench_component_store,
    bench_relationship_store,
    bench_world,
    bench_archetype_transitions,
    bench_relationship_advanced,
);

criterion_main!(benches);
