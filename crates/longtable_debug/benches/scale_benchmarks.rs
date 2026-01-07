//! Large-scale benchmarks for longtable_debug.
//!
//! Run with: `cargo bench --package longtable_debug --bench scale_benchmarks`
//!
//! WARNING: These benchmarks can take significant time.
//! Use `cargo bench --package longtable_debug --bench scale_benchmarks -- <filter>` to run specific tests.
//!
//! Benchmark groups:
//! - scale_timeline: Timeline operations with large history
//! - scale_diff: Diff operations on larger worlds
//! - scale_trace: Trace buffer at scale
//! - scale_debug: Debug infrastructure stress tests

use criterion::{
    BatchSize, BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main,
};
use longtable_debug::{
    BreakpointRegistry, DiffGranularity, HistoryBuffer, MergeStrategy, TickSummary, Timeline,
    TimelineConfig, TraceBuffer, TraceEvent, Tracer, TracerConfig, diff_worlds, merge,
};
use longtable_foundation::{EntityId, Interner, KeywordId, LtMap, Type, Value};
use longtable_storage::World;
use longtable_storage::schema::{ComponentSchema, FieldSchema};

// =============================================================================
// Helper Functions
// =============================================================================

/// Creates a world with registered component schemas.
fn create_world_with_schema() -> (World, KeywordId, KeywordId) {
    let mut world = World::new(0);
    let health = world.interner_mut().intern_keyword("health");
    let value_field = world.interner_mut().intern_keyword("value");

    world = world
        .register_component(
            ComponentSchema::new(health).with_field(FieldSchema::required(value_field, Type::Int)),
        )
        .unwrap();

    (world, health, value_field)
}

/// Creates a value map for a component.
fn make_value_map(value_field: KeywordId, value: Value) -> Value {
    let mut map = LtMap::new();
    map = map.insert(Value::Keyword(value_field), value);
    Value::Map(map)
}

/// Creates a world with N entities having a health component.
fn create_populated_world(n: usize) -> (World, KeywordId, KeywordId) {
    let (mut world, health, value_field) = create_world_with_schema();

    for i in 0..n {
        let (w, e) = world.spawn(&LtMap::new()).unwrap();
        world = w
            .set(e, health, make_value_map(value_field, Value::Int(i as i64)))
            .unwrap();
    }

    (world, health, value_field)
}

/// Creates a typical tick summary.
fn typical_summary() -> TickSummary {
    TickSummary::success()
        .with_spawned(5)
        .with_destroyed(1)
        .with_writes(20)
        .with_rules(10)
}

// =============================================================================
// Scale Timeline Benchmarks
// =============================================================================

fn bench_scale_timeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_timeline");
    group.sample_size(20);

    // Timeline capture at increasing history sizes
    for size in [100, 500, 1_000] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::new("capture_to_size", size),
            &size,
            |b, &size| {
                b.iter_batched(
                    || {
                        let config = TimelineConfig::default().with_history_size(size * 2);
                        Timeline::with_config(config)
                    },
                    |mut timeline| {
                        for i in 0..size {
                            let world = World::new(i as u64);
                            timeline.capture(i as u64, world, typical_summary());
                        }
                        black_box(timeline)
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }

    // Rollback at various depths from large history
    for depth in [10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("rollback_from_large", depth),
            &depth,
            |b, &depth| {
                let config = TimelineConfig::default().with_history_size(500);
                let mut timeline = Timeline::with_config(config);
                for i in 0..500 {
                    timeline.capture(i, World::new(i), TickSummary::success());
                }

                b.iter(|| black_box(timeline.rollback(black_box(depth))))
            },
        );
    }

    // Goto tick in large history
    for history_size in [100, 500] {
        group.bench_with_input(
            BenchmarkId::new("goto_tick_large_history", history_size),
            &history_size,
            |b, &history_size| {
                let config = TimelineConfig::default().with_history_size(history_size);
                let mut timeline = Timeline::with_config(config);
                for i in 0..history_size {
                    timeline.capture(i as u64, World::new(i as u64), TickSummary::success());
                }
                let middle_tick = history_size as u64 / 2;

                b.iter(|| black_box(timeline.goto_tick(black_box(middle_tick))))
            },
        );
    }

    group.finish();
}

// =============================================================================
// Scale History Buffer Benchmarks
// =============================================================================

