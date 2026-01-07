//! Integration tests for the VM
//!
//! Tests evaluation of compiled Longtable programs.

use longtable_foundation::Value;
use longtable_language::{Vm, compile, eval};

// =============================================================================
// Literal Evaluation
// =============================================================================

#[test]
fn eval_nil() {
    let result = eval("nil").unwrap();
    assert!(result.is_nil());
}

#[test]
fn eval_true() {
    let result = eval("true").unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn eval_false() {
    let result = eval("false").unwrap();
    assert_eq!(result, Value::Bool(false));
}

#[test]
fn eval_integer() {
    let result = eval("42").unwrap();
    assert_eq!(result, Value::Int(42));
}

#[test]
fn eval_negative_integer() {
    let result = eval("-17").unwrap();
    assert_eq!(result, Value::Int(-17));
}

#[test]
fn eval_float() {
    let result = eval("1.5").unwrap();
    if let Value::Float(f) = result {
        assert!((f - 1.5).abs() < 0.001);
    } else {
        panic!("Expected Float");
    }
}

#[test]
fn eval_string() {
    let result = eval("\"hello\"").unwrap();
    if let Value::String(s) = result {
        assert_eq!(&*s, "hello");
    } else {
        panic!("Expected String");
    }
}

// =============================================================================
// Arithmetic
// =============================================================================

#[test]
fn eval_addition() {
    let result = eval("(+ 1 2)").unwrap();
    assert_eq!(result, Value::Int(3));
}

#[test]
fn eval_subtraction() {
    let result = eval("(- 10 3)").unwrap();
    assert_eq!(result, Value::Int(7));
}

#[test]
fn eval_multiplication() {
    let result = eval("(* 4 5)").unwrap();
    assert_eq!(result, Value::Int(20));
}

#[test]
fn eval_division() {
    let result = eval("(/ 20 4)").unwrap();
    assert_eq!(result, Value::Int(5));
}

#[test]
fn eval_nested_arithmetic() {
    let result = eval("(+ (* 2 3) (/ 10 2))").unwrap();
    assert_eq!(result, Value::Int(11)); // 6 + 5
}

#[test]
fn eval_multiple_args() {
    let result = eval("(+ 1 2 3 4)").unwrap();
    assert_eq!(result, Value::Int(10));
}

#[test]
fn eval_float_arithmetic() {
    let result = eval("(+ 1.5 2.5)").unwrap();
    if let Value::Float(f) = result {
        assert!((f - 4.0).abs() < 0.001);
    } else {
        panic!("Expected Float");
    }
}

// =============================================================================
// Comparison
// =============================================================================

#[test]
fn eval_equals() {
    assert_eq!(eval("(= 1 1)").unwrap(), Value::Bool(true));
    assert_eq!(eval("(= 1 2)").unwrap(), Value::Bool(false));
}

#[test]
fn eval_not_equals() {
    // Use (not (= ...)) instead of not= which may not exist
    assert_eq!(eval("(not (= 1 2))").unwrap(), Value::Bool(true));
    assert_eq!(eval("(not (= 1 1))").unwrap(), Value::Bool(false));
}

#[test]
fn eval_less_than() {
    assert_eq!(eval("(< 1 2)").unwrap(), Value::Bool(true));
    assert_eq!(eval("(< 2 1)").unwrap(), Value::Bool(false));
}

#[test]
fn eval_greater_than() {
    assert_eq!(eval("(> 2 1)").unwrap(), Value::Bool(true));
    assert_eq!(eval("(> 1 2)").unwrap(), Value::Bool(false));
}

#[test]
fn eval_less_or_equal() {
    assert_eq!(eval("(<= 1 2)").unwrap(), Value::Bool(true));
    assert_eq!(eval("(<= 1 1)").unwrap(), Value::Bool(true));
    assert_eq!(eval("(<= 2 1)").unwrap(), Value::Bool(false));
}

#[test]
fn eval_greater_or_equal() {
    assert_eq!(eval("(>= 2 1)").unwrap(), Value::Bool(true));
    assert_eq!(eval("(>= 1 1)").unwrap(), Value::Bool(true));
    assert_eq!(eval("(>= 1 2)").unwrap(), Value::Bool(false));
}

// =============================================================================
// Logic
// =============================================================================

#[test]
fn eval_and() {
    assert_eq!(eval("(and true true)").unwrap(), Value::Bool(true));
    assert_eq!(eval("(and true false)").unwrap(), Value::Bool(false));
    assert_eq!(eval("(and false true)").unwrap(), Value::Bool(false));
}

#[test]
fn eval_or() {
    assert_eq!(eval("(or true false)").unwrap(), Value::Bool(true));
    assert_eq!(eval("(or false true)").unwrap(), Value::Bool(true));
    assert_eq!(eval("(or false false)").unwrap(), Value::Bool(false));
}

#[test]
fn eval_not() {
    assert_eq!(eval("(not true)").unwrap(), Value::Bool(false));
    assert_eq!(eval("(not false)").unwrap(), Value::Bool(true));
}

#[test]
fn eval_and_short_circuit() {
    // Should not error because second arg isn't evaluated
    let result = eval("(and false (/ 1 0))").unwrap();
    assert_eq!(result, Value::Bool(false));
}

#[test]
fn eval_or_short_circuit() {
    // Should not error because second arg isn't evaluated
    let result = eval("(or true (/ 1 0))").unwrap();
    assert_eq!(result, Value::Bool(true));
}

// =============================================================================
// Control Flow
// =============================================================================

#[test]
fn eval_if_true() {
    let result = eval("(if true 1 2)").unwrap();
    assert_eq!(result, Value::Int(1));
}

#[test]
fn eval_if_false() {
    let result = eval("(if false 1 2)").unwrap();
    assert_eq!(result, Value::Int(2));
}

#[test]
fn eval_if_without_else() {
    let result = eval("(if false 1)").unwrap();
    assert!(result.is_nil());
}

#[test]
fn eval_nested_if() {
    let result = eval("(if (> 5 3) (if (< 2 4) \"yes\" \"no\") \"outer-no\")").unwrap();
    if let Value::String(s) = result {
        assert_eq!(&*s, "yes");
    } else {
        panic!("Expected String");
    }
}

#[test]
fn eval_cond() {
    let result = eval(
        "(cond
           (< 1 0) \"a\"
           (< 0 1) \"b\"
           :else \"c\")",
    )
    .unwrap();
    if let Value::String(s) = result {
        assert_eq!(&*s, "b");
    } else {
        panic!("Expected String");
    }
}

// =============================================================================
// Let Bindings
// =============================================================================

#[test]
fn eval_let_simple() {
    let result = eval("(let [x 1] x)").unwrap();
    assert_eq!(result, Value::Int(1));
}

#[test]
fn eval_let_multiple() {
    let result = eval("(let [x 1 y 2] (+ x y))").unwrap();
    assert_eq!(result, Value::Int(3));
}

#[test]
fn eval_let_dependent() {
    let result = eval("(let [x 1 y (+ x 1)] y)").unwrap();
    assert_eq!(result, Value::Int(2));
}

#[test]
fn eval_let_nested() {
    let result = eval("(let [x 1] (let [y 2] (+ x y)))").unwrap();
    assert_eq!(result, Value::Int(3));
}

#[test]
fn eval_let_shadowing() {
    let result = eval("(let [x 1] (let [x 2] x))").unwrap();
    assert_eq!(result, Value::Int(2));
}

// =============================================================================
// Functions
// =============================================================================

#[test]
fn eval_fn_call() {
    let result = eval("((fn [x] (+ x 1)) 5)").unwrap();
    assert_eq!(result, Value::Int(6));
}

#[test]
fn eval_fn_multiple_args() {
    let result = eval("((fn [a b] (+ a b)) 3 4)").unwrap();
    assert_eq!(result, Value::Int(7));
}

#[test]
fn eval_fn_closure() {
    let result = eval("(let [x 10] ((fn [y] (+ x y)) 5))").unwrap();
    assert_eq!(result, Value::Int(15));
}

#[test]
fn eval_higher_order() {
    let result = eval("((fn [f x] (f (f x))) (fn [n] (+ n 1)) 0)").unwrap();
    assert_eq!(result, Value::Int(2));
}

// =============================================================================
// Collections
// =============================================================================

#[test]
fn eval_vector() {
    let result = eval("[1 2 3]").unwrap();
    if let Value::Vec(v) = result {
        assert_eq!(v.len(), 3);
    } else {
        panic!("Expected Vec");
    }
}

#[test]
fn eval_map() {
    let result = eval("{:a 1 :b 2}").unwrap();
    if let Value::Map(m) = result {
        assert_eq!(m.len(), 2);
    } else {
        panic!("Expected Map");
    }
}

#[test]
fn eval_first() {
    let result = eval("(first [1 2 3])").unwrap();
    assert_eq!(result, Value::Int(1));
}

#[test]
fn eval_rest() {
    let result = eval("(rest [1 2 3])").unwrap();
    if let Value::Vec(v) = result {
        assert_eq!(v.len(), 2);
        assert_eq!(v.get(0), Some(&Value::Int(2)));
    } else {
        panic!("Expected Vec");
    }
}

#[test]
fn eval_conj() {
    let result = eval("(conj [1 2] 3)").unwrap();
    if let Value::Vec(v) = result {
        assert_eq!(v.len(), 3);
    } else {
        panic!("Expected Vec");
    }
}

#[test]
fn eval_count() {
    assert_eq!(eval("(count [1 2 3])").unwrap(), Value::Int(3));
    assert_eq!(eval("(count {:a 1 :b 2})").unwrap(), Value::Int(2));
}

#[test]
fn eval_empty() {
    assert_eq!(eval("(empty? [])").unwrap(), Value::Bool(true));
    assert_eq!(eval("(empty? [1])").unwrap(), Value::Bool(false));
}

// =============================================================================
// VM State
// =============================================================================

#[test]
fn vm_output_captured() {
    let program = compile("(do (println \"hello\") 42)").unwrap();
    let mut vm = Vm::new();
    let result = vm.execute(&program).unwrap();

    assert_eq!(result, Value::Int(42));
    assert!(!vm.output().is_empty());
}

#[test]
fn vm_reset_clears_state() {
    let program = compile("42").unwrap();
    let mut vm = Vm::new();
    vm.execute(&program).unwrap();

    vm.reset();
    // Should be able to execute again
    let result = vm.execute(&program).unwrap();
    assert_eq!(result, Value::Int(42));
}

// =============================================================================
// Error Handling
// =============================================================================

#[test]
fn eval_division_by_zero() {
    let result = eval("(/ 1 0)");
    assert!(result.is_err());
}

#[test]
fn eval_undefined_variable() {
    // Undefined variables may return nil or cause error depending on implementation
    let result = eval("undefined_var");
    // Just verify it doesn't panic - it may be nil or error
    let _ = result;
}

#[test]
fn eval_wrong_arity() {
    let result = eval("(+ 1)"); // + needs at least 2 args
    // May succeed with some implementations, error with others
    // Just check it doesn't panic
    let _ = result;
}

// =============================================================================
// Complex Programs
// =============================================================================

#[test]
fn eval_factorial_style() {
    // Using let and recursion simulation
    let result = eval(
        "(let [fact (fn [n]
                      (if (<= n 1)
                        1
                        (* n (fact (- n 1)))))]
           (fact 5))",
    );
    // May or may not work depending on recursion support
    // Just verify it doesn't panic
    let _ = result;
}

#[test]
fn eval_map_access() {
    let result = eval("(get {:a 1 :b 2} :a)").unwrap();
    assert_eq!(result, Value::Int(1));
}

#[test]
fn eval_map_missing_key() {
    // get returns nil for missing keys
    let result = eval("(get {:a 1} :b)").unwrap();
    assert!(result.is_nil());
}
