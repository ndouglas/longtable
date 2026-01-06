//! Stack-based virtual machine for Longtable bytecode.
//!
//! The VM executes compiled bytecode and produces results.

#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::unused_self)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::redundant_closure_for_method_calls)]

use longtable_foundation::{Error, ErrorKind, LtMap, LtSet, LtVec, Result, Value};

use crate::compiler::CompiledProgram;
use crate::opcode::{Bytecode, Opcode};

/// Stack-based virtual machine.
pub struct Vm {
    /// Operand stack.
    stack: Vec<Value>,
    /// Local variable slots.
    locals: Vec<Value>,
    /// Pattern bindings (for rule execution).
    bindings: Vec<Value>,
    /// Instruction pointer.
    ip: usize,
    /// Output from print statements.
    output: Vec<String>,
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

impl Vm {
    /// Creates a new VM.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stack: Vec::with_capacity(256),
            locals: vec![Value::Nil; 256],
            bindings: Vec::new(),
            ip: 0,
            output: Vec::new(),
        }
    }

    /// Resets the VM state.
    pub fn reset(&mut self) {
        self.stack.clear();
        self.locals.fill(Value::Nil);
        self.bindings.clear();
        self.ip = 0;
        self.output.clear();
    }

    /// Sets pattern bindings for rule execution.
    pub fn set_bindings(&mut self, bindings: Vec<Value>) {
        self.bindings = bindings;
    }

    /// Returns the output from print statements.
    #[must_use]
    pub fn output(&self) -> &[String] {
        &self.output
    }

    /// Clears the output buffer.
    pub fn clear_output(&mut self) {
        self.output.clear();
    }

    /// Executes a compiled program and returns the result.
    pub fn execute(&mut self, program: &CompiledProgram) -> Result<Value> {
        self.execute_bytecode(&program.code, &program.constants)
    }

    /// Executes bytecode with a constants pool.
    pub fn execute_bytecode(&mut self, code: &Bytecode, constants: &[Value]) -> Result<Value> {
        self.ip = 0;

        while self.ip < code.ops.len() {
            let op = &code.ops[self.ip];
            self.ip += 1;

            match op {
                Opcode::Nop => {}

                Opcode::Const(idx) => {
                    let value = constants.get(*idx as usize).cloned().unwrap_or(Value::Nil);
                    self.push(value);
                }

                Opcode::Pop => {
                    self.pop()?;
                }

                Opcode::Dup => {
                    let value = self.peek()?.clone();
                    self.push(value);
                }

                // Arithmetic
                Opcode::Add => self.binary_op(|a, b| add_values(a, b))?,
                Opcode::Sub => self.binary_op(|a, b| sub_values(a, b))?,
                Opcode::Mul => self.binary_op(|a, b| mul_values(a, b))?,
                Opcode::Div => self.binary_op(|a, b| div_values(a, b))?,
                Opcode::Mod => self.binary_op(|a, b| mod_values(a, b))?,
                Opcode::Neg => {
                    let a = self.pop()?;
                    self.push(neg_value(a)?);
                }

                // Comparison
                Opcode::Eq => self.binary_op(|a, b| Ok(Value::Bool(a == b)))?,
                Opcode::Ne => self.binary_op(|a, b| Ok(Value::Bool(a != b)))?,
                Opcode::Lt => self.binary_op(|a, b| compare_values(a, b, |ord| ord.is_lt()))?,
                Opcode::Le => self.binary_op(|a, b| compare_values(a, b, |ord| ord.is_le()))?,
                Opcode::Gt => self.binary_op(|a, b| compare_values(a, b, |ord| ord.is_gt()))?,
                Opcode::Ge => self.binary_op(|a, b| compare_values(a, b, |ord| ord.is_ge()))?,

                // Logic
                Opcode::Not => {
                    let a = self.pop()?;
                    self.push(Value::Bool(!is_truthy(&a)));
                }
                Opcode::And => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(Value::Bool(is_truthy(&a) && is_truthy(&b)));
                }
                Opcode::Or => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(Value::Bool(is_truthy(&a) || is_truthy(&b)));
                }

                // Control flow
                Opcode::Jump(offset) => {
                    self.ip = ((self.ip as i32) + (*offset as i32)) as usize;
                }
                Opcode::JumpIf(offset) => {
                    let cond = self.pop()?;
                    if is_truthy(&cond) {
                        self.ip = ((self.ip as i32) + (*offset as i32)) as usize;
                    }
                }
                Opcode::JumpIfNot(offset) => {
                    let cond = self.pop()?;
                    if !is_truthy(&cond) {
                        self.ip = ((self.ip as i32) + (*offset as i32)) as usize;
                    }
                }
                Opcode::Call(_) => {
                    // User-defined functions not yet supported
                    return Err(Error::new(ErrorKind::Internal(
                        "user-defined functions not yet supported".to_string(),
                    )));
                }
                Opcode::CallNative(idx) => {
                    self.call_native(*idx)?;
                }
                Opcode::Return => {
                    // Return top of stack
                    break;
                }

                // Variables
                Opcode::LoadLocal(slot) => {
                    let value = self.locals[*slot as usize].clone();
                    self.push(value);
                }
                Opcode::StoreLocal(slot) => {
                    let value = self.pop()?;
                    self.locals[*slot as usize] = value;
                }
                Opcode::LoadBinding(idx) => {
                    let value = self
                        .bindings
                        .get(*idx as usize)
                        .cloned()
                        .unwrap_or(Value::Nil);
                    self.push(value);
                }

                // Data access (World operations - not yet implemented)
                Opcode::GetComponent | Opcode::GetField => {
                    return Err(Error::new(ErrorKind::Internal(
                        "world operations not yet supported".to_string(),
                    )));
                }

                // Effects (not yet implemented)
                Opcode::Spawn
                | Opcode::Destroy
                | Opcode::SetComponent
                | Opcode::SetField
                | Opcode::Link
                | Opcode::Unlink => {
                    return Err(Error::new(ErrorKind::Internal(
                        "effect operations not yet supported".to_string(),
                    )));
                }

                // Collections
                Opcode::VecNew => {
                    self.push(Value::Vec(LtVec::new()));
                }
                Opcode::VecPush => {
                    let value = self.pop()?;
                    let vec = self.pop()?;
                    match vec {
                        Value::Vec(v) => {
                            self.push(Value::Vec(v.push_back(value)));
                        }
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Vec(Box::new(
                                    longtable_foundation::Type::Any,
                                )),
                                actual: vec.value_type(),
                            }));
                        }
                    }
                }
                Opcode::VecGet => {
                    let idx = self.pop()?;
                    let vec = self.pop()?;
                    match (&vec, &idx) {
                        (Value::Vec(v), Value::Int(i)) => {
                            let value = v.get(*i as usize).cloned().unwrap_or(Value::Nil);
                            self.push(value);
                        }
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Vec(Box::new(
                                    longtable_foundation::Type::Any,
                                )),
                                actual: vec.value_type(),
                            }));
                        }
                    }
                }
                Opcode::VecLen => {
                    let vec = self.pop()?;
                    match vec {
                        Value::Vec(v) => {
                            self.push(Value::Int(v.len() as i64));
                        }
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Vec(Box::new(
                                    longtable_foundation::Type::Any,
                                )),
                                actual: vec.value_type(),
                            }));
                        }
                    }
                }

                Opcode::MapNew => {
                    self.push(Value::Map(LtMap::new()));
                }
                Opcode::MapInsert => {
                    let value = self.pop()?;
                    let key = self.pop()?;
                    let map = self.pop()?;
                    match map {
                        Value::Map(m) => {
                            self.push(Value::Map(m.insert(key, value)));
                        }
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Map(
                                    Box::new(longtable_foundation::Type::Any),
                                    Box::new(longtable_foundation::Type::Any),
                                ),
                                actual: map.value_type(),
                            }));
                        }
                    }
                }
                Opcode::MapGet => {
                    let key = self.pop()?;
                    let map = self.pop()?;
                    match map {
                        Value::Map(m) => {
                            let value = m.get(&key).cloned().unwrap_or(Value::Nil);
                            self.push(value);
                        }
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Map(
                                    Box::new(longtable_foundation::Type::Any),
                                    Box::new(longtable_foundation::Type::Any),
                                ),
                                actual: map.value_type(),
                            }));
                        }
                    }
                }
                Opcode::MapContains => {
                    let key = self.pop()?;
                    let map = self.pop()?;
                    match map {
                        Value::Map(m) => {
                            self.push(Value::Bool(m.contains_key(&key)));
                        }
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Map(
                                    Box::new(longtable_foundation::Type::Any),
                                    Box::new(longtable_foundation::Type::Any),
                                ),
                                actual: map.value_type(),
                            }));
                        }
                    }
                }

                Opcode::SetNew => {
                    self.push(Value::Set(LtSet::new()));
                }
                Opcode::SetInsert => {
                    let value = self.pop()?;
                    let set = self.pop()?;
                    match set {
                        Value::Set(s) => {
                            self.push(Value::Set(s.insert(value)));
                        }
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Set(Box::new(
                                    longtable_foundation::Type::Any,
                                )),
                                actual: set.value_type(),
                            }));
                        }
                    }
                }
                Opcode::SetContains => {
                    let value = self.pop()?;
                    let set = self.pop()?;
                    match set {
                        Value::Set(s) => {
                            self.push(Value::Bool(s.contains(&value)));
                        }
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Set(Box::new(
                                    longtable_foundation::Type::Any,
                                )),
                                actual: set.value_type(),
                            }));
                        }
                    }
                }

                Opcode::Print => {
                    let value = self.pop()?;
                    self.output.push(format_value(&value));
                }
            }
        }

        // Return top of stack or nil
        Ok(self.stack.pop().unwrap_or(Value::Nil))
    }

    /// Pushes a value onto the stack.
    fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    /// Pops a value from the stack.
    fn pop(&mut self) -> Result<Value> {
        self.stack
            .pop()
            .ok_or_else(|| Error::new(ErrorKind::Internal("stack underflow".to_string())))
    }

    /// Peeks at the top of the stack.
    fn peek(&self) -> Result<&Value> {
        self.stack
            .last()
            .ok_or_else(|| Error::new(ErrorKind::Internal("stack underflow".to_string())))
    }

    /// Applies a binary operation.
    fn binary_op<F>(&mut self, op: F) -> Result<()>
    where
        F: FnOnce(Value, Value) -> Result<Value>,
    {
        let b = self.pop()?;
        let a = self.pop()?;
        let result = op(a, b)?;
        self.push(result);
        Ok(())
    }

    /// Calls a native function.
    fn call_native(&mut self, _idx: u16) -> Result<()> {
        // Native functions not yet implemented
        Err(Error::new(ErrorKind::Internal(
            "native functions not yet implemented".to_string(),
        )))
    }
}

