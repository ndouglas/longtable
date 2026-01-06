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

use longtable_foundation::{
    EntityId, Error, ErrorKind, KeywordId, LtMap, LtSet, LtVec, Result, Value,
};

use crate::compiler::CompiledProgram;
use crate::opcode::{Bytecode, Opcode};

// =============================================================================
// VmContext Trait
// =============================================================================

/// Provides read-only World access for VM execution.
///
/// Implement this trait to allow the VM to read entity data during rule evaluation.
pub trait VmContext {
    /// Gets a component value for an entity.
    fn get_component(&self, entity: EntityId, component: KeywordId) -> Result<Option<Value>>;

    /// Gets a specific field from a component.
    fn get_field(
        &self,
        entity: EntityId,
        component: KeywordId,
        field: KeywordId,
    ) -> Result<Option<Value>>;

    /// Checks if an entity exists.
    fn exists(&self, entity: EntityId) -> bool;

    /// Checks if an entity has a component.
    fn has_component(&self, entity: EntityId, component: KeywordId) -> bool;

    /// Resolves a keyword value to its `KeywordId` (for dynamic keyword access).
    fn resolve_keyword(&self, value: &Value) -> Option<KeywordId>;
}

// =============================================================================
// VM Effects
// =============================================================================

/// An effect produced by VM execution.
///
/// Effects represent mutations that should be applied to the World after
/// successful rule execution. Effects are collected during execution and
/// can be retrieved via [`Vm::take_effects`].
#[derive(Clone, Debug, PartialEq)]
pub enum VmEffect {
    /// Spawn a new entity with components.
    Spawn {
        /// Initial components as a map of keyword -> value.
        components: LtMap<Value, Value>,
    },

    /// Destroy an entity.
    Destroy {
        /// The entity to destroy.
        entity: EntityId,
    },

    /// Set a component on an entity.
    SetComponent {
        /// The target entity.
        entity: EntityId,
        /// The component name.
        component: KeywordId,
        /// The component value.
        value: Value,
    },

    /// Set a field within a component.
    SetField {
        /// The target entity.
        entity: EntityId,
        /// The component name.
        component: KeywordId,
        /// The field name.
        field: KeywordId,
        /// The field value.
        value: Value,
    },

    /// Create a relationship.
    Link {
        /// The source entity.
        source: EntityId,
        /// The relationship type.
        relationship: KeywordId,
        /// The target entity.
        target: EntityId,
    },

    /// Remove a relationship.
    Unlink {
        /// The source entity.
        source: EntityId,
        /// The relationship type.
        relationship: KeywordId,
        /// The target entity.
        target: EntityId,
    },
}

// =============================================================================
// WorldContext (VmContext implementation for World)
// =============================================================================

use longtable_storage::World;

/// A context that provides access to a World for VM execution.
///
/// This allows the VM to read entity data during rule evaluation.
pub struct WorldContext<'a> {
    /// Reference to the World.
    world: &'a World,
}

impl<'a> WorldContext<'a> {
    /// Creates a new `WorldContext` wrapping a World reference.
    #[must_use]
    pub fn new(world: &'a World) -> Self {
        Self { world }
    }

    /// Returns a reference to the underlying World.
    #[must_use]
    pub fn world(&self) -> &World {
        self.world
    }
}

impl VmContext for WorldContext<'_> {
    fn get_component(&self, entity: EntityId, component: KeywordId) -> Result<Option<Value>> {
        self.world.get(entity, component)
    }

    fn get_field(
        &self,
        entity: EntityId,
        component: KeywordId,
        field: KeywordId,
    ) -> Result<Option<Value>> {
        self.world.get_field(entity, component, field)
    }

    fn exists(&self, entity: EntityId) -> bool {
        self.world.exists(entity)
    }

    fn has_component(&self, entity: EntityId, component: KeywordId) -> bool {
        self.world.has(entity, component)
    }

    fn resolve_keyword(&self, value: &Value) -> Option<KeywordId> {
        // Keywords are already interned and carry their ID
        if let Value::Keyword(k) = value {
            Some(*k)
        } else {
            None
        }
    }
}

