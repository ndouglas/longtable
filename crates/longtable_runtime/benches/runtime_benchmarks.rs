//! Benchmarks for Longtable runtime (Session, REPL evaluation).
//!
//! Run with: `cargo bench --package longtable_runtime --bench runtime_benchmarks`

use criterion::{Criterion, black_box, criterion_group, criterion_main};

use longtable_foundation::{LtMap, Type, Value};
use longtable_runtime::Session;
use longtable_storage::World;
use longtable_storage::schema::{ComponentSchema, FieldSchema};

// =============================================================================
// Helper Functions
// =============================================================================

/// Creates a minimal world with health component registered.
fn create_minimal_world() -> World {
    let mut world = World::new(0);

    let health = world.interner_mut().intern_keyword("health");
    let current = world.interner_mut().intern_keyword("current");
    let max = world.interner_mut().intern_keyword("max");

    let schema = ComponentSchema::new(health)
        .with_field(FieldSchema::required(current, Type::Int))
        .with_field(FieldSchema::required(max, Type::Int));
    world = world.register_component(schema).unwrap();

    world
}

/// Creates a world with the given number of entities.
fn create_world_with_entities(count: usize) -> World {
    let mut world = create_minimal_world();

    let health = world.interner_mut().intern_keyword("health");
    let current = world.interner_mut().intern_keyword("current");
    let max = world.interner_mut().intern_keyword("max");

    for i in 0..count {
        let mut components = LtMap::new();
        let mut health_data = LtMap::new();
        health_data = health_data.insert(Value::Keyword(current), Value::Int((i % 100) as i64));
        health_data = health_data.insert(Value::Keyword(max), Value::Int(100));
        components = components.insert(Value::Keyword(health), Value::Map(health_data));

        let (w, _) = world.spawn(&components).unwrap();
        world = w;
    }

    world
}

// =============================================================================
// Session Benchmarks
// =============================================================================

fn bench_session(c: &mut Criterion) {
    let mut group = c.benchmark_group("session");

    // Session creation
    group.bench_function("create", |b| b.iter(|| black_box(Session::new())));

    // Session with world
    let world = create_minimal_world();
    group.bench_function("with_world", |b| {
        b.iter(|| black_box(Session::with_world(world.clone())))
    });

    // Variable operations
    group.bench_function("set_variable", |b| {
        let mut session = Session::new();
        let value = Value::Int(42);
        b.iter(|| {
            session.set_variable("test".to_string(), value.clone());
            black_box(())
        })
    });

    group.bench_function("get_variable", |b| {
        let mut session = Session::new();
        session.set_variable("test".to_string(), Value::Int(42));
        b.iter(|| black_box(session.get_variable("test")))
    });

    // Entity registration
    group.bench_function("register_entity", |b| {
        let mut session = Session::new();
        let entity = longtable_foundation::EntityId::new(1, 1);
        b.iter(|| {
            session.register_entity("player".to_string(), entity);
            black_box(())
        })
    });

    group.bench_function("get_entity", |b| {
        let mut session = Session::new();
        let entity = longtable_foundation::EntityId::new(1, 1);
        session.register_entity("player".to_string(), entity);
        b.iter(|| black_box(session.get_entity("player")))
    });

    // World operations
    group.bench_function("world_access", |b| {
        let session = Session::with_world(create_minimal_world());
        b.iter(|| black_box(session.world()))
    });

    group.bench_function("set_world", |b| {
        let mut session = Session::new();
        let world = create_minimal_world();
        b.iter(|| {
            session.set_world(world.clone());
            black_box(())
        })
    });

    // World with entities
    let world_100 = create_world_with_entities(100);
    group.bench_function("with_world_100_entities", |b| {
        b.iter(|| black_box(Session::with_world(world_100.clone())))
    });

    let world_1k = create_world_with_entities(1000);
    group.bench_function("with_world_1k_entities", |b| {
        b.iter(|| black_box(Session::with_world(world_1k.clone())))
    });

    group.finish();
}

// =============================================================================
// REPL Eval Benchmarks
// =============================================================================

