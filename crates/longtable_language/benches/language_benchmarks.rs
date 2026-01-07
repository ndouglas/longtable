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
// Standard Library Benchmarks
// =============================================================================

fn bench_stdlib(c: &mut Criterion) {
    let mut group = c.benchmark_group("stdlib");

    // Higher-order functions: map, filter, reduce
    let map_small = compile("(map inc [1 2 3 4 5])").unwrap();
    group.bench_function("map_small", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&map_small))
        })
    });

    let map_medium = compile("(map inc (range 100))").unwrap();
    group.bench_function("map_medium", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&map_medium))
        })
    });

    let filter_small = compile("(filter even? [1 2 3 4 5 6 7 8 9 10])").unwrap();
    group.bench_function("filter_small", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&filter_small))
        })
    });

    let filter_medium = compile("(filter even? (range 100))").unwrap();
    group.bench_function("filter_medium", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&filter_medium))
        })
    });

    let reduce_sum = compile("(reduce + 0 (range 100))").unwrap();
    group.bench_function("reduce_sum", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&reduce_sum))
        })
    });

    let reduce_product = compile("(reduce * 1 [1 2 3 4 5 6 7 8 9 10])").unwrap();
    group.bench_function("reduce_product", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&reduce_product))
        })
    });

    // Sequence operations
    let take_drop = compile("(take 50 (drop 25 (range 100)))").unwrap();
    group.bench_function("take_drop", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&take_drop))
        })
    });

    let concat_vecs = compile("(concat [1 2 3] [4 5 6] [7 8 9])").unwrap();
    group.bench_function("concat", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&concat_vecs))
        })
    });

    let reverse_vec = compile("(reverse (range 100))").unwrap();
    group.bench_function("reverse", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&reverse_vec))
        })
    });

    let sort_vec = compile("(sort [5 2 8 1 9 3 7 4 6 10])").unwrap();
    group.bench_function("sort", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&sort_vec))
        })
    });

    // Range generation
    let range_100 = compile("(range 100)").unwrap();
    group.bench_function("range_100", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&range_100))
        })
    });

    let range_step = compile("(range 0 100 2)").unwrap();
    group.bench_function("range_step", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&range_step))
        })
    });

    // String operations
    let str_split = compile(r#"(str/split "a,b,c,d,e,f,g,h,i,j" ",")"#).unwrap();
    group.bench_function("str_split", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&str_split))
        })
    });

    let str_join = compile(r#"(str/join "," ["a" "b" "c" "d" "e" "f" "g" "h" "i" "j"])"#).unwrap();
    group.bench_function("str_join", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&str_join))
        })
    });

    let str_replace = compile(r#"(str/replace-all "hello world hello" "hello" "hi")"#).unwrap();
    group.bench_function("str_replace_all", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&str_replace))
        })
    });

    let format_str = compile(r#"(format "x={} y={} z={}" 1 2 3)"#).unwrap();
    group.bench_function("format", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&format_str))
        })
    });

    // Math operations
    let trig_sin = compile("(sin 1.5)").unwrap();
    group.bench_function("sin", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&trig_sin))
        })
    });

    let trig_cos = compile("(cos 1.5)").unwrap();
    group.bench_function("cos", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&trig_cos))
        })
    });

    let pow_calc = compile("(pow 2 10)").unwrap();
    group.bench_function("pow", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&pow_calc))
        })
    });

    let log_calc = compile("(log 100)").unwrap();
    group.bench_function("log", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&log_calc))
        })
    });

    // Vector math
    let vec_add = compile("(vec+ [1.0 2.0 3.0] [4.0 5.0 6.0])").unwrap();
    group.bench_function("vec_add", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&vec_add))
        })
    });

    let vec_dot = compile("(vec-dot [1.0 2.0 3.0] [4.0 5.0 6.0])").unwrap();
    group.bench_function("vec_dot", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&vec_dot))
        })
    });

    let vec_normalize = compile("(vec-normalize [3.0 4.0 0.0])").unwrap();
    group.bench_function("vec_normalize", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&vec_normalize))
        })
    });

    let vec_cross = compile("(vec-cross [1.0 0.0 0.0] [0.0 1.0 0.0])").unwrap();
    group.bench_function("vec_cross", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&vec_cross))
        })
    });

    // Collection predicates
    let every_check = compile("(every? even? [2 4 6 8 10])").unwrap();
    group.bench_function("every?", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&every_check))
        })
    });

    let some_check = compile("(some even? [1 3 5 7 8 9])").unwrap();
    group.bench_function("some", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&some_check))
        })
    });

    // Extended collections
    let flatten_vec = compile("(flatten [[1 2] [3 [4 5]] [6]])").unwrap();
    group.bench_function("flatten", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&flatten_vec))
        })
    });

    let distinct_vec = compile("(distinct [1 2 1 3 2 4 3 5 4 1])").unwrap();
    group.bench_function("distinct", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&distinct_vec))
        })
    });

    let partition_vec = compile("(partition 3 [1 2 3 4 5 6 7 8 9])").unwrap();
    group.bench_function("partition", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&partition_vec))
        })
    });

    let interleave_vecs = compile("(interleave [1 2 3] [:a :b :c])").unwrap();
    group.bench_function("interleave", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&interleave_vecs))
        })
    });

    let zip_vecs = compile("(zip [1 2 3] [:a :b :c])").unwrap();
    group.bench_function("zip", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&zip_vecs))
        })
    });

    // Chained operations (realistic use)
    let chain_ops = compile("(reduce + 0 (filter even? (map inc (range 100))))").unwrap();
    group.bench_function("chain_ops", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&chain_ops))
        })
    });

    group.finish();
}

