//! Benchmarks for longtable_debug.
//!
//! Covers timeline, diff, merge, explain, trace, and debug functionality.

use criterion::{
    BatchSize, BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main,
};
use longtable_debug::explain::{
    CausalChain, CausalLink, ClauseMatchStats, QueryExplanation, QueryExplanationBuilder, WhyResult,
};
use longtable_debug::timeline::{
    Branch, BranchId, BranchRegistry, DiffGranularity, HistoryBuffer, MergeStrategy, TickSummary,
    Timeline, TimelineConfig, diff_summary, diff_worlds, format_diff, merge,
};
use longtable_debug::{DebugSession, PauseReason, TraceBuffer, TraceEvent, Tracer, TracerConfig};
use longtable_engine::provenance::{ProvenanceTracker, ProvenanceVerbosity};
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

/// Creates a tick summary with typical values.
fn typical_summary() -> TickSummary {
    TickSummary::success()
        .with_spawned(5)
        .with_destroyed(1)
        .with_writes(20)
        .with_rules(10)
}

// =============================================================================
// Timeline Benchmarks
// =============================================================================

fn timeline_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("timeline");

    // Timeline creation
    group.bench_function("timeline_new", |b| b.iter(|| black_box(Timeline::new())));

    group.bench_function("timeline_with_config", |b| {
        b.iter(|| {
            let config = TimelineConfig::default()
                .with_history_size(200)
                .with_granularity(DiffGranularity::Field);
            black_box(Timeline::with_config(config))
        })
    });

    // Capture at various history sizes
    for size in [10, 100, 1000] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::new("timeline_capture", size),
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

    // Snapshot retrieval
    group.bench_function("timeline_get_snapshot", |b| {
        let mut timeline = Timeline::new();
        for i in 0..50 {
            timeline.capture(i, World::new(i), TickSummary::success());
        }

        b.iter(|| black_box(timeline.get_snapshot(black_box(25))))
    });

    group.bench_function("timeline_latest_snapshot", |b| {
        let mut timeline = Timeline::new();
        for i in 0..50 {
            timeline.capture(i, World::new(i), TickSummary::success());
        }

        b.iter(|| black_box(timeline.latest_snapshot()))
    });

    // Rollback operations
    for ticks_back in [1, 5, 25, 50] {
        group.bench_with_input(
            BenchmarkId::new("timeline_rollback", ticks_back),
            &ticks_back,
            |b, &ticks_back| {
                let mut timeline = Timeline::new();
                for i in 0..100 {
                    timeline.capture(i, World::new(i), TickSummary::success());
                }

                b.iter(|| black_box(timeline.rollback(black_box(ticks_back))))
            },
        );
    }

    // Goto tick
    group.bench_function("timeline_goto_tick", |b| {
        let mut timeline = Timeline::new();
        for i in 0..100 {
            timeline.capture(i, World::new(i), TickSummary::success());
        }

        b.iter(|| black_box(timeline.goto_tick(black_box(42))))
    });

    // Tick range
    group.bench_function("timeline_tick_range", |b| {
        let mut timeline = Timeline::new();
        for i in 0..100 {
            timeline.capture(i, World::new(i), TickSummary::success());
        }

        b.iter(|| black_box(timeline.tick_range()))
    });

    group.finish();
}

// =============================================================================
// History Buffer Benchmarks
// =============================================================================

