//! Tests for the VM.

use super::*;

fn eval_test(source: &str) -> Value {
    eval(source).expect("eval failed")
}

#[test]
fn eval_nil() {
    assert_eq!(eval_test("nil"), Value::Nil);
}

#[test]
fn eval_bool() {
    assert_eq!(eval_test("true"), Value::Bool(true));
    assert_eq!(eval_test("false"), Value::Bool(false));
}

#[test]
fn eval_int() {
    assert_eq!(eval_test("42"), Value::Int(42));
    assert_eq!(eval_test("-17"), Value::Int(-17));
}

#[test]
fn eval_float() {
    assert!(matches!(eval_test("3.14"), Value::Float(f) if (f - 3.14).abs() < 0.001));
}

#[test]
fn eval_string() {
    assert_eq!(eval_test(r#""hello""#), Value::String("hello".into()));
}

#[test]
fn eval_addition() {
    assert_eq!(eval_test("(+ 1 2)"), Value::Int(3));
    assert_eq!(eval_test("(+ 1 2 3)"), Value::Int(6));
    assert_eq!(eval_test("(+ 1 2 3 4)"), Value::Int(10));
}

#[test]
fn eval_subtraction() {
    assert_eq!(eval_test("(- 10 3)"), Value::Int(7));
    assert_eq!(eval_test("(- 10 3 2)"), Value::Int(5));
}

#[test]
fn eval_multiplication() {
    assert_eq!(eval_test("(* 3 4)"), Value::Int(12));
    assert_eq!(eval_test("(* 2 3 4)"), Value::Int(24));
}

#[test]
fn eval_division() {
    assert_eq!(eval_test("(/ 10 2)"), Value::Int(5));
    assert_eq!(eval_test("(/ 20 2 2)"), Value::Int(5));
}

#[test]
fn eval_division_by_zero() {
    let result = eval("(/ 10 0)");
    assert!(result.is_err());
}

#[test]
fn eval_nested_arithmetic() {
    assert_eq!(eval_test("(+ (* 2 3) (- 10 5))"), Value::Int(11));
    assert_eq!(eval_test("(* (+ 1 2) (+ 3 4))"), Value::Int(21));
}

#[test]
fn eval_comparison() {
    assert_eq!(eval_test("(< 1 2)"), Value::Bool(true));
    assert_eq!(eval_test("(< 2 1)"), Value::Bool(false));
    assert_eq!(eval_test("(<= 2 2)"), Value::Bool(true));
    assert_eq!(eval_test("(> 3 2)"), Value::Bool(true));
    assert_eq!(eval_test("(>= 2 2)"), Value::Bool(true));
    assert_eq!(eval_test("(= 1 1)"), Value::Bool(true));
    assert_eq!(eval_test("(!= 1 2)"), Value::Bool(true));
}

#[test]
fn eval_not() {
    assert_eq!(eval_test("(not true)"), Value::Bool(false));
    assert_eq!(eval_test("(not false)"), Value::Bool(true));
    assert_eq!(eval_test("(not nil)"), Value::Bool(true));
    assert_eq!(eval_test("(not 1)"), Value::Bool(false));
}

#[test]
fn eval_if_then_else() {
    assert_eq!(eval_test("(if true 1 2)"), Value::Int(1));
    assert_eq!(eval_test("(if false 1 2)"), Value::Int(2));
    assert_eq!(eval_test("(if nil 1 2)"), Value::Int(2));
    assert_eq!(eval_test("(if 42 1 2)"), Value::Int(1));
}

#[test]
fn eval_if_without_else() {
    assert_eq!(eval_test("(if true 1)"), Value::Int(1));
    assert_eq!(eval_test("(if false 1)"), Value::Nil);
}

#[test]
fn eval_let() {
    assert_eq!(eval_test("(let [x 1] x)"), Value::Int(1));
    assert_eq!(eval_test("(let [x 1 y 2] (+ x y))"), Value::Int(3));
}

#[test]
fn eval_let_shadowing() {
    assert_eq!(eval_test("(let [x 1] (let [x 2] x))"), Value::Int(2));
}

#[test]
fn eval_let_uses_previous_bindings() {
    assert_eq!(eval_test("(let [x 1 y (+ x 1)] y)"), Value::Int(2));
}

#[test]
fn eval_do() {
    assert_eq!(eval_test("(do 1 2 3)"), Value::Int(3));
    assert_eq!(eval_test("(do)"), Value::Nil);
}

#[test]
fn eval_vector() {
    let result = eval_test("[1 2 3]");
    match result {
        Value::Vec(v) => {
            assert_eq!(v.len(), 3);
            assert_eq!(v.get(0), Some(&Value::Int(1)));
            assert_eq!(v.get(1), Some(&Value::Int(2)));
            assert_eq!(v.get(2), Some(&Value::Int(3)));
        }
        _ => panic!("expected vector"),
    }
}

#[test]
fn eval_empty_list_is_nil() {
    assert_eq!(eval_test("()"), Value::Nil);
}

#[test]
fn eval_mixed_types() {
    assert!(matches!(eval_test("(+ 1 2.0)"), Value::Float(f) if (f - 3.0).abs() < 0.001));
    assert!(matches!(eval_test("(+ 1.0 2)"), Value::Float(f) if (f - 3.0).abs() < 0.001));
}

#[test]
fn eval_string_concat() {
    assert_eq!(
        eval_test(r#"(+ "hello" " world")"#),
        Value::String("hello world".into())
    );
}

#[test]
fn eval_complex_expression() {
    // (let [x 10 y 20] (if (> x 5) (+ x y) (* x y)))
    let result = eval_test("(let [x 10 y 20] (if (> x 5) (+ x y) (* x y)))");
    assert_eq!(result, Value::Int(30));
}

// =========================================================================
// Native Function Tests
// =========================================================================

#[test]
fn eval_predicate_nil() {
    assert_eq!(eval_test("(nil? nil)"), Value::Bool(true));
    assert_eq!(eval_test("(nil? 1)"), Value::Bool(false));
    assert_eq!(eval_test("(nil? false)"), Value::Bool(false));
}

#[test]
fn eval_predicate_some() {
    assert_eq!(eval_test("(some? 1)"), Value::Bool(true));
    assert_eq!(eval_test("(some? nil)"), Value::Bool(false));
    assert_eq!(eval_test("(some? false)"), Value::Bool(true));
}

#[test]
fn eval_predicate_int() {
    assert_eq!(eval_test("(int? 42)"), Value::Bool(true));
    assert_eq!(eval_test("(int? 3.14)"), Value::Bool(false));
    assert_eq!(eval_test("(int? nil)"), Value::Bool(false));
}

#[test]
fn eval_predicate_float() {
    assert_eq!(eval_test("(float? 3.14)"), Value::Bool(true));
    assert_eq!(eval_test("(float? 42)"), Value::Bool(false));
}

#[test]
fn eval_predicate_string() {
    assert_eq!(eval_test(r#"(string? "hello")"#), Value::Bool(true));
    assert_eq!(eval_test("(string? 42)"), Value::Bool(false));
}

#[test]
fn eval_predicate_vector() {
    assert_eq!(eval_test("(vector? [1 2 3])"), Value::Bool(true));
    assert_eq!(eval_test("(vector? nil)"), Value::Bool(false));
}

#[test]
fn eval_predicate_map() {
    assert_eq!(eval_test("(map? {:a 1})"), Value::Bool(true));
    assert_eq!(eval_test("(map? [1 2])"), Value::Bool(false));
}

#[test]
fn eval_predicate_set() {
    assert_eq!(eval_test("(set? #{1 2})"), Value::Bool(true));
    assert_eq!(eval_test("(set? [1 2])"), Value::Bool(false));
}

#[test]
fn eval_count() {
    assert_eq!(eval_test("(count [1 2 3])"), Value::Int(3));
    assert_eq!(eval_test("(count [])"), Value::Int(0));
    assert_eq!(eval_test("(count {:a 1 :b 2})"), Value::Int(2));
    assert_eq!(eval_test("(count #{1 2 3})"), Value::Int(3));
    assert_eq!(eval_test(r#"(count "hello")"#), Value::Int(5));
    assert_eq!(eval_test("(count nil)"), Value::Int(0));
}

#[test]
fn eval_empty() {
    assert_eq!(eval_test("(empty? [])"), Value::Bool(true));
    assert_eq!(eval_test("(empty? [1])"), Value::Bool(false));
    assert_eq!(eval_test("(empty? nil)"), Value::Bool(true));
    assert_eq!(eval_test("(empty? {})"), Value::Bool(true));
}

#[test]
fn eval_first() {
    assert_eq!(eval_test("(first [1 2 3])"), Value::Int(1));
    assert_eq!(eval_test("(first [])"), Value::Nil);
    assert_eq!(eval_test("(first nil)"), Value::Nil);
}

#[test]
fn eval_rest() {
    let result = eval_test("(rest [1 2 3])");
    match result {
        Value::Vec(v) => {
            assert_eq!(v.len(), 2);
            assert_eq!(v.get(0), Some(&Value::Int(2)));
            assert_eq!(v.get(1), Some(&Value::Int(3)));
        }
        _ => panic!("expected vector"),
    }
    let result = eval_test("(rest [])");
    match result {
        Value::Vec(v) => assert!(v.is_empty()),
        _ => panic!("expected vector"),
    }
}

#[test]
fn eval_nth() {
    assert_eq!(eval_test("(nth [10 20 30] 0)"), Value::Int(10));
    assert_eq!(eval_test("(nth [10 20 30] 2)"), Value::Int(30));
    assert_eq!(eval_test("(nth [10 20 30] 5)"), Value::Nil);
}

#[test]
fn eval_conj() {
    let result = eval_test("(conj [1 2] 3)");
    match result {
        Value::Vec(v) => {
            assert_eq!(v.len(), 3);
            assert_eq!(v.get(2), Some(&Value::Int(3)));
        }
        _ => panic!("expected vector"),
    }
    let result = eval_test("(conj nil 1)");
    match result {
        Value::Vec(v) => {
            assert_eq!(v.len(), 1);
            assert_eq!(v.get(0), Some(&Value::Int(1)));
        }
        _ => panic!("expected vector"),
    }
}

#[test]
fn eval_cons() {
    let result = eval_test("(cons 0 [1 2])");
    match result {
        Value::Vec(v) => {
            assert_eq!(v.len(), 3);
            assert_eq!(v.get(0), Some(&Value::Int(0)));
            assert_eq!(v.get(1), Some(&Value::Int(1)));
        }
        _ => panic!("expected vector"),
    }
}

#[test]
fn eval_get() {
    assert_eq!(eval_test("(get [10 20 30] 1)"), Value::Int(20));
    assert_eq!(eval_test("(get {:a 1} :a)"), Value::Int(1));
    assert_eq!(eval_test("(get {:a 1} :b)"), Value::Nil);
    assert_eq!(eval_test("(get nil :a)"), Value::Nil);
}

#[test]
fn eval_assoc() {
    let result = eval_test("(assoc {:a 1} :b 2)");
    match result {
        Value::Map(m) => {
            assert_eq!(m.len(), 2);
        }
        _ => panic!("expected map"),
    }
}

#[test]
fn eval_dissoc() {
    let result = eval_test("(dissoc {:a 1 :b 2} :a)");
    match result {
        Value::Map(m) => {
            assert_eq!(m.len(), 1);
        }
        _ => panic!("expected map"),
    }
}

#[test]
fn eval_contains() {
    assert_eq!(eval_test("(contains? {:a 1} :a)"), Value::Bool(true));
    assert_eq!(eval_test("(contains? {:a 1} :b)"), Value::Bool(false));
    assert_eq!(eval_test("(contains? #{1 2 3} 2)"), Value::Bool(true));
    assert_eq!(eval_test("(contains? #{1 2 3} 4)"), Value::Bool(false));
}

#[test]
fn eval_keys() {
    let result = eval_test("(keys {:a 1 :b 2})");
    match result {
        Value::Vec(v) => {
            assert_eq!(v.len(), 2);
        }
        _ => panic!("expected vector"),
    }
}

#[test]
fn eval_vals() {
    let result = eval_test("(vals {:a 1 :b 2})");
    match result {
        Value::Vec(v) => {
            assert_eq!(v.len(), 2);
        }
        _ => panic!("expected vector"),
    }
}

#[test]
fn eval_str() {
    assert_eq!(
        eval_test(r#"(str "hello" " " "world")"#),
        Value::String("hello world".into())
    );
    assert_eq!(eval_test("(str 1 2 3)"), Value::String("123".into()));
}

#[test]
fn eval_str_len() {
    assert_eq!(eval_test(r#"(str/len "hello")"#), Value::Int(5));
    assert_eq!(eval_test(r#"(str/len "")"#), Value::Int(0));
}

#[test]
fn eval_str_upper() {
    assert_eq!(
        eval_test(r#"(str/upper "hello")"#),
        Value::String("HELLO".into())
    );
}

#[test]
fn eval_str_lower() {
    assert_eq!(
        eval_test(r#"(str/lower "HELLO")"#),
        Value::String("hello".into())
    );
}

#[test]
fn eval_abs() {
    assert_eq!(eval_test("(abs -5)"), Value::Int(5));
    assert_eq!(eval_test("(abs 5)"), Value::Int(5));
    assert!(matches!(eval_test("(abs -3.14)"), Value::Float(f) if (f - 3.14).abs() < 0.001));
}

#[test]
fn eval_min() {
    assert_eq!(eval_test("(min 3 1 2)"), Value::Int(1));
    assert!(matches!(eval_test("(min 1.5 2.5)"), Value::Float(f) if (f - 1.5).abs() < 0.001));
}

#[test]
fn eval_max() {
    assert_eq!(eval_test("(max 3 1 2)"), Value::Int(3));
    assert!(matches!(eval_test("(max 1.5 2.5)"), Value::Float(f) if (f - 2.5).abs() < 0.001));
}

#[test]
fn eval_floor() {
    assert!(matches!(eval_test("(floor 3.7)"), Value::Float(f) if (f - 3.0).abs() < 0.001));
    assert_eq!(eval_test("(floor 3)"), Value::Int(3));
}

#[test]
fn eval_ceil() {
    assert!(matches!(eval_test("(ceil 3.2)"), Value::Float(f) if (f - 4.0).abs() < 0.001));
    assert_eq!(eval_test("(ceil 3)"), Value::Int(3));
}

#[test]
fn eval_round() {
    assert!(matches!(eval_test("(round 3.7)"), Value::Float(f) if (f - 4.0).abs() < 0.001));
    assert!(matches!(eval_test("(round 3.2)"), Value::Float(f) if (f - 3.0).abs() < 0.001));
}

#[test]
fn eval_sqrt() {
    assert!(matches!(eval_test("(sqrt 4)"), Value::Float(f) if (f - 2.0).abs() < 0.001));
    assert!(matches!(eval_test("(sqrt 2.0)"), Value::Float(f) if (f - 1.414).abs() < 0.01));
}

#[test]
fn eval_type() {
    assert_eq!(eval_test("(type nil)"), Value::String(":nil".into()));
    assert_eq!(eval_test("(type 42)"), Value::String(":int".into()));
    assert_eq!(eval_test("(type 3.14)"), Value::String(":float".into()));
    assert_eq!(eval_test("(type true)"), Value::String(":bool".into()));
    assert_eq!(
        eval_test(r#"(type "hello")"#),
        Value::String(":string".into())
    );
    assert_eq!(eval_test("(type [1 2])"), Value::String(":vector".into()));
    assert_eq!(eval_test("(type {:a 1})"), Value::String(":map".into()));
    assert_eq!(eval_test("(type #{1})"), Value::String(":set".into()));
}

// =========================================================================
// User-Defined Function Tests
// =========================================================================

#[test]
fn eval_fn_simple() {
    // Define and immediately call a function
    let result = eval_test("((fn [x] x) 42)");
    assert_eq!(result, Value::Int(42));
}

#[test]
fn eval_fn_with_body() {
    // Function with arithmetic in body
    let result = eval_test("((fn [x] (+ x 1)) 5)");
    assert_eq!(result, Value::Int(6));
}

#[test]
fn eval_fn_multiple_params() {
    // Function with two parameters
    let result = eval_test("((fn [a b] (+ a b)) 3 4)");
    assert_eq!(result, Value::Int(7));
}

#[test]
fn eval_fn_no_params() {
    // Function with no parameters
    let result = eval_test("((fn [] 42))");
    assert_eq!(result, Value::Int(42));
}

#[test]
fn eval_fn_nested_call() {
    // Nested function calls
    let result = eval_test("((fn [x] ((fn [y] (+ y 1)) x)) 10)");
    assert_eq!(result, Value::Int(11));
}

#[test]
fn eval_fn_with_let() {
    // Function with let binding
    let result = eval_test("((fn [x] (let [y 10] (+ x y))) 5)");
    assert_eq!(result, Value::Int(15));
}

#[test]
fn eval_fn_stored_in_let() {
    // Store function in let binding and call it
    let result = eval_test("(let [f (fn [x] (* x 2))] (f 5))");
    assert_eq!(result, Value::Int(10));
}

#[test]
fn eval_fn_higher_order() {
    // Function that takes a function and applies it
    let result =
        eval_test("(let [apply (fn [f x] (f x)) double (fn [n] (* n 2))] (apply double 7))");
    assert_eq!(result, Value::Int(14));
}

#[test]
fn eval_fn_recursive() {
    // Recursive function using let binding
    // The function captures itself from the outer scope
    let result = eval_test("(let [fact (fn [n] (if (<= n 1) 1 (* n (fact (- n 1)))))] (fact 5))");
    assert_eq!(result, Value::Int(120));
}

#[test]
fn eval_fn_multi_body() {
    // Function with multiple expressions in body (implicit do)
    let result = eval_test("((fn [x] (+ 1 1) (+ x 10)) 5)");
    assert_eq!(result, Value::Int(15));
}

// =========================================================================
// Closure Tests
// =========================================================================

#[test]
fn eval_closure_capture_single() {
    // Simple closure capturing one variable
    let result = eval_test("(let [x 10] ((fn [y] (+ x y)) 5))");
    assert_eq!(result, Value::Int(15));
}

#[test]
fn eval_closure_capture_multiple() {
    // Closure capturing multiple variables
    let result = eval_test("(let [a 1 b 2 c 3] ((fn [x] (+ a b c x)) 4))");
    assert_eq!(result, Value::Int(10));
}

#[test]
fn eval_closure_nested() {
    // Nested closures
    let result = eval_test("(let [x 10] (let [f (fn [y] (+ x y))] (f 5)))");
    assert_eq!(result, Value::Int(15));
}

#[test]
fn eval_closure_returned() {
    // Closure returned from function
    let result = eval_test(
        "(let [make-adder (fn [n] (fn [x] (+ n x)))] (let [add5 (make-adder 5)] (add5 10)))",
    );
    assert_eq!(result, Value::Int(15));
}

#[test]
fn eval_closure_counter() {
    // Closure creating a counter-like pattern
    let result = eval_test("(let [base 100] (let [f (fn [x] (+ base x))] (+ (f 1) (f 2))))");
    assert_eq!(result, Value::Int(203));
}

#[test]
fn eval_fn_fibonacci() {
    // Recursive Fibonacci
    let result =
        eval_test("(let [fib (fn [n] (if (<= n 1) n (+ (fib (- n 1)) (fib (- n 2)))))] (fib 10))");
    assert_eq!(result, Value::Int(55));
}