// =============================================================================
// Additional Stdlib Benchmarks (Stage 2)
// =============================================================================

fn bench_stdlib_extended(c: &mut Criterion) {
    let mut group = c.benchmark_group("stdlib_extended");

    // Additional collection functions
    let empty_vec = compile("(empty? [])").unwrap();
    group.bench_function("empty?_vec", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&empty_vec))
        })
    });

    let last_vec = compile("(last [1 2 3 4 5])").unwrap();
    group.bench_function("last", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&last_vec))
        })
    });

    let nth_vec = compile("(nth [1 2 3 4 5] 3)").unwrap();
    group.bench_function("nth", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&nth_vec))
        })
    });

    let cons_vec = compile("(cons 0 [1 2 3])").unwrap();
    group.bench_function("cons", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&cons_vec))
        })
    });

    let dissoc_map = compile("(dissoc {:a 1 :b 2 :c 3} :b)").unwrap();
    group.bench_function("dissoc", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&dissoc_map))
        })
    });

    let contains_map = compile("(contains? {:a 1 :b 2} :b)").unwrap();
    group.bench_function("contains?_map", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&contains_map))
        })
    });

    let contains_set = compile("(contains? #{1 2 3 4 5} 3)").unwrap();
    group.bench_function("contains?_set", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&contains_set))
        })
    });

    let keys_map = compile("(keys {:a 1 :b 2 :c 3 :d 4 :e 5})").unwrap();
    group.bench_function("keys", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&keys_map))
        })
    });

    let vals_map = compile("(vals {:a 1 :b 2 :c 3 :d 4 :e 5})").unwrap();
    group.bench_function("vals", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&vals_map))
        })
    });

    let merge_maps = compile("(merge {:a 1 :b 2} {:c 3 :d 4} {:e 5})").unwrap();
    group.bench_function("merge", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&merge_maps))
        })
    });

    let into_vec = compile("(into [1 2 3] [4 5 6])").unwrap();
    group.bench_function("into", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&into_vec))
        })
    });

    // Extended sequence ops
    let dedupe_vec = compile("(dedupe [1 1 2 2 3 3 2 2 1 1])").unwrap();
    group.bench_function("dedupe", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&dedupe_vec))
        })
    });

    let partition_all = compile("(partition-all 3 [1 2 3 4 5 6 7 8])").unwrap();
    group.bench_function("partition-all", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&partition_all))
        })
    });

    let interpose_vec = compile("(interpose :sep [1 2 3 4 5])").unwrap();
    group.bench_function("interpose", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&interpose_vec))
        })
    });

    let repeat_val = compile("(repeat 10 :x)").unwrap();
    group.bench_function("repeat", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&repeat_val))
        })
    });

    group.finish();
}

