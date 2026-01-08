# REPL Special Forms Refactoring Plan

## Problem

The REPL has accumulated 54 special forms that should be handled by the compiler/VM. The REPL is essentially a second compiler, which is wrong.

## Goal

The REPL should be a thin wrapper that:
1. Reads input
2. Parses to AST
3. Compiles to bytecode
4. Executes on VM
5. Prints result
6. Handles mode switching (`run`/`repl`)

Everything else should be in the language itself.

---

## Phase 1: Core Language Forms → Compiler ✅ DONE

**Target: `longtable_language/src/compiler.rs`**

| Form         | Current              | Target               | Notes                                            |
| ------------ | -------------------- | -------------------- | ------------------------------------------------ |
| `fn:`        | `def`/`defn` in REPL | Compiler declaration | ✅ DONE - Matches `component:`, `action:`, etc. convention |
| `entity-ref` | REPL + Compiler      | Compiler only        | ✅ DONE                                           |

### Implementation ✅ COMPLETED

**`fn:`** - Function/variable definition following the declaration convention:

```clojure
;; Define a value
(fn: my-value 42)

;; Define a function
(fn: add [a b]
  (+ a b))

;; Define a function with docstring
(fn: add
  "Adds two numbers"
  [a b]
  (+ a b))
```

The compiler now has a "global environment" that persists across compilations:
- `globals: HashMap<String, u16>` - maps names to slot indices
- `next_global: u16` - next available slot
- `prepare_for_compilation()` - resets per-compilation state while preserving globals
- `LoadGlobal(slot)` / `StoreGlobal(slot)` opcodes added
- REPL uses a persistent `Compiler` instance

---

## Phase 2: World Operations → VM Opcodes

**Target: `longtable_language/src/opcode.rs` + `vm.rs`**

| Form            | Current       | Target                 | Notes            |
| --------------- | ------------- | ---------------------- | ---------------- |
| `spawn`         | REPL `spawn:` | `Opcode::Spawn`        | ✅ Already exists |
| `link`          | REPL `link:`  | `Opcode::Link`         | ✅ Already exists |
| `unlink`        | ?             | `Opcode::Unlink`       | ✅ Already exists |
| `destroy`       | ?             | `Opcode::Destroy`      | ✅ Already exists |
| `set-component` | ?             | `Opcode::SetComponent` | ✅ Already exists |
| `set-field`     | ?             | `Opcode::SetField`     | ✅ Already exists |

### Implementation

The opcodes exist but need compiler support to generate them from function calls:
- `(spawn {:name "foo"})` → compile map, emit `Opcode::Spawn`
- `(link source :rel-type target)` → compile args, emit `Opcode::Link`
- etc.

---

## Phase 3: Schema Declarations → Compiler + World

**Target: Compile-time processing with world mutation**

| Form            | Current | Target               | Notes                     |
| --------------- | ------- | -------------------- | ------------------------- |
| `component:`    | REPL    | Compiler declaration | Registers schema in World |
| `relationship:` | REPL    | Compiler declaration | Registers schema in World |

### Implementation

Two options:

**Option A: Compile-time effects**
- Compiler recognizes declarations
- Emits "schema registration" effects
- REPL/runtime applies effects to World

**Option B: World passed to compiler**
- Compiler receives `&mut World`
- Directly registers schemas during compilation
- Simpler but tighter coupling

Recommend **Option A** for cleaner separation.

---

## Phase 4: Parser Vocabulary → Compiler Declarations

**Target: Compiler + Parser integration**

| Form           | Current | Target               | Notes                          |
| -------------- | ------- | -------------------- | ------------------------------ |
| `verb:`        | REPL    | Compiler declaration | Register in VocabularyRegistry |
| `direction:`   | REPL    | Compiler declaration | Register direction + vocab     |
| `preposition:` | REPL    | Compiler declaration | Register in VocabularyRegistry |
| `pronoun:`     | REPL    | Compiler declaration | Register in VocabularyRegistry |
| `adverb:`      | REPL    | Compiler declaration | Register in VocabularyRegistry |
| `type:`        | REPL    | Compiler declaration | Register type in parser        |
| `scope:`       | REPL    | Compiler declaration | Register scope                 |
| `command:`     | REPL    | Compiler declaration | Register command syntax        |
| `action:`      | REPL    | Compiler declaration | Register action + handler      |

### Implementation

Similar to Phase 3 - declarations produce effects that get applied to registries.

---

## Phase 5: Native Functions ✅ DONE

**Target: `longtable_language/src/vm/native/`**