// =============================================================================
// NoContext (for pure evaluation without World)
// =============================================================================

/// A no-op context that returns errors for World operations.
///
/// Used internally when executing without a World context.
struct NoContext;

impl VmContext for NoContext {
    fn get_component(&self, _entity: EntityId, _component: KeywordId) -> Result<Option<Value>> {
        Err(Error::new(ErrorKind::Internal(
            "world operations not available in this context".to_string(),
        )))
    }

    fn get_field(
        &self,
        _entity: EntityId,
        _component: KeywordId,
        _field: KeywordId,
    ) -> Result<Option<Value>> {
        Err(Error::new(ErrorKind::Internal(
            "world operations not available in this context".to_string(),
        )))
    }

    fn exists(&self, _entity: EntityId) -> bool {
        false
    }

    fn has_component(&self, _entity: EntityId, _component: KeywordId) -> bool {
        false
    }

    fn resolve_keyword(&self, _value: &Value) -> Option<KeywordId> {
        None
    }
}

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
            bindings: Vec::new(),
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
                        Value::Fn(longtable_foundation::LtFn::Compiled(f)) => f,
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::Fn(
                                    longtable_foundation::types::Arity::Variadic(0),
                                ),
                                actual: func_val.value_type(),
                            }));
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

                    // Set up arguments as locals
                    for (i, arg) in args.into_iter().enumerate() {
                        self.locals[i] = arg;
                    }

                    // Execute the function body
                    let result =
                        self.execute_internal::<C>(&func.code, constants, functions, ctx)?;

                    // Restore VM state
                    self.ip = saved_ip;
                    self.locals = saved_locals;

                    // Push result
                    self.push(result);
                }
                Opcode::CallNative(idx, arg_count) => {
                    self.call_native(*idx, *arg_count)?;
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

                // Data access (World operations)
                Opcode::GetComponent => {
                    // Stack: [entity, component_kw] -> [value]
                    let component_val = self.pop()?;
                    let entity_val = self.pop()?;

                    let entity = match entity_val {
                        Value::EntityRef(e) => e,
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::EntityRef,
                                actual: entity_val.value_type(),
                            }));
                        }
                    };

                    let component = match &component_val {
                        Value::Keyword(k) => *k,
                        _ => {
                            if let Some(c) =
                                ctx.as_ref().and_then(|c| c.resolve_keyword(&component_val))
                            {
                                c
                            } else {
                                return Err(Error::new(ErrorKind::TypeMismatch {
                                    expected: longtable_foundation::Type::Keyword,
                                    actual: component_val.value_type(),
                                }));
                            }
                        }
                    };

                    let value = if let Some(c) = ctx.as_ref() {
                        c.get_component(entity, component)?.unwrap_or(Value::Nil)
                    } else {
                        return Err(Error::new(ErrorKind::Internal(
                            "world context required for GetComponent".to_string(),
                        )));
                    };
                    self.push(value);
                }

                Opcode::GetField => {
                    // Stack: [entity, component_kw, field_kw] -> [value]
                    let field_val = self.pop()?;
                    let component_val = self.pop()?;
                    let entity_val = self.pop()?;

                    let entity = match entity_val {
                        Value::EntityRef(e) => e,
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::EntityRef,
                                actual: entity_val.value_type(),
                            }));
                        }
                    };

                    let component = match &component_val {
                        Value::Keyword(k) => *k,
                        _ => {
                            if let Some(c) =
                                ctx.as_ref().and_then(|c| c.resolve_keyword(&component_val))
                            {
                                c
                            } else {
                                return Err(Error::new(ErrorKind::TypeMismatch {
                                    expected: longtable_foundation::Type::Keyword,
                                    actual: component_val.value_type(),
                                }));
                            }
                        }
                    };

                    let field = match &field_val {
                        Value::Keyword(k) => *k,
                        _ => {
                            if let Some(f) =
                                ctx.as_ref().and_then(|c| c.resolve_keyword(&field_val))
                            {
                                f
                            } else {
                                return Err(Error::new(ErrorKind::TypeMismatch {
                                    expected: longtable_foundation::Type::Keyword,
                                    actual: field_val.value_type(),
                                }));
                            }
                        }
                    };

                    let value = if let Some(c) = ctx.as_ref() {
                        c.get_field(entity, component, field)?.unwrap_or(Value::Nil)
                    } else {
                        return Err(Error::new(ErrorKind::Internal(
                            "world context required for GetField".to_string(),
                        )));
                    };
                    self.push(value);
                }

                // Effects (collected for deferred application)
                Opcode::Spawn => {
                    // Stack: [components_map] -> [entity_id]
                    let components_val = self.pop()?;

                    let components = match components_val {
                        Value::Map(m) => m,
                        _ => LtMap::new(), // Empty spawn if no map
                    };

                    // Create a temporary entity ID for tracking
                    self.spawn_counter += 1;
                    let temp_id = EntityId {
                        index: self.spawn_counter,
                        generation: 0, // Generation 0 indicates a temporary spawn ID
                    };

                    self.effects.push(VmEffect::Spawn { components });
                    self.push(Value::EntityRef(temp_id));
                }

                Opcode::Destroy => {
                    // Stack: [entity] -> []
                    let entity_val = self.pop()?;

                    let entity = match entity_val {
                        Value::EntityRef(e) => e,
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::EntityRef,
                                actual: entity_val.value_type(),
                            }));
                        }
                    };

                    self.effects.push(VmEffect::Destroy { entity });
                }

                Opcode::SetComponent => {
                    // Stack: [entity, component_kw, value] -> []
                    let value = self.pop()?;
                    let component_val = self.pop()?;
                    let entity_val = self.pop()?;

                    let entity = match entity_val {
                        Value::EntityRef(e) => e,
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::EntityRef,
                                actual: entity_val.value_type(),
                            }));
                        }
                    };

                    let component = match &component_val {
                        Value::Keyword(k) => *k,
                        _ => {
                            if let Some(c) =
                                ctx.as_ref().and_then(|c| c.resolve_keyword(&component_val))
                            {
                                c
                            } else {
                                return Err(Error::new(ErrorKind::TypeMismatch {
                                    expected: longtable_foundation::Type::Keyword,
                                    actual: component_val.value_type(),
                                }));
                            }
                        }
                    };

                    self.effects.push(VmEffect::SetComponent {
                        entity,
                        component,
                        value,
                    });
                }

                Opcode::SetField => {
                    // Stack: [entity, component_kw, field_kw, value] -> []
                    let value = self.pop()?;
                    let field_val = self.pop()?;
                    let component_val = self.pop()?;
                    let entity_val = self.pop()?;

                    let entity = match entity_val {
                        Value::EntityRef(e) => e,
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::EntityRef,
                                actual: entity_val.value_type(),
                            }));
                        }
                    };

                    let component = match &component_val {
                        Value::Keyword(k) => *k,
                        _ => {
                            if let Some(c) =
                                ctx.as_ref().and_then(|c| c.resolve_keyword(&component_val))
                            {
                                c
                            } else {
                                return Err(Error::new(ErrorKind::TypeMismatch {
                                    expected: longtable_foundation::Type::Keyword,
                                    actual: component_val.value_type(),
                                }));
                            }
                        }
                    };

                    let field = match &field_val {
                        Value::Keyword(k) => *k,
                        _ => {
                            if let Some(f) =
                                ctx.as_ref().and_then(|c| c.resolve_keyword(&field_val))
                            {
                                f
                            } else {
                                return Err(Error::new(ErrorKind::TypeMismatch {
                                    expected: longtable_foundation::Type::Keyword,
                                    actual: field_val.value_type(),
                                }));
                            }
                        }
                    };

                    self.effects.push(VmEffect::SetField {
                        entity,
                        component,
                        field,
                        value,
                    });
                }

                Opcode::Link => {
                    // Stack: [source, rel_kw, target] -> []
                    let target_val = self.pop()?;
                    let relationship_val = self.pop()?;
                    let source_val = self.pop()?;

                    let source = match source_val {
                        Value::EntityRef(e) => e,
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::EntityRef,
                                actual: source_val.value_type(),
                            }));
                        }
                    };

                    let target = match target_val {
                        Value::EntityRef(e) => e,
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::EntityRef,
                                actual: target_val.value_type(),
                            }));
                        }
                    };

                    let relationship = match &relationship_val {
                        Value::Keyword(k) => *k,
                        _ => {
                            if let Some(r) = ctx
                                .as_ref()
                                .and_then(|c| c.resolve_keyword(&relationship_val))
                            {
                                r
                            } else {
                                return Err(Error::new(ErrorKind::TypeMismatch {
                                    expected: longtable_foundation::Type::Keyword,
                                    actual: relationship_val.value_type(),
                                }));
                            }
                        }
                    };

                    self.effects.push(VmEffect::Link {
                        source,
                        relationship,
                        target,
                    });
                }

                Opcode::Unlink => {
                    // Stack: [source, rel_kw, target] -> []
                    let target_val = self.pop()?;
                    let relationship_val = self.pop()?;
                    let source_val = self.pop()?;

                    let source = match source_val {
                        Value::EntityRef(e) => e,
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::EntityRef,
                                actual: source_val.value_type(),
                            }));
                        }
                    };

                    let target = match target_val {
                        Value::EntityRef(e) => e,
                        _ => {
                            return Err(Error::new(ErrorKind::TypeMismatch {
                                expected: longtable_foundation::Type::EntityRef,
                                actual: target_val.value_type(),
                            }));
                        }
                    };

                    let relationship = match &relationship_val {
                        Value::Keyword(k) => *k,
                        _ => {
                            if let Some(r) = ctx
                                .as_ref()
                                .and_then(|c| c.resolve_keyword(&relationship_val))
                            {
                                r
                            } else {
                                return Err(Error::new(ErrorKind::TypeMismatch {
                                    expected: longtable_foundation::Type::Keyword,
                                    actual: relationship_val.value_type(),
                                }));
                            }
                        }
                    };

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
            _ => Err(Error::new(ErrorKind::Internal(format!(
                "unknown native function index: {idx}"
            )))),
        }?;

        self.push(result);
        Ok(())
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