fn bench_type_predicates(c: &mut Criterion) {
    let mut group = c.benchmark_group("type_predicates");

    let bool_check = compile("(bool? true)").unwrap();
    group.bench_function("bool?", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&bool_check))
        })
    });

    let string_check = compile(r#"(string? "hello")"#).unwrap();
    group.bench_function("string?", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&string_check))
        })
    });

    let keyword_check = compile("(keyword? :test)").unwrap();
    group.bench_function("keyword?", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&keyword_check))
        })
    });

    let vector_check = compile("(vector? [1 2 3])").unwrap();
    group.bench_function("vector?", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&vector_check))
        })
    });

    let map_check = compile("(map? {:a 1})").unwrap();
    group.bench_function("map?", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&map_check))
        })
    });

    let set_check = compile("(set? #{1 2 3})").unwrap();
    group.bench_function("set?", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&set_check))
        })
    });

    let number_check = compile("(number? 42)").unwrap();
    group.bench_function("number?", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&number_check))
        })
    });

    let coll_check = compile("(coll? [1 2 3])").unwrap();
    group.bench_function("coll?", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&coll_check))
        })
    });

    let fn_check = compile("(fn? inc)").unwrap();
    group.bench_function("fn?", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&fn_check))
        })
    });

    let type_of = compile("(type [1 2 3])").unwrap();
    group.bench_function("type", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&type_of))
        })
    });

    group.finish();
}

fn bench_math_extended(c: &mut Criterion) {
    let mut group = c.benchmark_group("math_extended");

    let abs_int = compile("(abs -42)").unwrap();
    group.bench_function("abs", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&abs_int))
        })
    });

    let floor_float = compile("(floor 3.7)").unwrap();
    group.bench_function("floor", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&floor_float))
        })
    });

    let ceil_float = compile("(ceil 3.2)").unwrap();
    group.bench_function("ceil", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&ceil_float))
        })
    });

    let round_float = compile("(round 3.5)").unwrap();
    group.bench_function("round", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&round_float))
        })
    });

    let tan_calc = compile("(tan 0.5)").unwrap();
    group.bench_function("tan", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&tan_calc))
        })
    });

    let clamp_calc = compile("(clamp 15 0 10)").unwrap();
    group.bench_function("clamp", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&clamp_calc))
        })
    });

    let rem_calc = compile("(rem 17 5)").unwrap();
    group.bench_function("rem", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&rem_calc))
        })
    });

    let inc_val = compile("(inc 41)").unwrap();
    group.bench_function("inc", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&inc_val))
        })
    });

    let dec_val = compile("(dec 43)").unwrap();
    group.bench_function("dec", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&dec_val))
        })
    });

    group.finish();
}

fn bench_scale_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_operations");

    // Map at various scales
    let map_1k = compile("(map inc (range 1000))").unwrap();
    group.bench_function("map_1k", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&map_1k))
        })
    });

    // Filter at various scales
    let filter_1k = compile("(filter even? (range 1000))").unwrap();
    group.bench_function("filter_1k", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&filter_1k))
        })
    });

    // Reduce at various scales
    let reduce_1k = compile("(reduce + 0 (range 1000))").unwrap();
    group.bench_function("reduce_1k", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&reduce_1k))
        })
    });

    // Sort at various scales
    let sort_100 = compile("(sort (reverse (range 100)))").unwrap();
    group.bench_function("sort_100", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&sort_100))
        })
    });

    let sort_1k = compile("(sort (reverse (range 1000)))").unwrap();
    group.bench_function("sort_1k", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&sort_1k))
        })
    });

    // Chained operations at scale
    let chain_1k = compile("(reduce + 0 (filter even? (map inc (range 1000))))").unwrap();
    group.bench_function("chain_1k", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&chain_1k))
        })
    });

    // Distinct at scale
    let distinct_with_dups =
        compile("(distinct (concat (range 100) (range 100) (range 100)))").unwrap();
    group.bench_function("distinct_300_to_100", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&distinct_with_dups))
        })
    });

    // Range generation
    let range_1k = compile("(range 1000)").unwrap();
    group.bench_function("range_1k", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&range_1k))
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

// =============================================================================
// VM Edge Cases (Stage 4)
// =============================================================================