| Form      | Current       | Target          | Notes                      |
| --------- | ------------- | --------------- | -------------------------- |
| `print`   | N/A           | Native function | ✅ DONE - Print without newline      |
| `println` | `say` in REPL | Native function | ✅ DONE - Print with newline         |
| `inspect` | REPL          | Native function | Pending - Debug print with type info |

**Note:** `say` is reserved for the DSL (action handlers use `(say "message")` which will call `println` internally).

### Implementation ✅ COMPLETED

`print` and `println` are implemented in `vm.rs` as native functions (indices 49 and 50).
They add to `vm.output` which is collected and printed by the REPL.

The DSL's `(say ...)` in action handlers will be translated to `(println ...)` during action execution.

---

## Phase 6: Query & Rule Engine → Compiler/VM

**Target: Proper compilation of queries and rules**

| Form            | Current | Target                       | Notes                  |
| --------------- | ------- | ---------------------------- | ---------------------- |
| `query`         | REPL    | Compiler form → VM execution | Pattern matching in VM |
| `rule:`         | REPL    | Compiler declaration         | Compile rule, register |
| `why`           | REPL    | Native function              | Debugging aid          |
| `explain-query` | REPL    | Native function              | Debugging aid          |

### Implementation

1. Query patterns compile to pattern-matching bytecode
2. Rules compile to conditional bytecode with pattern matching
3. Rule engine runs as part of `tick` execution

---

## Phase 7: Debugging & Time Travel

**Target: Keep in REPL or move to debug module**

| Form                            | Current | Target       | Notes               |
| ------------------------------- | ------- | ------------ | ------------------- |
| `trace`, `break`, `watch`, etc. | REPL    | Stay in REPL | Need session state  |
| `rollback!`, `branch!`, etc.    | REPL    | Stay in REPL | Need timeline state |
| `history`, `timeline`           | REPL    | Stay in REPL | Display functions   |

These legitimately need REPL/session state. Could be refactored to:
- Debug module provides API
- REPL calls debug module
- But special forms can stay for now

---

## Phase 8: File I/O

**Target: Native functions or minimal REPL**

| Form          | Current | Target         | Notes                       |
| ------------- | ------- | -------------- | --------------------------- |
| `load`        | REPL    | Native or REPL | Needs load path context     |
| `save!`       | REPL    | Native or REPL | Needs world serialization   |
| `load-world!` | REPL    | Native or REPL | Needs world deserialization |

Could stay as REPL forms since they need session context, OR pass session to native functions.

---

## Final REPL Special Forms

After refactoring, the REPL should only have:

1. **`run`** - Enter input mode
2. **`repl`** - Exit input mode
3. **Debugging commands** (if not moved to functions)
4. **Time travel commands** (if not moved to functions)
5. **File I/O** (if not moved to functions)

Target: **~10-15 forms** down from **54**.

---

## Execution Order

1. **Phase 1** - `fn:` declaration (unblocks stdlib loading) ✅ DONE
2. **Phase 5** - `print`/`println` as native (unblocks action handlers) ✅ DONE
3. **Phase 2** - World operations as compiler forms
4. **Phase 3** - Schema declarations
5. **Phase 4** - Parser vocabulary
6. **Phase 6** - Query/rule compilation
7. **Phase 7-8** - Cleanup remaining forms

## Convention Summary

All declarations follow the `name:` pattern:
- `fn:` - Function/value definition
- `component:` - Component schema
- `relationship:` - Relationship schema
- `rule:` - Rule definition
- `action:` - Action definition
- `verb:`, `direction:`, etc. - Parser vocabulary

DSL commands in action handlers:
- `(say ...)` - DSL command, internally calls `println`
- `(print ...)` / `(println ...)` - Direct output functions

---

## Architecture After Refactoring

```
┌─────────────────────────────────────────────────────────────┐
│                         REPL                                 │
│  - Read input                                                │
│  - Mode switching (run/repl)                                 │
│  - File loading (delegates to compiler)                      │
│  - Debug UI (delegates to debug module)                      │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                       Compiler                               │
│  - All language forms (def, defn, fn, if, let, ...)         │
│  - All declarations (component:, action:, rule:, ...)       │
│  - Emits bytecode + declaration effects                      │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                          VM                                  │
│  - Executes bytecode                                         │
│  - Native functions (say, str, math, ...)                   │
│  - World operations (spawn, link, get, set, ...)            │
│  - Produces effects for world mutation                       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                        World                                 │
│  - Entity storage                                            │
│  - Component storage                                         │
│  - Relationship storage                                      │
│  - Schema registry                                           │
└─────────────────────────────────────────────────────────────┘
```
