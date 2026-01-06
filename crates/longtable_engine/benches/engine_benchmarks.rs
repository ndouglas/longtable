//! Benchmarks for the Longtable engine layer.
//!
//! Run with: `cargo bench --package longtable_engine`
//!
//! Benchmark groups:
//! - pattern_compilation: Pattern compilation performance
//! - pattern_matching: Pattern matching at various scales
//! - pattern_negation: Negation and variable unification
//! - query_execution: Query execution performance
//! - relationship_traversal: Relationship query performance
//! - rule_engine: Rule activation finding and execution
//! - derived_components: Derived component caching and evaluation
//! - constraint_checking: Constraint validation performance
//! - tick_orchestration: Full tick cycle performance
//! - bindings: Bindings data structure operations
//! - throughput: High-level throughput measurements

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};

use longtable_engine::{
    Bindings, CompiledRule, ConstraintChecker, ConstraintCompiler, DerivedCache, DerivedCompiler,
    DerivedEvaluator, InputEvent, PatternCompiler, PatternMatcher, ProductionRuleEngine,
    QueryCompiler, QueryExecutor, TickExecutor,
};
use longtable_foundation::{LtMap, Type, Value};
use longtable_language::declaration::{
    ConstraintDecl, ConstraintViolation, DerivedDecl, Pattern as DeclPattern,
    PatternClause as DeclClause, PatternValue, QueryDecl,
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
    let processed = world.interner_mut().intern_keyword("processed");

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
    world = world
        .register_component(ComponentSchema::tag(processed))
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

/// Helper to create a pattern with negations.
fn make_pattern_with_negations(
    clauses: Vec<DeclClause>,
    negations: Vec<DeclClause>,
) -> DeclPattern {
    DeclPattern { clauses, negations }
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

/// Helper to create a `QueryDecl`.
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
    group.bench_function("multi_clause_3", |b| {
        let mut world = World::new(42);
        let pattern = make_pattern(vec![
            make_clause("e", "tag/player", PatternValue::Wildcard),
            make_clause("e", "health", PatternValue::Wildcard),
            make_clause("e", "position", PatternValue::Wildcard),
        ]);

        b.iter(|| black_box(PatternCompiler::compile(&pattern, world.interner_mut())))
    });

    // Pattern with 5 clauses
    group.bench_function("multi_clause_5", |b| {
        let mut world = World::new(42);
        let pattern = make_pattern(vec![
            make_clause("e", "tag/player", PatternValue::Wildcard),
            make_clause("e", "health", PatternValue::Wildcard),
            make_clause("e", "position", PatternValue::Wildcard),
            make_clause("e", "name", PatternValue::Wildcard),
            make_clause("e", "processed", PatternValue::Wildcard),
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

    // Pattern with negation
    group.bench_function("with_negation", |b| {
        let mut world = World::new(42);
        let pattern = make_pattern_with_negations(
            vec![make_clause("e", "health", PatternValue::Wildcard)],
            vec![make_clause("e", "processed", PatternValue::Wildcard)],
        );

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
            BenchmarkId::new("multi_component_2", entity_count),
            &(world, compiled),
            |b, (w, p)| {
                b.iter(|| {
                    let results = PatternMatcher::match_pattern(p, w);
                    black_box(results.len())
                })
            },
        );
    }

    // Three component pattern
    for entity_count in [100, 1_000, 10_000] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern(vec![
            make_clause("e", "tag/player", PatternValue::Wildcard),
            make_clause("e", "health", PatternValue::Wildcard),
            make_clause("e", "position", PatternValue::Wildcard),
        ]);
        let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();

        let expected = entity_count / 10;
        group.throughput(Throughput::Elements(expected as u64));

        group.bench_with_input(
            BenchmarkId::new("multi_component_3", entity_count),
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
// Pattern Negation and Unification Benchmarks
// =============================================================================

fn bench_pattern_negation(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_negation");

    // Pattern with negation: [?e :health] (not [?e :processed])
    // No entities have :processed, so all should match
    for entity_count in [100, 1_000, 10_000] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern_with_negations(
            vec![make_clause("e", "health", PatternValue::Wildcard)],
            vec![make_clause("e", "processed", PatternValue::Wildcard)],
        );
        let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();

        group.throughput(Throughput::Elements(entity_count as u64));

        group.bench_with_input(
            BenchmarkId::new("negation_no_matches", entity_count),
            &(world, compiled),
            |b, (w, p)| {
                b.iter(|| {
                    let results = PatternMatcher::match_pattern(p, w);
                    black_box(results.len())
                })
            },
        );
    }

    // Pattern with negation where some entities are excluded
    // Use tag/player as negation (10% of entities)
    for entity_count in [100, 1_000, 10_000] {
        let mut world = create_world_with_entities(entity_count);

        // Match enemies that are NOT players (should be all enemies = 90%)
        let pattern = make_pattern_with_negations(
            vec![make_clause("e", "tag/enemy", PatternValue::Wildcard)],
            vec![make_clause("e", "tag/player", PatternValue::Wildcard)],
        );
        let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();

        // 90% are enemies, none are both enemy and player
        let expected = entity_count * 9 / 10;
        group.throughput(Throughput::Elements(expected as u64));

        group.bench_with_input(
            BenchmarkId::new("negation_with_exclusion", entity_count),
            &(world, compiled),
            |b, (w, p)| {
                b.iter(|| {
                    let results = PatternMatcher::match_pattern(p, w);
                    black_box(results.len())
                })
            },
        );
    }

    // Variable unification: same variable in multiple clauses
    for entity_count in [100, 1_000, 10_000] {
        let mut world = create_world_with_entities(entity_count);

        // Pattern where we bind the same entity variable twice
        let pattern = make_pattern(vec![
            make_clause("e", "health", PatternValue::Variable("hp".to_string())),
            make_clause("e", "position", PatternValue::Variable("pos".to_string())),
        ]);
        let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();

        group.throughput(Throughput::Elements(entity_count as u64));

        group.bench_with_input(
            BenchmarkId::new("variable_unification", entity_count),
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

    // Query returning component value
    for entity_count in [100, 1_000, 10_000] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern(vec![make_clause(
            "e",
            "health",
            PatternValue::Variable("hp".to_string()),
        )]);
        let query_decl = make_query(pattern, "hp");

        let query = QueryCompiler::compile(&query_decl, world.interner_mut()).unwrap();
        group.throughput(Throughput::Elements(entity_count as u64));

        group.bench_with_input(
            BenchmarkId::new("return_component", entity_count),
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

    // Query-one (first result only)
    for entity_count in [1_000, 10_000] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern(vec![make_clause("e", "health", PatternValue::Wildcard)]);
        let query_decl = make_query(pattern, "e");

        let query = QueryCompiler::compile(&query_decl, world.interner_mut()).unwrap();

        group.bench_with_input(
            BenchmarkId::new("query_one", entity_count),
            &(world, query),
            |b, (w, q)| b.iter(|| black_box(QueryExecutor::execute_one(q, w).unwrap())),
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
// Rule Engine Benchmarks
// =============================================================================

fn bench_rule_engine(c: &mut Criterion) {
    let mut group = c.benchmark_group("rule_engine");

    // Finding activations - single rule
    for entity_count in [100, 1_000, 10_000] {
        let mut world = create_world_with_entities(entity_count);

        // Rule: [?e :tag/player]
        let pattern = make_pattern(vec![make_clause("e", "tag/player", PatternValue::Wildcard)]);
        let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();
        let rule_name = world.interner_mut().intern_keyword("player-rule");
        let rule = CompiledRule::new(rule_name, compiled);
        let rules = vec![rule];

        let expected = entity_count / 10; // 10% are players
        group.throughput(Throughput::Elements(expected as u64));

        group.bench_with_input(
            BenchmarkId::new("find_activations_single_rule", entity_count),
            &(world, rules),
            |b, (w, r)| {
                b.iter(|| {
                    let mut engine = ProductionRuleEngine::new();
                    engine.begin_tick();
                    let activations = engine.find_activations(r, w);
                    black_box(activations.len())
                })
            },
        );
    }

    // Finding activations - multiple rules
    for entity_count in [100, 1_000, 10_000] {
        let mut world = create_world_with_entities(entity_count);

        // Multiple rules with different patterns
        let pattern1 = make_pattern(vec![make_clause("e", "tag/player", PatternValue::Wildcard)]);
        let compiled1 = PatternCompiler::compile(&pattern1, world.interner_mut()).unwrap();
        let rule1_name = world.interner_mut().intern_keyword("player-rule");
        let rule1 = CompiledRule::new(rule1_name, compiled1);

        let pattern2 = make_pattern(vec![make_clause("e", "tag/enemy", PatternValue::Wildcard)]);
        let compiled2 = PatternCompiler::compile(&pattern2, world.interner_mut()).unwrap();
        let rule2_name = world.interner_mut().intern_keyword("enemy-rule");
        let rule2 = CompiledRule::new(rule2_name, compiled2);

        let pattern3 = make_pattern(vec![make_clause("e", "health", PatternValue::Wildcard)]);
        let compiled3 = PatternCompiler::compile(&pattern3, world.interner_mut()).unwrap();
        let rule3_name = world.interner_mut().intern_keyword("health-rule");
        let rule3 = CompiledRule::new(rule3_name, compiled3);

        let rules = vec![rule1, rule2, rule3];

        // 10% players + 90% enemies + 100% health = 200% = 2 * entity_count
        group.throughput(Throughput::Elements((entity_count * 2) as u64));

        group.bench_with_input(
            BenchmarkId::new("find_activations_3_rules", entity_count),
            &(world, rules),
            |b, (w, r)| {
                b.iter(|| {
                    let mut engine = ProductionRuleEngine::new();
                    engine.begin_tick();
                    let activations = engine.find_activations(r, w);
                    black_box(activations.len())
                })
            },
        );
    }

    // Finding activations with salience sorting
    for entity_count in [100, 1_000] {
        let mut world = create_world_with_entities(entity_count);

        // Rules with different saliences
        let pattern1 = make_pattern(vec![make_clause("e", "health", PatternValue::Wildcard)]);
        let compiled1 = PatternCompiler::compile(&pattern1, world.interner_mut()).unwrap();
        let rule1_name = world.interner_mut().intern_keyword("low-priority");
        let rule1 = CompiledRule::new(rule1_name, compiled1).with_salience(1);

        let pattern2 = make_pattern(vec![make_clause("e", "health", PatternValue::Wildcard)]);
        let compiled2 = PatternCompiler::compile(&pattern2, world.interner_mut()).unwrap();
        let rule2_name = world.interner_mut().intern_keyword("high-priority");
        let rule2 = CompiledRule::new(rule2_name, compiled2).with_salience(100);

        let pattern3 = make_pattern(vec![make_clause("e", "health", PatternValue::Wildcard)]);
        let compiled3 = PatternCompiler::compile(&pattern3, world.interner_mut()).unwrap();
        let rule3_name = world.interner_mut().intern_keyword("medium-priority");
        let rule3 = CompiledRule::new(rule3_name, compiled3).with_salience(50);

        let rules = vec![rule1, rule2, rule3];

        group.throughput(Throughput::Elements((entity_count * 3) as u64));

        group.bench_with_input(
            BenchmarkId::new("find_activations_with_salience", entity_count),
            &(world, rules),
            |b, (w, r)| {
                b.iter(|| {
                    let mut engine = ProductionRuleEngine::new();
                    engine.begin_tick();
                    let activations = engine.find_activations(r, w);
                    black_box(activations.len())
                })
            },
        );
    }

    // Run to quiescence (no-op executor)
    for entity_count in [100, 500, 1_000] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern_with_negations(
            vec![make_clause("e", "health", PatternValue::Wildcard)],
            vec![make_clause("e", "processed", PatternValue::Wildcard)],
        );
        let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();
        let rule_name = world.interner_mut().intern_keyword("process-rule");
        let rule = CompiledRule::new(rule_name, compiled);
        let rules = vec![rule];

        group.bench_with_input(
            BenchmarkId::new("run_to_quiescence_noop", entity_count),
            &(world.clone(), rules),
            |b, (w, r)| {
                b.iter(|| {
                    let mut engine = ProductionRuleEngine::new();
                    engine.begin_tick();
                    let result = engine.run_to_quiescence(r, w.clone(), |_activation, world| {
                        // No-op executor - just return empty effects
                        Ok((vec![], world.clone()))
                    });
                    black_box(result)
                })
            },
        );
    }

    group.finish();
}

// =============================================================================
// Derived Components Benchmarks
// =============================================================================

fn bench_derived_components(c: &mut Criterion) {
    let mut group = c.benchmark_group("derived_components");

    // Compile derived component
    group.bench_function("compile_derived", |b| {
        let mut interner = longtable_foundation::Interner::new();

        let mut pattern = DeclPattern::default();
        pattern.clauses.push(DeclClause {
            entity_var: "self".to_string(),
            component: "health".to_string(),
            value: PatternValue::Wildcard,
            span: Span::default(),
        });

        let decl = DerivedDecl::new(
            "health-percent",
            "self",
            Ast::Int(100, Span::default()),
            Span::default(),
        );

        b.iter(|| black_box(DerivedCompiler::compile(&decl, &mut interner)))
    });

    // Cache operations
    group.bench_function("cache_set_and_get", |b| {
        let mut interner = longtable_foundation::Interner::new();
        let derived_id = interner.intern_keyword("test-derived");
        let mut cache = DerivedCache::new();
        let entity = longtable_foundation::EntityId::new(1, 0);

        b.iter(|| {
            cache.set(entity, derived_id, Value::Int(100));
            let _ = black_box(cache.get(entity, derived_id).is_some());
        })
    });

    // Cache invalidation
    for cache_size in [100, 1_000] {
        let mut interner = longtable_foundation::Interner::new();
        let derived_id = interner.intern_keyword("test-derived");
        let mut cache = DerivedCache::new();

        // Pre-populate cache
        for i in 0..cache_size {
            let entity = longtable_foundation::EntityId::new(i as u64, 0);
            cache.set(entity, derived_id, Value::Int(i as i64));
        }

        group.bench_with_input(
            BenchmarkId::new("cache_clear", cache_size),
            &cache,
            |b, c| {
                b.iter(|| {
                    let mut cache = c.clone();
                    cache.clear();
                    black_box(cache)
                })
            },
        );
    }

    // Cache version advance
    group.bench_function("cache_advance_version", |b| {
        let mut interner = longtable_foundation::Interner::new();
        let derived_id = interner.intern_keyword("test-derived");
        let mut cache = DerivedCache::new();
        for i in 0..100 {
            let entity = longtable_foundation::EntityId::new(i, 0);
            cache.set(entity, derived_id, Value::Int(i as i64));
        }

        b.iter(|| {
            cache.advance_version();
            black_box(cache.version())
        })
    });

    // DerivedEvaluator operations
    group.bench_function("evaluator_begin_tick", |b| {
        let mut evaluator = DerivedEvaluator::new();

        b.iter(|| {
            evaluator.begin_tick();
            black_box(())
        })
    });

    group.finish();
}

// =============================================================================
// Constraint Checking Benchmarks
// =============================================================================

fn bench_constraint_checking(c: &mut Criterion) {
    let mut group = c.benchmark_group("constraint_checking");

    // Compile constraint
    group.bench_function("compile_constraint", |b| {
        let mut interner = longtable_foundation::Interner::new();

        let mut decl = ConstraintDecl::new("health-positive", Span::default());
        decl.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "health".to_string(),
                value: PatternValue::Variable("hp".to_string()),
                span: Span::default(),
            }],
            negations: vec![],
        };
        decl.on_violation = ConstraintViolation::Rollback;

        b.iter(|| black_box(ConstraintCompiler::compile(&decl, &mut interner)))
    });

    // Check constraints - no violations (empty world)
    group.bench_function("check_no_entities", |b| {
        let world = World::new(42);
        let checker = ConstraintChecker::new();

        b.iter(|| black_box(checker.check_all(&world)))
    });

    // Check single constraint at scale
    for entity_count in [100, 1_000, 10_000] {
        let mut world = create_world_with_entities(entity_count);

        let mut decl = ConstraintDecl::new("health-check", Span::default());
        decl.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "health".to_string(),
                value: PatternValue::Wildcard,
                span: Span::default(),
            }],
            negations: vec![],
        };
        decl.on_violation = ConstraintViolation::Warn;

        let compiled = ConstraintCompiler::compile(&decl, world.interner_mut()).unwrap();
        let checker = ConstraintChecker::new().with_constraints(vec![compiled]);

        group.throughput(Throughput::Elements(entity_count as u64));

        group.bench_with_input(
            BenchmarkId::new("single_constraint", entity_count),
            &(world, checker),
            |b, (w, ch)| b.iter(|| black_box(ch.check_all(w))),
        );
    }

    // Check multiple constraints
    for entity_count in [100, 1_000] {
        let mut world = create_world_with_entities(entity_count);

        // Create 5 constraints
        let mut constraints = Vec::new();
        for i in 0..5 {
            let component = match i {
                0 => "health",
                1 => "position",
                2 => "tag/player",
                3 => "tag/enemy",
                _ => "name",
            };

            let mut decl = ConstraintDecl::new(format!("constraint-{i}"), Span::default());
            decl.pattern = DeclPattern {
                clauses: vec![DeclClause {
                    entity_var: "e".to_string(),
                    component: component.to_string(),
                    value: PatternValue::Wildcard,
                    span: Span::default(),
                }],
                negations: vec![],
            };
            decl.on_violation = ConstraintViolation::Warn;

            let compiled = ConstraintCompiler::compile(&decl, world.interner_mut()).unwrap();
            constraints.push(compiled);
        }

        let checker = ConstraintChecker::new().with_constraints(constraints);

        group.bench_with_input(
            BenchmarkId::new("5_constraints", entity_count),
            &(world, checker),
            |b, (w, ch)| b.iter(|| black_box(ch.check_all(w))),
        );
    }

    group.finish();
}

// =============================================================================
// Tick Orchestration Benchmarks
// =============================================================================

fn bench_tick_orchestration(c: &mut Criterion) {
    let mut group = c.benchmark_group("tick_orchestration");
    group.sample_size(50); // Tick operations are more expensive

    // Empty tick (no rules, no inputs)
    for entity_count in [100, 1_000, 10_000] {
        let world = create_world_with_entities(entity_count);

        group.bench_with_input(
            BenchmarkId::new("empty_tick", entity_count),
            &world,
            |b, w| {
                b.iter(|| {
                    let mut executor = TickExecutor::new();
                    let result = executor.tick(w.clone(), &[]);
                    black_box(result)
                })
            },
        );
    }

    // Tick with input injection
    for entity_count in [100, 1_000] {
        let mut world = create_world_with_entities(entity_count);
        let health = world.interner_mut().intern_keyword("health");

        // Get first entity
        let entities: Vec<_> = world.with_component(health).collect();
        let entity = entities[0];

        let current = world.interner_mut().intern_keyword("current");
        let mut health_data = LtMap::new();
        health_data = health_data.insert(Value::Keyword(current), Value::Int(50));

        let inputs = vec![InputEvent::Set {
            entity,
            component: health,
            value: Value::Map(health_data),
        }];

        group.bench_with_input(
            BenchmarkId::new("tick_with_1_input", entity_count),
            &(world, inputs),
            |b, (w, inp)| {
                b.iter(|| {
                    let mut executor = TickExecutor::new();
                    let result = executor.tick(w.clone(), inp);
                    black_box(result)
                })
            },
        );
    }

    // Tick with multiple inputs
    for entity_count in [100, 1_000] {
        let mut world = create_world_with_entities(entity_count);
        let health = world.interner_mut().intern_keyword("health");
        let current = world.interner_mut().intern_keyword("current");

        // Get first 10 entities
        let entities: Vec<_> = world.with_component(health).take(10).collect();

        let inputs: Vec<_> = entities
            .iter()
            .map(|&entity| {
                let mut health_data = LtMap::new();
                health_data = health_data.insert(Value::Keyword(current), Value::Int(50));
                InputEvent::Set {
                    entity,
                    component: health,
                    value: Value::Map(health_data),
                }
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("tick_with_10_inputs", entity_count),
            &(world, inputs),
            |b, (w, inp)| {
                b.iter(|| {
                    let mut executor = TickExecutor::new();
                    let result = executor.tick(w.clone(), inp);
                    black_box(result)
                })
            },
        );
    }

    // Tick with rules (no-op execution)
    for entity_count in [100, 500, 1_000] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern_with_negations(
            vec![make_clause("e", "health", PatternValue::Wildcard)],
            vec![make_clause("e", "processed", PatternValue::Wildcard)],
        );
        let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();
        let rule_name = world.interner_mut().intern_keyword("process-rule");
        let rule = CompiledRule::new(rule_name, compiled);

        let executor = TickExecutor::new().with_rules(vec![rule]);

        group.bench_with_input(
            BenchmarkId::new("tick_with_rule", entity_count),
            &(world, executor),
            |b, (w, ex)| {
                b.iter(|| {
                    let mut executor = ex.clone();
                    let result = executor.tick(w.clone(), &[]);
                    black_box(result)
                })
            },
        );
    }

    // Tick with constraints
    for entity_count in [100, 500, 1_000] {
        let mut world = create_world_with_entities(entity_count);

        let mut decl = ConstraintDecl::new("health-constraint", Span::default());
        decl.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "health".to_string(),
                value: PatternValue::Wildcard,
                span: Span::default(),
            }],
            negations: vec![],
        };
        decl.on_violation = ConstraintViolation::Warn;

        let compiled = ConstraintCompiler::compile(&decl, world.interner_mut()).unwrap();
        let checker = ConstraintChecker::new().with_constraints(vec![compiled]);

        let executor = TickExecutor::new().with_constraints(checker);

        group.bench_with_input(
            BenchmarkId::new("tick_with_constraint", entity_count),
            &(world, executor),
            |b, (w, ex)| {
                b.iter(|| {
                    let mut executor = ex.clone();
                    let result = executor.tick(w.clone(), &[]);
                    black_box(result)
                })
            },
        );
    }

    // Full tick: rules + constraints + inputs
    for entity_count in [100, 500] {
        let mut world = create_world_with_entities(entity_count);
        let health = world.interner_mut().intern_keyword("health");
        let current = world.interner_mut().intern_keyword("current");

        // Rule
        let pattern = make_pattern_with_negations(
            vec![make_clause("e", "health", PatternValue::Wildcard)],
            vec![make_clause("e", "processed", PatternValue::Wildcard)],
        );
        let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();
        let rule_name = world.interner_mut().intern_keyword("process-rule");
        let rule = CompiledRule::new(rule_name, compiled);

        // Constraint
        let mut decl = ConstraintDecl::new("health-constraint", Span::default());
        decl.pattern = DeclPattern {
            clauses: vec![DeclClause {
                entity_var: "e".to_string(),
                component: "health".to_string(),
                value: PatternValue::Wildcard,
                span: Span::default(),
            }],
            negations: vec![],
        };
        decl.on_violation = ConstraintViolation::Warn;
        let constraint = ConstraintCompiler::compile(&decl, world.interner_mut()).unwrap();
        let checker = ConstraintChecker::new().with_constraints(vec![constraint]);

        // Inputs
        let entities: Vec<_> = world.with_component(health).take(5).collect();
        let inputs: Vec<_> = entities
            .iter()
            .map(|&entity| {
                let mut health_data = LtMap::new();
                health_data = health_data.insert(Value::Keyword(current), Value::Int(50));
                InputEvent::Set {
                    entity,
                    component: health,
                    value: Value::Map(health_data),
                }
            })
            .collect();

        let executor = TickExecutor::new()
            .with_rules(vec![rule])
            .with_constraints(checker);

        group.bench_with_input(
            BenchmarkId::new("full_tick", entity_count),
            &(world, executor, inputs),
            |b, (w, ex, inp)| {
                b.iter(|| {
                    let mut executor = ex.clone();
                    let result = executor.tick(w.clone(), inp);
                    black_box(result)
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

    // Rule activations per second
    let mut world = create_world_with_entities(10_000);
    let pattern = make_pattern(vec![make_clause("e", "health", PatternValue::Wildcard)]);
    let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();
    let rule_name = world.interner_mut().intern_keyword("health-rule");
    let rule = CompiledRule::new(rule_name, compiled);
    let rules = vec![rule];

    group.throughput(Throughput::Elements(10_000));
    group.bench_with_input("rule_activations_per_sec", &(world, rules), |b, (w, r)| {
        b.iter(|| {
            let mut engine = ProductionRuleEngine::new();
            engine.begin_tick();
            let activations = engine.find_activations(r, w);
            black_box(activations.len())
        })
    });

    // Ticks per second (empty)
    let world = create_world_with_entities(1_000);
    group.throughput(Throughput::Elements(1));
    group.bench_with_input("ticks_per_sec_1k_entities", &world, |b, w| {
        b.iter(|| {
            let mut executor = TickExecutor::new();
            let result = executor.tick(w.clone(), &[]);
            black_box(result)
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
    group.bench_function("create_empty", |b| b.iter(|| black_box(Bindings::new())));

    // Creating and setting bindings
    group.bench_function("create_and_set_3", |b| {
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

    // Setting 10 bindings
    group.bench_function("create_and_set_10", |b| {
        b.iter(|| {
            let mut bindings = Bindings::new();
            for i in 0..10 {
                bindings.set(format!("var{i}"), Value::Int(i as i64));
            }
            black_box(bindings)
        })
    });

    // Lookup
    group.bench_function("get_existing", |b| {
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

    // Lookup missing key
    group.bench_function("get_missing", |b| {
        let mut bindings = Bindings::new();
        bindings.set("e".to_string(), Value::Int(1));

        b.iter(|| black_box(bindings.get("missing")))
    });

    // Refraction key generation
    group.bench_function("refraction_key_3_vars", |b| {
        let mut bindings = Bindings::new();
        bindings.set(
            "e".to_string(),
            Value::EntityRef(longtable_foundation::EntityId::new(42, 1)),
        );
        bindings.set("hp".to_string(), Value::Int(100));
        bindings.set("name".to_string(), Value::String("Player".into()));

        b.iter(|| black_box(bindings.refraction_key()))
    });

    // Refraction key with more variables
    group.bench_function("refraction_key_10_vars", |b| {
        let mut bindings = Bindings::new();
        for i in 0..10 {
            bindings.set(format!("var{i}"), Value::Int(i as i64));
        }

        b.iter(|| black_box(bindings.refraction_key()))
    });

    // Get entity helper
    group.bench_function("get_entity", |b| {
        let mut bindings = Bindings::new();
        let entity = longtable_foundation::EntityId::new(42, 1);
        bindings.set("e".to_string(), Value::EntityRef(entity));

        b.iter(|| black_box(bindings.get_entity("e")))
    });

    // Clone bindings
    group.bench_function("clone_3_vars", |b| {
        let mut bindings = Bindings::new();
        bindings.set(
            "e".to_string(),
            Value::EntityRef(longtable_foundation::EntityId::new(42, 1)),
        );
        bindings.set("hp".to_string(), Value::Int(100));
        bindings.set("name".to_string(), Value::String("Player".into()));

        b.iter(|| black_box(bindings.clone()))
    });

    group.finish();
}

// =============================================================================
// World Operations Benchmarks (for context)
// =============================================================================

fn bench_world_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("world_operations");

    // World clone (important for tick rollback)
    for entity_count in [100, 1_000, 10_000] {
        let world = create_world_with_entities(entity_count);

        group.bench_with_input(BenchmarkId::new("clone", entity_count), &world, |b, w| {
            b.iter(|| black_box(w.clone()))
        });
    }

    // Entity iteration
    for entity_count in [100, 1_000, 10_000] {
        let mut world = create_world_with_entities(entity_count);
        let health = world.interner_mut().intern_keyword("health");

        group.throughput(Throughput::Elements(entity_count as u64));
        group.bench_with_input(
            BenchmarkId::new("with_component_iter", entity_count),
            &world,
            |b, w| {
                b.iter(|| {
                    let count = w.with_component(health).count();
                    black_box(count)
                })
            },
        );
    }

    // Component get
    for entity_count in [100, 1_000, 10_000] {
        let mut world = create_world_with_entities(entity_count);
        let health = world.interner_mut().intern_keyword("health");
        let entities: Vec<_> = world.with_component(health).collect();

        group.bench_with_input(
            BenchmarkId::new("get_component", entity_count),
            &(world, entities),
            |b, (w, ents)| {
                b.iter(|| {
                    for &e in ents.iter().take(100) {
                        let _ = black_box(w.get(e, health));
                    }
                })
            },
        );
    }

    // Component set
    for entity_count in [100, 1_000] {
        let mut world = create_world_with_entities(entity_count);
        let health = world.interner_mut().intern_keyword("health");
        let current = world.interner_mut().intern_keyword("current");
        let entities: Vec<_> = world.with_component(health).collect();

        let mut new_health = LtMap::new();
        new_health = new_health.insert(Value::Keyword(current), Value::Int(50));
        let new_value = Value::Map(new_health);

        group.bench_with_input(
            BenchmarkId::new("set_component", entity_count),
            &(world, entities, new_value),
            |b, (w, ents, val)| {
                b.iter(|| {
                    let mut world = w.clone();
                    for &e in ents.iter().take(10) {
                        world = world.set(e, health, val.clone()).unwrap();
                    }
                    black_box(world)
                })
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
    bench_pattern_compilation,
    bench_pattern_matching,
    bench_pattern_negation,
    bench_query_execution,
    bench_relationship_queries,
    bench_rule_engine,
    bench_derived_components,
    bench_constraint_checking,
    bench_tick_orchestration,
    bench_throughput,
    bench_bindings,
    bench_world_operations,
);

criterion_main!(benches);
