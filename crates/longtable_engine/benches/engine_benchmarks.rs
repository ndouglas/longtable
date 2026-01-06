//! Benchmarks for the Longtable engine layer.
//!
//! Run with: `cargo bench --package longtable_engine`

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};

use longtable_engine::{Bindings, PatternCompiler, PatternMatcher, QueryCompiler, QueryExecutor};
use longtable_foundation::{LtMap, Type, Value};
use longtable_language::declaration::{
    Pattern as DeclPattern, PatternClause as DeclClause, PatternValue, QueryDecl,
};
use longtable_language::{Ast, Span};
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
    let tag_player = world.interner_mut().intern_keyword("tag/player");
    let tag_enemy = world.interner_mut().intern_keyword("tag/enemy");
    let name = world.interner_mut().intern_keyword("name");
    let value = world.interner_mut().intern_keyword("value");

    let health_schema = ComponentSchema::new(health)
        .with_field(FieldSchema::required(current, Type::Int))
        .with_field(FieldSchema::optional(max, Type::Int, Value::Int(100)));
    world = world.register_component(health_schema).unwrap();

    let position_schema = ComponentSchema::new(position)
        .with_field(FieldSchema::required(x, Type::Int))
        .with_field(FieldSchema::required(y, Type::Int));
    world = world.register_component(position_schema).unwrap();

    world = world
        .register_component(ComponentSchema::tag(tag_player))
        .unwrap();
    world = world
        .register_component(ComponentSchema::tag(tag_enemy))
        .unwrap();

    let name_schema =
        ComponentSchema::new(name).with_field(FieldSchema::required(value, Type::String));
    world = world.register_component(name_schema).unwrap();

    // Register relationships
    let in_room = world.interner_mut().intern_keyword("in-room");
    world = world
        .register_relationship(
            RelationshipSchema::new(in_room)
                .with_cardinality(Cardinality::ManyToOne)
                .with_on_delete(OnDelete::Remove),
        )
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
        pos_data = pos_data.insert(Value::Keyword(x), Value::Int((i % 100) as i64));
        pos_data = pos_data.insert(Value::Keyword(y), Value::Int((i / 100) as i64));
        components = components.insert(Value::Keyword(position), Value::Map(pos_data));

        // Alternate between player and enemy tags
        if i % 10 == 0 {
            components = components.insert(Value::Keyword(tag_player), Value::Bool(true));
        } else {
            components = components.insert(Value::Keyword(tag_enemy), Value::Bool(true));
        }

        // Name component
        let mut name_data = LtMap::new();
        name_data = name_data.insert(
            Value::Keyword(value),
            Value::String(format!("Entity{i}").into()),
        );
        components = components.insert(Value::Keyword(name), Value::Map(name_data));

        let (w, _) = world.spawn(&components).unwrap();
        world = w;
    }

    world
}