fn bench_scale_history_buffer(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_history_buffer");
    group.sample_size(20);

    // Push to large capacity
    for capacity in [500, 1_000, 2_500] {
        group.throughput(Throughput::Elements(capacity as u64));
        group.bench_with_input(
            BenchmarkId::new("push_large_capacity", capacity),
            &capacity,
            |b, &capacity| {
                b.iter_batched(
                    || HistoryBuffer::new(capacity),
                    |mut buffer| {
                        for i in 0..capacity {
                            buffer.push_tick(
                                i as u64,
                                World::new(i as u64),
                                TickSummary::success(),
                            );
                        }
                        black_box(buffer)
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }

    // Recent N from large buffer
    for count in [50, 100, 250] {
        group.bench_with_input(
            BenchmarkId::new("recent_from_large", count),
            &count,
            |b, &count| {
                let mut buffer = HistoryBuffer::new(1000);
                for i in 0..1000 {
                    buffer.push_tick(i, World::new(i), TickSummary::success());
                }

                b.iter(|| {
                    let recent: Vec<_> = buffer.recent(black_box(count)).collect();
                    black_box(recent)
                })
            },
        );
    }

    // Iterate large buffer
    for capacity in [500, 1_000] {
        group.bench_with_input(
            BenchmarkId::new("iter_large_buffer", capacity),
            &capacity,
            |b, &capacity| {
                let mut buffer = HistoryBuffer::new(capacity);
                for i in 0..capacity {
                    buffer.push_tick(i as u64, World::new(i as u64), TickSummary::success());
                }

                b.iter(|| {
                    let sum: u64 = buffer.iter().map(|s| s.tick()).sum();
                    black_box(sum)
                })
            },
        );
    }

    group.finish();
}

// =============================================================================
// Scale Diff Benchmarks
// =============================================================================

fn bench_scale_diff(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_diff");
    group.sample_size(20);

    // Diff identical worlds of increasing size
    for size in [100, 250, 500] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::new("diff_identical", size),
            &size,
            |b, &size| {
                let (world, _, _) = create_populated_world(size);
                let world2 = world.clone();

                b.iter(|| {
                    black_box(diff_worlds(
                        black_box(&world),
                        black_box(&world2),
                        DiffGranularity::Field,
                    ))
                })
            },
        );
    }

    // Diff with varying change percentages
    for (name, change_pct) in [("5pct", 5), ("20pct", 20), ("50pct", 50)] {
        group.bench_with_input(
            BenchmarkId::new("diff_changes", name),
            &change_pct,
            |b, &change_pct| {
                let (world1, health, value_field) = create_populated_world(200);
                let mut world2 = world1.clone();

                let entities: Vec<_> = world2.entities().collect();
                for (i, &entity) in entities.iter().enumerate() {
                    if i % (100 / change_pct) == 0 {
                        world2 = world2
                            .set(entity, health, make_value_map(value_field, Value::Int(999)))
                            .unwrap();
                    }
                }

                b.iter(|| {
                    black_box(diff_worlds(
                        black_box(&world1),
                        black_box(&world2),
                        DiffGranularity::Field,
                    ))
                })
            },
        );
    }

    // Diff at different granularities on larger world
    for granularity in [
        DiffGranularity::Entity,
        DiffGranularity::Component,
        DiffGranularity::Field,
    ] {
        let name = match granularity {
            DiffGranularity::Entity => "entity",
            DiffGranularity::Component => "component",
            DiffGranularity::Field => "field",
        };

        group.bench_with_input(
            BenchmarkId::new("granularity_scale", name),
            &granularity,
            |b, &granularity| {
                let (world1, health, value_field) = create_populated_world(200);
                let mut world2 = world1.clone();

                let entities: Vec<_> = world2.entities().collect();
                for (i, &entity) in entities.iter().enumerate() {
                    if i % 10 == 0 {
                        world2 = world2
                            .set(entity, health, make_value_map(value_field, Value::Int(999)))
                            .unwrap();
                    }
                }

                b.iter(|| {
                    black_box(diff_worlds(
                        black_box(&world1),
                        black_box(&world2),
                        granularity,
                    ))
                })
            },
        );
    }

    group.finish();
}

// =============================================================================
// Scale Merge Benchmarks
// =============================================================================

fn bench_scale_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_merge");
    group.sample_size(20);

    // Merge strategies at scale
    for size in [100, 250, 500] {
        for strategy in [MergeStrategy::Replace, MergeStrategy::Compare] {
            let strategy_name = match strategy {
                MergeStrategy::Replace => "replace",
                MergeStrategy::Compare => "compare",
            };

            group.bench_with_input(
                BenchmarkId::new(format!("merge_{strategy_name}"), size),
                &(size, strategy),
                |b, &(size, strategy)| {
                    let (base, _, _) = create_populated_world(size);
                    let (current, _, _) = create_populated_world(size);
                    let (incoming, _, _) = create_populated_world(size);

                    b.iter(|| {
                        black_box(merge(
                            black_box(&base),
                            black_box(&current),
                            black_box(&incoming),
                            strategy,
                        ))
                    })
                },
            );
        }
    }

    group.finish();
}

// =============================================================================
// Scale Trace Benchmarks
// =============================================================================

fn bench_scale_trace(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_trace");
    group.sample_size(20);

    // Trace buffer at larger sizes
    for capacity in [1_000, 5_000, 10_000] {
        group.throughput(Throughput::Elements(capacity as u64));
        group.bench_with_input(
            BenchmarkId::new("buffer_push_scale", capacity),
            &capacity,
            |b, &capacity| {
                b.iter_batched(
                    || TraceBuffer::new(capacity),
                    |mut buffer| {
                        for i in 0..capacity {
                            buffer.push(i as u64, 0, TraceEvent::TickStart { tick: i as u64 });
                        }
                        black_box(buffer)
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }

    // Tracer batch recording at scale
    for count in [500, 1_000, 2_500] {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("tracer_batch_record", count),
            &count,
            |b, &count| {
                b.iter_batched(
                    || {
                        let config = TracerConfig::new().enabled().with_buffer_size(count * 3);
                        Tracer::new(config)
                    },
                    |mut tracer| {
                        for i in 0..count {
                            tracer.tick_start(i as u64);
                            tracer.tick_end(i as u64, true);
                        }
                        black_box(tracer)
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }

    // Recent records from large buffer
    for count in [100, 250, 500] {
        group.bench_with_input(
            BenchmarkId::new("buffer_recent_scale", count),
            &count,
            |b, &count| {
                let mut buffer = TraceBuffer::new(5000);
                for i in 0..5000 {
                    buffer.push(i, 0, TraceEvent::TickStart { tick: i });
                }

                b.iter(|| {
                    let recent = buffer.recent(black_box(count));
                    black_box(recent)
                })
            },
        );
    }

    // Records in range
    for range_size in [100, 500] {
        group.bench_with_input(
            BenchmarkId::new("buffer_range_scale", range_size),
            &range_size,
            |b, &range_size| {
                let mut buffer = TraceBuffer::new(5000);
                for i in 0..5000 {
                    buffer.push(i, 0, TraceEvent::TickStart { tick: i });
                }

                b.iter(|| {
                    let range = buffer
                        .records_in_range(black_box(1000), black_box(1000 + range_size as u64));
                    black_box(range)
                })
            },
        );
    }

    group.finish();
}

// =============================================================================
// Scale Debug Infrastructure Benchmarks
// =============================================================================

fn bench_scale_debug(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_debug");
    group.sample_size(20);

    // Many breakpoints lookup
    for bp_count in [50, 100, 250] {
        group.bench_with_input(
            BenchmarkId::new("breakpoint_lookup_many", bp_count),
            &bp_count,
            |b, &bp_count| {
                let mut registry = BreakpointRegistry::new();
                let mut interner = Interner::new();

                for i in 0..bp_count {
                    let rule = interner.intern_keyword(&format!("rule-{i}"));
                    registry.add_rule(rule);
                }

                let target_rule = interner.intern_keyword(&format!("rule-{}", bp_count / 2));

                b.iter(|| black_box(registry.get_rule_breakpoints(black_box(target_rule))))
            },
        );
    }

    // Component write breakpoint lookup with many breakpoints
    for bp_count in [50, 100, 250] {
        group.bench_with_input(
            BenchmarkId::new("component_bp_lookup_many", bp_count),
            &bp_count,
            |b, &bp_count| {
                let mut registry = BreakpointRegistry::new();
                let mut interner = Interner::new();
                let entity = EntityId::new(1, 0);

                for i in 0..bp_count {
                    let comp = interner.intern_keyword(&format!("component-{i}"));
                    registry.add_component_write(None, comp);
                }

                let target_comp = interner.intern_keyword(&format!("component-{}", bp_count / 2));

                b.iter(|| {
                    black_box(
                        registry.get_component_write_breakpoints(
                            black_box(entity),
                            black_box(target_comp),
                        ),
                    )
                })
            },
        );
    }

    // Adding many breakpoints
    for count in [50, 100] {
        group.bench_with_input(
            BenchmarkId::new("add_many_breakpoints", count),
            &count,
            |b, &count| {
                let mut interner = Interner::new();
                let rules: Vec<_> = (0..count)
                    .map(|i| interner.intern_keyword(&format!("rule-{i}")))
                    .collect();

                b.iter_batched(
                    BreakpointRegistry::new,
                    |mut registry| {
                        for &rule in &rules {
                            registry.add_rule(rule);
                        }
                        black_box(registry)
                    },
                    BatchSize::SmallInput,
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
    bench_scale_timeline,
    bench_scale_history_buffer,
    bench_scale_diff,
    bench_scale_merge,
    bench_scale_trace,
    bench_scale_debug,
);

criterion_main!(benches);
