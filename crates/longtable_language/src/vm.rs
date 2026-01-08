//! Stack-based virtual machine for Longtable bytecode.
//!
//! The VM executes compiled bytecode and produces results.
//!
//! # World Access
//!
//! The VM can optionally access a World via the [`VmContext`] trait. This enables
//! execution of ECS operations (reading components, spawning entities, etc.).
//! When no context is provided, the VM operates in "pure evaluation" mode where
//! World operations will return errors.
//!
//! Effects (mutations) are collected during execution and can be retrieved via
//! [`Vm::take_effects`]. The caller is responsible for applying these effects
//! to the World.

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

mod context;
mod native;
#[cfg(test)]
mod tests;

pub use context::{RuntimeContext, VmContext, VmEffect, WorldContext};

use context::NoContext;
use native::{
    add_values, compare_values, div_values, format_value, is_truthy, mod_values, mul_values,
    native_abs, native_acos, native_and, native_asin, native_assoc, native_atan, native_atan2,
    native_bool_p, native_cbrt, native_ceil, native_clamp, native_coll_p, native_concat,
    native_conj, native_cons, native_contains_p, native_cos, native_cosh, native_count, native_dec,
    native_dedupe, native_dissoc, native_distinct, native_drop, native_e, native_empty_p,
    native_entity_p, native_exp, native_first, native_flatten, native_float_p, native_floor,
    native_fn_p, native_format, native_get, native_inc, native_int_p, native_interleave,
    native_interpose, native_into, native_keys, native_keyword_p, native_last, native_list_p,
    native_log, native_log2, native_log10, native_map_p, native_max, native_merge, native_min,
    native_nil_p, native_nth, native_number_p, native_or, native_partition, native_partition_all,
    native_pi, native_pow, native_range, native_rem, native_repeat, native_rest, native_reverse,
    native_round, native_set, native_set_p, native_sin, native_sinh, native_some_p, native_sort,
    native_sqrt, native_str, native_str_blank, native_str_contains, native_str_ends_with,
    native_str_join, native_str_len, native_str_lower, native_str_replace, native_str_replace_all,
    native_str_split, native_str_starts_with, native_str_substring, native_str_trim,
    native_str_trim_left, native_str_trim_right, native_str_upper, native_string_p,
    native_symbol_p, native_take, native_tan, native_tanh, native_trunc, native_type, native_vals,
    native_vec, native_vec_add, native_vec_angle, native_vec_cross, native_vec_distance,
    native_vec_dot, native_vec_length, native_vec_length_sq, native_vec_lerp, native_vec_mul,
    native_vec_normalize, native_vec_scale, native_vec_sub, native_vector_p, native_zip, neg_value,
    sub_values,
};

use longtable_foundation::{
    EntityId, Error, ErrorKind, KeywordId, LtMap, LtSet, LtVec, Result, Value,
};

use crate::compiler::CompiledProgram;
use crate::opcode::{Bytecode, Opcode};

