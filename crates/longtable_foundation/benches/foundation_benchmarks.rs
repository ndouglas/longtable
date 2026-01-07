//! Benchmarks for the Longtable foundation layer.
//!
//! Run with: `cargo bench --package longtable_foundation`

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use longtable_foundation::{Interner, LtMap, LtSet, LtVec, Value};

// =============================================================================
// Value System Benchmarks
// =============================================================================

fn bench_value_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("value/clone");

    // Scalar values
    group.bench_function("nil", |b| {
        let v = Value::Nil;
        b.iter(|| black_box(v.clone()))
    });

    group.bench_function("int", |b| {
        let v = Value::Int(42);
        b.iter(|| black_box(v.clone()))
    });

    group.bench_function("float", |b| {
        let v = Value::Float(2.5);
        b.iter(|| black_box(v.clone()))
    });

    group.bench_function("string_short", |b| {
        let v = Value::from("hello");
        b.iter(|| black_box(v.clone()))
    });

    group.bench_function("string_long", |b| {
        let v = Value::from("a".repeat(1000));
        b.iter(|| black_box(v.clone()))
    });

    // Collection values
    group.bench_function("vec_10", |b| {
        let v = Value::Vec((0..10).map(Value::Int).collect());
        b.iter(|| black_box(v.clone()))
    });

    group.bench_function("vec_1000", |b| {
        let v = Value::Vec((0..1000).map(Value::Int).collect());
        b.iter(|| black_box(v.clone()))
    });

    group.bench_function("map_10", |b| {
        let map: LtMap<Value, Value> = (0..10)
            .map(|i| (Value::Int(i), Value::Int(i * 2)))
            .collect();
        let v = Value::Map(map);
        b.iter(|| black_box(v.clone()))
    });

    group.bench_function("map_1000", |b| {
        let map: LtMap<Value, Value> = (0..1000)
            .map(|i| (Value::Int(i), Value::Int(i * 2)))
            .collect();
        let v = Value::Map(map);
        b.iter(|| black_box(v.clone()))
    });

    group.finish();
}

fn bench_value_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("value/compare");

    // Scalar comparison
    group.bench_function("int_eq", |b| {
        let a = Value::Int(42);
        let b_val = Value::Int(42);
        b.iter(|| black_box(&a) == black_box(&b_val))
    });

    group.bench_function("string_eq_short", |b| {
        let a = Value::from("hello");
        let b_val = Value::from("hello");
        b.iter(|| black_box(&a) == black_box(&b_val))
    });

    group.bench_function("string_eq_long", |b| {
        let s = "a".repeat(1000);
        let a = Value::from(s.clone());
        let b_val = Value::from(s);
        b.iter(|| black_box(&a) == black_box(&b_val))
    });

    // Collection comparison
    group.bench_function("vec_eq_10", |b| {
        let a = Value::Vec((0..10).map(Value::Int).collect());
        let b_val = Value::Vec((0..10).map(Value::Int).collect());
        b.iter(|| black_box(&a) == black_box(&b_val))
    });

    group.bench_function("vec_eq_1000", |b| {
        let a = Value::Vec((0..1000).map(Value::Int).collect());
        let b_val = Value::Vec((0..1000).map(Value::Int).collect());
        b.iter(|| black_box(&a) == black_box(&b_val))
    });

    group.bench_function("vec_ne_first", |b| {
        let a = Value::Vec((0..1000).map(Value::Int).collect());
        let mut items: LtVec<Value> = (0..1000).map(Value::Int).collect();
        items = items.update(0, Value::Int(-1)).unwrap();
        let b_val = Value::Vec(items);
        b.iter(|| black_box(&a) == black_box(&b_val))
    });

    group.finish();
}

fn bench_value_hashing(c: &mut Criterion) {
    let mut group = c.benchmark_group("value/hash");

    fn hash_value(v: &Value) -> u64 {
        let mut hasher = DefaultHasher::new();
        v.hash(&mut hasher);
        hasher.finish()
    }

    group.bench_function("int", |b| {
        let v = Value::Int(42);
        b.iter(|| hash_value(black_box(&v)))
    });

    group.bench_function("string_short", |b| {
        let v = Value::from("hello");
        b.iter(|| hash_value(black_box(&v)))
    });

    group.bench_function("string_long", |b| {
        let v = Value::from("a".repeat(1000));
        b.iter(|| hash_value(black_box(&v)))
    });

    group.bench_function("vec_10", |b| {
        let v = Value::Vec((0..10).map(Value::Int).collect());
        b.iter(|| hash_value(black_box(&v)))
    });

    group.bench_function("vec_1000", |b| {
        let v = Value::Vec((0..1000).map(Value::Int).collect());
        b.iter(|| hash_value(black_box(&v)))
    });

    group.finish();
}

