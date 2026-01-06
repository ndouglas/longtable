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

criterion_group!(
    benches,
    bench_entity_store,
    bench_component_store,
    bench_relationship_store,
    bench_world,
);

criterion_main!(benches);
