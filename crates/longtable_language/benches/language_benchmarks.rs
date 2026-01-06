//! Benchmarks for the Longtable language implementation.
//!
//! Run with: `cargo bench --package longtable_language`

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use longtable_language::{Lexer, Vm, compile, eval};

// =============================================================================
// Lexer Benchmarks
// =============================================================================

fn bench_lexer(c: &mut Criterion) {
    let mut group = c.benchmark_group("lexer");

    // Simple tokens
    let simple = "42";
    group.throughput(Throughput::Bytes(simple.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("simple_int", simple.len()),
        simple,
        |b, s| b.iter(|| Lexer::tokenize_all(black_box(s))),
    );

    // Expression
    let expr = "(+ 1 2 3)";
    group.throughput(Throughput::Bytes(expr.len() as u64));
    group.bench_with_input(BenchmarkId::new("expression", expr.len()), expr, |b, s| {
        b.iter(|| Lexer::tokenize_all(black_box(s)))
    });

    // Nested expression
    let nested = "(let [x (+ 1 2)] (if (> x 0) (* x x) (- x)))";
    group.throughput(Throughput::Bytes(nested.len() as u64));
    group.bench_with_input(BenchmarkId::new("nested", nested.len()), nested, |b, s| {
        b.iter(|| Lexer::tokenize_all(black_box(s)))
    });

    // Large expression with collections
    let large = r#"
        {:name "test"
         :values [1 2 3 4 5 6 7 8 9 10]
         :nested {:a 1 :b 2 :c 3}
         :set #{:x :y :z}}
    "#;
    group.throughput(Throughput::Bytes(large.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("collections", large.len()),
        large,
        |b, s| b.iter(|| Lexer::tokenize_all(black_box(s))),
    );

    group.finish();
}

// =============================================================================
// Parser Benchmarks
// =============================================================================

fn bench_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser");

    let expr = "(+ 1 2 3)";
    group.bench_with_input(BenchmarkId::new("expression", expr.len()), expr, |b, s| {
        b.iter(|| longtable_language::parse(black_box(s)))
    });

    let nested = "(let [x (+ 1 2)] (if (> x 0) (* x x) (- x)))";
    group.bench_with_input(BenchmarkId::new("nested", nested.len()), nested, |b, s| {
        b.iter(|| longtable_language::parse(black_box(s)))
    });

    let function = "(let [f (fn [x y] (+ x y))] (f 10 20))";
    group.bench_with_input(
        BenchmarkId::new("function", function.len()),
        function,
        |b, s| b.iter(|| longtable_language::parse(black_box(s))),
    );

    let collections = r#"
        {:name "test"
         :values [1 2 3 4 5 6 7 8 9 10]
         :nested {:a 1 :b 2 :c 3}
         :set #{:x :y :z}}
    "#;
    group.bench_with_input(
        BenchmarkId::new("collections", collections.len()),
        collections,
        |b, s| b.iter(|| longtable_language::parse(black_box(s))),
    );

    group.finish();
}

// =============================================================================
// Compiler Benchmarks
// =============================================================================

fn bench_compiler(c: &mut Criterion) {
    let mut group = c.benchmark_group("compiler");

    let simple = "(+ 1 2)";
    group.bench_with_input(
        BenchmarkId::new("simple_add", simple.len()),
        simple,
        |b, s| b.iter(|| compile(black_box(s))),
    );

    let arithmetic = "(+ (* 2 3) (- 10 (/ 20 4)))";
    group.bench_with_input(
        BenchmarkId::new("arithmetic", arithmetic.len()),
        arithmetic,
        |b, s| b.iter(|| compile(black_box(s))),
    );

    let control_flow = "(if (> 10 5) (+ 1 2) (* 3 4))";
    group.bench_with_input(
        BenchmarkId::new("control_flow", control_flow.len()),
        control_flow,
        |b, s| b.iter(|| compile(black_box(s))),
    );

    let let_binding = "(let [a 1 b 2 c 3 d 4 e 5] (+ a b c d e))";
    group.bench_with_input(
        BenchmarkId::new("let_binding", let_binding.len()),
        let_binding,
        |b, s| b.iter(|| compile(black_box(s))),
    );

    let function = "(let [f (fn [x y z] (+ x (* y z)))] (f 1 2 3))";
    group.bench_with_input(
        BenchmarkId::new("function", function.len()),
        function,
        |b, s| b.iter(|| compile(black_box(s))),
    );

    let closure = "(let [x 10] (let [f (fn [y] (+ x y))] (f 5)))";
    group.bench_with_input(
        BenchmarkId::new("closure", closure.len()),
        closure,
        |b, s| b.iter(|| compile(black_box(s))),
    );

    let recursive = "(let [fact (fn [n] (if (<= n 1) 1 (* n (fact (- n 1)))))] (fact 5))";
    group.bench_with_input(
        BenchmarkId::new("recursive", recursive.len()),
        recursive,
        |b, s| b.iter(|| compile(black_box(s))),
    );

    group.finish();
}

// =============================================================================
// VM Execution Benchmarks
// =============================================================================

fn bench_vm_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("vm_execution");

    // Constant loading
    let const_program = compile("42").unwrap();
    group.bench_function("constant", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&const_program))
        })
    });

    // Simple addition
    let add_program = compile("(+ 1 2)").unwrap();
    group.bench_function("add_simple", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&add_program))
        })
    });

    // Chained arithmetic
    let arithmetic_program = compile("(+ (* 2 3) (- 10 (/ 20 4)))").unwrap();
    group.bench_function("arithmetic_chain", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&arithmetic_program))
        })
    });

    // Comparison
    let compare_program = compile("(< 1 2)").unwrap();
    group.bench_function("comparison", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&compare_program))
        })
    });

    // If-then-else (true branch)
    let if_true_program = compile("(if true 1 2)").unwrap();
    group.bench_function("if_true", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&if_true_program))
        })
    });

    // If-then-else (false branch)
    let if_false_program = compile("(if false 1 2)").unwrap();
    group.bench_function("if_false", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&if_false_program))
        })
    });

    // Let binding
    let let_program = compile("(let [x 1 y 2] (+ x y))").unwrap();
    group.bench_function("let_binding", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&let_program))
        })
    });

    // Nested let
    let nested_let_program = compile("(let [x 1] (let [y 2] (let [z 3] (+ x y z))))").unwrap();
    group.bench_function("let_nested", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&nested_let_program))
        })
    });

    // Vector creation
    let vector_program = compile("[1 2 3 4 5]").unwrap();
    group.bench_function("vector_create", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&vector_program))
        })
    });

    // Map creation
    let map_program = compile("{:a 1 :b 2 :c 3}").unwrap();
    group.bench_function("map_create", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&map_program))
        })
    });

    group.finish();
}