fn bench_repl_eval(c: &mut Criterion) {
    use longtable_language::{Compiler, Vm, parse};

    let mut group = c.benchmark_group("repl_eval");

    // Simple expression (full pipeline: parse → compile → execute)
    group.bench_function("simple_add", |b| {
        b.iter(|| {
            let forms = parse("(+ 1 2)").unwrap();
            let mut compiler = Compiler::new();
            let program = compiler.compile(&forms).unwrap();
            let mut vm = Vm::new();
            black_box(vm.execute(&program).unwrap())
        })
    });

    // More complex expression
    group.bench_function("nested_arithmetic", |b| {
        b.iter(|| {
            let forms = parse("(* (+ 1 2) (- 10 5))").unwrap();
            let mut compiler = Compiler::new();
            let program = compiler.compile(&forms).unwrap();
            let mut vm = Vm::new();
            black_box(vm.execute(&program).unwrap())
        })
    });

    // Let binding
    group.bench_function("let_binding", |b| {
        b.iter(|| {
            let forms = parse("(let [x 10 y 20] (+ x y))").unwrap();
            let mut compiler = Compiler::new();
            let program = compiler.compile(&forms).unwrap();
            let mut vm = Vm::new();
            black_box(vm.execute(&program).unwrap())
        })
    });

    // Function definition and call
    group.bench_function("fn_define_call", |b| {
        b.iter(|| {
            let forms = parse("((fn [x] (* x x)) 5)").unwrap();
            let mut compiler = Compiler::new();
            let program = compiler.compile(&forms).unwrap();
            let mut vm = Vm::new();
            black_box(vm.execute(&program).unwrap())
        })
    });

    // Collection operations
    group.bench_function("map_filter", |b| {
        b.iter(|| {
            let forms = parse("(filter even? (map inc [1 2 3 4 5]))").unwrap();
            let mut compiler = Compiler::new();
            let program = compiler.compile(&forms).unwrap();
            let mut vm = Vm::new();
            black_box(vm.execute(&program).unwrap())
        })
    });

    // Map literal
    group.bench_function("map_literal", |b| {
        b.iter(|| {
            let forms = parse("{:a 1 :b 2 :c 3 :d 4 :e 5}").unwrap();
            let mut compiler = Compiler::new();
            let program = compiler.compile(&forms).unwrap();
            let mut vm = Vm::new();
            black_box(vm.execute(&program).unwrap())
        })
    });

    // Conditional
    group.bench_function("conditional", |b| {
        b.iter(|| {
            let forms = parse("(if (> 10 5) :yes :no)").unwrap();
            let mut compiler = Compiler::new();
            let program = compiler.compile(&forms).unwrap();
            let mut vm = Vm::new();
            black_box(vm.execute(&program).unwrap())
        })
    });

    // Multiple forms
    group.bench_function("multiple_forms", |b| {
        b.iter(|| {
            let forms = parse("(+ 1 2) (* 3 4) (- 10 5)").unwrap();
            let mut compiler = Compiler::new();
            let program = compiler.compile(&forms).unwrap();
            let mut vm = Vm::new();
            black_box(vm.execute(&program).unwrap())
        })
    });

    group.finish();
}

// =============================================================================
// Parse-Only Benchmarks (to isolate parsing cost)
// =============================================================================

fn bench_parse_only(c: &mut Criterion) {
    use longtable_language::parse;

    let mut group = c.benchmark_group("parse_only");

    group.bench_function("simple", |b| {
        b.iter(|| black_box(parse("(+ 1 2)").unwrap()))
    });

    group.bench_function("nested", |b| {
        b.iter(|| black_box(parse("(* (+ 1 2) (- 10 5) (/ 20 4))").unwrap()))
    });

    group.bench_function("let_binding", |b| {
        b.iter(|| black_box(parse("(let [x 10 y 20 z 30] (+ x y z))").unwrap()))
    });

    group.bench_function("function_def", |b| {
        b.iter(|| {
            black_box(
                parse("(defn factorial [n] (if (<= n 1) 1 (* n (factorial (dec n)))))").unwrap(),
            )
        })
    });

    group.bench_function("map_literal", |b| {
        b.iter(|| {
            black_box(
                parse("{:name \"test\" :health {:current 100 :max 100} :position {:x 0 :y 0}}")
                    .unwrap(),
            )
        })
    });

    group.bench_function("vector_literal", |b| {
        b.iter(|| black_box(parse("[1 2 3 4 5 6 7 8 9 10]").unwrap()))
    });

    // Larger expressions
    let large_expr = "(do
        (let [a 1 b 2 c 3 d 4 e 5]
            (+ a b c d e)
            (* a b c d e)
            (- a b c d e))
        (if (> 10 5)
            {:result :yes :value 42}
            {:result :no :value 0})
        (map inc [1 2 3 4 5 6 7 8 9 10]))";

    group.bench_function("large_expression", |b| {
        b.iter(|| black_box(parse(large_expr).unwrap()))
    });

    group.finish();
}

// =============================================================================
// Compile-Only Benchmarks (to isolate compilation cost)
// =============================================================================

