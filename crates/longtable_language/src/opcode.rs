//! Bytecode instruction set for the Longtable VM.
//!
//! The VM is stack-based. Most operations consume operands from the stack
//! and push results back.

#![allow(clippy::doc_markdown)]

/// A single bytecode instruction.
#[derive(Clone, Debug, PartialEq)]
pub enum Opcode {
    // === Stack Operations ===
    /// No operation.
    Nop,
    /// Push a constant from the constant pool.
    Const(u16),
    /// Pop and discard the top of stack.
    Pop,
    /// Duplicate the top of stack.
    Dup,

    // === Arithmetic ===
    /// Add: `[a, b] -> [a + b]`
    Add,
    /// Subtract: `[a, b] -> [a - b]`
    Sub,
    /// Multiply: `[a, b] -> [a * b]`
    Mul,
    /// Divide: `[a, b] -> [a / b]`
    Div,
    /// Modulo: `[a, b] -> [a % b]`
    Mod,
    /// Negate: `[a] -> [-a]`
    Neg,

    // === Comparison ===
    /// Equal: `[a, b] -> [a == b]`
    Eq,
    /// Not equal: `[a, b] -> [a != b]`
    Ne,
    /// Less than: `[a, b] -> [a < b]`
    Lt,
    /// Less than or equal: `[a, b] -> [a <= b]`
    Le,
    /// Greater than: `[a, b] -> [a > b]`
    Gt,
    /// Greater than or equal: `[a, b] -> [a >= b]`
    Ge,

    // === Logic ===
    /// Logical not: `[a] -> [!a]`
    Not,
    /// Logical and (short-circuit handled by jumps)
    And,
    /// Logical or (short-circuit handled by jumps)
    Or,

    // === Control Flow ===
    /// Unconditional jump (relative offset).
    Jump(i16),
    /// Jump if top of stack is truthy (relative offset).
    JumpIf(i16),
    /// Jump if top of stack is falsy (relative offset).
    JumpIfNot(i16),
    /// Call a function by index in the function table.
    Call(u16),
    /// Call a native/builtin function by index with argument count.
    CallNative(u16, u8),
    /// Return from function, top of stack is return value.
    Return,

    // === Variables ===
    /// Load a local variable by slot index.
    LoadLocal(u16),
    /// Store top of stack to local variable slot.
    StoreLocal(u16),
    /// Load a value from pattern bindings (during rule execution).
    LoadBinding(u16),
    /// Load a captured variable by index (for closures).
    LoadCapture(u16),
    /// Create a closure: pops `capture_count` values, creates function with captures.
    /// Arguments: (function_index, capture_count)
    MakeClosure(u32, u16),
    /// Patch a closure's capture slot (for recursive closures).
    /// Pops the new value, then pops the closure, patches `capture[index]`, pushes closure back.
    PatchCapture(u16),

    // === Data Access (World Operations) ===
    /// Get component value: `[entity, component_kw] -> [value]`
    GetComponent,
    /// Get field from component: `[entity, component_kw, field_kw] -> [value]`
    GetField,

    // === Effects (Mutation Operations) ===
    /// Spawn entity with components map: `[components_map] -> [entity_id]`
    Spawn,
    /// Destroy entity: `[entity] -> []`
    Destroy,
    /// Set component: `[entity, component_kw, value] -> []`
    SetComponent,
    /// Set field in component: `[entity, component_kw, field_kw, value] -> []`
    SetField,
    /// Create relationship: `[source, rel_kw, target] -> []`
    Link,
    /// Remove relationship: `[source, rel_kw, target] -> []`
    Unlink,

    // === Collections ===
    /// Create empty vector: `[] -> [vec]`
    VecNew,
    /// Push to vector: `[vec, value] -> [vec']`
    VecPush,
    /// Get from vector: `[vec, index] -> [value]`
    VecGet,
    /// Vector length: `[vec] -> [len]`
    VecLen,

    /// Create empty map: `[] -> [map]`
    MapNew,
    /// Insert into map: `[map, key, value] -> [map']`
    MapInsert,
    /// Get from map: `[map, key] -> [value]`
    MapGet,
    /// Check if map contains key: `[map, key] -> [bool]`
    MapContains,

    /// Create empty set: `[] -> [set]`
    SetNew,
    /// Insert into set: `[set, value] -> [set']`
    SetInsert,
    /// Check if set contains value: `[set, value] -> [bool]`
    SetContains,

    // === Misc ===
    /// Print value (for debugging): `[value] -> []`
    Print,

    // === Higher-Order Functions ===
    /// Map function over collection: `[fn, coll] -> [result_vec]`
    /// Applies fn to each element of coll, collects results into vector.
    Map,
    /// Filter collection by predicate: `[fn, coll] -> [result_vec]`
    /// Keeps elements where fn returns truthy.
    Filter,
    /// Reduce collection with function: `[fn, init, coll] -> [result]`
    /// Folds left: (fn (fn (fn init e1) e2) e3) ...
    Reduce,
    /// Check if predicate returns truthy for all elements: `[fn, coll] -> [bool]`
    Every,
    /// Check if predicate returns truthy for any element: `[fn, coll] -> [bool | value]`
    Some,
}

/// A sequence of bytecode instructions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bytecode {
    /// The instructions.
    pub ops: Vec<Opcode>,
}

impl Bytecode {
    /// Creates an empty bytecode sequence.
    #[must_use]
    pub fn new() -> Self {
        Self { ops: Vec::new() }
    }

    /// Adds an instruction and returns its index.
    pub fn emit(&mut self, op: Opcode) -> usize {
        let idx = self.ops.len();
        self.ops.push(op);
        idx
    }

    /// Returns the current instruction count (next instruction index).
    #[must_use]
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Returns true if there are no instructions.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Patches a jump instruction at the given index with a new offset.
    ///
    /// # Panics
    /// Panics if the instruction at `idx` is not a jump instruction.
    pub fn patch_jump(&mut self, idx: usize, offset: i16) {
        match &mut self.ops[idx] {
            Opcode::Jump(o) | Opcode::JumpIf(o) | Opcode::JumpIfNot(o) => {
                *o = offset;
            }
            other => panic!("Cannot patch non-jump instruction: {other:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytecode_emit() {
        let mut bc = Bytecode::new();
        assert!(bc.is_empty());

        let idx = bc.emit(Opcode::Const(0));
        assert_eq!(idx, 0);
        assert_eq!(bc.len(), 1);

        let idx = bc.emit(Opcode::Add);
        assert_eq!(idx, 1);
        assert_eq!(bc.len(), 2);
    }

    #[test]
    fn bytecode_patch_jump() {
        let mut bc = Bytecode::new();
        let jump_idx = bc.emit(Opcode::Jump(0));
        bc.emit(Opcode::Const(1));
        bc.emit(Opcode::Const(2));

        // Patch to jump over the two Const instructions
        bc.patch_jump(jump_idx, 2);

        assert_eq!(bc.ops[jump_idx], Opcode::Jump(2));
    }

    #[test]
    #[should_panic(expected = "Cannot patch non-jump")]
    fn bytecode_patch_non_jump_panics() {
        let mut bc = Bytecode::new();
        bc.emit(Opcode::Const(0));
        bc.patch_jump(0, 5);
    }
}
