# Benchmark Implementation Plan

## Current Status

| Crate | Benchmark File | Coverage |
|-------|----------------|----------|
| `longtable_foundation` | `foundation_benchmarks.rs` (363 LOC) | Good - Value, LtVec, LtSet, LtMap, Interner |
| `longtable_storage` | `storage_benchmarks.rs` (549 LOC) | Good - EntityStore, ComponentStore, RelationshipStore, World |
| `longtable_language` | `language_benchmarks.rs` (1320 LOC) | **Complete** - Lexer, Parser, Compiler, VM, Stdlib (expanded) |
| `longtable_engine` | `engine_benchmarks.rs` (1544 LOC) | Comprehensive - Patterns, Queries, Rules, Derived, Constraints, Tick |
| `longtable_runtime` | `serialization_benchmarks.rs` (320 LOC) | Partial - Serialization only |
| `longtable_stdlib` | N/A (placeholder crate) | Stdlib in language crate |
| `longtable_debug` | `debug_benchmarks.rs` (1450 LOC) | **Complete** - Timeline, Diff, Merge, Trace, Debug, Explain |

---

## Stage 1: longtable_debug Benchmarks ✅ COMPLETE

All Phase 6 observability code now has benchmark coverage.

### File: `crates/longtable_debug/benches/debug_benchmarks.rs`

#### 1.1 Timeline Benchmarks

```rust
// HistoryBuffer operations
- history_buffer_push (100, 1K, 10K snapshots)
- history_buffer_get_by_tick
- history_buffer_recent (last N)
- history_buffer_truncate_after
- history_buffer_eviction (at capacity)

// Branch operations
- branch_create
- branch_checkout (by name, by ID)
- branch_registry_lookup
- branch_delete

// Timeline operations
- timeline_capture_snapshot
- timeline_rollback (various depths)
- timeline_goto_tick
- timeline_tick_range
```

#### 1.2 Diff Benchmarks

```rust
// WorldDiff computation at different scales
- diff_worlds_identical (100, 1K, 10K entities)
- diff_worlds_few_changes (1% changed)
- diff_worlds_many_changes (50% changed)
- diff_worlds_all_different

// Diff granularity comparison
- diff_entity_granularity
- diff_component_granularity
- diff_field_granularity

// Diff output
- diff_summary_generation
- format_diff (small, medium, large)
```

#### 1.3 Merge Benchmarks

```rust
- merge_replace_strategy
- merge_compare_strategy
- merge_result_construction
```

#### 1.4 Explain Benchmarks

```rust
// ExplainContext operations
- explain_context_create
- explain_context_with_world

// WhyQuery
- why_result_construction
- why_single_hop
- why_multi_hop (depth 3, 5, 10)

// Provenance
- provenance_tracker_record
- provenance_tracker_lookup
- provenance_history_traversal
```

#### 1.5 Trace Benchmarks

```rust
// TraceBuffer operations
- trace_buffer_push (100, 1K, 10K events)
- trace_buffer_get_by_id
- trace_buffer_get_range
- trace_buffer_events_for_tick
- trace_buffer_query (with filters)
- trace_buffer_eviction

// Tracer overhead (critical for production)
- tracer_disabled_overhead
- tracer_enabled_minimal_config
- tracer_enabled_full_config

// Formatting
- format_trace_human_readable
- format_trace_json
- format_trace_event (each event type)
```

#### 1.6 Debug Session Benchmarks

```rust
// Breakpoint registry
- breakpoint_registry_add
- breakpoint_registry_remove
- breakpoint_registry_check_rule (0, 10, 100 breakpoints)
- breakpoint_registry_check_component_read
- breakpoint_registry_check_component_write
- breakpoint_registry_check_entity

// Watch expressions
- watch_expression_evaluate
- watch_expression_batch_evaluate

// VmSnapshot
- vm_snapshot_capture
- vm_snapshot_inspect

// Debug state transitions
- debug_state_pause
- debug_state_resume
- debug_state_step
```

**Estimated LOC**: ~800

---

## Stage 2: longtable_stdlib Benchmarks ✅ COMPLETE

**Note**: The `longtable_stdlib` crate is a placeholder. All stdlib functions are implemented in `longtable_language/src/vm/native/`. Benchmarks are added to `language_benchmarks.rs`.

### File: `crates/longtable_language/benches/language_benchmarks.rs`

**Existing coverage** (from original file):
- Type predicates: nil?, int?
- Collection basics: count, first, rest, conj, get, assoc
- String ops: str, str/upper, str/split, str/join, str/replace-all, format
- Math: sqrt, min, max, sin, cos, pow, log
- HOFs: map, filter, reduce
- Sequences: take, drop, concat, reverse, sort, range
- Vector math: vec+, vec-dot, vec-normalize, vec-cross
- Predicates: every?, some
- Extended collections: flatten, distinct, partition, interleave, zip

**Added benchmarks**:

#### 2.1 Additional Collection Functions
```rust
- empty?, last, nth, cons, update
- dissoc, contains?, keys, vals, merge
- sort-by (with comparison fn)
- sort at scale (100, 1K, 10K)
```

#### 2.2 Set Operations
```rust
- union, intersection, difference
- subset?
```

#### 2.3 Additional Type Predicates
```rust
- bool?, string?, keyword?, symbol?
- vector?, map?, set?, fn?, number?
```