/// Checks if a value is truthy.
fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Nil => false,
        Value::Bool(b) => *b,
        _ => true,
    }
}

/// Adds two values.
fn add_values(a: Value, b: Value) -> Result<Value> {
    match (&a, &b) {
        (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x + y)),
        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x + y)),
        (Value::Int(x), Value::Float(y)) => Ok(Value::Float(*x as f64 + y)),
        (Value::Float(x), Value::Int(y)) => Ok(Value::Float(x + *y as f64)),
        (Value::String(x), Value::String(y)) => Ok(Value::String(format!("{x}{y}").into())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
            actual: a.value_type(),
        })),
    }
}

/// Subtracts two values.
fn sub_values(a: Value, b: Value) -> Result<Value> {
    match (&a, &b) {
        (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x - y)),
        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x - y)),
        (Value::Int(x), Value::Float(y)) => Ok(Value::Float(*x as f64 - y)),
        (Value::Float(x), Value::Int(y)) => Ok(Value::Float(x - *y as f64)),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
            actual: a.value_type(),
        })),
    }
}

/// Multiplies two values.
fn mul_values(a: Value, b: Value) -> Result<Value> {
    match (&a, &b) {
        (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x * y)),
        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x * y)),
        (Value::Int(x), Value::Float(y)) => Ok(Value::Float(*x as f64 * y)),
        (Value::Float(x), Value::Int(y)) => Ok(Value::Float(x * *y as f64)),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
            actual: a.value_type(),
        })),
    }
}