/// Creates a world with rooms and entities in relationships.
fn create_world_with_rooms(room_count: usize, entities_per_room: usize) -> World {
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

    // Register relationships
    let in_room = world.interner_mut().intern_keyword("in-room");
    world = world
        .register_relationship(
            RelationshipSchema::new(in_room)
                .with_cardinality(Cardinality::ManyToOne)
                .with_on_delete(OnDelete::Remove),
        )
        .unwrap();

    // Create rooms
    let mut rooms = Vec::new();
    for i in 0..room_count {
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
        rooms.push(room);
    }

    // Create items in rooms
    for (room_idx, room) in rooms.iter().enumerate() {
        for j in 0..entities_per_room {
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

/// Helper to create a simple declaration pattern.
fn make_pattern(clauses: Vec<DeclClause>) -> DeclPattern {
    DeclPattern {
        clauses,
        negations: vec![],
    }
}

/// Helper to create a pattern clause with default span.
fn make_clause(entity_var: &str, component: &str, value: PatternValue) -> DeclClause {
    DeclClause {
        entity_var: entity_var.to_string(),
        component: component.to_string(),
        value,
        span: Span::default(),
    }
}

/// Helper to create a QueryDecl.
fn make_query(pattern: DeclPattern, return_var: &str) -> QueryDecl {
    QueryDecl {
        pattern,
        bindings: vec![],
        aggregates: vec![],
        group_by: vec![],
        guards: vec![],
        order_by: vec![],
        limit: None,
        return_expr: Some(Ast::Symbol(return_var.to_string(), Span::default())),
        span: Span::default(),
    }
}

// =============================================================================
// Pattern Compilation Benchmarks
// =============================================================================

fn bench_pattern_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_compilation");

    // Simple component pattern
    group.bench_function("simple_component", |b| {
        let mut world = World::new(42);
        let pattern = make_pattern(vec![make_clause("e", "health", PatternValue::Wildcard)]);

        b.iter(|| black_box(PatternCompiler::compile(&pattern, world.interner_mut())))
    });

    // Multi-clause pattern
    group.bench_function("multi_clause", |b| {
        let mut world = World::new(42);
        let pattern = make_pattern(vec![
            make_clause("e", "tag/player", PatternValue::Wildcard),
            make_clause("e", "health", PatternValue::Wildcard),
            make_clause("e", "position", PatternValue::Wildcard),
        ]);

        b.iter(|| black_box(PatternCompiler::compile(&pattern, world.interner_mut())))
    });

    // Pattern with variable binding
    group.bench_function("with_variable_binding", |b| {
        let mut world = World::new(42);
        let pattern = make_pattern(vec![make_clause(
            "e",
            "health",
            PatternValue::Variable("hp".to_string()),
        )]);

        b.iter(|| black_box(PatternCompiler::compile(&pattern, world.interner_mut())))
    });

    group.finish();
}

// =============================================================================
// Pattern Matching Benchmarks
// =============================================================================

fn bench_pattern_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_matching");

    // Simple component match at different scales
    for entity_count in [100, 1_000, 10_000] {
        let mut world = create_world_with_entities(entity_count);

        // Pattern: find all players
        let pattern = make_pattern(vec![make_clause("e", "tag/player", PatternValue::Wildcard)]);
        let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();

        // Expected matches: entity_count / 10 (every 10th entity is a player)
        let expected = entity_count / 10;
        group.throughput(Throughput::Elements(expected as u64));

        group.bench_with_input(
            BenchmarkId::new("single_component", entity_count),
            &(world, compiled),
            |b, (w, p)| {
                b.iter(|| {
                    let results = PatternMatcher::match_pattern(p, w);
                    black_box(results.len())
                })
            },
        );
    }

    // Multi-component match (player AND health)
    for entity_count in [100, 1_000, 10_000] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern(vec![
            make_clause("e", "tag/player", PatternValue::Wildcard),
            make_clause("e", "health", PatternValue::Wildcard),
        ]);
        let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();

        let expected = entity_count / 10;
        group.throughput(Throughput::Elements(expected as u64));

        group.bench_with_input(
            BenchmarkId::new("multi_component", entity_count),
            &(world, compiled),
            |b, (w, p)| {
                b.iter(|| {
                    let results = PatternMatcher::match_pattern(p, w);
                    black_box(results.len())
                })
            },
        );
    }

    // Pattern with value binding
    for entity_count in [100, 1_000, 10_000] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern(vec![make_clause(
            "e",
            "health",
            PatternValue::Variable("hp".to_string()),
        )]);
        let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();

        group.throughput(Throughput::Elements(entity_count as u64));

        group.bench_with_input(
            BenchmarkId::new("with_value_binding", entity_count),
            &(world, compiled),
            |b, (w, p)| {
                b.iter(|| {
                    let results = PatternMatcher::match_pattern(p, w);
                    black_box(results.len())
                })
            },
        );
    }

    group.finish();
}

// =============================================================================
// Query Execution Benchmarks
// =============================================================================

fn bench_query_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_execution");

    // Simple query
    for entity_count in [100, 1_000, 10_000] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern(vec![make_clause("e", "tag/player", PatternValue::Wildcard)]);
        let query_decl = make_query(pattern, "e");

        let query = QueryCompiler::compile(&query_decl, world.interner_mut()).unwrap();
        let expected = entity_count / 10;
        group.throughput(Throughput::Elements(expected as u64));

        group.bench_with_input(
            BenchmarkId::new("simple_return", entity_count),
            &(world, query),
            |b, (w, q)| {
                b.iter(|| {
                    let results = QueryExecutor::execute(q, w).unwrap();
                    black_box(results.len())
                })
            },
        );
    }

    // Query with count
    for entity_count in [100, 1_000, 10_000] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern(vec![make_clause("e", "tag/player", PatternValue::Wildcard)]);
        let query_decl = make_query(pattern, "e");

        let query = QueryCompiler::compile(&query_decl, world.interner_mut()).unwrap();

        group.bench_with_input(
            BenchmarkId::new("count", entity_count),
            &(world, query),
            |b, (w, q)| b.iter(|| black_box(QueryExecutor::count(q, w).unwrap())),
        );
    }

    // Query with exists check
    for entity_count in [100, 1_000, 10_000] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern(vec![make_clause("e", "tag/player", PatternValue::Wildcard)]);
        let query_decl = make_query(pattern, "e");

        let query = QueryCompiler::compile(&query_decl, world.interner_mut()).unwrap();

        group.bench_with_input(
            BenchmarkId::new("exists", entity_count),
            &(world, query),
            |b, (w, q)| b.iter(|| black_box(QueryExecutor::exists(q, w).unwrap())),
        );
    }

    // Query with limit
    for entity_count in [1_000, 10_000] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern(vec![make_clause("e", "health", PatternValue::Wildcard)]);
        let mut query_decl = make_query(pattern, "e");
        query_decl.limit = Some(10);

        let query = QueryCompiler::compile(&query_decl, world.interner_mut()).unwrap();

        group.bench_with_input(
            BenchmarkId::new("with_limit_10", entity_count),
            &(world, query),
            |b, (w, q)| {
                b.iter(|| {
                    let results = QueryExecutor::execute(q, w).unwrap();
                    black_box(results.len())
                })
            },
        );
    }

    group.finish();
}