// =============================================================================
// Native Function Implementations
// =============================================================================

/// Logic: and - returns first falsy value or last value
fn native_and(args: &[Value]) -> Result<Value> {
    for arg in args {
        if !is_truthy(arg) {
            return Ok(arg.clone());
        }
    }
    Ok(args.last().cloned().unwrap_or(Value::Bool(true)))
}

/// Logic: or - returns first truthy value or last value
fn native_or(args: &[Value]) -> Result<Value> {
    for arg in args {
        if is_truthy(arg) {
            return Ok(arg.clone());
        }
    }
    Ok(args.last().cloned().unwrap_or(Value::Bool(false)))
}

/// Predicate: nil?
fn native_nil_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Nil))))
}

/// Predicate: some?
fn native_some_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(!matches!(
        args.first(),
        Some(Value::Nil) | None
    )))
}

/// Predicate: int?
fn native_int_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Int(_)))))
}

/// Predicate: float?
fn native_float_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Float(_)))))
}

/// Predicate: string?
fn native_string_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::String(_)))))
}

/// Predicate: keyword?
fn native_keyword_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Keyword(_)))))
}

/// Predicate: symbol?
fn native_symbol_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Symbol(_)))))
}

/// Predicate: list? (vectors are lists in our model)
fn native_list_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Vec(_)))))
}

