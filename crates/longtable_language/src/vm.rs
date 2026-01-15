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

pub use context::{ReadOnlyContext, RuntimeContext, VmContext, VmEffect, WorldContext};

use context::NoRuntimeContext;
use native::{
    add_values, compare_for_sort, compare_values, div_values, format_value, is_truthy, mod_values,
    mul_values, native_abs, native_acos, native_and, native_asin, native_assoc, native_atan,
    native_atan2, native_bool_p, native_cbrt, native_ceil, native_char_at, native_clamp,
    native_coll_p, native_concat, native_conj, native_cons, native_contains_p, native_cos,
    native_cosh, native_count, native_dec, native_dedupe, native_disj, native_dissoc,
    native_distinct, native_drop, native_e, native_empty_p, native_entity_p, native_exp,
    native_first, native_flatten, native_float_p, native_floor, native_fn_p, native_format,
    native_get, native_inc, native_int_p, native_interleave, native_interpose, native_into,
    native_keys, native_keyword_p, native_last, native_list_p, native_log, native_log2,
    native_log10, native_map_p, native_max, native_merge, native_min, native_nil_p, native_nth,
    native_number_p, native_or, native_parse_int, native_partition, native_partition_all,
    native_pi, native_pow, native_range, native_rem, native_repeat, native_rest, native_reverse,
    native_round, native_set, native_set_p, native_sin, native_sinh, native_some_p, native_sort,
    native_sqrt, native_str_blank, native_str_contains, native_str_ends_with, native_str_join,
    native_str_len, native_str_lower, native_str_replace, native_str_replace_all, native_str_split,
    native_str_starts_with, native_str_substring, native_str_trim, native_str_trim_left,
    native_str_trim_right, native_str_upper, native_string_p, native_symbol_p, native_take,
    native_tan, native_tanh, native_trunc, native_type, native_vals, native_vec, native_vec_add,
    native_vec_angle, native_vec_cross, native_vec_distance, native_vec_dot, native_vec_length,
    native_vec_length_sq, native_vec_lerp, native_vec_mul, native_vec_normalize, native_vec_scale,
    native_vec_sub, native_vector_p, native_zip, neg_value, sub_values,
};

use std::collections::HashMap;

use longtable_foundation::{
    EntityId, Error, ErrorKind, KeywordId, LtMap, LtSet, LtVec, Result, Value,
};

use crate::compiler::CompiledProgram;
use crate::opcode::{Bytecode, Opcode};

/// Key for tracking pending field mutations.
/// Allows reads within the same execution to see previous writes.
type FieldKey = (EntityId, KeywordId, KeywordId);

/// Key for tracking pending component mutations.
type ComponentKey = (EntityId, KeywordId);

/// Pending vector modifications for a field.
/// Tracks removals and additions to apply when reading.
#[derive(Default, Clone)]
struct PendingVecOps {
    removals: Vec<Value>,
    additions: Vec<Value>,
}

// =============================================================================
// Macros for reducing VM code duplication
// =============================================================================

/// Dispatches native function calls by index.
///
/// Usage:
/// ```ignore
/// native_dispatch!(idx, args;
///     12 => native_and,
///     13 => native_or,
///     // ...
/// )
/// ```
macro_rules! native_dispatch {
    ($idx:expr, $args:expr; $($num:literal => $func:ident),* $(,)?) => {
        match $idx {
            $($num => $func($args),)*
            _ => Err(Error::new(ErrorKind::Internal(format!(
                "unknown native function index: {}", $idx
            )))),
        }
    };
}

