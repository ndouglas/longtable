//! Large-scale benchmarks for Longtable language layer.
//!
//! Run with: `cargo bench --package longtable_language --bench scale_benchmarks`
//!
//! WARNING: These benchmarks can take significant time (10-30 minutes for full suite).
//! Use `cargo bench --package longtable_language --bench scale_benchmarks -- <filter>` to run specific tests.
//!
//! Benchmark groups:
//! - scale_collections: Large collection operations (10K, 100K elements)
//! - scale_range: Range generation at scale
//! - allocation_pressure: Memory allocation patterns
//! - persistent_overhead: Persistent data structure costs

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};

use longtable_language::eval;

// =============================================================================
// Large-Scale Collection Operations
// =============================================================================

fn bench_scale_collections(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_collections");
    group.sample_size(20); // Fewer samples for expensive operations

    // Map at scale
    for size in [1_000, 10_000, 50_000] {
        let source = format!("(count (map inc (range {size})))");

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("map", size), &source, |b, s| {
            b.iter(|| black_box(eval(s).unwrap()))
        });
    }

    // Filter at scale
    for size in [1_000, 10_000, 50_000] {
        let source = format!("(count (filter even? (range {size})))");

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("filter", size), &source, |b, s| {
            b.iter(|| black_box(eval(s).unwrap()))
        });
    }

    // Reduce at scale
    for size in [1_000, 10_000, 50_000] {
        let source = format!("(reduce + 0 (range {size}))");

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("reduce", size), &source, |b, s| {
            b.iter(|| black_box(eval(s).unwrap()))
        });
    }

    // Sort at scale (expensive!)
    for size in [100, 1_000, 5_000] {
        // Generate a reverse-sorted list to maximize sort work
        let source = format!("(count (sort (reverse (range {size}))))");

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("sort_reversed", size), &source, |b, s| {
            b.iter(|| black_box(eval(s).unwrap()))
        });
    }

    // Distinct at scale
    for size in [1_000, 10_000] {
        // Create list with 50% duplicates
        let half = size / 2;
        let source = format!("(count (distinct (map (fn [x] (rem x {half})) (range {size}))))");

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::new("distinct_50pct_dups", size),
            &source,
            |b, s| b.iter(|| black_box(eval(s).unwrap())),
        );
    }

    // Take from large collection (should be fast with lazy evaluation)
    for size in [10_000, 100_000] {
        let source = format!("(count (take 10 (range {size})))");

        group.bench_with_input(BenchmarkId::new("take_10_from", size), &source, |b, s| {
            b.iter(|| black_box(eval(s).unwrap()))
        });
    }

    // Drop from large collection
    for size in [10_000, 50_000] {
        let source = format!("(count (drop 100 (range {size})))");

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("drop_100_from", size), &source, |b, s| {
            b.iter(|| black_box(eval(s).unwrap()))
        });
    }

    group.finish();
}

// =============================================================================
// Range Generation Benchmarks
// =============================================================================

fn bench_scale_range(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_range");
    group.sample_size(20);

    // Range generation at scale
    for size in [1_000, 10_000, 100_000] {
        let source = format!("(count (range {size}))");

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("range_count", size), &source, |b, s| {
            b.iter(|| black_box(eval(s).unwrap()))
        });
    }

    // Range with step
    for size in [10_000, 100_000] {
        let source = format!("(count (range 0 {size} 2))");

        group.throughput(Throughput::Elements((size / 2) as u64));
        group.bench_with_input(BenchmarkId::new("range_step_2", size), &source, |b, s| {
            b.iter(|| black_box(eval(s).unwrap()))
        });
    }

    // Concat large ranges
    for size in [1_000, 10_000] {
        let source = format!("(count (concat (range {size}) (range {size}) (range {size})))",);

        group.throughput(Throughput::Elements((size * 3) as u64));
        group.bench_with_input(
            BenchmarkId::new("concat_3_ranges", size),
            &source,
            |b, s| b.iter(|| black_box(eval(s).unwrap())),
        );
    }

    group.finish();
}

// =============================================================================
// Allocation Pressure Benchmarks
// =============================================================================

fn bench_allocation_pressure(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocation_pressure");
    group.sample_size(30);

    // Vector append sequence - measures structural sharing
    for count in [100, 1_000, 5_000] {
        // Build up a vector one element at a time
        let source = format!("(count (reduce (fn [v x] (conj v x)) [] (range {count})))");

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("vector_append_sequence", count),
            &source,
            |b, s| b.iter(|| black_box(eval(s).unwrap())),
        );
    }

    // Map insert sequence - measures HAMT overhead
    for count in [100, 1_000, 5_000] {
        let source = format!("(count (reduce (fn [m x] (assoc m x x)) {{}} (range {count})))");

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("map_insert_sequence", count),
            &source,
            |b, s| b.iter(|| black_box(eval(s).unwrap())),
        );
    }

    // Set insert sequence
    for count in [100, 1_000, 5_000] {
        let source = format!("(count (reduce (fn [s x] (conj s x)) #{{}} (range {count})))");

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("set_insert_sequence", count),
            &source,
            |b, s| b.iter(|| black_box(eval(s).unwrap())),
        );
    }

    // Churn: create and discard many intermediate values
    for count in [100, 1_000] {
        let source = format!("(reduce + 0 (map (fn [x] (count (range x))) (range 1 {count})))");

        group.bench_with_input(
            BenchmarkId::new("churn_intermediate_values", count),
            &source,
            |b, s| b.iter(|| black_box(eval(s).unwrap())),
        );
    }

    group.finish();
}

// =============================================================================
// Persistent Data Structure Overhead
// =============================================================================