/// Predicate: vector?
fn native_vector_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Vec(_)))))
}

/// Predicate: map?
fn native_map_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Map(_)))))
}

/// Predicate: set?
fn native_set_p(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Set(_)))))
}

/// Collection: count
fn native_count(args: &[Value]) -> Result<Value> {
    let count = match args.first() {
        Some(Value::Vec(v)) => v.len() as i64,
        Some(Value::Set(s)) => s.len() as i64,
        Some(Value::Map(m)) => m.len() as i64,
        Some(Value::String(s)) => s.len() as i64,
        Some(Value::Nil) => 0,
        _ => {
            return Err(Error::new(ErrorKind::TypeMismatch {
                expected: longtable_foundation::Type::Vec(Box::new(
                    longtable_foundation::Type::Any,
                )),
                actual: args
                    .first()
                    .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
            }));
        }
    };
    Ok(Value::Int(count))
}

/// Collection: empty?
fn native_empty_p(args: &[Value]) -> Result<Value> {
    let empty = match args.first() {
        Some(Value::Vec(v)) => v.is_empty(),
        Some(Value::Set(s)) => s.is_empty(),
        Some(Value::Map(m)) => m.is_empty(),
        Some(Value::String(s)) => s.is_empty(),
        Some(Value::Nil) => true,
        _ => false,
    };
    Ok(Value::Bool(empty))
}

