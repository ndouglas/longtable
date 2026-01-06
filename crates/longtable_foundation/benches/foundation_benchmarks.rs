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

criterion_group!(
    benches,
    bench_value_clone,
    bench_value_comparison,
    bench_value_hashing,
    bench_ltvec,
    bench_ltset,
    bench_ltmap,
    bench_interner,
);

criterion_main!(benches);