// =============================================================================
// Function Call Benchmarks
// =============================================================================

fn bench_function_calls(c: &mut Criterion) {
    let mut group = c.benchmark_group("function_calls");

    // Simple function call
    let simple_fn = compile("((fn [x] x) 42)").unwrap();
    group.bench_function("identity", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&simple_fn))
        })
    });

    // Function with computation
    let compute_fn = compile("((fn [x y] (+ (* x x) (* y y))) 3 4)").unwrap();
    group.bench_function("compute", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&compute_fn))
        })
    });

    // Function stored in let
    let let_fn = compile("(let [f (fn [x] (* x 2))] (f 21))").unwrap();
    group.bench_function("let_stored", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&let_fn))
        })
    });

    // Higher-order function
    let higher_order =
        compile("(let [apply (fn [f x] (f x)) double (fn [n] (* n 2))] (apply double 21))")
            .unwrap();
    group.bench_function("higher_order", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&higher_order))
        })
    });

    // Closure capture
    let closure = compile("(let [x 10] ((fn [y] (+ x y)) 5))").unwrap();
    group.bench_function("closure_capture", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&closure))
        })
    });

    // Returned closure
    let returned_closure =
        compile("(let [make-adder (fn [n] (fn [x] (+ n x)))] ((make-adder 5) 10))").unwrap();
    group.bench_function("closure_returned", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&returned_closure))
        })
    });

    group.finish();
}

// =============================================================================
// Recursion Benchmarks
// =============================================================================