/// Collection: first
fn native_first(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Vec(v)) => Ok(v.first().cloned().unwrap_or(Value::Nil)),
        Some(Value::Nil) => Ok(Value::Nil),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Vec(Box::new(longtable_foundation::Type::Any)),
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: rest
fn native_rest(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Vec(v)) => {
            if v.is_empty() {
                Ok(Value::Vec(LtVec::new()))
            } else {
                // Skip the first element
                let rest: LtVec<Value> = v.iter().skip(1).cloned().collect();
                Ok(Value::Vec(rest))
            }
        }
        Some(Value::Nil) => Ok(Value::Vec(LtVec::new())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Vec(Box::new(longtable_foundation::Type::Any)),
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: nth
fn native_nth(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Vec(v)), Some(Value::Int(idx))) => {
            let idx = *idx as usize;
            Ok(v.get(idx).cloned().unwrap_or(Value::Nil))
        }
        (Some(Value::String(s)), Some(Value::Int(idx))) => {
            let idx = *idx as usize;
            Ok(s.chars()
                .nth(idx)
                .map_or(Value::Nil, |c| Value::String(c.to_string().into())))
        }
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Vec(Box::new(longtable_foundation::Type::Any)),
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: conj (add to collection)
fn native_conj(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Vec(v)) => {
            let mut result = v.clone();
            for arg in args.iter().skip(1) {
                result = result.push_back(arg.clone());
            }
            Ok(Value::Vec(result))
        }
        Some(Value::Set(s)) => {
            let mut result = s.clone();
            for arg in args.iter().skip(1) {
                result = result.insert(arg.clone());
            }
            Ok(Value::Set(result))
        }
        Some(Value::Nil) => {
            // conj on nil creates a vector
            let mut result = LtVec::new();
            for arg in args.iter().skip(1) {
                result = result.push_back(arg.clone());
            }
            Ok(Value::Vec(result))
        }
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Vec(Box::new(longtable_foundation::Type::Any)),
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: cons (prepend to collection)
fn native_cons(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(elem), Some(Value::Vec(v))) => {
            let mut result = LtVec::new();
            result = result.push_back(elem.clone());
            for item in v.iter() {
                result = result.push_back(item.clone());
            }
            Ok(Value::Vec(result))
        }
        (Some(elem), Some(Value::Nil)) => {
            let mut result = LtVec::new();
            result = result.push_back(elem.clone());
            Ok(Value::Vec(result))
        }
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Vec(Box::new(longtable_foundation::Type::Any)),
            actual: args
                .get(1)
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: get
fn native_get(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Map(m)), Some(key)) => Ok(m.get(key).cloned().unwrap_or(Value::Nil)),
        (Some(Value::Vec(v)), Some(Value::Int(idx))) => {
            let idx = *idx as usize;
            Ok(v.get(idx).cloned().unwrap_or(Value::Nil))
        }
        (Some(Value::Nil), _) => Ok(Value::Nil),
        _ => Ok(Value::Nil),
    }
}