#### 2.4 Additional Math Functions
```rust
- abs, floor, ceil, round, tan
- clamp
```

#### 2.5 Scale Testing
```rust
- map/filter/reduce at 1K, 10K elements
- sort at 1K, 10K elements
```

**Added LOC**: ~420 (1320 - 901 = 419)

---

## Stage 3: longtable_runtime Additional Benchmarks

Expand beyond serialization to cover REPL and session operations.

### File: `crates/longtable_runtime/benches/runtime_benchmarks.rs`

#### 3.1 REPL Benchmarks

```rust
// Command parsing
- repl_parse_simple_command
- repl_parse_complex_expression
- repl_parse_multiline

// Evaluation
- repl_eval_simple_expression
- repl_eval_query
- repl_eval_rule_definition
- repl_eval_component_definition

// Special forms
- repl_eval_spawn
- repl_eval_set
- repl_eval_tick
- repl_eval_query_execution
```

#### 3.2 Session Benchmarks

```rust
// Session lifecycle
- session_create
- session_with_config

// World management
- session_get_world
- session_set_world
- session_world_clone

// State operations
- session_tick (0, 10, 100 rules)
- session_tick_with_inputs
```

#### 3.3 File Operations

```rust
// Load benchmarks
- load_small_file (10 definitions)
- load_medium_file (100 definitions)
- load_large_file (1000 definitions)

// Parse and compile
- parse_file_content
- compile_file_definitions
```

**Estimated LOC**: ~400

---

## Stage 4: Expand Existing Benchmarks

### 4.1 Foundation Additions

```rust
// Value memory/allocation
- value_clone_deep_nested (depth 5, 10, 20)
- value_memory_size_estimation

// LtMap stress tests
- ltmap_insert_sequential_keys
- ltmap_insert_random_keys
- ltmap_lookup_miss_rate (0%, 50%, 100%)

// Interner stress tests
- interner_high_collision (similar strings)
- interner_concurrent_simulation
```

### 4.2 Storage Additions

```rust
// Archetype transitions
- storage_add_component_archetype_change
- storage_remove_component_archetype_change
- storage_batch_archetype_migration

// Relationship traversal
- relationship_multi_hop (2, 3, 5 hops)
- relationship_cyclic_detection
- relationship_bidirectional_lookup
```

### 4.3 Language Additions

```rust
// VM edge cases
- vm_deep_recursion (100, 1000 frames)
- vm_large_closures
- vm_exception_handling

// Compiler optimization impact
- compiler_constant_folding
- compiler_dead_code_elimination
```

### 4.4 Engine Additions

```rust
// Pattern edge cases
- pattern_deeply_nested
- pattern_many_variables (10, 20, 50)
- pattern_complex_guards

// Query optimization
- query_index_utilization
- query_join_ordering_impact

// Rule priority
- rule_priority_sorting (10, 100, 1000 rules)
- rule_conflict_resolution
```

**Estimated LOC**: ~500

---

## Implementation Order

| Priority | Stage | Crate | Rationale |
|----------|-------|-------|-----------|
| 1 | Stage 1 | `longtable_debug` | Phase 6 code completely unbenchmarked |
| 2 | Stage 2 | `longtable_stdlib` | Core functionality, partial coverage exists |
| 3 | Stage 3 | `longtable_runtime` | User-facing operations need measurement |
| 4 | Stage 4 | All | Polish and edge case coverage |

---

## Benchmark Conventions

Follow existing patterns from codebase:

```rust
// Group organization
fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("category_name");

    // Throughput for data-processing benchmarks
    group.throughput(Throughput::Elements(n as u64));

    // Multiple sizes
    for size in [100, 1_000, 10_000] {
        group.bench_with_input(
            BenchmarkId::new("operation", size),
            &size,
            |b, &size| { ... }
        );
    }

    group.finish();
}

// Use black_box to prevent optimization
b.iter(|| black_box(operation(black_box(&input))));

// Batch iteration for setup-heavy benchmarks
b.iter_batched(
    || setup_data(),
    |data| operation(data),
    BatchSize::SmallInput,
);
```

---

## Cargo.toml Updates Required

### longtable_debug/Cargo.toml
```toml
[dev-dependencies]
criterion = "0.5"

[[bench]]
name = "debug_benchmarks"
harness = false
```

### longtable_stdlib/Cargo.toml
```toml
[dev-dependencies]
criterion = "0.5"

[[bench]]
name = "stdlib_benchmarks"
harness = false
```

### longtable_runtime/Cargo.toml
```toml
[[bench]]
name = "runtime_benchmarks"
harness = false
```

---

## Success Criteria

- [ ] All 6 crates have benchmark files
- [ ] Zero-overhead verification for disabled debug features
- [ ] Throughput metrics for all data-processing operations
- [ ] Multi-scale testing (100, 1K, 10K) where applicable
- [ ] All benchmarks pass `cargo bench` without warnings
- [ ] Baseline measurements documented for regression tracking

---

## Estimated Total New Benchmarks

| Stage | Benchmarks | LOC |
|-------|------------|-----|
| Stage 1 (debug) | ~50 | ~800 |
| Stage 2 (stdlib) | ~40 | ~600 |
| Stage 3 (runtime) | ~20 | ~400 |
| Stage 4 (expansions) | ~25 | ~500 |
| **Total** | **~135** | **~2300** |