// =============================================================================
// Persistent Collections Benchmarks
// =============================================================================

fn bench_ltvec(c: &mut Criterion) {
    let mut group = c.benchmark_group("collections/vec");

    // Insert
    for size in [100, 1_000, 10_000, 100_000] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("push_back", size), &size, |b, &size| {
            b.iter(|| {
                let mut v = LtVec::new();
                for i in 0..size {
                    v = v.push_back(i);
                }
                black_box(v)
            })
        });
    }

    // Lookup
    for size in [100, 1_000, 10_000, 100_000] {
        let vec: LtVec<i32> = (0..size).collect();
        group.bench_with_input(BenchmarkId::new("get_middle", size), &vec, |b, v| {
            let mid = v.len() / 2;
            b.iter(|| black_box(v.get(mid)))
        });
    }

    // Iteration
    for size in [100, 1_000, 10_000, 100_000] {
        let vec: LtVec<i32> = (0..size).collect();
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("iterate", size), &vec, |b, v| {
            b.iter(|| {
                let mut sum = 0i64;
                for &x in v.iter() {
                    sum += x as i64;
                }
                black_box(sum)
            })
        });
    }

    // Clone (structural sharing)
    for size in [100, 1_000, 10_000, 100_000] {
        let vec: LtVec<i32> = (0..size).collect();
        group.bench_with_input(BenchmarkId::new("clone", size), &vec, |b, v| {
            b.iter(|| black_box(v.clone()))
        });
    }

    group.finish();
}

fn bench_ltset(c: &mut Criterion) {
    let mut group = c.benchmark_group("collections/set");

    // Insert
    for size in [100, 1_000, 10_000, 100_000] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("insert", size), &size, |b, &size| {
            b.iter(|| {
                let mut s = LtSet::new();
                for i in 0..size {
                    s = s.insert(i);
                }
                black_box(s)
            })
        });
    }

    // Contains
    for size in [100, 1_000, 10_000, 100_000] {
        let set: LtSet<i32> = (0..size).collect();
        let mid = size / 2;
        group.bench_with_input(BenchmarkId::new("contains", size), &set, |b, s| {
            b.iter(|| black_box(s.contains(&mid)))
        });
    }

    // Iteration
    for size in [100, 1_000, 10_000, 100_000] {
        let set: LtSet<i32> = (0..size).collect();
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("iterate", size), &set, |b, s| {
            b.iter(|| {
                let mut sum = 0i64;
                for &x in s.iter() {
                    sum += x as i64;
                }
                black_box(sum)
            })
        });
    }

    group.finish();
}

fn bench_ltmap(c: &mut Criterion) {
    let mut group = c.benchmark_group("collections/map");

    // Insert
    for size in [100, 1_000, 10_000, 100_000] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("insert", size), &size, |b, &size| {
            b.iter(|| {
                let mut m = LtMap::new();
                for i in 0..size {
                    m = m.insert(i, i * 2);
                }
                black_box(m)
            })
        });
    }

    // Lookup
    for size in [100, 1_000, 10_000, 100_000] {
        let map: LtMap<i32, i32> = (0..size).map(|i| (i, i * 2)).collect();
        let mid = size / 2;
        group.bench_with_input(BenchmarkId::new("get", size), &map, |b, m| {
            b.iter(|| black_box(m.get(&mid)))
        });
    }

    // Iteration
    for size in [100, 1_000, 10_000, 100_000] {
        let map: LtMap<i32, i32> = (0..size).map(|i| (i, i * 2)).collect();
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("iterate", size), &map, |b, m| {
            b.iter(|| {
                let mut sum = 0i64;
                for (&k, &v) in m.iter() {
                    sum += (k + v) as i64;
                }
                black_box(sum)
            })
        });
    }

    group.finish();
}

// =============================================================================
// Interner Benchmarks
// =============================================================================