fn history_buffer_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("history_buffer");

    // Push operations
    for capacity in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(capacity as u64));
        group.bench_with_input(
            BenchmarkId::new("push_to_capacity", capacity),
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

    // Push with eviction (buffer at capacity)
    group.bench_function("push_with_eviction", |b| {
        b.iter_batched(
            || {
                let mut buffer = HistoryBuffer::new(100);
                for i in 0..100 {
                    buffer.push_tick(i, World::new(i), TickSummary::success());
                }
                buffer
            },
            |mut buffer| {
                buffer.push_tick(100, World::new(100), TickSummary::success());
                black_box(buffer)
            },
            BatchSize::SmallInput,
        )
    });

    // Get by tick (various positions)
    for position in ["first", "middle", "last", "missing"] {
        group.bench_with_input(
            BenchmarkId::new("get_by_tick", position),
            &position,
            |b, &position| {
                let mut buffer = HistoryBuffer::new(100);
                for i in 0..100 {
                    buffer.push_tick(i, World::new(i), TickSummary::success());
                }

                let tick = match position {
                    "first" => 0,
                    "middle" => 50,
                    "last" => 99,
                    "missing" => 200,
                    _ => 0,
                };

                b.iter(|| black_box(buffer.get(black_box(tick))))
            },
        );
    }

    // Latest/oldest retrieval
    group.bench_function("latest", |b| {
        let mut buffer = HistoryBuffer::new(100);
        for i in 0..100 {
            buffer.push_tick(i, World::new(i), TickSummary::success());
        }

        b.iter(|| black_box(buffer.latest()))
    });

    group.bench_function("oldest", |b| {
        let mut buffer = HistoryBuffer::new(100);
        for i in 0..100 {
            buffer.push_tick(i, World::new(i), TickSummary::success());
        }

        b.iter(|| black_box(buffer.oldest()))
    });

    // Recent N
    for count in [5, 10, 50] {
        group.bench_with_input(BenchmarkId::new("recent", count), &count, |b, &count| {
            let mut buffer = HistoryBuffer::new(100);
            for i in 0..100 {
                buffer.push_tick(i, World::new(i), TickSummary::success());
            }

            b.iter(|| {
                let recent: Vec<_> = buffer.recent(black_box(count)).collect();
                black_box(recent)
            })
        });
    }

    // Truncate after
    group.bench_function("truncate_after", |b| {
        b.iter_batched(
            || {
                let mut buffer = HistoryBuffer::new(100);
                for i in 0..100 {
                    buffer.push_tick(i, World::new(i), TickSummary::success());
                }
                buffer
            },
            |mut buffer| {
                buffer.truncate_after(50);
                black_box(buffer)
            },
            BatchSize::SmallInput,
        )
    });

    // Iteration
    group.bench_function("iter", |b| {
        let mut buffer = HistoryBuffer::new(100);
        for i in 0..100 {
            buffer.push_tick(i, World::new(i), TickSummary::success());
        }

        b.iter(|| {
            let sum: u64 = buffer.iter().map(|s| s.tick()).sum();
            black_box(sum)
        })
    });

    group.finish();
}

// =============================================================================
// Branch Benchmarks
// =============================================================================

