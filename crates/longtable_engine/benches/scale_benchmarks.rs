//! Large-scale benchmarks for Longtable engine layer.
//!
//! Run with: `cargo bench --package longtable_engine --bench scale_benchmarks`
//!
//! WARNING: These benchmarks can take significant time.
//! Use `cargo bench --package longtable_engine --bench scale_benchmarks -- <filter>` to run specific tests.
//!
//! Benchmark groups:
//! - scale_pattern_matching: Pattern matching at larger scales
//! - scale_rule_engine: Rule engine with many rules and entities
//! - scale_queries: Query execution stress tests
//! - scale_tick: Tick execution at scale

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};

use longtable_engine::{
    CompiledRule, ConstraintChecker, ConstraintCompiler, PatternCompiler, PatternMatcher,
    ProductionRuleEngine, QueryCompiler, QueryExecutor, TickExecutor,
};
use longtable_foundation::{LtMap, Type, Value};
use longtable_language::declaration::{
    ConstraintDecl, ConstraintViolation, Pattern as DeclPattern, PatternClause as DeclClause,
    PatternValue, QueryDecl,
};
use longtable_language::{Ast, Span};
use longtable_storage::World;
use longtable_storage::schema::{ComponentSchema, FieldSchema};

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
    let tag_active = world.interner_mut().intern_keyword("tag/active");
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
        .register_component(ComponentSchema::tag(tag_active))
        .unwrap();
    world = world
        .register_component(ComponentSchema::tag(processed))
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

        // 10% are players, 90% are enemies
        if i % 10 == 0 {
            components = components.insert(Value::Keyword(tag_player), Value::Bool(true));
        } else {
            components = components.insert(Value::Keyword(tag_enemy), Value::Bool(true));
        }

        // 20% are active
        if i % 5 == 0 {
            components = components.insert(Value::Keyword(tag_active), Value::Bool(true));
        }

        let (w, _) = world.spawn(&components).unwrap();
        world = w;
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
// Scale Pattern Matching Benchmarks
// =============================================================================