/// Formats a value for display, using the context to resolve keywords.
fn format_value_with_ctx<C: VmContext>(value: &Value, ctx: &C) -> String {
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
        Value::Keyword(id) => ctx
            .keyword_to_string(*id)
            .map_or_else(|| format!("Keyword({})", id.index()), |s| format!(":{s}")),
        Value::EntityRef(id) => format!("Entity({}, {})", id.index, id.generation),
        Value::Vec(v) => {
            let items: Vec<_> = v.iter().map(|v| format_value_with_ctx(v, ctx)).collect();
            format!("[{}]", items.join(" "))
        }
        Value::List(l) => {
            let items: Vec<_> = l.iter().map(|v| format_value_with_ctx(v, ctx)).collect();
            format!("({})", items.join(" "))
        }
        Value::Set(s) => {
            let items: Vec<_> = s.iter().map(|v| format_value_with_ctx(v, ctx)).collect();
            format!("#{{{}}}", items.join(" "))
        }
        Value::Map(m) => {
            let pairs: Vec<_> = m
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{} {}",
                        format_value_with_ctx(k, ctx),
                        format_value_with_ctx(v, ctx)
                    )
                })
                .collect();
            format!("{{{}}}", pairs.join(" "))
        }
        Value::Fn(_) => "<fn>".to_string(),
    }
}

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
    /// Pending field mutations for read-your-writes semantics.
    /// Maps (entity, component, field) -> value for `SetField` effects.
    /// This allows `GetField` to see mutations made earlier in the same execution.
    pending_fields: HashMap<FieldKey, Value>,
    /// Pending component mutations for read-your-writes semantics.
    /// Maps (entity, component) -> `Option<Value>`.
    /// - `Some(value)` means the component was set to this value
    /// - `None` means the component was retracted
    pending_components: HashMap<ComponentKey, Option<Value>>,
    /// Pending vector field operations for read-your-writes semantics.
    /// Maps (entity, component, field) -> pending removals/additions.
    pending_vec_ops: HashMap<FieldKey, PendingVecOps>,
    /// Map from global names to slot indices (for late-bound lookups).
    globals_by_name: HashMap<String, u16>,
    /// Effects count at each snapshot for proper restoration during backtracking.
    effects_counts: HashMap<u64, usize>,
    /// Pending spawned entities for read-your-writes semantics.
    /// Maps temp `EntityId` to its components map, allowing queries to see
    /// spawned entities before effects are applied to the World.
    pending_spawns: HashMap<EntityId, LtMap<Value, Value>>,
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
            pending_fields: HashMap::new(),
            pending_components: HashMap::new(),
            pending_vec_ops: HashMap::new(),
            globals_by_name: HashMap::new(),
            effects_counts: HashMap::new(),
            pending_spawns: HashMap::new(),
        }
    }

    /// Registers a global variable by name and slot for late-binding support.
    pub fn register_global(&mut self, name: String, slot: u16) {
        self.globals_by_name.insert(name, slot);
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
        self.pending_fields.clear();
        self.pending_components.clear();
        self.pending_vec_ops.clear();
        self.pending_spawns.clear();
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
        self.pending_fields.clear();
        self.pending_components.clear();
        self.pending_vec_ops.clear();
        self.pending_spawns.clear();
        std::mem::take(&mut self.effects)
    }

    /// Clears the effects buffer.
    pub fn clear_effects(&mut self) {
        self.effects.clear();
        self.pending_fields.clear();
        self.pending_components.clear();
        self.pending_vec_ops.clear();
        self.pending_spawns.clear();
    }

    /// Executes a compiled program and returns the result.
    ///
    /// This does not support registration opcodes. Use `execute_with_runtime_context`
    /// for full support including schema/vocabulary registration.
    pub fn execute(&mut self, program: &CompiledProgram) -> Result<Value> {
        let mut ctx = NoRuntimeContext;
        self.execute_internal(
            &program.code,
            &program.constants,
            &program.functions,
            &mut ctx,
        )
    }

    /// Executes a compiled program with World context.
    ///
    /// This supports read operations on the World but not registration opcodes.
    /// Use `execute_with_runtime_context` for full support.
    pub fn execute_with_context<C: VmContext>(
        &mut self,
        program: &CompiledProgram,
        ctx: &C,
    ) -> Result<Value> {
        let mut wrapper = ReadOnlyContext::new(ctx);
        self.execute_internal(
            &program.code,
            &program.constants,
            &program.functions,
            &mut wrapper,
        )
    }

    /// Executes a compiled program with full runtime context.
    ///
    /// This method supports all opcodes including registration opcodes
    /// (`RegisterComponent`, `RegisterVerb`, etc.) that require mutable access
    /// to the runtime environment.
    pub fn execute_with_runtime_context<C: RuntimeContext>(
        &mut self,
        program: &CompiledProgram,
        ctx: &mut C,
    ) -> Result<Value> {
        self.execute_internal(&program.code, &program.constants, &program.functions, ctx)
    }

    /// Executes bytecode with a constants pool (no functions available).
    pub fn execute_bytecode(&mut self, code: &Bytecode, constants: &[Value]) -> Result<Value> {
        let mut ctx = NoRuntimeContext;
        self.execute_internal(code, constants, &[], &mut ctx)
    }

    /// Executes bytecode with a `RuntimeContext`.
    ///
    /// This is the unified internal execution method that handles all opcodes.
    /// Uses an explicit call stack for tail-call optimization.
    fn execute_internal<C: RuntimeContext>(
        &mut self,
        initial_code: &Bytecode,
        constants: &[Value],
        functions: &[crate::compiler::CompiledFunction],
        ctx: &mut C,
    ) -> Result<Value> {
        /// A call frame on the explicit call stack (for TCO support).
        struct CallFrame {
            /// The function index we were executing (None for initial/top-level code).
            function_idx: Option<usize>,
            /// Return address (instruction pointer after the call).
            return_ip: usize,
            /// Saved locals.
            saved_locals: Vec<Value>,
            /// Saved captures.
            saved_captures: Vec<Value>,
        }

        let mut call_stack: Vec<CallFrame> = Vec::with_capacity(256);
        let mut current_function_idx: Option<usize> = None;
        self.ip = 0;

        loop {
            // Get current code based on which function we're executing
            let code: &Bytecode = match current_function_idx {
                None => initial_code,
                Some(idx) => &functions[idx].code,
            };

            // Check for end of code (implicit return)
            if self.ip >= code.ops.len() {
                let result = if self.stack.is_empty() {
                    Value::Nil
                } else {
                    self.pop()?
                };

                if let Some(frame) = call_stack.pop() {
                    self.ip = frame.return_ip;
                    current_function_idx = frame.function_idx;
                    self.locals = frame.saved_locals;
                    self.captures = frame.saved_captures;
                    self.push(result);
                    continue;
                }
                return Ok(result);
            }

            // Clone opcode so we can modify current_function_idx
            let op = code.ops[self.ip].clone();
            self.ip += 1;

            match op {
                Opcode::Nop => {}

                Opcode::Const(idx) => {
                    let value = constants.get(idx as usize).cloned().unwrap_or(Value::Nil);
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
                    self.ip = ((self.ip as i32) + (offset as i32)) as usize;
                }
                Opcode::JumpIf(offset) => {
                    let cond = self.pop()?;
                    if is_truthy(&cond) {
                        self.ip = ((self.ip as i32) + (offset as i32)) as usize;
                    }
                }
                Opcode::JumpIfNot(offset) => {
                    let cond = self.pop()?;
                    if !is_truthy(&cond) {
                        self.ip = ((self.ip as i32) + (offset as i32)) as usize;
                    }
                }
                Opcode::Call(arg_count) => {
                    // Pop arguments in reverse order
                    let arg_count = arg_count as usize;
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

                    // Push call frame (for TCO support - explicit stack instead of Rust recursion)
                    call_stack.push(CallFrame {
                        function_idx: current_function_idx,
                        return_ip: self.ip,
                        saved_locals: std::mem::replace(&mut self.locals, vec![Value::Nil; 256]),
                        saved_captures: std::mem::take(&mut self.captures),
                    });

                    // Set up new function execution context
                    current_function_idx = Some(func_idx);
                    self.ip = 0;

                    // Set up arguments as locals
                    for (i, arg) in args.into_iter().enumerate() {
                        self.locals[i] = arg;
                    }

                    // Set up captures from the function's closure
                    if let Some(caps) = &func_ref.captures {
                        self.captures.clone_from(&caps.lock().unwrap());
                    }
                    // Continue main loop with new function's code
                }
                Opcode::TailCall(arg_count) => {
                    // Tail call - reuse current frame instead of pushing a new one
                    let arg_count = arg_count as usize;
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

                    // DON'T push a call frame - reuse current frame (this is the TCO!)
                    // Just switch to new function
                    current_function_idx = Some(func_idx);
                    self.ip = 0;

                    // Clear locals and set up arguments
                    for i in 0..self.locals.len() {
                        self.locals[i] = Value::Nil;
                    }
                    for (i, arg) in args.into_iter().enumerate() {
                        self.locals[i] = arg;
                    }

                    // Set up captures from the function's closure
                    if let Some(caps) = &func_ref.captures {
                        self.captures.clone_from(&caps.lock().unwrap());
                    } else {
                        self.captures.clear();
                    }
                    // Continue main loop with new function's code
                }
                Opcode::CallNative(idx, arg_count) => {
                    self.call_native(idx, arg_count, ctx)?;
                }
                Opcode::Return => {
                    // Return from function using explicit call stack
                    let result = self.pop()?;

                    if let Some(frame) = call_stack.pop() {
                        // Return to caller
                        self.ip = frame.return_ip;
                        current_function_idx = frame.function_idx;
                        self.locals = frame.saved_locals;
                        self.captures = frame.saved_captures;
                        self.push(result);
                        continue;
                    }
                    // No more frames - return from top-level
                    return Ok(result);
                }

                // Variables
                Opcode::LoadLocal(slot) => {
                    let value = self
                        .locals
                        .get(slot as usize)
                        .cloned()
                        .unwrap_or(Value::Nil);
                    self.push(value);
                }
                Opcode::StoreLocal(slot) => {
                    let value = self.pop()?;
                    let slot = slot as usize;
                    if slot >= self.locals.len() {
                        self.locals.resize(slot + 1, Value::Nil);
                    }
                    self.locals[slot] = value;
                }
                Opcode::LoadGlobal(slot) => {
                    let value = self
                        .globals
                        .get(slot as usize)
                        .cloned()
                        .unwrap_or(Value::Nil);
                    self.push(value);
                }
                Opcode::LoadGlobalByName(name_idx) => {
                    // Late-bound global lookup by name
                    let name = constants.get(name_idx as usize).ok_or_else(|| {
                        Error::new(ErrorKind::Internal(format!(
                            "constant index {name_idx} out of bounds"
                        )))
                    })?;
                    let name_str = match name {
                        Value::String(s) => s.as_ref(),
                        _ => {
                            return Err(Error::new(ErrorKind::Internal(format!(
                                "expected string constant for global name, got {}",
                                name.value_type()
                            ))));
                        }
                    };
                    if let Some(&slot) = self.globals_by_name.get(name_str) {
                        let value = self
                            .globals
                            .get(slot as usize)
                            .cloned()
                            .unwrap_or(Value::Nil);
                        self.push(value);
                    } else {
                        return Err(Error::new(ErrorKind::UndefinedSymbol(name_str.to_string())));
                    }
                }
                Opcode::StoreGlobal(slot) => {
                    let value = self.pop()?;
                    let slot = slot as usize;
                    if slot >= self.globals.len() {
                        self.globals.resize(slot + 1, Value::Nil);
                    }
                    self.globals[slot] = value;
                }
                Opcode::LoadBinding(idx) => {
                    let value = self
                        .bindings
                        .get(idx as usize)
                        .cloned()
                        .unwrap_or(Value::Nil);
                    self.push(value);
                }
                Opcode::LoadCapture(idx) => {
                    let value = self
                        .captures
                        .get(idx as usize)
                        .cloned()
                        .unwrap_or(Value::Nil);
                    self.push(value);
                }
                Opcode::MakeClosure(fn_index, capture_count) => {
                    // Pop captured values in reverse order
                    let capture_count = capture_count as usize;
                    let mut captured = Vec::with_capacity(capture_count);
                    for _ in 0..capture_count {
                        captured.push(self.pop()?);
                    }
                    captured.reverse();

                    // Create function value with captures (using RefCell for mutability)
                    let fn_value = Value::Fn(longtable_foundation::LtFn::Compiled(
                        longtable_foundation::CompiledFn::with_captures(fn_index, captured),
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
                        func.patch_capture(capture_idx as usize, new_value);
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

                    // Check pending_components first for read-your-writes semantics
                    let key = (entity, component);
                    let result = if let Some(pending) = self.pending_components.get(&key) {
                        // Clone the pending value (Some(value) or None if retracted)
                        pending.clone()
                    } else {
                        ctx.get_component(entity, component)?
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

                    // Check pending_fields first for read-your-writes semantics
                    let key = (entity, component, field);
                    let base_value = if let Some(value) = self.pending_fields.get(&key) {
                        Some(value.clone())
                    } else if let Some(components) = self.pending_spawns.get(&entity) {
                        // Entity is a pending spawn - look up field from its components
                        if let Some(Value::Map(fields)) = components.get(&Value::Keyword(component))
                        {
                            fields.get(&Value::Keyword(field)).cloned()
                        } else {
                            None
                        }
                    } else {
                        ctx.get_field(entity, component, field)?
                    };

                    // Apply any pending vec operations to the result
                    let result = if let Some(pending_ops) = self.pending_vec_ops.get(&key) {
                        match base_value {
                            Some(Value::Vec(vec)) => {
                                // Apply removals and additions
                                let mut items: Vec<Value> = vec.iter().cloned().collect();
                                for removal in &pending_ops.removals {
                                    items.retain(|v| v != removal);
                                }
                                for addition in &pending_ops.additions {
                                    items.push(addition.clone());
                                }
                                Some(Value::Vec(items.into_iter().collect()))
                            }
                            other => other,
                        }
                    } else {
                        base_value
                    };
                    self.push(result.unwrap_or(Value::Nil));
                }

                // Entity Search
                Opcode::WithComponent => {
                    let component_val = self.pop()?;
                    let component = extract_keyword(&component_val, ctx)?;

                    // Get entities from the committed world
                    let entities: LtVec<Value> = ctx
                        .with_component(component)
                        .into_iter()
                        .map(Value::EntityRef)
                        .collect();

                    // Also include pending spawns that have this component
                    // This enables read-your-writes: spawned entities are visible
                    // to queries within the same execution.
                    let entities = self.pending_spawns.iter().fold(
                        entities,
                        |acc, (entity_id, components)| {
                            if components.contains_key(&Value::Keyword(component)) {
                                acc.push_back(Value::EntityRef(*entity_id))
                            } else {
                                acc
                            }
                        },
                    );

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

                    let entities: LtVec<Value> = ctx
                        .find_relationships(rel_type, source, target)
                        .into_iter()
                        .map(Value::EntityRef)
                        .collect();
                    self.push(Value::Vec(entities));
                }

                Opcode::FindRelationshipsByPrefix => {
                    let target_val = self.pop()?;
                    let source_val = self.pop()?;
                    let prefix_val = self.pop()?;

                    let prefix = match &prefix_val {
                        Value::String(s) => s.as_ref(),
                        Value::Nil => "",
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::String,
                                actual: prefix_val.value_type(),
                            }));
                        }
                    };
                    let source = match &source_val {
                        Value::Nil => None,
                        _ => Some(extract_entity(&source_val)?),
                    };
                    let target = match &target_val {
                        Value::Nil => None,
                        _ => Some(extract_entity(&target_val)?),
                    };

                    let entities: LtVec<Value> = ctx
                        .find_relationships_by_prefix(prefix, source, target)
                        .into_iter()
                        .map(Value::EntityRef)
                        .collect();
                    self.push(Value::Vec(entities));
                }

                Opcode::Targets => {
                    let rel_type_val = self.pop()?;
                    let source_val = self.pop()?;

                    let source = extract_entity(&source_val)?;
                    let rel_type = extract_keyword(&rel_type_val, ctx)?;

                    let entities: LtVec<Value> = ctx
                        .targets(source, rel_type)
                        .into_iter()
                        .map(Value::EntityRef)
                        .collect();
                    self.push(Value::Vec(entities));
                }

                Opcode::Sources => {
                    let rel_type_val = self.pop()?;
                    let target_val = self.pop()?;

                    let target = extract_entity(&target_val)?;
                    let rel_type = extract_keyword(&rel_type_val, ctx)?;

                    let entities: LtVec<Value> = ctx
                        .sources(target, rel_type)
                        .into_iter()
                        .map(Value::EntityRef)
                        .collect();
                    self.push(Value::Vec(entities));
                }

                // Effects
                Opcode::Spawn => {
                    let components_val = self.pop()?;
                    let components = match components_val {
                        Value::Map(m) => m,
                        _ => LtMap::new(),
                    };

                    // Generate a temporary entity ID with a large base offset to avoid
                    // conflicts with real entity IDs in the World. Uses generation 1
                    // (odd = alive) to match EntityStore conventions.
                    self.spawn_counter += 1;
                    let temp_id = EntityId {
                        index: 1_000_000_000 + self.spawn_counter,
                        generation: 1,
                    };

                    // Store in pending_spawns for read-your-writes semantics
                    // This allows queries (with-component, get-field) to see spawned
                    // entities before effects are applied to the World.
                    self.pending_spawns.insert(temp_id, components.clone());

                    // Also populate pending_components so has-component works
                    for (key, value) in components.iter() {
                        if let Value::Keyword(comp_kw) = key {
                            self.pending_components
                                .insert((temp_id, *comp_kw), Some(value.clone()));
                        }
                    }

                    self.effects.push(VmEffect::Spawn {
                        temp_id,
                        components,
                    });
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

                    // Store in pending_components for read-your-writes semantics
                    self.pending_components
                        .insert((entity, component), Some(value.clone()));

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

                    // Store in pending_fields for read-your-writes semantics
                    self.pending_fields
                        .insert((entity, component, field), value.clone());

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

                Opcode::HasComponent => {
                    let component_val = self.pop()?;
                    let entity_val = self.pop()?;

                    let entity = extract_entity(&entity_val)?;
                    let component = extract_keyword(&component_val, ctx)?;

                    // Check pending_components first for read-your-writes semantics
                    let key = (entity, component);
                    let has = if let Some(pending) = self.pending_components.get(&key) {
                        // Some(value) means component was set, None means retracted
                        pending.is_some()
                    } else if let Some(components) = self.pending_spawns.get(&entity) {
                        // Also check pending spawns for entities not yet committed
                        components.contains_key(&Value::Keyword(component))
                    } else {
                        ctx.has_component(entity, component)
                    };
                    self.push(Value::Bool(has));
                }

                Opcode::RemoveComponent => {
                    let component_val = self.pop()?;
                    let entity_val = self.pop()?;

                    let entity = extract_entity(&entity_val)?;
                    let component = extract_keyword(&component_val, ctx)?;

                    // Mark as removed (None) in pending_components for read-your-writes
                    self.pending_components.insert((entity, component), None);

                    self.effects
                        .push(VmEffect::RemoveComponent { entity, component });
                }

                // Mergeable collection field mutations
                Opcode::VecRemove => {
                    let value = self.pop()?;
                    let field_val = self.pop()?;
                    let component_val = self.pop()?;
                    let entity_val = self.pop()?;

                    let entity = extract_entity(&entity_val)?;
                    let component = extract_keyword(&component_val, ctx)?;
                    let field = extract_keyword(&field_val, ctx)?;

                    // Track pending removal for read-your-writes semantics
                    let key = (entity, component, field);
                    self.pending_vec_ops
                        .entry(key)
                        .or_default()
                        .removals
                        .push(value.clone());

                    self.effects.push(VmEffect::VecRemove {
                        entity,
                        component,
                        field,
                        value,
                    });
                }

                Opcode::VecAdd => {
                    let value = self.pop()?;
                    let field_val = self.pop()?;
                    let component_val = self.pop()?;
                    let entity_val = self.pop()?;

                    let entity = extract_entity(&entity_val)?;
                    let component = extract_keyword(&component_val, ctx)?;
                    let field = extract_keyword(&field_val, ctx)?;

                    // Track pending addition for read-your-writes semantics
                    let key = (entity, component, field);
                    self.pending_vec_ops
                        .entry(key)
                        .or_default()
                        .additions
                        .push(value.clone());

                    self.effects.push(VmEffect::VecAdd {
                        entity,
                        component,
                        field,
                        value,
                    });
                }

                Opcode::SetRemove => {
                    let value = self.pop()?;
                    let field_val = self.pop()?;
                    let component_val = self.pop()?;
                    let entity_val = self.pop()?;

                    let entity = extract_entity(&entity_val)?;
                    let component = extract_keyword(&component_val, ctx)?;
                    let field = extract_keyword(&field_val, ctx)?;

                    self.effects.push(VmEffect::SetRemove {
                        entity,
                        component,
                        field,
                        value,
                    });
                }

                Opcode::SetAdd => {
                    let value = self.pop()?;
                    let field_val = self.pop()?;
                    let component_val = self.pop()?;
                    let entity_val = self.pop()?;

                    let entity = extract_entity(&entity_val)?;
                    let component = extract_keyword(&component_val, ctx)?;
                    let field = extract_keyword(&field_val, ctx)?;

                    self.effects.push(VmEffect::SetAdd {
                        entity,
                        component,
                        field,
                        value,
                    });
                }

                // State Management (Backtracking Support)
                Opcode::SaveState => {
                    // Save state immediately through the context for backtracking support.
                    // This allows save/restore to work during execution, not just after.
                    let snapshot_id = ctx.save_state();
                    // Also record the current effects count so we can truncate on restore.
                    let effects_len = self.effects.len();
                    self.effects_counts.insert(snapshot_id, effects_len);
                    self.push(Value::Int(snapshot_id as i64));
                }
                Opcode::RestoreState => {
                    let id_val = self.pop()?;
                    match id_val {
                        Value::Int(id) => {
                            let snapshot_id = id as u64;
                            // Restore effects count (truncate effects accumulated after save)
                            if let Some(&effects_count) = self.effects_counts.get(&snapshot_id) {
                                self.effects.truncate(effects_count);
                            }
                            // Clear pending mutations since we're restoring state
                            self.pending_fields.clear();
                            self.pending_components.clear();
                            self.pending_vec_ops.clear();
                            self.pending_spawns.clear();
                            // Restore world state through the context.
                            ctx.restore_state(snapshot_id)?;
                        }
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Int,
                                actual: id_val.value_type(),
                            }));
                        }
                    }
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

                Opcode::KeywordToString => {
                    let value = self.pop()?;
                    match value {
                        Value::Keyword(kw) => {
                            let result = ctx.keyword_to_string(kw).unwrap_or_default();
                            self.push(Value::String(result.into()));
                        }
                        Value::Nil => {
                            self.push(Value::String(String::new().into()));
                        }
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Keyword,
                                actual: value.value_type(),
                            }));
                        }
                    }
                }

                Opcode::StringToKeyword => {
                    let value = self.pop()?;
                    match value {
                        Value::String(s) => {
                            let keyword_id = ctx.intern_keyword(&s);
                            self.push(Value::Keyword(keyword_id));
                        }
                        Value::Nil => {
                            self.push(Value::Nil);
                        }
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::String,
                                actual: value.value_type(),
                            }));
                        }
                    }
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
                            self.execute_internal(&func.code, constants, functions, ctx)?;

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
                            self.execute_internal(&func.code, constants, functions, ctx)?;

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
                            self.execute_internal(&func.code, constants, functions, ctx)?;

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
                                self.execute_internal(&func.code, constants, functions, ctx)?;

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
                            self.execute_internal(&func.code, constants, functions, ctx)?;

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
                            self.execute_internal(&func.code, constants, functions, ctx)?;

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
                            self.execute_internal(&func.code, constants, functions, ctx)?;

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
                                self.execute_internal(&func.code, constants, functions, ctx)?;

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
                            self.execute_internal(&func.code, constants, functions, ctx)?;

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
                        let key = self.execute_internal(&func.code, constants, functions, ctx)?;

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

                Opcode::SortBy => {
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

                    // Compute keys for each element
                    let mut keyed: Vec<(Value, Value)> = Vec::with_capacity(elements.len());
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
                        let key = self.execute_internal(&func.code, constants, functions, ctx)?;

                        // Restore state
                        self.ip = saved_ip;
                        self.locals = saved_locals;
                        self.captures = saved_captures;

                        keyed.push((key, elem));
                    }

                    // Sort by keys
                    keyed.sort_by(|(k1, _), (k2, _)| compare_for_sort(k1, k2));

                    // Extract sorted elements
                    let result: LtVec<Value> = keyed.into_iter().map(|(_, elem)| elem).collect();
                    self.push(Value::Vec(result));
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
                                self.execute_internal(&func.code, constants, functions, ctx)?;

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
                            self.execute_internal(&func.code, constants, functions, ctx)?;

                        // Restore state
                        self.ip = saved_ip;
                        self.locals = saved_locals;
                        self.captures = saved_captures;

                        results = results.push_back(result);
                    }

                    self.push(Value::Vec(results));
                }

                // Machine Configuration opcodes
                // These call RuntimeContext methods to register schemas, vocabulary, etc.
                // When using ReadOnlyContext or NoRuntimeContext, these will error.
                Opcode::RegisterComponent => {
                    let schema = self.pop()?;
                    ctx.register_component_schema(&schema)?;
                    self.push(Value::Nil);
                }
                Opcode::RegisterRelationship => {
                    let schema = self.pop()?;
                    ctx.register_relationship_schema(&schema)?;
                    self.push(Value::Nil);
                }
                Opcode::RegisterVerb => {
                    let data = self.pop()?;
                    ctx.register_verb(&data)?;
                    self.push(Value::Nil);
                }
                Opcode::RegisterDirection => {
                    let data = self.pop()?;
                    ctx.register_direction(&data)?;
                    self.push(Value::Nil);
                }
                Opcode::RegisterPreposition => {
                    let data = self.pop()?;
                    ctx.register_preposition(&data)?;
                    self.push(Value::Nil);
                }
                Opcode::RegisterPronoun => {
                    let data = self.pop()?;
                    ctx.register_pronoun(&data)?;
                    self.push(Value::Nil);
                }
                Opcode::RegisterAdverb => {
                    let data = self.pop()?;
                    ctx.register_adverb(&data)?;
                    self.push(Value::Nil);
                }
                Opcode::RegisterType => {
                    let data = self.pop()?;
                    ctx.register_type(&data)?;
                    self.push(Value::Nil);
                }
                Opcode::RegisterScope => {
                    let data = self.pop()?;
                    ctx.register_scope(&data)?;
                    self.push(Value::Nil);
                }
                Opcode::RegisterCommand => {
                    let data = self.pop()?;
                    ctx.register_command(&data)?;
                    self.push(Value::Nil);
                }
                Opcode::RegisterAction => {
                    let data = self.pop()?;
                    ctx.register_action(&data)?;
                    self.push(Value::Nil);
                }
                Opcode::RegisterRule => {
                    let data = self.pop()?;
                    let entity_id = ctx.register_rule(&data)?;
                    self.push(Value::EntityRef(entity_id));
                }
            }
        }
        // Note: loop always returns from inside, no code reaches here
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
    fn call_native<C: RuntimeContext>(&mut self, idx: u16, arg_count: u8, ctx: &C) -> Result<()> {
        // Pop arguments in reverse order
        let mut args = Vec::with_capacity(arg_count as usize);
        for _ in 0..arg_count {
            args.push(self.pop()?);
        }
        args.reverse();

        // Debug: trace native calls
        if std::env::var("LONGTABLE_DEBUG_NATIVE").is_ok() {
            eprintln!("[DEBUG NATIVE] idx={idx} arg_count={arg_count} args={args:?}");
        }

        // Helper to format values with keyword resolution
        let format_val = |v: &Value| -> String { format_value_with_ctx(v, ctx) };

        // Handle special cases that need VM access (print/println)
        let result = match idx {
            50 => {
                // print
                if let Some(v) = args.first() {
                    self.output.push(format_val(v));
                }
                Ok(Value::Nil)
            }
            51 => {
                // println
                if let Some(v) = args.first() {
                    self.output.push(format!("{}\n", format_val(v)));
                }
                Ok(Value::Nil)
            }
            52 => {
                // say (alias for println)
                if let Some(v) = args.first() {
                    self.output.push(format!("{}\n", format_val(v)));
                }
                Ok(Value::Nil)
            }
            39 => {
                // str - concatenate values to string with keyword resolution
                let result: String = args.iter().map(|v| format_val(v)).collect();
                Ok(Value::String(result.into()))
            }
            // All other natives use the dispatch macro
            // Index matches order in compiler's register_natives()
            _ => native_dispatch!(idx, &args;
                // 12-13: Logic (and, or)
                12 => native_and,
                13 => native_or,
                // 14-24: Type predicates
                14 => native_nil_p,
                15 => native_some_p,
                16 => native_int_p,
                17 => native_float_p,
                18 => native_string_p,
                19 => native_keyword_p,
                20 => native_symbol_p,
                21 => native_list_p,
                22 => native_vector_p,
                23 => native_map_p,
                24 => native_set_p,
                // 25-37: Collections
                25 => native_count,
                26 => native_empty_p,
                27 => native_first,
                28 => native_rest,
                29 => native_nth,
                30 => native_conj,
                31 => native_cons,
                32 => native_get,
                33 => native_assoc,
                34 => native_dissoc,
                35 => native_disj,
                36 => native_contains_p,
                37 => native_keys,
                38 => native_vals,
                // 40-42: String basics (39 = str is handled above with context)
                40 => native_str_len,
                41 => native_str_upper,
                42 => native_str_lower,
                // 43-49: Math basics
                43 => native_abs,
                44 => native_min,
                45 => native_max,
                46 => native_floor,
                47 => native_ceil,
                48 => native_round,
                49 => native_sqrt,
                // 53: type (50-52 are print/println/say, handled above)
                53 => native_type,
                // 54-57: Critical functions
                54 => native_inc,
                55 => native_dec,
                56 => native_last,
                57 => native_range,
                // 58-71: String functions
                58 => native_str_split,
                59 => native_str_join,
                60 => native_str_trim,
                61 => native_str_trim_left,
                62 => native_str_trim_right,
                63 => native_str_starts_with,
                64 => native_str_ends_with,
                65 => native_str_contains,
                66 => native_str_replace,
                67 => native_str_replace_all,
                68 => native_str_blank,
                69 => native_str_substring,
                70 => native_format,
                71 => native_char_at,
                72 => native_parse_int,
                // 73-81: Collection functions
                73 => native_take,
                74 => native_drop,
                75 => native_concat,
                76 => native_reverse,
                77 => native_vec,
                78 => native_set,
                79 => native_into,
                80 => native_sort,
                81 => native_merge,
                // 82-99: Math functions
                82 => native_rem,
                83 => native_clamp,
                84 => native_trunc,
                85 => native_pow,
                86 => native_cbrt,
                87 => native_exp,
                88 => native_log,
                89 => native_log10,
                90 => native_log2,
                91 => native_sin,
                92 => native_cos,
                93 => native_tan,
                94 => native_asin,
                95 => native_acos,
                96 => native_atan,
                97 => native_atan2,
                98 => native_pi,
                99 => native_e,
                // 100-104: Extended collection functions
                100 => native_flatten,
                101 => native_distinct,
                102 => native_dedupe,
                103 => native_partition,
                104 => native_partition_all,
                // 105-116: Vector math functions
                105 => native_vec_add,
                106 => native_vec_sub,
                107 => native_vec_mul,
                108 => native_vec_scale,
                109 => native_vec_dot,
                110 => native_vec_cross,
                111 => native_vec_length,
                112 => native_vec_length_sq,
                113 => native_vec_normalize,
                114 => native_vec_distance,
                115 => native_vec_lerp,
                116 => native_vec_angle,
                // 117-128: Remaining functions
                117 => native_bool_p,
                118 => native_number_p,
                119 => native_coll_p,
                120 => native_fn_p,
                121 => native_entity_p,
                122 => native_sinh,
                123 => native_cosh,
                124 => native_tanh,
                125 => native_interleave,
                126 => native_interpose,
                127 => native_zip,
                128 => native_repeat,
            ),
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

fn extract_keyword<C: VmContext>(val: &Value, ctx: &C) -> Result<KeywordId> {
    match val {
        Value::Keyword(k) => Ok(*k),
        _ => {
            if let Some(k) = ctx.resolve_keyword(val) {
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