/// Stack-based virtual machine.
pub struct Vm {
    /// Operand stack.
    stack: Vec<Value>,
    /// Local variable slots.
    locals: Vec<Value>,
    /// Global variable slots (persists across executions).
    globals: Vec<Value>,
    /// Pattern bindings (for rule execution).
    bindings: Vec<Value>,
    /// Captured values for current closure execution.
    captures: Vec<Value>,
    /// Instruction pointer.
    ip: usize,
    /// Output from print statements.
    output: Vec<String>,
    /// Collected effects from execution.
    effects: Vec<VmEffect>,
    /// Counter for spawned entities (used for temporary IDs).
    spawn_counter: u64,
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
            globals: vec![Value::Nil; 1024], // More globals than locals
            bindings: Vec::new(),
            captures: Vec::new(),
            ip: 0,
            output: Vec::new(),
            effects: Vec::new(),
            spawn_counter: 0,
        }
    }

    /// Resets the VM state.
    pub fn reset(&mut self) {
        self.stack.clear();
        self.locals.fill(Value::Nil);
        self.bindings.clear();
        self.captures.clear();
        self.ip = 0;
        self.output.clear();
        self.effects.clear();
        self.spawn_counter = 0;
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

    /// Returns the collected effects from execution.
    #[must_use]
    pub fn effects(&self) -> &[VmEffect] {
        &self.effects
    }

    /// Takes and clears the collected effects.
    pub fn take_effects(&mut self) -> Vec<VmEffect> {
        std::mem::take(&mut self.effects)
    }

    /// Clears the effects buffer.
    pub fn clear_effects(&mut self) {
        self.effects.clear();
    }

    /// Executes a compiled program and returns the result.
    pub fn execute(&mut self, program: &CompiledProgram) -> Result<Value> {
        self.execute_internal::<NoContext>(
            &program.code,
            &program.constants,
            &program.functions,
            None,
        )
    }

    /// Executes a compiled program with World context.
    pub fn execute_with_context<C: VmContext>(
        &mut self,
        program: &CompiledProgram,
        ctx: &C,
    ) -> Result<Value> {
        self.execute_internal(
            &program.code,
            &program.constants,
            &program.functions,
            Some(ctx),
        )
    }

    /// Executes bytecode with a constants pool (no functions available).
    pub fn execute_bytecode(&mut self, code: &Bytecode, constants: &[Value]) -> Result<Value> {
        self.execute_internal::<NoContext>(code, constants, &[], None)
    }

    /// Executes bytecode with optional World context.
    fn execute_internal<C: VmContext>(
        &mut self,
        code: &Bytecode,
        constants: &[Value],
        functions: &[crate::compiler::CompiledFunction],
        ctx: Option<&C>,
    ) -> Result<Value> {
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
                Opcode::Call(arg_count) => {
                    // Pop arguments in reverse order
                    let arg_count = *arg_count as usize;
                    let mut args = Vec::with_capacity(arg_count);
                    for _ in 0..arg_count {
                        args.push(self.pop()?);
                    }
                    args.reverse();

                    // Pop the function value
                    let func_val = self.pop()?;

                    // Extract function reference
                    let func_ref = match &func_val {
                        Value::Fn(longtable_foundation::LtFn::Compiled(f)) => f.clone(),
                        Value::String(s) if s.starts_with('\'') => {
                            // This is an undefined symbol that was compiled as a quoted string
                            let symbol_name = s.strip_prefix('\'').unwrap_or(s);
                            return Err(Error::new(ErrorKind::UndefinedSymbol(
                                symbol_name.to_string(),
                            )));
                        }
                        Value::String(s) => {
                            return Err(Error::new(ErrorKind::Internal(format!(
                                "cannot call string value as function: \"{s}\""
                            ))));
                        }
                        _ => {
                            return Err(Error::new(ErrorKind::Internal(format!(
                                "cannot call {} as function (value: {})",
                                func_val.value_type(),
                                func_val
                            ))));
                        }
                    };

                    // Look up the function
                    let func_idx = func_ref.index as usize;
                    let func = functions.get(func_idx).ok_or_else(|| {
                        Error::new(ErrorKind::Internal(format!(
                            "function index {func_idx} out of bounds"
                        )))
                    })?;

                    // Check arity
                    if args.len() != func.arity as usize {
                        return Err(Error::new(ErrorKind::Internal(format!(
                            "expected {} arguments, got {}",
                            func.arity,
                            args.len()
                        ))));
                    }

                    // Save current VM state
                    let saved_ip = self.ip;
                    let saved_locals: Vec<Value> = self.locals.clone();
                    let saved_captures = std::mem::take(&mut self.captures);

                    // Set up arguments as locals
                    for (i, arg) in args.into_iter().enumerate() {
                        self.locals[i] = arg;
                    }

                    // Set up captures from the function's closure
                    if let Some(caps) = &func_ref.captures {
                        self.captures.clone_from(&caps.lock().unwrap());
                    }

                    // Execute the function body
                    let result =
                        self.execute_internal::<C>(&func.code, constants, functions, ctx)?;

                    // Restore VM state
                    self.ip = saved_ip;
                    self.locals = saved_locals;
                    self.captures = saved_captures;

                    // Push result
                    self.push(result);
                }
                Opcode::CallNative(idx, arg_count) => {
                    self.call_native(*idx, *arg_count)?;
                }
                Opcode::Return => {
                    // Return top of stack
                    return self.pop();
                }

                // Variables
                Opcode::LoadLocal(slot) => {
                    let value = self
                        .locals
                        .get(*slot as usize)
                        .cloned()
                        .unwrap_or(Value::Nil);
                    self.push(value);
                }
                Opcode::StoreLocal(slot) => {
                    let value = self.pop()?;
                    let slot = *slot as usize;
                    if slot >= self.locals.len() {
                        self.locals.resize(slot + 1, Value::Nil);
                    }
                    self.locals[slot] = value;
                }
                Opcode::LoadGlobal(slot) => {
                    let value = self
                        .globals
                        .get(*slot as usize)
                        .cloned()
                        .unwrap_or(Value::Nil);
                    self.push(value);
                }
                Opcode::StoreGlobal(slot) => {
                    let value = self.pop()?;
                    let slot = *slot as usize;
                    if slot >= self.globals.len() {
                        self.globals.resize(slot + 1, Value::Nil);
                    }
                    self.globals[slot] = value;
                }
                Opcode::LoadBinding(idx) => {
                    let value = self
                        .bindings
                        .get(*idx as usize)
                        .cloned()
                        .unwrap_or(Value::Nil);
                    self.push(value);
                }
                Opcode::LoadCapture(idx) => {
                    let value = self
                        .captures
                        .get(*idx as usize)
                        .cloned()
                        .unwrap_or(Value::Nil);
                    self.push(value);
                }
                Opcode::MakeClosure(fn_index, capture_count) => {
                    // Pop captured values in reverse order
                    let capture_count = *capture_count as usize;
                    let mut captured = Vec::with_capacity(capture_count);
                    for _ in 0..capture_count {
                        captured.push(self.pop()?);
                    }
                    captured.reverse();

                    // Create function value with captures (using RefCell for mutability)
                    let fn_value = Value::Fn(longtable_foundation::LtFn::Compiled(
                        longtable_foundation::CompiledFn::with_captures(*fn_index, captured),
                    ));
                    self.push(fn_value);
                }
                Opcode::PatchCapture(capture_idx) => {
                    // Patch a closure's capture slot (for recursive closures)
                    // Stack: [closure, new_value] -> [closure]
                    let new_value = self.pop()?;
                    let closure_val = self.pop()?;

                    // Patch the capture slot
                    if let Value::Fn(longtable_foundation::LtFn::Compiled(ref func)) = closure_val {
                        func.patch_capture(*capture_idx as usize, new_value);
                    }

                    // Push the closure back
                    self.push(closure_val);
                }

                // World access
                Opcode::GetComponent => {
                    let component_val = self.pop()?;
                    let entity_val = self.pop()?;

                    let entity = extract_entity(&entity_val)?;
                    let component = extract_keyword(&component_val, ctx)?;

                    let result = if let Some(c) = ctx {
                        c.get_component(entity, component)?
                    } else {
                        None
                    };
                    self.push(result.unwrap_or(Value::Nil));
                }

                Opcode::GetField => {
                    let field_val = self.pop()?;
                    let component_val = self.pop()?;
                    let entity_val = self.pop()?;

                    let entity = extract_entity(&entity_val)?;
                    let component = extract_keyword(&component_val, ctx)?;
                    let field = extract_keyword(&field_val, ctx)?;

                    let result = if let Some(c) = ctx {
                        c.get_field(entity, component, field)?
                    } else {
                        None
                    };
                    self.push(result.unwrap_or(Value::Nil));
                }

                // Entity Search
                Opcode::WithComponent => {
                    let component_val = self.pop()?;
                    let component = extract_keyword(&component_val, ctx)?;

                    let entities: LtVec<Value> = if let Some(c) = ctx {
                        c.with_component(component)
                            .into_iter()
                            .map(Value::EntityRef)
                            .collect()
                    } else {
                        LtVec::new()
                    };
                    self.push(Value::Vec(entities));
                }

                Opcode::FindRelationships => {
                    let target_val = self.pop()?;
                    let source_val = self.pop()?;
                    let rel_type_val = self.pop()?;

                    let rel_type = match &rel_type_val {
                        Value::Nil => None,
                        _ => Some(extract_keyword(&rel_type_val, ctx)?),
                    };
                    let source = match &source_val {
                        Value::Nil => None,
                        _ => Some(extract_entity(&source_val)?),
                    };
                    let target = match &target_val {
                        Value::Nil => None,
                        _ => Some(extract_entity(&target_val)?),
                    };

                    let entities: LtVec<Value> = if let Some(c) = ctx {
                        c.find_relationships(rel_type, source, target)
                            .into_iter()
                            .map(Value::EntityRef)
                            .collect()
                    } else {
                        LtVec::new()
                    };
                    self.push(Value::Vec(entities));
                }

                Opcode::Targets => {
                    let rel_type_val = self.pop()?;
                    let source_val = self.pop()?;

                    let source = extract_entity(&source_val)?;
                    let rel_type = extract_keyword(&rel_type_val, ctx)?;

                    let entities: LtVec<Value> = if let Some(c) = ctx {
                        c.targets(source, rel_type)
                            .into_iter()
                            .map(Value::EntityRef)
                            .collect()
                    } else {
                        LtVec::new()
                    };
                    self.push(Value::Vec(entities));
                }

                Opcode::Sources => {
                    let rel_type_val = self.pop()?;
                    let target_val = self.pop()?;

                    let target = extract_entity(&target_val)?;
                    let rel_type = extract_keyword(&rel_type_val, ctx)?;

                    let entities: LtVec<Value> = if let Some(c) = ctx {
                        c.sources(target, rel_type)
                            .into_iter()
                            .map(Value::EntityRef)
                            .collect()
                    } else {
                        LtVec::new()
                    };
                    self.push(Value::Vec(entities));
                }

                // Effects
                Opcode::Spawn => {
                    let components_val = self.pop()?;
                    let components = match components_val {
                        Value::Map(m) => m,
                        _ => LtMap::new(),
                    };

                    // Generate a temporary entity ID
                    self.spawn_counter += 1;
                    let temp_id = EntityId {
                        index: self.spawn_counter,
                        generation: 0,
                    };

                    self.effects.push(VmEffect::Spawn { components });
                    self.push(Value::EntityRef(temp_id));
                }

                Opcode::Destroy => {
                    let entity_val = self.pop()?;
                    let entity = extract_entity(&entity_val)?;
                    self.effects.push(VmEffect::Destroy { entity });
                }

                Opcode::SetComponent => {
                    let value = self.pop()?;
                    let component_val = self.pop()?;
                    let entity_val = self.pop()?;

                    let entity = extract_entity(&entity_val)?;
                    let component = extract_keyword(&component_val, ctx)?;

                    self.effects.push(VmEffect::SetComponent {
                        entity,
                        component,
                        value,
                    });
                }

                Opcode::SetField => {
                    let value = self.pop()?;
                    let field_val = self.pop()?;
                    let component_val = self.pop()?;
                    let entity_val = self.pop()?;

                    let entity = extract_entity(&entity_val)?;
                    let component = extract_keyword(&component_val, ctx)?;
                    let field = extract_keyword(&field_val, ctx)?;

                    self.effects.push(VmEffect::SetField {
                        entity,
                        component,
                        field,
                        value,
                    });
                }

                Opcode::Link => {
                    let target_val = self.pop()?;
                    let relationship_val = self.pop()?;
                    let source_val = self.pop()?;

                    let source = extract_entity(&source_val)?;
                    let target = extract_entity(&target_val)?;
                    let relationship = extract_keyword(&relationship_val, ctx)?;

                    self.effects.push(VmEffect::Link {
                        source,
                        relationship,
                        target,
                    });
                }

                Opcode::Unlink => {
                    let target_val = self.pop()?;
                    let relationship_val = self.pop()?;
                    let source_val = self.pop()?;

                    let source = extract_entity(&source_val)?;
                    let target = extract_entity(&target_val)?;
                    let relationship = extract_keyword(&relationship_val, ctx)?;

                    self.effects.push(VmEffect::Unlink {
                        source,
                        relationship,
                        target,
                    });
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
                    let idx_val = self.pop()?;
                    let vec = self.pop()?;
                    match (&vec, &idx_val) {
                        (Value::Vec(v), Value::Int(idx)) => {
                            let result = v.get(*idx as usize).cloned().unwrap_or(Value::Nil);
                            self.push(result);
                        }
                        _ => {
                            self.push(Value::Nil);
                        }
                    }
                }
                Opcode::VecLen => {
                    let vec = self.pop()?;
                    match vec {
                        Value::Vec(v) => self.push(Value::Int(v.len() as i64)),
                        _ => self.push(Value::Int(0)),
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
                            let result = m.get(&key).cloned().unwrap_or(Value::Nil);
                            self.push(result);
                        }
                        _ => {
                            self.push(Value::Nil);
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
                            self.push(Value::Bool(false));
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
                            self.push(Value::Bool(false));
                        }
                    }
                }

                // Misc
                Opcode::Print => {
                    let value = self.pop()?;
                    self.output.push(format_value(&value));
                }

                // Higher-order functions
                Opcode::Map => {
                    let coll = self.pop()?;
                    let func_val = self.pop()?;

                    // Extract function reference
                    let func_ref = match &func_val {
                        Value::Fn(longtable_foundation::LtFn::Compiled(f)) => f.clone(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Fn(
                                    longtable_foundation::types::Arity::Variadic(0),
                                ),
                                actual: func_val.value_type(),
                            }));
                        }
                    };

                    // Get the function
                    let func_idx = func_ref.index as usize;
                    let func = functions.get(func_idx).ok_or_else(|| {
                        Error::new(ErrorKind::Internal(format!(
                            "function index {func_idx} out of bounds"
                        )))
                    })?;

                    // Extract collection elements
                    let elements: Vec<Value> = match coll {
                        Value::Vec(v) => v.iter().cloned().collect(),
                        Value::Set(s) => s.iter().cloned().collect(),
                        Value::Nil => Vec::new(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Vec(Box::new(
                                    longtable_foundation::Type::Any,
                                )),
                                actual: coll.value_type(),
                            }));
                        }
                    };

                    // Apply function to each element
                    let mut results = LtVec::new();
                    for elem in elements {
                        // Save current VM state
                        let saved_ip = self.ip;
                        let saved_locals = self.locals.clone();
                        let saved_captures = std::mem::take(&mut self.captures);

                        // Set up argument
                        self.locals[0] = elem;

                        // Set up captures from function's closure
                        if let Some(caps) = &func_ref.captures {
                            self.captures.clone_from(&caps.lock().unwrap());
                        }

                        // Execute function
                        let result =
                            self.execute_internal::<C>(&func.code, constants, functions, ctx)?;

                        // Restore state
                        self.ip = saved_ip;
                        self.locals = saved_locals;
                        self.captures = saved_captures;

                        results = results.push_back(result);
                    }

                    self.push(Value::Vec(results));
                }

                Opcode::Filter => {
                    let coll = self.pop()?;
                    let func_val = self.pop()?;

                    // Extract function reference
                    let func_ref = match &func_val {
                        Value::Fn(longtable_foundation::LtFn::Compiled(f)) => f.clone(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Fn(
                                    longtable_foundation::types::Arity::Variadic(0),
                                ),
                                actual: func_val.value_type(),
                            }));
                        }
                    };

                    // Get the function
                    let func_idx = func_ref.index as usize;
                    let func = functions.get(func_idx).ok_or_else(|| {
                        Error::new(ErrorKind::Internal(format!(
                            "function index {func_idx} out of bounds"
                        )))
                    })?;

                    // Extract collection elements
                    let elements: Vec<Value> = match coll {
                        Value::Vec(v) => v.iter().cloned().collect(),
                        Value::Set(s) => s.iter().cloned().collect(),
                        Value::Nil => Vec::new(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Vec(Box::new(
                                    longtable_foundation::Type::Any,
                                )),
                                actual: coll.value_type(),
                            }));
                        }
                    };

                    // Filter elements
                    let mut results = LtVec::new();
                    for elem in elements {
                        // Save current VM state
                        let saved_ip = self.ip;
                        let saved_locals = self.locals.clone();
                        let saved_captures = std::mem::take(&mut self.captures);

                        // Set up argument
                        self.locals[0] = elem.clone();

                        // Set up captures from function's closure
                        if let Some(caps) = &func_ref.captures {
                            self.captures.clone_from(&caps.lock().unwrap());
                        }

                        // Execute function
                        let result =
                            self.execute_internal::<C>(&func.code, constants, functions, ctx)?;

                        // Restore state
                        self.ip = saved_ip;
                        self.locals = saved_locals;
                        self.captures = saved_captures;

                        // Keep element if result is truthy
                        if is_truthy(&result) {
                            results = results.push_back(elem);
                        }
                    }

                    self.push(Value::Vec(results));
                }

                Opcode::Reduce => {
                    let coll = self.pop()?;
                    let init = self.pop()?;
                    let func_val = self.pop()?;

                    // Extract function reference
                    let func_ref = match &func_val {
                        Value::Fn(longtable_foundation::LtFn::Compiled(f)) => f.clone(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Fn(
                                    longtable_foundation::types::Arity::Variadic(0),
                                ),
                                actual: func_val.value_type(),
                            }));
                        }
                    };

                    // Get the function
                    let func_idx = func_ref.index as usize;
                    let func = functions.get(func_idx).ok_or_else(|| {
                        Error::new(ErrorKind::Internal(format!(
                            "function index {func_idx} out of bounds"
                        )))
                    })?;

                    // Extract collection elements
                    let elements: Vec<Value> = match coll {
                        Value::Vec(v) => v.iter().cloned().collect(),
                        Value::Set(s) => s.iter().cloned().collect(),
                        Value::Nil => Vec::new(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Vec(Box::new(
                                    longtable_foundation::Type::Any,
                                )),
                                actual: coll.value_type(),
                            }));
                        }
                    };

                    // Fold left
                    let mut acc = init;
                    for elem in elements {
                        // Save current VM state
                        let saved_ip = self.ip;
                        let saved_locals = self.locals.clone();
                        let saved_captures = std::mem::take(&mut self.captures);

                        // Set up arguments (acc, elem)
                        self.locals[0] = acc;
                        self.locals[1] = elem;

                        // Set up captures from function's closure
                        if let Some(caps) = &func_ref.captures {
                            self.captures.clone_from(&caps.lock().unwrap());
                        }

                        // Execute function
                        let result =
                            self.execute_internal::<C>(&func.code, constants, functions, ctx)?;

                        // Restore state
                        self.ip = saved_ip;
                        self.locals = saved_locals;
                        self.captures = saved_captures;

                        acc = result;
                    }

                    self.push(acc);
                }

                Opcode::ReduceNoInit => {
                    let coll = self.pop()?;
                    let func_val = self.pop()?;

                    // Extract function reference
                    let func_ref = match &func_val {
                        Value::Fn(longtable_foundation::LtFn::Compiled(f)) => f.clone(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Fn(
                                    longtable_foundation::types::Arity::Variadic(0),
                                ),
                                actual: func_val.value_type(),
                            }));
                        }
                    };

                    // Get the function
                    let func_idx = func_ref.index as usize;
                    let func = functions.get(func_idx).ok_or_else(|| {
                        Error::new(ErrorKind::Internal(format!(
                            "function index {func_idx} out of bounds"
                        )))
                    })?;

                    // Extract collection elements
                    let elements: Vec<Value> = match coll {
                        Value::Vec(v) => v.iter().cloned().collect(),
                        Value::Set(s) => s.iter().cloned().collect(),
                        Value::Nil => Vec::new(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Vec(Box::new(
                                    longtable_foundation::Type::Any,
                                )),
                                actual: coll.value_type(),
                            }));
                        }
                    };

                    // Handle empty collection
                    if elements.is_empty() {
                        self.push(Value::Nil);
                    } else {
                        // Use first element as initial value, fold rest
                        let mut iter = elements.into_iter();
                        let mut acc = iter.next().unwrap();

                        for elem in iter {
                            // Save current VM state
                            let saved_ip = self.ip;
                            let saved_locals = self.locals.clone();
                            let saved_captures = std::mem::take(&mut self.captures);

                            // Set up arguments (acc, elem)
                            self.locals[0] = acc;
                            self.locals[1] = elem;

                            // Set up captures from function's closure
                            if let Some(caps) = &func_ref.captures {
                                self.captures.clone_from(&caps.lock().unwrap());
                            }

                            // Execute function
                            let result =
                                self.execute_internal::<C>(&func.code, constants, functions, ctx)?;

                            // Restore state
                            self.ip = saved_ip;
                            self.locals = saved_locals;
                            self.captures = saved_captures;

                            acc = result;
                        }

                        self.push(acc);
                    }
                }

                Opcode::Every => {
                    let coll = self.pop()?;
                    let func_val = self.pop()?;

                    // Extract function reference
                    let func_ref = match &func_val {
                        Value::Fn(longtable_foundation::LtFn::Compiled(f)) => f.clone(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Fn(
                                    longtable_foundation::types::Arity::Variadic(0),
                                ),
                                actual: func_val.value_type(),
                            }));
                        }
                    };

                    // Get the function
                    let func_idx = func_ref.index as usize;
                    let func = functions.get(func_idx).ok_or_else(|| {
                        Error::new(ErrorKind::Internal(format!(
                            "function index {func_idx} out of bounds"
                        )))
                    })?;

                    // Extract collection elements
                    let elements: Vec<Value> = match coll {
                        Value::Vec(v) => v.iter().cloned().collect(),
                        Value::Set(s) => s.iter().cloned().collect(),
                        Value::Nil => Vec::new(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Vec(Box::new(
                                    longtable_foundation::Type::Any,
                                )),
                                actual: coll.value_type(),
                            }));
                        }
                    };

                    // Check if all elements satisfy predicate
                    let mut all_true = true;
                    for elem in elements {
                        // Save current VM state
                        let saved_ip = self.ip;
                        let saved_locals = self.locals.clone();
                        let saved_captures = std::mem::take(&mut self.captures);

                        // Set up argument
                        self.locals[0] = elem;

                        // Set up captures from function's closure
                        if let Some(caps) = &func_ref.captures {
                            self.captures.clone_from(&caps.lock().unwrap());
                        }

                        // Execute function
                        let result =
                            self.execute_internal::<C>(&func.code, constants, functions, ctx)?;

                        // Restore state
                        self.ip = saved_ip;
                        self.locals = saved_locals;
                        self.captures = saved_captures;

                        if !is_truthy(&result) {
                            all_true = false;
                            break;
                        }
                    }

                    self.push(Value::Bool(all_true));
                }

                Opcode::Some => {
                    let coll = self.pop()?;
                    let func_val = self.pop()?;

                    // Extract function reference
                    let func_ref = match &func_val {
                        Value::Fn(longtable_foundation::LtFn::Compiled(f)) => f.clone(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Fn(
                                    longtable_foundation::types::Arity::Variadic(0),
                                ),
                                actual: func_val.value_type(),
                            }));
                        }
                    };

                    // Get the function
                    let func_idx = func_ref.index as usize;
                    let func = functions.get(func_idx).ok_or_else(|| {
                        Error::new(ErrorKind::Internal(format!(
                            "function index {func_idx} out of bounds"
                        )))
                    })?;

                    // Extract collection elements
                    let elements: Vec<Value> = match coll {
                        Value::Vec(v) => v.iter().cloned().collect(),
                        Value::Set(s) => s.iter().cloned().collect(),
                        Value::Nil => Vec::new(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Vec(Box::new(
                                    longtable_foundation::Type::Any,
                                )),
                                actual: coll.value_type(),
                            }));
                        }
                    };

                    // Find first element that satisfies predicate
                    let mut found: Option<Value> = None;
                    for elem in elements {
                        // Save current VM state
                        let saved_ip = self.ip;
                        let saved_locals = self.locals.clone();
                        let saved_captures = std::mem::take(&mut self.captures);

                        // Set up argument
                        self.locals[0] = elem;

                        // Set up captures from function's closure
                        if let Some(caps) = &func_ref.captures {
                            self.captures.clone_from(&caps.lock().unwrap());
                        }

                        // Execute function
                        let result =
                            self.execute_internal::<C>(&func.code, constants, functions, ctx)?;

                        // Restore state
                        self.ip = saved_ip;
                        self.locals = saved_locals;
                        self.captures = saved_captures;

                        if is_truthy(&result) {
                            found = Some(result);
                            break;
                        }
                    }

                    // Return first truthy result or nil
                    self.push(found.unwrap_or(Value::Nil));
                }

                Opcode::TakeWhile => {
                    let coll = self.pop()?;
                    let func_val = self.pop()?;

                    // Extract function reference
                    let func_ref = match &func_val {
                        Value::Fn(longtable_foundation::LtFn::Compiled(f)) => f.clone(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Fn(
                                    longtable_foundation::types::Arity::Variadic(0),
                                ),
                                actual: func_val.value_type(),
                            }));
                        }
                    };

                    // Get the function
                    let func_idx = func_ref.index as usize;
                    let func = functions.get(func_idx).ok_or_else(|| {
                        Error::new(ErrorKind::Internal(format!(
                            "function index {func_idx} out of bounds"
                        )))
                    })?;

                    // Extract collection elements
                    let elements: Vec<Value> = match coll {
                        Value::Vec(v) => v.iter().cloned().collect(),
                        Value::Set(s) => s.iter().cloned().collect(),
                        Value::Nil => Vec::new(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Vec(Box::new(
                                    longtable_foundation::Type::Any,
                                )),
                                actual: coll.value_type(),
                            }));
                        }
                    };

                    // Take elements while predicate returns truthy
                    let mut results = LtVec::new();
                    for elem in elements {
                        // Save current VM state
                        let saved_ip = self.ip;
                        let saved_locals = self.locals.clone();
                        let saved_captures = std::mem::take(&mut self.captures);

                        // Set up argument
                        self.locals[0] = elem.clone();

                        // Set up captures from function's closure
                        if let Some(caps) = &func_ref.captures {
                            self.captures.clone_from(&caps.lock().unwrap());
                        }

                        // Execute function
                        let result =
                            self.execute_internal::<C>(&func.code, constants, functions, ctx)?;

                        // Restore state
                        self.ip = saved_ip;
                        self.locals = saved_locals;
                        self.captures = saved_captures;

                        if is_truthy(&result) {
                            results = results.push_back(elem);
                        } else {
                            break; // Stop at first falsy
                        }
                    }

                    self.push(Value::Vec(results));
                }

                Opcode::DropWhile => {
                    let coll = self.pop()?;
                    let func_val = self.pop()?;

                    // Extract function reference
                    let func_ref = match &func_val {
                        Value::Fn(longtable_foundation::LtFn::Compiled(f)) => f.clone(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Fn(
                                    longtable_foundation::types::Arity::Variadic(0),
                                ),
                                actual: func_val.value_type(),
                            }));
                        }
                    };

                    // Get the function
                    let func_idx = func_ref.index as usize;
                    let func = functions.get(func_idx).ok_or_else(|| {
                        Error::new(ErrorKind::Internal(format!(
                            "function index {func_idx} out of bounds"
                        )))
                    })?;

                    // Extract collection elements
                    let elements: Vec<Value> = match coll {
                        Value::Vec(v) => v.iter().cloned().collect(),
                        Value::Set(s) => s.iter().cloned().collect(),
                        Value::Nil => Vec::new(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Vec(Box::new(
                                    longtable_foundation::Type::Any,
                                )),
                                actual: coll.value_type(),
                            }));
                        }
                    };

                    // Drop elements while predicate returns truthy
                    let mut dropping = true;
                    let mut results = LtVec::new();
                    for elem in elements {
                        if dropping {
                            // Save current VM state
                            let saved_ip = self.ip;
                            let saved_locals = self.locals.clone();
                            let saved_captures = std::mem::take(&mut self.captures);

                            // Set up argument
                            self.locals[0] = elem.clone();

                            // Set up captures from function's closure
                            if let Some(caps) = &func_ref.captures {
                                self.captures.clone_from(&caps.lock().unwrap());
                            }

                            // Execute function
                            let result =
                                self.execute_internal::<C>(&func.code, constants, functions, ctx)?;

                            // Restore state
                            self.ip = saved_ip;
                            self.locals = saved_locals;
                            self.captures = saved_captures;

                            if !is_truthy(&result) {
                                dropping = false;
                                results = results.push_back(elem);
                            }
                        } else {
                            results = results.push_back(elem);
                        }
                    }

                    self.push(Value::Vec(results));
                }

                Opcode::Remove => {
                    let coll = self.pop()?;
                    let func_val = self.pop()?;

                    // Extract function reference
                    let func_ref = match &func_val {
                        Value::Fn(longtable_foundation::LtFn::Compiled(f)) => f.clone(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Fn(
                                    longtable_foundation::types::Arity::Variadic(0),
                                ),
                                actual: func_val.value_type(),
                            }));
                        }
                    };

                    // Get the function
                    let func_idx = func_ref.index as usize;
                    let func = functions.get(func_idx).ok_or_else(|| {
                        Error::new(ErrorKind::Internal(format!(
                            "function index {func_idx} out of bounds"
                        )))
                    })?;

                    // Extract collection elements
                    let elements: Vec<Value> = match coll {
                        Value::Vec(v) => v.iter().cloned().collect(),
                        Value::Set(s) => s.iter().cloned().collect(),
                        Value::Nil => Vec::new(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Vec(Box::new(
                                    longtable_foundation::Type::Any,
                                )),
                                actual: coll.value_type(),
                            }));
                        }
                    };

                    // Remove elements where predicate returns truthy (inverse of filter)
                    let mut results = LtVec::new();
                    for elem in elements {
                        // Save current VM state
                        let saved_ip = self.ip;
                        let saved_locals = self.locals.clone();
                        let saved_captures = std::mem::take(&mut self.captures);

                        // Set up argument
                        self.locals[0] = elem.clone();

                        // Set up captures from function's closure
                        if let Some(caps) = &func_ref.captures {
                            self.captures.clone_from(&caps.lock().unwrap());
                        }

                        // Execute function
                        let result =
                            self.execute_internal::<C>(&func.code, constants, functions, ctx)?;

                        // Restore state
                        self.ip = saved_ip;
                        self.locals = saved_locals;
                        self.captures = saved_captures;

                        // Keep element if result is NOT truthy (inverse of filter)
                        if !is_truthy(&result) {
                            results = results.push_back(elem);
                        }
                    }

                    self.push(Value::Vec(results));
                }

                Opcode::GroupBy => {
                    let coll = self.pop()?;
                    let func_val = self.pop()?;

                    // Extract function reference
                    let func_ref = match &func_val {
                        Value::Fn(longtable_foundation::LtFn::Compiled(f)) => f.clone(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Fn(
                                    longtable_foundation::types::Arity::Variadic(0),
                                ),
                                actual: func_val.value_type(),
                            }));
                        }
                    };

                    // Get the function
                    let func_idx = func_ref.index as usize;
                    let func = functions.get(func_idx).ok_or_else(|| {
                        Error::new(ErrorKind::Internal(format!(
                            "function index {func_idx} out of bounds"
                        )))
                    })?;

                    // Extract collection elements
                    let elements: Vec<Value> = match coll {
                        Value::Vec(v) => v.iter().cloned().collect(),
                        Value::Set(s) => s.iter().cloned().collect(),
                        Value::Nil => Vec::new(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Vec(Box::new(
                                    longtable_foundation::Type::Any,
                                )),
                                actual: coll.value_type(),
                            }));
                        }
                    };

                    // Group elements by key function result
                    let mut groups: LtMap<Value, Value> = LtMap::new();
                    for elem in elements {
                        // Save current VM state
                        let saved_ip = self.ip;
                        let saved_locals = self.locals.clone();
                        let saved_captures = std::mem::take(&mut self.captures);

                        // Set up argument
                        self.locals[0] = elem.clone();

                        // Set up captures from function's closure
                        if let Some(caps) = &func_ref.captures {
                            self.captures.clone_from(&caps.lock().unwrap());
                        }

                        // Execute function to get key
                        let key =
                            self.execute_internal::<C>(&func.code, constants, functions, ctx)?;

                        // Restore state
                        self.ip = saved_ip;
                        self.locals = saved_locals;
                        self.captures = saved_captures;

                        // Add to appropriate group
                        let group_vec = groups
                            .get(&key)
                            .and_then(|v| {
                                if let Value::Vec(vec) = v {
                                    Some(vec.clone())
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_else(LtVec::new);
                        let new_group = group_vec.push_back(elem);
                        groups = groups.insert(key, Value::Vec(new_group));
                    }

                    self.push(Value::Map(groups));
                }

                Opcode::ZipWith => {
                    let colls_vec = self.pop()?;
                    let func_val = self.pop()?;

                    // Extract function reference
                    let func_ref = match &func_val {
                        Value::Fn(longtable_foundation::LtFn::Compiled(f)) => f.clone(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Fn(
                                    longtable_foundation::types::Arity::Variadic(0),
                                ),
                                actual: func_val.value_type(),
                            }));
                        }
                    };

                    // Get the function
                    let func_idx = func_ref.index as usize;
                    let func = functions.get(func_idx).ok_or_else(|| {
                        Error::new(ErrorKind::Internal(format!(
                            "function index {func_idx} out of bounds"
                        )))
                    })?;

                    // Extract collections from the vector
                    let collections: Vec<Vec<Value>> = match colls_vec {
                        Value::Vec(v) => v
                            .iter()
                            .map(|c| match c {
                                Value::Vec(inner) => Ok(inner.iter().cloned().collect()),
                                Value::Nil => Ok(Vec::new()),
                                _ => Err(Error::new(ErrorKind::TypeMismatch {
                                    expected: longtable_foundation::Type::Vec(Box::new(
                                        longtable_foundation::Type::Any,
                                    )),
                                    actual: c.value_type(),
                                })),
                            })
                            .collect::<Result<Vec<_>>>()?,
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Vec(Box::new(
                                    longtable_foundation::Type::Any,
                                )),
                                actual: colls_vec.value_type(),
                            }));
                        }
                    };

                    if collections.is_empty() {
                        self.push(Value::Vec(LtVec::new()));
                    } else {
                        // Find minimum length
                        let min_len = collections.iter().map(Vec::len).min().unwrap_or(0);
                        let mut results: LtVec<Value> = LtVec::new();

                        for i in 0..min_len {
                            // Save current VM state
                            let saved_ip = self.ip;
                            let saved_locals = self.locals.clone();
                            let saved_captures = std::mem::take(&mut self.captures);

                            // Set up arguments from each collection at index i
                            for (slot, coll) in collections.iter().enumerate() {
                                if slot < self.locals.len() {
                                    self.locals[slot] = coll[i].clone();
                                }
                            }

                            // Set up captures from function's closure
                            if let Some(caps) = &func_ref.captures {
                                self.captures.clone_from(&caps.lock().unwrap());
                            }

                            // Execute function
                            let result =
                                self.execute_internal::<C>(&func.code, constants, functions, ctx)?;

                            // Restore state
                            self.ip = saved_ip;
                            self.locals = saved_locals;
                            self.captures = saved_captures;

                            results = results.push_back(result);
                        }

                        self.push(Value::Vec(results));
                    }
                }

                Opcode::Repeatedly => {
                    let func_val = self.pop()?;
                    let count_val = self.pop()?;

                    // Extract count
                    let count = match count_val {
                        Value::Int(n) if n >= 0 => n as usize,
                        Value::Int(_) => {
                            return Err(Error::new(ErrorKind::Internal(
                                "repeatedly count must be non-negative".to_string(),
                            )));
                        }
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Int,
                                actual: count_val.value_type(),
                            }));
                        }
                    };

                    // Extract function reference
                    let func_ref = match &func_val {
                        Value::Fn(longtable_foundation::LtFn::Compiled(f)) => f.clone(),
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Fn(
                                    longtable_foundation::types::Arity::Variadic(0),
                                ),
                                actual: func_val.value_type(),
                            }));
                        }
                    };

                    // Get the function
                    let func_idx = func_ref.index as usize;
                    let func = functions.get(func_idx).ok_or_else(|| {
                        Error::new(ErrorKind::Internal(format!(
                            "function index {func_idx} out of bounds"
                        )))
                    })?;

                    let mut results: LtVec<Value> = LtVec::new();

                    for _ in 0..count {
                        // Save current VM state
                        let saved_ip = self.ip;
                        let saved_locals = self.locals.clone();
                        let saved_captures = std::mem::take(&mut self.captures);

                        // Set up captures from function's closure
                        if let Some(caps) = &func_ref.captures {
                            self.captures.clone_from(&caps.lock().unwrap());
                        }

                        // Execute zero-arg function
                        let result =
                            self.execute_internal::<C>(&func.code, constants, functions, ctx)?;

                        // Restore state
                        self.ip = saved_ip;
                        self.locals = saved_locals;
                        self.captures = saved_captures;

                        results = results.push_back(result);
                    }

                    self.push(Value::Vec(results));
                }

                // Machine Configuration opcodes - require RuntimeContext
                // In the basic VmContext execution path, these will error.
                // Use execute_with_runtime_context() for full support.
                Opcode::RegisterComponent
                | Opcode::RegisterRelationship
                | Opcode::RegisterVerb
                | Opcode::RegisterDirection
                | Opcode::RegisterPreposition
                | Opcode::RegisterPronoun
                | Opcode::RegisterAdverb
                | Opcode::RegisterType
                | Opcode::RegisterScope
                | Opcode::RegisterCommand
                | Opcode::RegisterAction
                | Opcode::RegisterRule => {
                    return Err(Error::new(ErrorKind::Internal(
                        "registration opcodes require RuntimeContext; use execute_with_runtime_context()".to_string(),
                    )));
                }
            }
        }

        // Return top of stack or nil
        self.stack.pop().ok_or_else(|| {
            Error::new(ErrorKind::Internal(
                "stack underflow at end of execution".to_string(),
            ))
        })
    }

    // Stack operations

    fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    fn pop(&mut self) -> Result<Value> {
        self.stack
            .pop()
            .ok_or_else(|| Error::new(ErrorKind::Internal("stack underflow".to_string())))
    }

    fn peek(&self) -> Result<&Value> {
        self.stack
            .last()
            .ok_or_else(|| Error::new(ErrorKind::Internal("stack underflow".to_string())))
    }

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
    fn call_native(&mut self, idx: u16, arg_count: u8) -> Result<()> {
        // Pop arguments in reverse order
        let mut args = Vec::with_capacity(arg_count as usize);
        for _ in 0..arg_count {
            args.push(self.pop()?);
        }
        args.reverse();

        // Dispatch to native function implementation
        // Index matches order in compiler's register_natives()
        let result = match idx {
            // 0-4: Arithmetic (+, -, *, /, mod) - handled by opcodes
            // 5-10: Comparison (=, !=, <, <=, >, >=) - handled by opcodes
            // 11: not - handled by opcode
            // 12-13: and, or - handled by opcodes/short-circuit
            12 => native_and(&args),
            13 => native_or(&args),
            // 14-24: Predicates
            14 => native_nil_p(&args),
            15 => native_some_p(&args),
            16 => native_int_p(&args),
            17 => native_float_p(&args),
            18 => native_string_p(&args),
            19 => native_keyword_p(&args),
            20 => native_symbol_p(&args),
            21 => native_list_p(&args),
            22 => native_vector_p(&args),
            23 => native_map_p(&args),
            24 => native_set_p(&args),
            // 25-37: Collections
            25 => native_count(&args),
            26 => native_empty_p(&args),
            27 => native_first(&args),
            28 => native_rest(&args),
            29 => native_nth(&args),
            30 => native_conj(&args),
            31 => native_cons(&args),
            32 => native_get(&args),
            33 => native_assoc(&args),
            34 => native_dissoc(&args),
            35 => native_contains_p(&args),
            36 => native_keys(&args),
            37 => native_vals(&args),
            // 38-41: String
            38 => native_str(&args),
            39 => native_str_len(&args),
            40 => native_str_upper(&args),
            41 => native_str_lower(&args),
            // 42-47: Math
            42 => native_abs(&args),
            43 => native_min(&args),
            44 => native_max(&args),
            45 => native_floor(&args),
            46 => native_ceil(&args),
            47 => native_round(&args),
            48 => native_sqrt(&args),
            // 49-51: Misc
            49 => {
                // print - already handled specially, but in case it comes through
                if let Some(v) = args.first() {
                    self.output.push(format_value(v));
                }
                Ok(Value::Nil)
            }
            50 => {
                // println
                if let Some(v) = args.first() {
                    self.output.push(format!("{}\n", format_value(v)));
                }
                Ok(Value::Nil)
            }
            51 => native_type(&args),
            // Stage S1: Critical functions
            52 => native_inc(&args),
            53 => native_dec(&args),
            54 => native_last(&args),
            55 => native_range(&args),
            // Stage S2: String functions
            56 => native_str_split(&args),
            57 => native_str_join(&args),
            58 => native_str_trim(&args),
            59 => native_str_trim_left(&args),
            60 => native_str_trim_right(&args),
            61 => native_str_starts_with(&args),
            62 => native_str_ends_with(&args),
            63 => native_str_contains(&args),
            64 => native_str_replace(&args),
            65 => native_str_replace_all(&args),
            66 => native_str_blank(&args),
            67 => native_str_substring(&args),
            68 => native_format(&args),
            // Stage S3: Collection functions
            69 => native_take(&args),
            70 => native_drop(&args),
            71 => native_concat(&args),
            72 => native_reverse(&args),
            73 => native_vec(&args),
            74 => native_set(&args),
            75 => native_into(&args),
            76 => native_sort(&args),
            77 => native_merge(&args),
            // Stage S4: Math functions
            78 => native_rem(&args),
            79 => native_clamp(&args),
            80 => native_trunc(&args),
            81 => native_pow(&args),
            82 => native_cbrt(&args),
            83 => native_exp(&args),
            84 => native_log(&args),
            85 => native_log10(&args),
            86 => native_log2(&args),
            87 => native_sin(&args),
            88 => native_cos(&args),
            89 => native_tan(&args),
            90 => native_asin(&args),
            91 => native_acos(&args),
            92 => native_atan(&args),
            93 => native_atan2(&args),
            94 => native_pi(&args),
            95 => native_e(&args),
            // Stage S5: Extended collection functions
            96 => native_flatten(&args),
            97 => native_distinct(&args),
            98 => native_dedupe(&args),
            99 => native_partition(&args),
            100 => native_partition_all(&args),
            // Stage S6: Vector math functions
            101 => native_vec_add(&args),
            102 => native_vec_sub(&args),
            103 => native_vec_mul(&args),
            104 => native_vec_scale(&args),
            105 => native_vec_dot(&args),
            106 => native_vec_cross(&args),
            107 => native_vec_length(&args),
            108 => native_vec_length_sq(&args),
            109 => native_vec_normalize(&args),
            110 => native_vec_distance(&args),
            111 => native_vec_lerp(&args),
            112 => native_vec_angle(&args),
            // Stage S7: Remaining functions (113-124)
            113 => native_bool_p(&args),
            114 => native_number_p(&args),
            115 => native_coll_p(&args),
            116 => native_fn_p(&args),
            117 => native_entity_p(&args),
            118 => native_sinh(&args),
            119 => native_cosh(&args),
            120 => native_tanh(&args),
            121 => native_interleave(&args),
            122 => native_interpose(&args),
            123 => native_zip(&args),
            124 => native_repeat(&args),
            _ => Err(Error::new(ErrorKind::Internal(format!(
                "unknown native function index: {idx}"
            )))),
        }?;

        self.push(result);
        Ok(())
    }
}

// Helper functions

fn extract_entity(val: &Value) -> Result<EntityId> {
    match val {
        Value::EntityRef(e) => Ok(*e),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::EntityRef,
            actual: val.value_type(),
        })),
    }
}

fn extract_keyword<C: VmContext>(val: &Value, ctx: Option<&C>) -> Result<KeywordId> {
    match val {
        Value::Keyword(k) => Ok(*k),
        _ => {
            if let Some(k) = ctx.and_then(|c| c.resolve_keyword(val)) {
                Ok(k)
            } else {
                Err(Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Keyword,
                    actual: val.value_type(),
                }))
            }
        }
    }
}

/// Evaluates source code and returns the result.
pub fn eval(source: &str) -> Result<Value> {
    let program = crate::compiler::compile(source)?;
    let mut vm = Vm::new();
    vm.execute(&program)
}