/// Divides two values.
fn div_values(a: Value, b: Value) -> Result<Value> {
    match (&a, &b) {
        (Value::Int(_) | Value::Float(_), Value::Int(0)) => {
            Err(Error::new(ErrorKind::DivisionByZero))
        }
        (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x / y)),
        (Value::Float(x), Value::Float(y)) => {
            if *y == 0.0 {
                Err(Error::new(ErrorKind::DivisionByZero))
            } else {
                Ok(Value::Float(x / y))
            }
        }
        (Value::Int(x), Value::Float(y)) => {
            if *y == 0.0 {
                Err(Error::new(ErrorKind::DivisionByZero))
            } else {
                Ok(Value::Float(*x as f64 / y))
            }
        }
        (Value::Float(x), Value::Int(y)) => Ok(Value::Float(x / *y as f64)),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
            actual: a.value_type(),
        })),
    }
}

/// Modulo of two values.
fn mod_values(a: Value, b: Value) -> Result<Value> {
    match (&a, &b) {
        (Value::Int(_), Value::Int(0)) => Err(Error::new(ErrorKind::DivisionByZero)),
        (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x % y)),
        (Value::Float(x), Value::Float(y)) => {
            if *y == 0.0 {
                Err(Error::new(ErrorKind::DivisionByZero))
            } else {
                Ok(Value::Float(x % y))
            }
        }
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
            actual: a.value_type(),
        })),
    }
}

