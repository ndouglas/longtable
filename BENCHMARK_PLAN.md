# Benchmark Implementation Plan

## Status: ✅ COMPLETE

All benchmark stages have been implemented.

## Current Status

| Crate                  | Benchmark Files                                                      | Benchmarks | Coverage                                                                                                      |
| ---------------------- | -------------------------------------------------------------------- | ---------- | ------------------------------------------------------------------------------------------------------------- |
| `longtable_foundation` | `foundation_benchmarks.rs`                                           | 96         | **Complete** - Value, LtVec, LtSet, LtMap, Interner + Stage 4 expansions                                      |
| `longtable_storage`    | `storage_benchmarks.rs`, `scale_benchmarks.rs`                       | ~90        | **Complete** - EntityStore, ComponentStore, RelationshipStore, World + Scale tests                            |
| `longtable_language`   | `language_benchmarks.rs`, `scale_benchmarks.rs`                      | ~220       | **Complete** - Lexer, Parser, Compiler, VM, Stdlib + Scale tests                                              |
| `longtable_engine`     | `engine_benchmarks.rs`, `scale_benchmarks.rs`                        | ~180       | **Complete** - Patterns, Queries, Rules, Derived, Constraints, Tick + Scale tests                             |
| `longtable_runtime`    | `serialization_benchmarks.rs`, `runtime_benchmarks.rs`               | 45         | **Complete** - Serialization, Session, REPL eval, Pipeline                                                    |
| `longtable_stdlib`     | N/A (placeholder crate)                                              | -          | Stdlib in language crate                                                                                      |
| `longtable_debug`      | `debug_benchmarks.rs`, `scale_benchmarks.rs`                         | ~150       | **Complete** - Timeline, Diff, Merge, Trace, Debug, Explain + Scale tests                                     |

**Total benchmarks: ~780+**

---

## Implementation Summary

| Priority | Stage   | Crate                | Rationale                                | Status      |
| -------- | ------- | -------------------- | ---------------------------------------- | ----------- |
| 1        | Stage 1 | `longtable_debug`    | Phase 6 code completely unbenchmarked    | ✅ Complete |
| 2        | Stage 2 | `longtable_language` | Stdlib coverage (was stdlib placeholder) | ✅ Complete |
| 3        | Stage 3 | `longtable_runtime`  | User-facing operations need measurement  | ✅ Complete |
| 4        | Stage 4 | All                  | Polish and edge case coverage            | ✅ Complete |
| 5        | Stage 5 | All                  | Memory profiling & large-scale tests     | ✅ Complete |

---

## Stage 5: Scale Benchmarks Summary

Created `scale_benchmarks.rs` files for four crates:

### longtable_language
- Large-scale collection operations (map, filter, reduce, sort, distinct at 1K-50K elements)
- Range generation benchmarks
- Allocation pressure tests (vector/map/set insert sequences)
- Persistent data structure overhead measurement
- Pipeline chain performance
- Recursion stress tests
- String operations at scale

### longtable_storage
- Entity scale (world creation, cloning, spawning at 500-2,500 entities)
- Relationship scale (chain/graph creation and traversal)
- Sparse world queries (10% active filtering)
- Batch spawn/destroy operations

### longtable_engine
- Pattern matching at scale (500-2,500 entities)
- Rule engine with multiple rules × entities combinations
- Query execution stress tests
- Tick orchestration at scale
- Constraint checking at scale

### longtable_debug
- Timeline operations with large history (100-1,000 snapshots)
- History buffer operations
- Diff/merge on larger worlds (100-500 entities)
- Trace buffer at scale (1K-10K events)
- Debug infrastructure stress tests

**Note**: Scale sizes were deliberately kept moderate (up to ~2,500 entities, ~10K trace events) to prevent OOM during benchmark initialization while still providing meaningful scaling data.

---

## Running Benchmarks

```bash
# All benchmarks
cargo bench

# Specific crate
cargo bench --package longtable_language

# Specific benchmark file
cargo bench --package longtable_language --bench scale_benchmarks

# Filter specific tests
cargo bench --package longtable_language --bench scale_benchmarks -- scale_collections
```

---

## Benchmark Conventions

```rust
// Group organization
fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("category_name");

    // Throughput for data-processing benchmarks
    group.throughput(Throughput::Elements(n as u64));

    // Reduce sample size for expensive operations
    group.sample_size(20);

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