fn bench_recursion(c: &mut Criterion) {
    let mut group = c.benchmark_group("recursion");

    // Factorial(5) = 120
    let fact_5 =
        compile("(let [fact (fn [n] (if (<= n 1) 1 (* n (fact (- n 1)))))] (fact 5))").unwrap();
    group.bench_function("factorial_5", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&fact_5))
        })
    });

    // Factorial(10) = 3628800
    let fact_10 =
        compile("(let [fact (fn [n] (if (<= n 1) 1 (* n (fact (- n 1)))))] (fact 10))").unwrap();
    group.bench_function("factorial_10", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&fact_10))
        })
    });

    // Fibonacci(10) = 55
    let fib_10 =
        compile("(let [fib (fn [n] (if (<= n 1) n (+ (fib (- n 1)) (fib (- n 2)))))] (fib 10))")
            .unwrap();
    group.bench_function("fibonacci_10", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&fib_10))
        })
    });

    // Fibonacci(15) = 610 (more recursive calls)
    let fib_15 =
        compile("(let [fib (fn [n] (if (<= n 1) n (+ (fib (- n 1)) (fib (- n 2)))))] (fib 15))")
            .unwrap();
    group.bench_function("fibonacci_15", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&fib_15))
        })
    });

    // Countdown (linear recursion)
    let countdown =
        compile("(let [count (fn [n] (if (<= n 0) 0 (count (- n 1))))] (count 100))").unwrap();
    group.bench_function("countdown_100", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&countdown))
        })
    });

    group.finish();
}

// =============================================================================
// Native Function Benchmarks
// =============================================================================

fn bench_native_functions(c: &mut Criterion) {
    let mut group = c.benchmark_group("native_functions");

    // Type predicates
    let nil_check = compile("(nil? nil)").unwrap();
    group.bench_function("nil?", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&nil_check))
        })
    });

    let int_check = compile("(int? 42)").unwrap();
    group.bench_function("int?", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&int_check))
        })
    });

    // Collection operations
    let count_vec = compile("(count [1 2 3 4 5 6 7 8 9 10])").unwrap();
    group.bench_function("count_vector", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&count_vec))
        })
    });

    let first_vec = compile("(first [1 2 3 4 5])").unwrap();
    group.bench_function("first", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&first_vec))
        })
    });

    let rest_vec = compile("(rest [1 2 3 4 5])").unwrap();
    group.bench_function("rest", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&rest_vec))
        })
    });

    let conj_vec = compile("(conj [1 2 3] 4 5)").unwrap();
    group.bench_function("conj", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&conj_vec))
        })
    });

    let get_map = compile("(get {:a 1 :b 2 :c 3} :b)").unwrap();
    group.bench_function("get_map", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&get_map))
        })
    });

    let assoc_map = compile("(assoc {:a 1} :b 2 :c 3)").unwrap();
    group.bench_function("assoc", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&assoc_map))
        })
    });

    // String operations
    let str_concat = compile(r#"(str "hello" " " "world")"#).unwrap();
    group.bench_function("str_concat", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&str_concat))
        })
    });

    let str_upper = compile(r#"(str/upper "hello")"#).unwrap();
    group.bench_function("str_upper", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&str_upper))
        })
    });

    // Math operations
    let math_sqrt = compile("(sqrt 144)").unwrap();
    group.bench_function("sqrt", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&math_sqrt))
        })
    });

    let math_min_max = compile("(max (min 5 10 3) (min 8 2 6))").unwrap();
    group.bench_function("min_max", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&math_min_max))
        })
    });

    group.finish();
}

// =============================================================================
// End-to-End Benchmarks
// =============================================================================

fn bench_end_to_end(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end");

    // Simple eval (compile + execute)
    let simple = "(+ 1 2)";
    group.bench_function("eval_simple", |b| b.iter(|| eval(black_box(simple))));

    // Complex eval
    let complex = "(let [x 10 y 20] (if (> x 5) (+ x y) (* x y)))";
    group.bench_function("eval_complex", |b| b.iter(|| eval(black_box(complex))));

    // Factorial eval
    let factorial = "(let [fact (fn [n] (if (<= n 1) 1 (* n (fact (- n 1)))))] (fact 10))";
    group.bench_function("eval_factorial", |b| b.iter(|| eval(black_box(factorial))));

    group.finish();
}

// =============================================================================
// Throughput Benchmarks (ops/sec estimation)
// =============================================================================

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");
    group.throughput(Throughput::Elements(1));

    // Simple operations to estimate ops/sec
    let add_program = compile("(+ 1 2)").unwrap();
    group.bench_function("simple_op", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&add_program))
        })
    });

    // 10 operations
    let ten_ops = compile("(+ 1 (+ 2 (+ 3 (+ 4 (+ 5 (+ 6 (+ 7 (+ 8 (+ 9 10)))))))))").unwrap();
    group.throughput(Throughput::Elements(10));
    group.bench_function("ten_ops", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&ten_ops))
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_lexer,
    bench_parser,
    bench_compiler,
    bench_vm_execution,
    bench_function_calls,
    bench_recursion,
    bench_native_functions,
    bench_end_to_end,
    bench_throughput,
);

criterion_main!(benches);