/// Collection: assoc
fn native_assoc(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Map(m)) => {
            let mut result = m.clone();
            let mut i = 1;
            while i + 1 < args.len() {
                result = result.insert(args[i].clone(), args[i + 1].clone());
                i += 2;
            }
            Ok(Value::Map(result))
        }
        Some(Value::Vec(v)) => {
            let mut result = v.clone();
            let mut i = 1;
            while i + 1 < args.len() {
                if let Value::Int(idx) = &args[i] {
                    let idx = *idx as usize;
                    if idx < result.len() {
                        result = result.update(idx, args[i + 1].clone()).unwrap_or(result);
                    }
                }
                i += 2;
            }
            Ok(Value::Vec(result))
        }
        Some(Value::Nil) => {
            // assoc on nil creates a map
            let mut result = LtMap::new();
            let mut i = 1;
            while i + 1 < args.len() {
                result = result.insert(args[i].clone(), args[i + 1].clone());
                i += 2;
            }
            Ok(Value::Map(result))
        }
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Map(
                Box::new(longtable_foundation::Type::Any),
                Box::new(longtable_foundation::Type::Any),
            ),
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: dissoc
fn native_dissoc(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Map(m)) => {
            let mut result = m.clone();
            for key in args.iter().skip(1) {
                result = result.remove(key);
            }
            Ok(Value::Map(result))
        }
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Map(
                Box::new(longtable_foundation::Type::Any),
                Box::new(longtable_foundation::Type::Any),
            ),
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: contains?
fn native_contains_p(args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Map(m)), Some(key)) => Ok(Value::Bool(m.contains_key(key))),
        (Some(Value::Set(s)), Some(elem)) => Ok(Value::Bool(s.contains(elem))),
        (Some(Value::Vec(v)), Some(Value::Int(idx))) => {
            let idx = *idx as usize;
            Ok(Value::Bool(idx < v.len()))
        }
        _ => Ok(Value::Bool(false)),
    }
}

/// Collection: keys
fn native_keys(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Map(m)) => {
            let keys: LtVec<Value> = m.keys().cloned().collect();
            Ok(Value::Vec(keys))
        }
        Some(Value::Nil) => Ok(Value::Vec(LtVec::new())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Map(
                Box::new(longtable_foundation::Type::Any),
                Box::new(longtable_foundation::Type::Any),
            ),
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Collection: vals
fn native_vals(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Map(m)) => {
            let vals: LtVec<Value> = m.values().cloned().collect();
            Ok(Value::Vec(vals))
        }
        Some(Value::Nil) => Ok(Value::Vec(LtVec::new())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Map(
                Box::new(longtable_foundation::Type::Any),
                Box::new(longtable_foundation::Type::Any),
            ),
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// String: str (concatenate to string)
fn native_str(args: &[Value]) -> Result<Value> {
    let result: String = args.iter().map(format_value).collect();
    Ok(Value::String(result.into()))
}

/// String: str/len
fn native_str_len(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::String(s)) => Ok(Value::Int(s.len() as i64)),
        Some(Value::Nil) => Ok(Value::Int(0)),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::String,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// String: str/upper
fn native_str_upper(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::String(s)) => Ok(Value::String(s.to_uppercase().into())),
        Some(Value::Nil) => Ok(Value::String("".into())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::String,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// String: str/lower
fn native_str_lower(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::String(s)) => Ok(Value::String(s.to_lowercase().into())),
        Some(Value::Nil) => Ok(Value::String("".into())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::String,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: abs
fn native_abs(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(n.abs())),
        Some(Value::Float(n)) => Ok(Value::Float(n.abs())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Int,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: min
fn native_min(args: &[Value]) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::new(ErrorKind::Internal(
            "min requires at least one argument".to_string(),
        )));
    }
    let mut result = args[0].clone();
    for arg in args.iter().skip(1) {
        result = match (&result, arg) {
            (Value::Int(a), Value::Int(b)) => Value::Int(*a.min(b)),
            (Value::Float(a), Value::Float(b)) => Value::Float(a.min(*b)),
            (Value::Int(a), Value::Float(b)) => Value::Float((*a as f64).min(*b)),
            (Value::Float(a), Value::Int(b)) => Value::Float(a.min(*b as f64)),
            _ => {
                return Err(Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Int,
                    actual: arg.value_type(),
                }));
            }
        };
    }
    Ok(result)
}