fn bench_persistent_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("persistent_overhead");
    group.sample_size(30);

    // Compare update-in-place patterns
    // Update single element repeatedly
    for count in [100, 500, 1_000] {
        let source = format!("(first (reduce (fn [v _] (update v 0 inc)) [0] (range {count})))");

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("repeated_update_same_index", count),
            &source,
            |b, s| b.iter(|| black_box(eval(s).unwrap())),
        );
    }

    // Update different elements (tree path variation)
    for size in [100, 1_000] {
        let source = format!(
            "(count (let [v (into [] (range {size}))] \
             (reduce (fn [v i] (update v i inc)) v (range {size}))))"
        );

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::new("update_all_indices", size),
            &source,
            |b, s| b.iter(|| black_box(eval(s).unwrap())),
        );
    }

    // Nested update (deep path modification)
    for depth in [3, 5, 7] {
        // Build nested structure and update deepest value
        let mut build = "0".to_string();
        for _ in 0..depth {
            build = format!("[{build}]");
        }

        let mut path = "0".to_string();
        for _ in 1..depth {
            path = format!("{path} 0");
        }

        let source = format!("(update-in {build} [{path}] inc)");

        group.bench_with_input(
            BenchmarkId::new("nested_update_depth", depth),
            &source,
            |b, s| b.iter(|| black_box(eval(s).unwrap())),
        );
    }

    // Map merge at scale
    for size in [100, 500, 1_000] {
        let source = format!(
            "(count (let [m1 (into {{}} (map (fn [x] [x x]) (range {size}))) \
                   m2 (into {{}} (map (fn [x] [x (* x 2)]) (range {size})))] \
             (merge m1 m2)))"
        );

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("map_merge", size), &source, |b, s| {
            b.iter(|| black_box(eval(s).unwrap()))
        });
    }

    group.finish();
}

// =============================================================================
// Chained Operations (Pipeline Cost)
// =============================================================================

fn bench_pipeline_chains(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_chains");
    group.sample_size(30);

    // Single operation baseline
    group.bench_function("chain_1_op", |b| {
        b.iter(|| black_box(eval("(count (map inc (range 1000)))").unwrap()))
    });

    // 2 chained operations
    group.bench_function("chain_2_ops", |b| {
        b.iter(|| black_box(eval("(count (filter even? (map inc (range 1000))))").unwrap()))
    });

    // 3 chained operations
    group.bench_function("chain_3_ops", |b| {
        b.iter(|| {
            black_box(eval("(count (take 100 (filter even? (map inc (range 1000)))))").unwrap())
        })
    });

    // 5 chained operations
    group.bench_function("chain_5_ops", |b| {
        b.iter(|| {
            black_box(eval(
            "(reduce + 0 (take 100 (filter even? (map inc (map (fn [x] (* x 2)) (range 1000))))))"
        ).unwrap())
        })
    });

    // Compare: same work, different chain lengths
    // Short chain (2 ops doing same work as 4 separate)
    group.bench_function("combined_2_ops", |b| {
        b.iter(|| {
            black_box(
                eval("(count (map (fn [x] (+ (* x 2) 1)) (filter even? (range 1000))))").unwrap(),
            )
        })
    });

    // Long chain (4 separate ops)
    group.bench_function("separate_3_ops", |b| {
        b.iter(|| {
            black_box(
                eval("(count (map inc (map (fn [x] (* x 2)) (filter even? (range 1000)))))")
                    .unwrap(),
            )
        })
    });

    group.finish();
}

// =============================================================================
// Recursion Depth Stress Test
// =============================================================================

fn bench_recursion_stress(c: &mut Criterion) {
    let mut group = c.benchmark_group("recursion_stress");
    group.sample_size(20);

    // Linear recursion at various depths
    for depth in [100, 500, 1_000, 2_000] {
        let source = format!(
            "(let [count-down (fn [n] (if (<= n 0) 0 (count-down (- n 1))))] \
             (count-down {depth}))"
        );

        group.bench_with_input(
            BenchmarkId::new("linear_recursion", depth),
            &source,
            |b, s| b.iter(|| black_box(eval(s).unwrap())),
        );
    }

    // Mutual recursion (even?/odd? style)
    for n in [50, 100, 200] {
        let source = format!(
            "(let [my-even? (fn [n] (if (= n 0) true (my-odd? (- n 1)))) \
                   my-odd? (fn [n] (if (= n 0) false (my-even? (- n 1))))] \
             (my-even? {n}))"
        );

        group.bench_with_input(BenchmarkId::new("mutual_recursion", n), &source, |b, s| {
            b.iter(|| black_box(eval(s).unwrap()))
        });
    }

    group.finish();
}

// =============================================================================
// String Operations at Scale
// =============================================================================

fn bench_scale_strings(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_strings");
    group.sample_size(30);

    // String concatenation
    for count in [100, 500, 1_000] {
        let source =
            format!("(count (reduce str \"\" (map (fn [x] (str x \"-\")) (range {count}))))");

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("str_concat", count), &source, |b, s| {
            b.iter(|| black_box(eval(s).unwrap()))
        });
    }

    // String join (should be more efficient than reduce str)
    for count in [100, 1_000, 5_000] {
        let source = format!("(count (str/join \"-\" (map str (range {count}))))");

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("str_join", count), &source, |b, s| {
            b.iter(|| black_box(eval(s).unwrap()))
        });
    }

    group.finish();
}

// =============================================================================
// Criterion Groups
// =============================================================================

criterion_group!(
    benches,
    bench_scale_collections,
    bench_scale_range,
    bench_allocation_pressure,
    bench_persistent_overhead,
    bench_pipeline_chains,
    bench_recursion_stress,
    bench_scale_strings,
);

criterion_main!(benches);