fn branch_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("branch");

    // Branch creation
    group.bench_function("branch_new", |b| {
        b.iter(|| {
            black_box(Branch::new(
                BranchId::new(1),
                "test".to_string(),
                10,
                Some(BranchId::new(0)),
            ))
        })
    });

    // Branch snapshot operations
    group.bench_function("branch_push_snapshot", |b| {
        b.iter_batched(
            || Branch::new(BranchId::new(1), "test".to_string(), 0, None),
            |mut branch| {
                branch.push_snapshot(1, World::new(1), TickSummary::success());
                black_box(branch)
            },
            BatchSize::SmallInput,
        )
    });

    // Branch registry
    group.bench_function("registry_new", |b| {
        b.iter(|| black_box(BranchRegistry::new()))
    });

    // Create branches
    for count in [5, 10, 50] {
        group.bench_with_input(
            BenchmarkId::new("registry_create_branches", count),
            &count,
            |b, &count| {
                b.iter_batched(
                    BranchRegistry::new,
                    |mut registry| {
                        for i in 0..count {
                            registry.create_branch(
                                format!("branch-{i}"),
                                registry.main_id(),
                                i as u64,
                            );
                        }
                        black_box(registry)
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }

    // Lookup by name
    group.bench_function("registry_get_by_name", |b| {
        let mut registry = BranchRegistry::new();
        for i in 0..20 {
            registry.create_branch(format!("branch-{i}"), registry.main_id(), 0);
        }

        b.iter(|| black_box(registry.get_by_name(black_box("branch-10"))))
    });

    // Lookup by ID
    group.bench_function("registry_get_by_id", |b| {
        let mut registry = BranchRegistry::new();
        let mut last_id = registry.main_id();
        for i in 0..20 {
            if let Some(id) = registry.create_branch(format!("branch-{i}"), registry.main_id(), 0) {
                last_id = id;
            }
        }

        b.iter(|| black_box(registry.get(black_box(last_id))))
    });

    // Delete branch
    group.bench_function("registry_delete", |b| {
        b.iter_batched(
            || {
                let mut registry = BranchRegistry::new();
                let id = registry
                    .create_branch("to-delete".to_string(), registry.main_id(), 0)
                    .unwrap();
                (registry, id)
            },
            |(mut registry, id)| {
                registry.delete(id);
                black_box(registry)
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

// =============================================================================
// Diff Benchmarks
// =============================================================================

fn diff_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("diff");

    // Diff identical worlds
    for size in [10, 100, 1000] {
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

    // Diff with some changes
    for (name, change_pct) in [("1pct", 1), ("10pct", 10), ("50pct", 50)] {
        group.bench_with_input(
            BenchmarkId::new("diff_changes", name),
            &change_pct,
            |b, &change_pct| {
                let (world1, health, value_field) = create_populated_world(100);
                let mut world2 = world1.clone();

                // Modify some entities
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

    // Diff granularity comparison
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
            BenchmarkId::new("diff_granularity", name),
            &granularity,
            |b, &granularity| {
                let (world1, health, value_field) = create_populated_world(100);
                let mut world2 = world1.clone();

                // Modify 10% of entities
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

    // Diff summary generation
    group.bench_function("diff_summary", |b| {
        let (world1, health, value_field) = create_populated_world(100);
        let mut world2 = world1.clone();

        let entities: Vec<_> = world2.entities().collect();
        for (i, &entity) in entities.iter().enumerate() {
            if i % 10 == 0 {
                world2 = world2
                    .set(entity, health, make_value_map(value_field, Value::Int(999)))
                    .unwrap();
            }
        }

        let diff = diff_worlds(&world1, &world2, DiffGranularity::Field);

        b.iter(|| black_box(diff_summary(black_box(&diff))))
    });

    // Format diff
    for max_entities in [5, 10, 50] {
        group.bench_with_input(
            BenchmarkId::new("format_diff", max_entities),
            &max_entities,
            |b, &max_entities| {
                let (world1, health, value_field) = create_populated_world(100);
                let mut world2 = world1.clone();

                let entities: Vec<_> = world2.entities().collect();
                for (i, &entity) in entities.iter().enumerate() {
                    if i % 5 == 0 {
                        world2 = world2
                            .set(entity, health, make_value_map(value_field, Value::Int(999)))
                            .unwrap();
                    }
                }

                let diff = diff_worlds(&world1, &world2, DiffGranularity::Field);
                let interner = world1.interner();

                b.iter(|| black_box(format_diff(black_box(&diff), interner, max_entities)))
            },
        );
    }

    group.finish();
}

// =============================================================================
// Merge Benchmarks
// =============================================================================

fn merge_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("merge");

    // Replace strategy
    for size in [10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::new("merge_replace", size),
            &size,
            |b, &size| {
                let (base, _, _) = create_populated_world(size);
                let (current, _, _) = create_populated_world(size);
                let (incoming, _, _) = create_populated_world(size);

                b.iter(|| {
                    black_box(merge(
                        black_box(&base),
                        black_box(&current),
                        black_box(&incoming),
                        MergeStrategy::Replace,
                    ))
                })
            },
        );
    }

    // Compare strategy
    for size in [10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::new("merge_compare", size),
            &size,
            |b, &size| {
                let (base, _, _) = create_populated_world(size);
                let (current, _, _) = create_populated_world(size);
                let (incoming, _, _) = create_populated_world(size);

                b.iter(|| {
                    black_box(merge(
                        black_box(&base),
                        black_box(&current),
                        black_box(&incoming),
                        MergeStrategy::Compare,
                    ))
                })
            },
        );
    }

    group.finish();
}

// =============================================================================
// Trace Benchmarks
// =============================================================================

fn trace_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("trace");

    // Tracer creation
    group.bench_function("tracer_new_disabled", |b| {
        b.iter(|| black_box(Tracer::disabled()))
    });

    group.bench_function("tracer_new_enabled", |b| {
        b.iter(|| {
            let config = TracerConfig::new().enabled().with_buffer_size(1000);
            black_box(Tracer::new(config))
        })
    });

    // CRITICAL: Record overhead when disabled
    group.bench_function("record_when_disabled", |b| {
        let mut tracer = Tracer::disabled();

        b.iter(|| {
            tracer.record(TraceEvent::TickStart { tick: 1 });
            black_box(())
        })
    });

    // Record overhead when enabled
    group.bench_function("record_when_enabled", |b| {
        let config = TracerConfig::new().enabled().with_buffer_size(10000);
        let mut tracer = Tracer::new(config);

        b.iter(|| {
            tracer.record(TraceEvent::TickStart { tick: black_box(1) });
        })
    });

    // Record various event types
    group.bench_function("record_tick_start", |b| {
        let config = TracerConfig::new().enabled().with_buffer_size(10000);
        let mut tracer = Tracer::new(config);

        b.iter(|| {
            tracer.tick_start(black_box(1));
        })
    });

    group.bench_function("record_component_write", |b| {
        let config = TracerConfig::new().enabled().with_buffer_size(10000);
        let mut tracer = Tracer::new(config);
        let entity = EntityId::new(1, 0);
        let mut interner = Interner::new();
        let component = interner.intern_keyword("health");

        b.iter(|| {
            tracer.component_write(
                black_box(entity),
                black_box(component),
                Some(Value::Int(100)),
                Value::Int(90),
                None,
            );
        })
    });

    // Batch recording
    for count in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("batch_record", count),
            &count,
            |b, &count| {
                b.iter_batched(
                    || {
                        let config = TracerConfig::new().enabled().with_buffer_size(count * 2);
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

    group.finish();
}

// =============================================================================
// Trace Buffer Benchmarks
// =============================================================================

fn trace_buffer_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("trace_buffer");

    // Push operations
    for capacity in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(capacity as u64));
        group.bench_with_input(
            BenchmarkId::new("push_to_capacity", capacity),
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

    // Push with eviction
    group.bench_function("push_with_eviction", |b| {
        b.iter_batched(
            || {
                let mut buffer = TraceBuffer::new(100);
                for i in 0..100 {
                    buffer.push(i, 0, TraceEvent::TickStart { tick: i });
                }
                buffer
            },
            |mut buffer| {
                buffer.push(100, 0, TraceEvent::TickStart { tick: 100 });
                black_box(buffer)
            },
            BatchSize::SmallInput,
        )
    });

    // Recent records
    for count in [10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("recent_records", count),
            &count,
            |b, &count| {
                let mut buffer = TraceBuffer::new(1000);
                for i in 0..1000 {
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
    for count in [10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("records_in_range", count),
            &count,
            |b, &count| {
                let mut buffer = TraceBuffer::new(1000);
                for i in 0..1000 {
                    buffer.push(i, 0, TraceEvent::TickStart { tick: i });
                }

                b.iter(|| {
                    let range =
                        buffer.records_in_range(black_box(400), black_box(400 + count as u64));
                    black_box(range)
                })
            },
        );
    }

    // Records for tick
    group.bench_function("records_for_tick", |b| {
        let mut buffer = TraceBuffer::new(1000);
        for tick in 0..100 {
            for _ in 0..10 {
                buffer.push(tick, 0, TraceEvent::TickStart { tick });
            }
        }

        b.iter(|| {
            let records = buffer.records_for_tick(black_box(50));
            black_box(records)
        })
    });

    // Iteration
    group.bench_function("iter", |b| {
        let mut buffer = TraceBuffer::new(1000);
        for i in 0..1000 {
            buffer.push(i, 0, TraceEvent::TickStart { tick: i });
        }

        b.iter(|| {
            let count = buffer.iter().count();
            black_box(count)
        })
    });

    // Stats
    group.bench_function("stats", |b| {
        let mut buffer = TraceBuffer::new(1000);
        for i in 0..1000 {
            buffer.push(i, 0, TraceEvent::TickStart { tick: i });
        }

        b.iter(|| black_box(buffer.stats()))
    });

    group.finish();
}

// =============================================================================
// Debug Session Benchmarks
// =============================================================================

fn debug_session_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("debug_session");

    // Session creation
    group.bench_function("session_new", |b| b.iter(|| black_box(DebugSession::new())));

    // State transitions
    group.bench_function("pause_resume", |b| {
        b.iter_batched(
            DebugSession::new,
            |mut session| {
                session.pause(PauseReason::UserRequest);
                session.resume();
                black_box(session)
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("step_modes", |b| {
        b.iter_batched(
            DebugSession::new,
            |mut session| {
                session.step_rule();
                session.resume();
                session.step_phase();
                session.resume();
                session.step_tick();
                black_box(session)
            },
            BatchSize::SmallInput,
        )
    });

    // Breakpoint checks (the hot path)
    group.bench_function("should_break_on_rule_no_bp", |b| {
        let session = DebugSession::new();
        let mut interner = Interner::new();
        let rule = interner.intern_keyword("test-rule");

        b.iter(|| black_box(session.should_break_on_rule(black_box(rule))))
    });

    group.bench_function("should_break_on_rule_with_bp", |b| {
        let mut session = DebugSession::new();
        let mut interner = Interner::new();
        let rule = interner.intern_keyword("test-rule");
        session.breakpoints_mut().add_rule(rule);

        b.iter(|| black_box(session.should_break_on_rule(black_box(rule))))
    });

    group.bench_function("should_break_on_component_write_no_bp", |b| {
        let session = DebugSession::new();
        let entity = EntityId::new(1, 0);
        let mut interner = Interner::new();
        let component = interner.intern_keyword("health");

        b.iter(|| {
            black_box(
                session.should_break_on_component_write(black_box(entity), black_box(component)),
            )
        })
    });

    // On-event handlers (for stepping)
    group.bench_function("on_rule_enter", |b| {
        b.iter_batched(
            || {
                let mut session = DebugSession::new();
                session.step_rule();
                session.on_rule_enter("first-rule");
                session.resume();
                session.step_rule();
                session
            },
            |mut session| {
                session.on_rule_enter("another-rule");
                black_box(session)
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("on_phase_enter", |b| {
        b.iter_batched(
            || {
                let mut session = DebugSession::new();
                session.step_phase();
                session.on_phase_enter("activation");
                session.resume();
                session.step_phase();
                session
            },
            |mut session| {
                session.on_phase_enter("firing");
                black_box(session)
            },
            BatchSize::SmallInput,
        )
    });

    // Status summary
    group.bench_function("status_summary", |b| {
        let mut session = DebugSession::new();
        session.set_current_tick(42);
        session.set_current_phase(Some("activation".to_string()));
        session.set_current_rule(Some("test-rule".to_string()));
        session.pause(PauseReason::UserRequest);

        b.iter(|| black_box(session.status_summary()))
    });

    group.finish();
}

// =============================================================================
// Breakpoint Registry Benchmarks
// =============================================================================

fn breakpoint_registry_benchmarks(c: &mut Criterion) {
    use longtable_debug::BreakpointRegistry;

    let mut group = c.benchmark_group("breakpoint_registry");

    // Add breakpoints
    group.bench_function("add_rule_breakpoint", |b| {
        let mut interner = Interner::new();
        let rule = interner.intern_keyword("test-rule");

        b.iter_batched(
            BreakpointRegistry::new,
            |mut registry| {
                registry.add_rule(rule);
                black_box(registry)
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("add_component_write_breakpoint", |b| {
        let mut interner = Interner::new();
        let component = interner.intern_keyword("health");

        b.iter_batched(
            BreakpointRegistry::new,
            |mut registry| {
                registry.add_component_write(None, component);
                black_box(registry)
            },
            BatchSize::SmallInput,
        )
    });

    // Lookup with varying number of breakpoints
    for bp_count in [0, 10, 100] {
        group.bench_with_input(
            BenchmarkId::new("get_rule_breakpoints", bp_count),
            &bp_count,
            |b, &bp_count| {
                let mut registry = BreakpointRegistry::new();
                let mut interner = Interner::new();

                for i in 0..bp_count {
                    let rule = interner.intern_keyword(&format!("rule-{i}"));
                    registry.add_rule(rule);
                }

                let target_rule = interner.intern_keyword("rule-5");

                b.iter(|| black_box(registry.get_rule_breakpoints(black_box(target_rule))))
            },
        );
    }

    // Get component write breakpoints
    for bp_count in [0, 10, 100] {
        group.bench_with_input(
            BenchmarkId::new("get_component_write_breakpoints", bp_count),
            &bp_count,
            |b, &bp_count| {
                let mut registry = BreakpointRegistry::new();
                let mut interner = Interner::new();
                let entity = EntityId::new(1, 0);

                for i in 0..bp_count {
                    let comp = interner.intern_keyword(&format!("component-{i}"));
                    registry.add_component_write(None, comp);
                }

                let target_comp = interner.intern_keyword("component-5");

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

    // Remove breakpoint
    group.bench_function("remove_breakpoint", |b| {
        let mut interner = Interner::new();
        let rule = interner.intern_keyword("test-rule");

        b.iter_batched(
            || {
                let mut registry = BreakpointRegistry::new();
                let id = registry.add_rule(rule);
                (registry, id)
            },
            |(mut registry, id)| {
                registry.remove(id);
                black_box(registry)
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

// =============================================================================
// Watch Registry Benchmarks
// =============================================================================

fn watch_registry_benchmarks(c: &mut Criterion) {
    use longtable_debug::WatchRegistry;

    let mut group = c.benchmark_group("watch_registry");

    // Add watches
    group.bench_function("add_watch", |b| {
        b.iter_batched(
            WatchRegistry::new,
            |mut registry| {
                registry.add("(get ?player :health)".to_string());
                black_box(registry)
            },
            BatchSize::SmallInput,
        )
    });

    // Add multiple watches
    for count in [5, 10, 50] {
        group.bench_with_input(
            BenchmarkId::new("add_many_watches", count),
            &count,
            |b, &count| {
                b.iter_batched(
                    WatchRegistry::new,
                    |mut registry| {
                        for i in 0..count {
                            registry.add(format!("(get ?entity-{i} :component-{i})"));
                        }
                        black_box(registry)
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }

    // Get watch
    group.bench_function("get_watch", |b| {
        let mut registry = WatchRegistry::new();
        let id = registry.add("test".to_string());

        b.iter(|| black_box(registry.get(black_box(id))))
    });

    // Iterate watches
    for count in [5, 10, 50] {
        group.bench_with_input(
            BenchmarkId::new("iter_watches", count),
            &count,
            |b, &count| {
                let mut registry = WatchRegistry::new();
                for i in 0..count {
                    registry.add(format!("expr-{i}"));
                }

                b.iter(|| {
                    let watches: Vec<_> = registry.iter().collect();
                    black_box(watches)
                })
            },
        );
    }

    // Remove watch
    group.bench_function("remove_watch", |b| {
        b.iter_batched(
            || {
                let mut registry = WatchRegistry::new();
                let id = registry.add("test".to_string());
                (registry, id)
            },
            |(mut registry, id)| {
                registry.remove(id);
                black_box(registry)
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

// =============================================================================
// Explain Benchmarks
// =============================================================================

fn explain_benchmarks(c: &mut Criterion) {
    use longtable_debug::WhyQuery;

    let mut group = c.benchmark_group("explain");

    // CausalLink creation
    group.bench_function("causal_link_create", |b| {
        let mut interner = Interner::new();
        let health = interner.intern_keyword("health");
        let apply_damage = interner.intern_keyword("apply-damage");
        let entity = EntityId::new(1, 0);

        b.iter(|| {
            black_box(CausalLink {
                entity,
                component: health,
                value: Some(Value::Int(75)),
                rule: apply_damage,
                tick: 5,
                context: vec![("?target".to_string(), entity)],
                bindings: None,
                previous_value: Some(Value::Int(100)),
            })
        })
    });

    // CausalChain operations
    group.bench_function("causal_chain_single", |b| {
        let mut interner = Interner::new();
        let health = interner.intern_keyword("health");
        let apply_damage = interner.intern_keyword("apply-damage");
        let entity = EntityId::new(1, 0);

        let link = CausalLink {
            entity,
            component: health,
            value: Some(Value::Int(75)),
            rule: apply_damage,
            tick: 5,
            context: vec![],
            bindings: None,
            previous_value: None,
        };

        b.iter(|| black_box(CausalChain::single(link.clone())))
    });

    group.bench_function("causal_chain_accessors", |b| {
        let mut interner = Interner::new();
        let health = interner.intern_keyword("health");
        let apply_damage = interner.intern_keyword("apply-damage");
        let entity = EntityId::new(1, 0);

        let link = CausalLink {
            entity,
            component: health,
            value: Some(Value::Int(75)),
            rule: apply_damage,
            tick: 5,
            context: vec![],
            bindings: None,
            previous_value: None,
        };
        let chain = CausalChain::single(link);

        b.iter(|| {
            black_box(chain.len());
            black_box(chain.is_empty());
            black_box(chain.immediate_cause());
            black_box(chain.root_cause());
        })
    });

    // WhyResult operations
    group.bench_function("why_result_found_check", |b| {
        let mut interner = Interner::new();
        let health = interner.intern_keyword("health");
        let apply_damage = interner.intern_keyword("apply-damage");
        let entity = EntityId::new(1, 0);

        let link = CausalLink {
            entity,
            component: health,
            value: Some(Value::Int(75)),
            rule: apply_damage,
            tick: 5,
            context: vec![],
            bindings: None,
            previous_value: None,
        };
        let result = WhyResult::Single(Some(link));

        b.iter(|| {
            black_box(result.found());
            black_box(result.immediate_cause());
            black_box(result.last_writer());
            black_box(result.last_write_tick());
        })
    });

    // WhyQuery with provenance
    group.bench_function("why_query_single_hop", |b| {
        let mut interner = Interner::new();
        let health = interner.intern_keyword("health");
        let apply_damage = interner.intern_keyword("apply-damage");
        let entity = EntityId::new(1, 0);

        b.iter_batched(
            || {
                let mut tracker = ProvenanceTracker::with_verbosity(ProvenanceVerbosity::Standard);
                tracker.record_write(entity, health, apply_damage);
                tracker
            },
            |tracker| {
                let query = WhyQuery::new(&tracker);
                black_box(query.why(entity, health))
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("why_query_miss", |b| {
        let mut interner = Interner::new();
        let health = interner.intern_keyword("health");
        let entity = EntityId::new(1, 0);

        let tracker = ProvenanceTracker::with_verbosity(ProvenanceVerbosity::Standard);

        b.iter(|| {
            let query = WhyQuery::new(&tracker);
            black_box(query.why(entity, health))
        })
    });

    // Why query with different depths
    for depth in [1, 3, 5] {
        group.bench_with_input(
            BenchmarkId::new("why_query_depth", depth),
            &depth,
            |b, &depth| {
                let mut interner = Interner::new();
                let health = interner.intern_keyword("health");
                let apply_damage = interner.intern_keyword("apply-damage");
                let entity = EntityId::new(1, 0);

                b.iter_batched(
                    || {
                        let mut tracker =
                            ProvenanceTracker::with_verbosity(ProvenanceVerbosity::Standard);
                        tracker.record_write(entity, health, apply_damage);
                        tracker
                    },
                    |tracker| {
                        let query = WhyQuery::new(&tracker);
                        black_box(query.why_depth(entity, health, depth, Some(Value::Int(75))))
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

fn query_explanation_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_explanation");

    // ClauseMatchStats
    group.bench_function("clause_stats_create", |b| {
        b.iter(|| black_box(ClauseMatchStats::new(0, "[?e :health ?hp]")))
    });

    group.bench_function("clause_stats_accessors", |b| {
        let mut stats = ClauseMatchStats::new(0, "[?e :health ?hp]");
        stats.input_count = 100;
        stats.output_count = 45;

        b.iter(|| {
            black_box(stats.filtered_count());
            black_box(stats.pass_rate());
        })
    });

    // QueryExplanation creation
    group.bench_function("query_explanation_new", |b| {
        b.iter(|| black_box(QueryExplanation::new()))
    });

    // QueryExplanation with clauses
    for clause_count in [2, 5, 10] {
        group.bench_with_input(
            BenchmarkId::new("add_clauses", clause_count),
            &clause_count,
            |b, &clause_count| {
                b.iter_batched(
                    QueryExplanation::new,
                    |mut explanation| {
                        for i in 0..clause_count {
                            let mut stats = ClauseMatchStats::new(i, format!("[?e :comp-{i} ?v]"));
                            stats.input_count = 100 - i * 10;
                            stats.output_count = 90 - i * 10;
                            explanation.add_clause_stats(stats);
                        }
                        black_box(explanation)
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }

    // QueryExplanation accessors
    group.bench_function("most_selective_clause", |b| {
        let mut explanation = QueryExplanation::new();
        for i in 0..5 {
            let mut stats = ClauseMatchStats::new(i, format!("[?e :comp-{i} ?v]"));
            stats.input_count = 100;
            stats.output_count = if i == 2 { 10 } else { 80 };
            explanation.add_clause_stats(stats);
        }

        b.iter(|| black_box(explanation.most_selective_clause()))
    });

    group.bench_function("least_selective_clause", |b| {
        let mut explanation = QueryExplanation::new();
        for i in 0..5 {
            let mut stats = ClauseMatchStats::new(i, format!("[?e :comp-{i} ?v]"));
            stats.input_count = 100;
            stats.output_count = if i == 2 { 10 } else { 80 };
            explanation.add_clause_stats(stats);
        }

        b.iter(|| black_box(explanation.least_selective_clause()))
    });

    // QueryExplanationBuilder
    group.bench_function("builder_pattern", |b| {
        b.iter_batched(
            QueryExplanationBuilder::new,
            |mut builder| {
                builder.record_clause(0, "[?e :health ?hp]", 100, 45);
                builder.record_clause(1, "[?e :tag/enemy]", 45, 12);
                builder.record_pre_guard_count(12);
                builder.record_post_guard_count(10);
                builder.record_result_count(10);
                black_box(builder.build())
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

// =============================================================================
// Criterion Groups
// =============================================================================

criterion_group!(
    timeline_benches,
    timeline_benchmarks,
    history_buffer_benchmarks,
    branch_benchmarks,
);

criterion_group!(diff_benches, diff_benchmarks, merge_benchmarks,);

criterion_group!(trace_benches, trace_benchmarks, trace_buffer_benchmarks,);

criterion_group!(
    debug_benches,
    debug_session_benchmarks,
    breakpoint_registry_benchmarks,
    watch_registry_benchmarks,
);

criterion_group!(
    explain_benches,
    explain_benchmarks,
    query_explanation_benchmarks,
);

criterion_main!(
    timeline_benches,
    diff_benches,
    trace_benches,
    debug_benches,
    explain_benches
);