fn bench_vm_edge_cases(c: &mut Criterion) {
    let mut group = c.benchmark_group("vm_edge_cases");

    // Deep recursion: test stack handling at various depths
    // Using linear recursion (countdown) to avoid exponential blowup
    let deep_50 =
        compile("(let [count (fn [n] (if (<= n 0) 0 (count (- n 1))))] (count 50))").unwrap();
    group.bench_function("deep_recursion_50", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&deep_50))
        })
    });

    let deep_100 =
        compile("(let [count (fn [n] (if (<= n 0) 0 (count (- n 1))))] (count 100))").unwrap();
    group.bench_function("deep_recursion_100", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&deep_100))
        })
    });

    let deep_500 =
        compile("(let [count (fn [n] (if (<= n 0) 0 (count (- n 1))))] (count 500))").unwrap();
    group.bench_function("deep_recursion_500", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&deep_500))
        })
    });

    // Large closures: closures capturing many variables
    let closure_1_capture = compile("(let [a 1] ((fn [x] (+ a x)) 10))").unwrap();
    group.bench_function("closure_1_capture", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&closure_1_capture))
        })
    });

    let closure_5_captures =
        compile("(let [a 1 b 2 c 3 d 4 e 5] ((fn [x] (+ a b c d e x)) 10))").unwrap();
    group.bench_function("closure_5_captures", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&closure_5_captures))
        })
    });

    let closure_10_captures = compile(
        "(let [a 1 b 2 c 3 d 4 e 5 f 6 g 7 h 8 i 9 j 10] ((fn [x] (+ a b c d e f g h i j x)) 10))",
    )
    .unwrap();
    group.bench_function("closure_10_captures", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&closure_10_captures))
        })
    });

    // Nested closures (closure returning closure)
    // Simpler version that avoids closure capture issues
    let nested_closure =
        compile("(let [make-adder (fn [x] (fn [y] (+ x y)))] ((make-adder 10) 5))").unwrap();
    group.bench_function("nested_closure_2_deep", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&nested_closure))
        })
    });

    // Repeated closure application (tests closure overhead)
    let repeated_closure =
        compile("(let [add1 (fn [x] (+ x 1))] (add1 (add1 (add1 (add1 (add1 0))))))").unwrap();
    group.bench_function("repeated_closure_5_calls", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&repeated_closure))
        })
    });

    // Many local bindings in one let
    let many_bindings = compile(
        "(let [a 1 b 2 c 3 d 4 e 5 f 6 g 7 h 8 i 9 j 10 k 11 l 12 m 13 n 14 o 15 p 16 q 17 r 18 s 19 t 20] (+ a b c d e f g h i j k l m n o p q r s t))",
    )
    .unwrap();
    group.bench_function("let_20_bindings", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&many_bindings))
        })
    });

    // Deeply nested let expressions
    let nested_let_5 = compile(
        "(let [a 1] (let [b (+ a 1)] (let [c (+ b 1)] (let [d (+ c 1)] (let [e (+ d 1)] e)))))",
    )
    .unwrap();
    group.bench_function("nested_let_5_deep", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&nested_let_5))
        })
    });

    // Many function arguments
    let fn_5_args = compile("((fn [a b c d e] (+ a b c d e)) 1 2 3 4 5)").unwrap();
    group.bench_function("fn_5_args", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&fn_5_args))
        })
    });

    let fn_10_args =
        compile("((fn [a b c d e f g h i j] (+ a b c d e f g h i j)) 1 2 3 4 5 6 7 8 9 10)")
            .unwrap();
    group.bench_function("fn_10_args", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&fn_10_args))
        })
    });

    group.finish();
}

// =============================================================================
// Compiler Optimization Benchmarks (Stage 4)
// =============================================================================