/// Negates a value.
fn neg_value(a: Value) -> Result<Value> {
    match a {
        Value::Int(x) => Ok(Value::Int(-x)),
        Value::Float(x) => Ok(Value::Float(-x)),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
            actual: a.value_type(),
        })),
    }
}

/// Compares two values with the given predicate.
fn compare_values<F>(a: Value, b: Value, pred: F) -> Result<Value>
where
    F: FnOnce(std::cmp::Ordering) -> bool,
{
    let ord = match (&a, &b) {
        (Value::Int(x), Value::Int(y)) => x.cmp(y),
        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
        (Value::Int(x), Value::Float(y)) => (*x as f64)
            .partial_cmp(y)
            .unwrap_or(std::cmp::Ordering::Equal),
        (Value::Float(x), Value::Int(y)) => x
            .partial_cmp(&(*y as f64))
            .unwrap_or(std::cmp::Ordering::Equal),
        (Value::String(x), Value::String(y)) => x.cmp(y),
        _ => {
            return Err(Error::new(ErrorKind::TypeMismatch {
                expected: longtable_foundation::Type::Int,
                actual: a.value_type(),
            }));
        }
    };
    Ok(Value::Bool(pred(ord)))
}

/// Formats a value for display.
fn format_value(value: &Value) -> String {
    match value {
        Value::Nil => "nil".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Float(n) => {
            if n.fract() == 0.0 {
                format!("{n}.0")
            } else {
                n.to_string()
            }
        }
        Value::String(s) => s.to_string(),
        Value::Symbol(id) => format!("Symbol({})", id.index()),
        Value::Keyword(id) => format!("Keyword({})", id.index()),
        Value::EntityRef(id) => format!("Entity({}, {})", id.index, id.generation),
        Value::Vec(v) => {
            let items: Vec<_> = v.iter().map(format_value).collect();
            format!("[{}]", items.join(" "))
        }
        Value::Set(s) => {
            let items: Vec<_> = s.iter().map(format_value).collect();
            format!("#{{{}}}", items.join(" "))
        }
        Value::Map(m) => {
            let pairs: Vec<_> = m
                .iter()
                .map(|(k, v)| format!("{} {}", format_value(k), format_value(v)))
                .collect();
            format!("{{{}}}", pairs.join(" "))
        }
        Value::Fn(_) => "<fn>".to_string(),
    }
}

/// Evaluates source code and returns the result.
pub fn eval(source: &str) -> Result<Value> {
    let program = crate::compiler::compile(source)?;
    let mut vm = Vm::new();
    vm.execute(&program)
}

#[cfg(test)]
mod tests {
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
}