/// Math: max
fn native_max(args: &[Value]) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::new(ErrorKind::Internal(
            "max requires at least one argument".to_string(),
        )));
    }
    let mut result = args[0].clone();
    for arg in args.iter().skip(1) {
        result = match (&result, arg) {
            (Value::Int(a), Value::Int(b)) => Value::Int(*a.max(b)),
            (Value::Float(a), Value::Float(b)) => Value::Float(a.max(*b)),
            (Value::Int(a), Value::Float(b)) => Value::Float((*a as f64).max(*b)),
            (Value::Float(a), Value::Int(b)) => Value::Float(a.max(*b as f64)),
            _ => {
                return Err(Error::new(ErrorKind::TypeMismatch {
                    expected: longtable_foundation::Type::Int,
                    actual: arg.value_type(),
                }));
            }
        };
    }
    Ok(result)
}

/// Math: floor
fn native_floor(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(*n)),
        Some(Value::Float(n)) => Ok(Value::Float(n.floor())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: ceil
fn native_ceil(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(*n)),
        Some(Value::Float(n)) => Ok(Value::Float(n.ceil())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: round
fn native_round(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(*n)),
        Some(Value::Float(n)) => Ok(Value::Float(n.round())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Math: sqrt
fn native_sqrt(args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).sqrt())),
        Some(Value::Float(n)) => Ok(Value::Float(n.sqrt())),
        _ => Err(Error::new(ErrorKind::TypeMismatch {
            expected: longtable_foundation::Type::Float,
            actual: args
                .first()
                .map_or(longtable_foundation::Type::Nil, |v| v.value_type()),
        })),
    }
}

/// Misc: type (returns type as keyword string)
fn native_type(args: &[Value]) -> Result<Value> {
    let type_name = match args.first() {
        Some(Value::Nil) => "nil",
        Some(Value::Bool(_)) => "bool",
        Some(Value::Int(_)) => "int",
        Some(Value::Float(_)) => "float",
        Some(Value::String(_)) => "string",
        Some(Value::Symbol(_)) => "symbol",
        Some(Value::Keyword(_)) => "keyword",
        Some(Value::EntityRef(_)) => "entity",
        Some(Value::Vec(_)) => "vector",
        Some(Value::Set(_)) => "set",
        Some(Value::Map(_)) => "map",
        Some(Value::Fn(_)) => "fn",
        None => "nil",
    };
    Ok(Value::String(format!(":{type_name}").into()))
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
    #[ignore = "recursive functions require closures, not yet implemented"]
    fn eval_fn_recursive_with_def() {
        // Recursive function using let binding
        // Note: This requires the function to be accessible from its own body via closures
        let result =
            eval_test("(let [fact (fn [n] (if (<= n 1) 1 (* n (fact (- n 1)))))] (fact 5))");
        assert_eq!(result, Value::Int(120));
    }

    #[test]
    fn eval_fn_multi_body() {
        // Function with multiple expressions in body (implicit do)
        let result = eval_test("((fn [x] (+ 1 1) (+ x 10)) 5)");
        assert_eq!(result, Value::Int(15));
    }
}