fn bench_compiler_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("compiler_optimization");

    // Constant folding potential: expressions that could be folded at compile time
    // These measure how well the compiler handles constant expressions

    // Pure constant expression
    let const_expr = compile("(+ 1 2 3 4 5)").unwrap();
    group.bench_function("constant_folding_add", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&const_expr))
        })
    });

    // Nested constant expression
    let nested_const = compile("(* (+ 1 2) (- 10 5))").unwrap();
    group.bench_function("constant_folding_nested", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&nested_const))
        })
    });

    // Constant boolean expression
    let const_bool = compile("(and true true (not false))").unwrap();
    group.bench_function("constant_folding_boolean", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&const_bool))
        })
    });

    // Constant comparison
    let const_cmp = compile("(if (> 10 5) :yes :no)").unwrap();
    group.bench_function("constant_folding_comparison", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&const_cmp))
        })
    });

    // Dead code potential: unreachable branches
    // If compiler eliminates dead code, these should be faster

    // Dead else branch (condition always true)
    let dead_else = compile("(if true 42 (+ 1 2 3 4 5 6 7 8 9 10))").unwrap();
    group.bench_function("dead_code_else_branch", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&dead_else))
        })
    });

    // Dead then branch (condition always false)
    let dead_then = compile("(if false (+ 1 2 3 4 5 6 7 8 9 10) 42)").unwrap();
    group.bench_function("dead_code_then_branch", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&dead_then))
        })
    });

    // Compare: dynamic condition (can't optimize)
    let dynamic_cond = compile("(let [x 5] (if (> x 3) 42 0))").unwrap();
    group.bench_function("dynamic_condition", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&dynamic_cond))
        })
    });

    // Identity operations (could be optimized away)
    let add_zero = compile("(+ 42 0)").unwrap();
    group.bench_function("identity_add_zero", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&add_zero))
        })
    });

    let mul_one = compile("(* 42 1)").unwrap();
    group.bench_function("identity_mul_one", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&mul_one))
        })
    });

    // Let binding unused (could be optimized away)
    let unused_binding = compile("(let [unused 999] 42)").unwrap();
    group.bench_function("unused_binding", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&unused_binding))
        })
    });

    // String constant handling
    let string_const = compile(r#""hello world""#).unwrap();
    group.bench_function("string_constant", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&string_const))
        })
    });

    // Keyword constant handling (should be interned)
    let keyword_const = compile(":my-keyword").unwrap();
    group.bench_function("keyword_constant", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&keyword_const))
        })
    });

    // Collection literals (compile-time construction)
    let vec_literal = compile("[1 2 3 4 5]").unwrap();
    group.bench_function("vector_literal", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&vec_literal))
        })
    });

    let map_literal = compile("{:a 1 :b 2 :c 3}").unwrap();
    group.bench_function("map_literal", |b| {
        let mut vm = Vm::new();
        b.iter(|| {
            vm.reset();
            vm.execute(black_box(&map_literal))
        })
    });

    group.finish();
}

// =============================================================================
// Compilation Cost Benchmarks (Stage 4)
// =============================================================================

fn bench_compilation_cost(c: &mut Criterion) {
    let mut group = c.benchmark_group("compilation_cost");

    // Measure compilation overhead for various expression sizes
    // This helps understand when bytecode caching would help

    // Tiny expression
    group.bench_function("compile_tiny", |b| b.iter(|| compile(black_box("42"))));

    // Small expression
    group.bench_function("compile_small", |b| {
        b.iter(|| compile(black_box("(+ 1 2)")))
    });

    // Medium expression
    group.bench_function("compile_medium", |b| {
        b.iter(|| compile(black_box("(let [x 10 y 20] (if (> x 5) (+ x y) (* x y)))")))
    });

    // Large expression with function
    group.bench_function("compile_large", |b| {
        b.iter(|| {
            compile(black_box(
                "(let [f (fn [n] (if (<= n 1) 1 (* n (f (- n 1)))))] (f 10))",
            ))
        })
    });

    // Very large expression with collections
    let very_large = r#"
        (let [data {:name "test"
                    :values [1 2 3 4 5 6 7 8 9 10]
                    :nested {:a 1 :b 2 :c 3}}
              f (fn [x] (+ x 1))
              g (fn [x y] (* x y))]
          (if (> (count (get data :values)) 5)
              (reduce + 0 (map f (get data :values)))
              (g 10 20)))
    "#;
    group.bench_function("compile_very_large", |b| {
        b.iter(|| compile(black_box(very_large)))
    });

    // Expression with many symbols (tests symbol resolution)
    let many_symbols = "(let [a 1 b 2 c 3 d 4 e 5 f 6 g 7 h 8 i 9 j 10] (+ a b c d e f g h i j))";
    group.bench_function("compile_many_symbols", |b| {
        b.iter(|| compile(black_box(many_symbols)))
    });

    // Expression with many native calls
    let many_natives = "(+ (- (* (/ (inc (dec (abs (min (max 1 2) 3)))) 2) 3) 4) 5)";
    group.bench_function("compile_many_natives", |b| {
        b.iter(|| compile(black_box(many_natives)))
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
    bench_stdlib,
    bench_stdlib_extended,
    bench_type_predicates,
    bench_math_extended,
    bench_scale_operations,
    bench_end_to_end,
    bench_throughput,
    bench_vm_edge_cases,
    bench_compiler_optimization,
    bench_compilation_cost,
);

criterion_main!(benches);