fn bench_scale_pattern_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_pattern_matching");
    group.sample_size(20);

    // Pattern matching at increasing scales
    for entity_count in [500, 1_000, 2_500] {
        let mut world = create_world_with_entities(entity_count);

        // Simple single-component pattern
        let pattern = make_pattern(vec![make_clause("e", "health", PatternValue::Wildcard)]);
        let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();

        group.throughput(Throughput::Elements(entity_count as u64));
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

    // Multi-component pattern at scale
    for entity_count in [500, 1_000, 2_500] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern(vec![
            make_clause("e", "health", PatternValue::Wildcard),
            make_clause("e", "position", PatternValue::Wildcard),
            make_clause("e", "tag/active", PatternValue::Wildcard),
        ]);
        let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();

        // 20% have active tag
        let expected = entity_count / 5;
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

    // Pattern with negation at scale
    for entity_count in [500, 1_000, 2_500] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern_with_negations(
            vec![make_clause("e", "health", PatternValue::Wildcard)],
            vec![make_clause("e", "processed", PatternValue::Wildcard)],
        );
        let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();

        group.throughput(Throughput::Elements(entity_count as u64));
        group.bench_with_input(
            BenchmarkId::new("with_negation", entity_count),
            &(world, compiled),
            |b, (w, p)| {
                b.iter(|| {
                    let results = PatternMatcher::match_pattern(p, w);
                    black_box(results.len())
                })
            },
        );
    }

    // Variable binding at scale
    for entity_count in [500, 1_000, 2_500] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern(vec![
            make_clause("e", "health", PatternValue::Variable("hp".to_string())),
            make_clause("e", "position", PatternValue::Variable("pos".to_string())),
        ]);
        let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();

        group.throughput(Throughput::Elements(entity_count as u64));
        group.bench_with_input(
            BenchmarkId::new("with_bindings", entity_count),
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
// Scale Rule Engine Benchmarks
// =============================================================================

fn bench_scale_rule_engine(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_rule_engine");
    group.sample_size(20);

    // Single rule at increasing entity counts
    for entity_count in [500, 1_000, 2_500] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern(vec![make_clause("e", "health", PatternValue::Wildcard)]);
        let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();
        let rule_name = world.interner_mut().intern_keyword("health-rule");
        let rule = CompiledRule::new(rule_name, compiled);
        let rules = vec![rule];

        group.throughput(Throughput::Elements(entity_count as u64));
        group.bench_with_input(
            BenchmarkId::new("single_rule", entity_count),
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

    // Many rules at moderate entity count
    for rule_count in [10, 25, 50] {
        let mut world = create_world_with_entities(500);

        let rules: Vec<_> = (0..rule_count)
            .map(|i| {
                let component = match i % 4 {
                    0 => "health",
                    1 => "position",
                    2 => "tag/player",
                    _ => "tag/enemy",
                };
                let pattern =
                    make_pattern(vec![make_clause("e", component, PatternValue::Wildcard)]);
                let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();
                let rule_name = world.interner_mut().intern_keyword(&format!("rule-{i}"));
                CompiledRule::new(rule_name, compiled).with_salience(i % 100)
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("many_rules", rule_count),
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

    // Multiple rules × entities combination
    for (entities, rules_count) in [(250, 10), (500, 5), (100, 25)] {
        let mut world = create_world_with_entities(entities);

        let rules: Vec<_> = (0..rules_count)
            .map(|i| {
                let pattern =
                    make_pattern(vec![make_clause("e", "health", PatternValue::Wildcard)]);
                let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();
                let rule_name = world
                    .interner_mut()
                    .intern_keyword(&format!("combo-rule-{i}"));
                CompiledRule::new(rule_name, compiled)
            })
            .collect();

        let label = format!("{entities}e_x_{rules_count}r");
        let total_activations = entities * rules_count;
        group.throughput(Throughput::Elements(total_activations as u64));

        group.bench_with_input(
            BenchmarkId::new("entities_x_rules", &label),
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

    // Rule with complex pattern at scale
    for entity_count in [250, 500, 1_000] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern_with_negations(
            vec![
                make_clause("e", "health", PatternValue::Variable("hp".to_string())),
                make_clause("e", "position", PatternValue::Variable("pos".to_string())),
                make_clause("e", "tag/active", PatternValue::Wildcard),
            ],
            vec![make_clause("e", "processed", PatternValue::Wildcard)],
        );
        let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();
        let rule_name = world.interner_mut().intern_keyword("complex-rule");
        let rule = CompiledRule::new(rule_name, compiled);
        let rules = vec![rule];

        // 20% active
        let expected = entity_count / 5;
        group.throughput(Throughput::Elements(expected as u64));

        group.bench_with_input(
            BenchmarkId::new("complex_pattern", entity_count),
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

    group.finish();
}

// =============================================================================
// Scale Query Benchmarks
// =============================================================================

fn bench_scale_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_queries");
    group.sample_size(20);

    // Query execution at scale
    for entity_count in [500, 1_000, 2_500] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern(vec![make_clause("e", "health", PatternValue::Wildcard)]);
        let query_decl = make_query(pattern, "e");
        let query = QueryCompiler::compile(&query_decl, world.interner_mut()).unwrap();

        group.throughput(Throughput::Elements(entity_count as u64));
        group.bench_with_input(
            BenchmarkId::new("full_results", entity_count),
            &(world, query),
            |b, (w, q)| {
                b.iter(|| {
                    let results = QueryExecutor::execute(q, w).unwrap();
                    black_box(results.len())
                })
            },
        );
    }

    // Query count at scale
    for entity_count in [500, 1_000, 2_500] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern(vec![make_clause("e", "health", PatternValue::Wildcard)]);
        let query_decl = make_query(pattern, "e");
        let query = QueryCompiler::compile(&query_decl, world.interner_mut()).unwrap();

        group.bench_with_input(
            BenchmarkId::new("count_only", entity_count),
            &(world, query),
            |b, (w, q)| b.iter(|| black_box(QueryExecutor::count(q, w).unwrap())),
        );
    }

    // Query with limit (early termination benefit)
    for entity_count in [500, 1_000, 2_500] {
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

    // Query-one at scale
    for entity_count in [500, 1_000, 2_500] {
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

    // Complex query with variable bindings
    for entity_count in [500, 1_000, 2_500] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern(vec![
            make_clause("e", "health", PatternValue::Variable("hp".to_string())),
            make_clause("e", "position", PatternValue::Variable("pos".to_string())),
        ]);
        let query_decl = make_query(pattern, "hp");
        let query = QueryCompiler::compile(&query_decl, world.interner_mut()).unwrap();

        group.throughput(Throughput::Elements(entity_count as u64));
        group.bench_with_input(
            BenchmarkId::new("complex_with_bindings", entity_count),
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
// Scale Tick Benchmarks
// =============================================================================

fn bench_scale_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_tick");
    group.sample_size(20);

    // Empty tick at scale (baseline)
    for entity_count in [500, 1_000, 2_500] {
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

    // Tick with rule at scale
    for entity_count in [250, 500, 1_000] {
        let mut world = create_world_with_entities(entity_count);

        let pattern = make_pattern_with_negations(
            vec![make_clause("e", "health", PatternValue::Wildcard)],
            vec![make_clause("e", "processed", PatternValue::Wildcard)],
        );
        let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();
        let rule_name = world.interner_mut().intern_keyword("tick-rule");
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

    // Tick with multiple rules at scale
    for entity_count in [250, 500] {
        let mut world = create_world_with_entities(entity_count);

        let rules: Vec<_> = (0..5)
            .map(|i| {
                let component = match i {
                    0 => "health",
                    1 => "position",
                    2 => "tag/player",
                    3 => "tag/enemy",
                    _ => "tag/active",
                };
                let pattern =
                    make_pattern(vec![make_clause("e", component, PatternValue::Wildcard)]);
                let compiled = PatternCompiler::compile(&pattern, world.interner_mut()).unwrap();
                let rule_name = world
                    .interner_mut()
                    .intern_keyword(&format!("tick-rule-{i}"));
                CompiledRule::new(rule_name, compiled)
            })
            .collect();

        let executor = TickExecutor::new().with_rules(rules);

        group.bench_with_input(
            BenchmarkId::new("tick_with_5_rules", entity_count),
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

    // Tick with constraint at scale
    for entity_count in [250, 500, 1_000] {
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

    // Full tick: rules + constraints
    for entity_count in [250, 500] {
        let mut world = create_world_with_entities(entity_count);

        // Rules
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

        let executor = TickExecutor::new()
            .with_rules(vec![rule])
            .with_constraints(checker);

        group.bench_with_input(
            BenchmarkId::new("full_tick", entity_count),
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

    group.finish();
}

// =============================================================================
// Scale Constraint Checking Benchmarks
// =============================================================================

fn bench_scale_constraints(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_constraints");
    group.sample_size(20);

    // Single constraint at scale
    for entity_count in [500, 1_000, 2_500] {
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

    // Multiple constraints at scale
    for entity_count in [500, 1_000] {
        let mut world = create_world_with_entities(entity_count);

        let constraints: Vec<_> = [
            "health",
            "position",
            "tag/player",
            "tag/enemy",
            "tag/active",
        ]
        .iter()
        .enumerate()
        .map(|(i, component)| {
            let mut decl = ConstraintDecl::new(format!("constraint-{i}"), Span::default());
            decl.pattern = DeclPattern {
                clauses: vec![DeclClause {
                    entity_var: "e".to_string(),
                    component: (*component).to_string(),
                    value: PatternValue::Wildcard,
                    span: Span::default(),
                }],
                negations: vec![],
            };
            decl.on_violation = ConstraintViolation::Warn;
            ConstraintCompiler::compile(&decl, world.interner_mut()).unwrap()
        })
        .collect();

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
// Criterion Groups
// =============================================================================

criterion_group!(
    benches,
    bench_scale_pattern_matching,
    bench_scale_rule_engine,
    bench_scale_queries,
    bench_scale_tick,
    bench_scale_constraints,
);

criterion_main!(benches);