// =============================================================================
// Relationship Traversal Benchmarks
// =============================================================================

fn bench_relationship_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("relationship_traversal");

    // Find all items in any room (relationship traversal)
    for (rooms, items_per_room) in [(10, 100), (100, 10), (50, 50)] {
        let mut world = create_world_with_rooms(rooms, items_per_room);
        let total = rooms * items_per_room;

        // Pattern: all items with their rooms
        let pattern = make_pattern(vec![make_clause(
            "item",
            "tag/item",
            PatternValue::Wildcard,
        )]);
        let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();

        let label = format!("{rooms}rooms_x_{items_per_room}items");
        group.throughput(Throughput::Elements(total as u64));

        group.bench_with_input(
            BenchmarkId::new("all_items", &label),
            &(world, compiled),
            |b, (w, p)| {
                b.iter(|| {
                    let results = PatternMatcher::match_pattern(p, w);
                    black_box(results.len())
                })
            },
        );
    }

    // Count items per room using manual traversal
    for (rooms, items_per_room) in [(10, 100), (100, 10)] {
        let mut world = create_world_with_rooms(rooms, items_per_room);
        let tag_room = world.interner_mut().intern_keyword("tag/room");
        let in_room = world.interner_mut().intern_keyword("in-room");

        let label = format!("{rooms}rooms_x_{items_per_room}items");

        group.bench_with_input(
            BenchmarkId::new("count_per_room", &label),
            &world,
            |b, w| {
                b.iter(|| {
                    let mut total = 0;
                    for room in w.with_component(tag_room) {
                        let count = w.sources(room, in_room).count();
                        total += count;
                    }
                    black_box(total)
                })
            },
        );
    }

    group.finish();
}

// =============================================================================
// Throughput Benchmarks
// =============================================================================

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");
    group.sample_size(50);

    // Matches per second for simple patterns
    let mut world = create_world_with_entities(10_000);
    let pattern = make_pattern(vec![make_clause("e", "tag/enemy", PatternValue::Wildcard)]);
    let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();

    // 9000 enemies (90% of 10000)
    group.throughput(Throughput::Elements(9000));
    group.bench_with_input(
        "pattern_matches_per_sec",
        &(world.clone(), compiled),
        |b, (w, p)| {
            b.iter(|| {
                let count = PatternMatcher::match_pattern(p, w).len();
                black_box(count)
            })
        },
    );

    // Query throughput
    let mut world = create_world_with_entities(10_000);
    let pattern = make_pattern(vec![make_clause("e", "tag/enemy", PatternValue::Wildcard)]);
    let query_decl = make_query(pattern, "e");
    let query = QueryCompiler::compile(&query_decl, world.interner_mut()).unwrap();

    group.throughput(Throughput::Elements(9000));
    group.bench_with_input("query_results_per_sec", &(world, query), |b, (w, q)| {
        b.iter(|| {
            let results = QueryExecutor::execute(q, w).unwrap();
            black_box(results.len())
        })
    });

    group.finish();
}

// =============================================================================
// Bindings Benchmarks
// =============================================================================

fn bench_bindings(c: &mut Criterion) {
    let mut group = c.benchmark_group("bindings");

    // Creating bindings
    group.bench_function("create_and_set", |b| {
        b.iter(|| {
            let mut bindings = Bindings::new();
            bindings.set(
                "e".to_string(),
                Value::EntityRef(longtable_foundation::EntityId::new(42, 1)),
            );
            bindings.set("hp".to_string(), Value::Int(100));
            bindings.set("name".to_string(), Value::String("Player".into()));
            black_box(bindings)
        })
    });

    // Lookup
    group.bench_function("get", |b| {
        let mut bindings = Bindings::new();
        bindings.set(
            "e".to_string(),
            Value::EntityRef(longtable_foundation::EntityId::new(42, 1)),
        );
        bindings.set("hp".to_string(), Value::Int(100));
        bindings.set("name".to_string(), Value::String("Player".into()));

        b.iter(|| {
            black_box(bindings.get("hp"));
            black_box(bindings.get("e"));
            black_box(bindings.get("name"))
        })
    });

    // Refraction key
    group.bench_function("refraction_key", |b| {
        let mut bindings = Bindings::new();
        bindings.set(
            "e".to_string(),
            Value::EntityRef(longtable_foundation::EntityId::new(42, 1)),
        );
        bindings.set("hp".to_string(), Value::Int(100));
        bindings.set("name".to_string(), Value::String("Player".into()));

        b.iter(|| black_box(bindings.refraction_key()))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_pattern_compilation,
    bench_pattern_matching,
    bench_query_execution,
    bench_relationship_queries,
    bench_throughput,
    bench_bindings,
);

criterion_main!(benches);