fn bench_compile_only(c: &mut Criterion) {
    use longtable_language::{Compiler, parse};

    let mut group = c.benchmark_group("compile_only");

    let simple = parse("(+ 1 2)").unwrap();
    group.bench_function("simple", |b| {
        b.iter(|| {
            let mut compiler = Compiler::new();
            black_box(compiler.compile(&simple).unwrap())
        })
    });

    let nested = parse("(* (+ 1 2) (- 10 5) (/ 20 4))").unwrap();
    group.bench_function("nested", |b| {
        b.iter(|| {
            let mut compiler = Compiler::new();
            black_box(compiler.compile(&nested).unwrap())
        })
    });

    let let_binding = parse("(let [x 10 y 20 z 30] (+ x y z))").unwrap();
    group.bench_function("let_binding", |b| {
        b.iter(|| {
            let mut compiler = Compiler::new();
            black_box(compiler.compile(&let_binding).unwrap())
        })
    });

    let function_def =
        parse("(defn factorial [n] (if (<= n 1) 1 (* n (factorial (dec n)))))").unwrap();
    group.bench_function("function_def", |b| {
        b.iter(|| {
            let mut compiler = Compiler::new();
            black_box(compiler.compile(&function_def).unwrap())
        })
    });

    let map_filter = parse("(filter even? (map inc (range 100)))").unwrap();
    group.bench_function("map_filter", |b| {
        b.iter(|| {
            let mut compiler = Compiler::new();
            black_box(compiler.compile(&map_filter).unwrap())
        })
    });

    group.finish();
}

// =============================================================================
// Pipeline Stage Comparison
// =============================================================================

fn bench_pipeline_stages(c: &mut Criterion) {
    use longtable_language::{Compiler, Vm, parse};

    let mut group = c.benchmark_group("pipeline_stages");

    // Show cost breakdown for a typical expression
    let expr = "(let [x (+ 1 2) y (* 3 4)] (+ x y))";

    // Parse only
    group.bench_function("1_parse", |b| b.iter(|| black_box(parse(expr).unwrap())));

    // Parse + Compile
    let parsed = parse(expr).unwrap();
    group.bench_function("2_compile", |b| {
        b.iter(|| {
            let mut compiler = Compiler::new();
            black_box(compiler.compile(&parsed).unwrap())
        })
    });

    // Execute only (pre-compiled)
    let mut compiler = Compiler::new();
    let program = compiler.compile(&parsed).unwrap();
    group.bench_function("3_execute", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            black_box(vm.execute(&program).unwrap())
        })
    });

    // Full pipeline
    group.bench_function("4_full_pipeline", |b| {
        b.iter(|| {
            let forms = parse(expr).unwrap();
            let mut compiler = Compiler::new();
            let program = compiler.compile(&forms).unwrap();
            let mut vm = Vm::new();
            black_box(vm.execute(&program).unwrap())
        })
    });

    group.finish();
}

// =============================================================================
// Session State Operations
// =============================================================================

fn bench_session_state(c: &mut Criterion) {
    let mut group = c.benchmark_group("session_state");

    // Variable lookup with many variables
    group.bench_function("lookup_in_100_vars", |b| {
        let mut session = Session::new();
        for i in 0..100 {
            session.set_variable(format!("var{i}"), Value::Int(i as i64));
        }
        b.iter(|| black_box(session.get_variable("var50")))
    });

    // Entity lookup with many entities
    group.bench_function("lookup_in_100_entities", |b| {
        let mut session = Session::new();
        for i in 0..100 {
            session.register_entity(
                format!("entity{i}"),
                longtable_foundation::EntityId::new(i as u64, 1),
            );
        }
        b.iter(|| black_box(session.get_entity("entity50")))
    });

    // Module registry and namespace operations
    group.bench_function("namespace_context_access", |b| {
        let session = Session::new();
        b.iter(|| black_box(session.namespace_context()))
    });

    group.bench_function("module_registry_access", |b| {
        let session = Session::new();
        b.iter(|| black_box(session.module_registry()))
    });

    // Timeline access
    group.bench_function("timeline_access", |b| {
        let session = Session::new();
        b.iter(|| black_box(session.timeline()))
    });

    // Tracer access (disabled by default)
    group.bench_function("tracer_access", |b| {
        let session = Session::new();
        b.iter(|| black_box(session.tracer()))
    });

    // Debug session access
    group.bench_function("debug_session_access", |b| {
        let session = Session::new();
        b.iter(|| black_box(session.debug_session()))
    });

    group.finish();
}

// =============================================================================
// World Clone Benchmarks (important for immutable world operations)
// =============================================================================

fn bench_world_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("world_clone");

    let world_empty = World::new(0);
    group.bench_function("empty", |b| b.iter(|| black_box(world_empty.clone())));

    let world_100 = create_world_with_entities(100);
    group.bench_function("100_entities", |b| b.iter(|| black_box(world_100.clone())));

    let world_1k = create_world_with_entities(1000);
    group.bench_function("1k_entities", |b| b.iter(|| black_box(world_1k.clone())));

    group.finish();
}

criterion_group!(
    benches,
    bench_session,
    bench_repl_eval,
    bench_parse_only,
    bench_compile_only,
    bench_pipeline_stages,
    bench_session_state,
    bench_world_clone,
);

criterion_main!(benches);