fn bench_interner(c: &mut Criterion) {
    let mut group = c.benchmark_group("interner");

    // Intern new symbol
    group.bench_function("intern_new_symbol", |b| {
        b.iter_batched(
            Interner::new,
            |mut interner| {
                black_box(interner.intern_symbol("test_symbol"));
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Intern duplicate symbol
    group.bench_function("intern_dup_symbol", |b| {
        let mut interner = Interner::new();
        interner.intern_symbol("test_symbol");
        b.iter(|| black_box(interner.intern_symbol("test_symbol")))
    });

    // Lookup symbol
    group.bench_function("get_symbol", |b| {
        let mut interner = Interner::new();
        let id = interner.intern_symbol("test_symbol");
        b.iter(|| black_box(interner.get_symbol(id)))
    });

    // Intern many symbols
    group.bench_function("intern_1000_unique", |b| {
        let symbols: Vec<String> = (0..1000).map(|i| format!("symbol_{i}")).collect();
        b.iter_batched(
            Interner::new,
            |mut interner| {
                for s in &symbols {
                    black_box(interner.intern_symbol(s));
                }
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

// =============================================================================
// Stage 4: Edge Cases and Stress Tests
// =============================================================================

fn bench_value_deep_nested(c: &mut Criterion) {
    let mut group = c.benchmark_group("value/deep_nested");

    // Create deeply nested vector: [[[...]]]
    fn make_nested_vec(depth: usize) -> Value {
        let mut v = Value::Int(42);
        for _ in 0..depth {
            v = Value::Vec(std::iter::once(v).collect());
        }
        v
    }

    // Create deeply nested map: {:a {:a {:a ...}}}
    fn make_nested_map(depth: usize) -> Value {
        let mut v = Value::Int(42);
        for _ in 0..depth {
            let mut map = LtMap::new();
            map = map.insert(Value::from("a"), v);
            v = Value::Map(map);
        }
        v
    }

    for depth in [5, 10, 20, 50] {
        let nested_vec = make_nested_vec(depth);
        group.bench_with_input(BenchmarkId::new("clone_vec", depth), &nested_vec, |b, v| {
            b.iter(|| black_box(v.clone()))
        });

        let nested_map = make_nested_map(depth);
        group.bench_with_input(BenchmarkId::new("clone_map", depth), &nested_map, |b, v| {
            b.iter(|| black_box(v.clone()))
        });
    }

    // Compare deeply nested values
    for depth in [5, 10, 20] {
        let a = make_nested_vec(depth);
        let b = make_nested_vec(depth);
        group.bench_with_input(
            BenchmarkId::new("compare_vec", depth),
            &(a, b),
            |bench, (a, b)| bench.iter(|| black_box(a == b)),
        );
    }

    group.finish();
}

fn bench_ltmap_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("collections/map_patterns");

    // Sequential key insertion (best case for some data structures)
    for size in [1_000, 10_000] {
        group.bench_with_input(
            BenchmarkId::new("insert_sequential", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let mut m = LtMap::new();
                    for i in 0..size {
                        m = m.insert(i, i);
                    }
                    black_box(m)
                })
            },
        );
    }

    // Random key insertion (more realistic)
    for size in [1_000, 10_000] {
        // Pre-generate "random" keys using a simple hash
        let keys: Vec<i32> = (0..size)
            .map(|i| {
                let mut h = DefaultHasher::new();
                i.hash(&mut h);
                (h.finish() % 1_000_000) as i32
            })
            .collect();

        group.bench_with_input(BenchmarkId::new("insert_random", size), &keys, |b, keys| {
            b.iter(|| {
                let mut m = LtMap::new();
                for (i, &k) in keys.iter().enumerate() {
                    m = m.insert(k, i as i32);
                }
                black_box(m)
            })
        });
    }

    // Lookup miss rates
    for size in [1_000, 10_000] {
        let map: LtMap<i32, i32> = (0..size).map(|i| (i, i)).collect();

        // 0% miss (all hits)
        group.bench_with_input(BenchmarkId::new("lookup_0pct_miss", size), &map, |b, m| {
            b.iter(|| {
                let mut sum = 0i64;
                for i in 0..100 {
                    if let Some(&v) = m.get(&(i % size)) {
                        sum += v as i64;
                    }
                }
                black_box(sum)
            })
        });

        // 50% miss
        group.bench_with_input(BenchmarkId::new("lookup_50pct_miss", size), &map, |b, m| {
            b.iter(|| {
                let mut sum = 0i64;
                for i in 0..100 {
                    // Even indices hit, odd indices miss
                    let key = if i % 2 == 0 { i / 2 } else { size + i };
                    if let Some(&v) = m.get(&key) {
                        sum += v as i64;
                    }
                }
                black_box(sum)
            })
        });

        // 100% miss
        group.bench_with_input(
            BenchmarkId::new("lookup_100pct_miss", size),
            &map,
            |b, m| {
                b.iter(|| {
                    let mut found = 0;
                    for i in 0..100 {
                        if m.get(&(size + i)).is_some() {
                            found += 1;
                        }
                    }
                    black_box(found)
                })
            },
        );
    }

    group.finish();
}

fn bench_interner_stress(c: &mut Criterion) {
    let mut group = c.benchmark_group("interner/stress");

    // High collision scenario: similar strings
    group.bench_function("similar_strings_1000", |b| {
        // Strings that differ only in suffix - tests hash distribution
        let strings: Vec<String> = (0..1000)
            .map(|i| format!("very_long_prefix_that_is_the_same_{i}"))
            .collect();
        b.iter_batched(
            Interner::new,
            |mut interner| {
                for s in &strings {
                    black_box(interner.intern_symbol(s));
                }
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Short strings (common case)
    group.bench_function("short_strings_1000", |b| {
        let strings: Vec<String> = (0..1000).map(|i| format!("s{i}")).collect();
        b.iter_batched(
            Interner::new,
            |mut interner| {
                for s in &strings {
                    black_box(interner.intern_symbol(s));
                }
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Keywords vs symbols (different intern paths)
    group.bench_function("keywords_1000", |b| {
        let keywords: Vec<String> = (0..1000).map(|i| format!("kw{i}")).collect();
        b.iter_batched(
            Interner::new,
            |mut interner| {
                for k in &keywords {
                    black_box(interner.intern_keyword(k));
                }
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Mixed intern and lookup (realistic usage)
    group.bench_function("mixed_intern_lookup", |b| {
        b.iter_batched(
            || {
                let mut interner = Interner::new();
                let ids: Vec<_> = (0..100)
                    .map(|i| interner.intern_symbol(&format!("sym{i}")))
                    .collect();
                (interner, ids)
            },
            |(mut interner, ids)| {
                // Intern 100 new, lookup 100 existing
                for i in 0..100 {
                    black_box(interner.intern_symbol(&format!("new{i}")));
                    black_box(interner.get_symbol(ids[i % ids.len()]));
                }
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Large interner (10K symbols already interned)
    group.bench_function("lookup_in_10k", |b| {
        let mut interner = Interner::new();
        let ids: Vec<_> = (0..10_000)
            .map(|i| interner.intern_symbol(&format!("symbol_{i}")))
            .collect();
        b.iter(|| {
            // Lookup random symbols
            let mut sum = 0usize;
            for i in 0..100 {
                if let Some(s) = interner.get_symbol(ids[(i * 97) % ids.len()]) {
                    sum += s.len();
                }
            }
            black_box(sum)
        })
    });

    group.finish();
}

fn bench_value_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("value/construct");

    // Measure construction costs
    group.bench_function("vec_from_iter_100", |b| {
        b.iter(|| {
            let v: LtVec<Value> = (0..100).map(Value::Int).collect();
            black_box(Value::Vec(v))
        })
    });

    group.bench_function("map_from_iter_100", |b| {
        b.iter(|| {
            let m: LtMap<Value, Value> = (0..100)
                .map(|i| (Value::Int(i), Value::Int(i * 2)))
                .collect();
            black_box(Value::Map(m))
        })
    });

    group.bench_function("set_from_iter_100", |b| {
        b.iter(|| {
            let s: LtSet<Value> = (0..100).map(Value::Int).collect();
            black_box(Value::Set(s))
        })
    });

    // String construction
    group.bench_function("string_from_str", |b| {
        b.iter(|| black_box(Value::from("hello world")))
    });

    group.bench_function("string_from_string", |b| {
        let s = "hello world".to_string();
        b.iter(|| black_box(Value::from(s.clone())))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_value_clone,
    bench_value_comparison,
    bench_value_hashing,
    bench_ltvec,
    bench_ltset,
    bench_ltmap,
    bench_interner,
    // Stage 4 additions
    bench_value_deep_nested,
    bench_ltmap_patterns,
    bench_interner_stress,
    bench_value_construction,
);

criterion_main!(benches);
