# VM-Centric Declaration Architecture

## Vision

The Longtable VM is a **non-Von Neumann machine** with specialized data structures as first-class citizens:
- ECS (entities, components, relationships)
- Vocabulary/Parser
- Rule engine

All declarations (`component:`, `verb:`, `rule:`, etc.) should compile to opcodes that modify these internal machine structures, not be handled by the REPL.

## Current State

```
REPL intercepts declarations → calls external APIs
  ↓
(component: health ...) → REPL.execute_component() → World.register_schema()
(verb: look ...) → REPL.execute_verb() → VocabularyRegistry.register()
```

## Target State

```
Compiler emits opcodes → VM modifies internal structures
  ↓
(component: health ...) → Opcode::RegisterComponent → VM.context.register_schema()
(verb: look ...) → Opcode::RegisterVerb → VM.context.register_verb()
```

---

## Stage 1: Extend VM Context

**Goal**: Give the VM access to all machine structures, not just World.

**Current**: `VmContext` trait provides `&World` and `&mut World`

**Target**: `RuntimeContext` that provides:
```rust
trait RuntimeContext {
    // ECS (existing)
    fn world(&self) -> &World;
    fn world_mut(&mut self) -> &mut World;

    // Vocabulary
    fn vocabulary(&self) -> &VocabularyRegistry;
    fn vocabulary_mut(&mut self) -> &mut VocabularyRegistry;

    // Parser
    fn parser(&self) -> &InputParser;
    fn parser_mut(&mut self) -> &mut InputParser;

    // Actions
    fn actions(&self) -> &ActionRegistry;
    fn actions_mut(&mut self) -> &mut ActionRegistry;
}
```

**Files**:
- `crates/longtable_language/src/vm/context.rs`
- `crates/longtable_runtime/src/session.rs` (implement trait)

**Success Criteria**: VM can access all runtime structures through context.

**Status**: Complete

**Implementation Notes**:
- Created `RuntimeContext` trait extending `VmContext` with registration methods
- Added `NoRuntimeContext` stub implementation for when full runtime isn't available
- Created `SessionContext<'a>` in `longtable_runtime` wrapping `&mut Session` and `&mut Interner`
- Implemented `VmContext` for `SessionContext` (read-only World access)
- Implemented `RuntimeContext` for `SessionContext` (registration capabilities)
- Added helper functions to parse `Value` maps into schema/vocabulary types

---

## Stage 2: Schema Registration Opcodes

**Goal**: `component:` and `relationship:` compile to opcodes.

**New Opcodes**:
```rust
/// Register a component schema: [schema_map] -> []
RegisterComponent,

/// Register a relationship schema: [schema_map] -> []
RegisterRelationship,
```

**Compiler Forms**:
```clojure
(component: health
  :fields [{:name :current :type :int}
           {:name :max :type :int}])
;; Compiles to: build schema map, emit RegisterComponent

(relationship: contained-in
  :cardinality :many-to-one)
;; Compiles to: build schema map, emit RegisterRelationship
```

**Files**:
- `crates/longtable_language/src/opcode.rs` - add opcodes
- `crates/longtable_language/src/vm.rs` - handle opcodes
- `crates/longtable_language/src/compiler.rs` - add forms
- `crates/longtable_runtime/src/repl.rs` - remove special forms

**Success Criteria**:
- `(component: ...)` compiles and executes via VM
- Schema appears in World's registry
- REPL `component:` form removed

**Status**: Not Started

---

## Stage 3: Vocabulary Opcodes

**Goal**: `verb:`, `direction:`, `preposition:`, `pronoun:`, `adverb:` compile to opcodes.

**New Opcodes**:
```rust
/// Register a verb: [verb_data_map] -> []
RegisterVerb,

/// Register a direction: [dir_data_map] -> []
RegisterDirection,

/// Register a preposition: [prep_data_map] -> []
RegisterPreposition,

/// Register a pronoun: [pronoun_data_map] -> []
RegisterPronoun,

/// Register an adverb: [adverb_data_map] -> []
RegisterAdverb,
```

**Compiler Forms**:
```clojure
(verb: look :synonyms [examine inspect])
;; Compiles to: build data map, emit RegisterVerb

(direction: north :synonyms [n] :opposite south)
;; Compiles to: build data map, emit RegisterDirection
```

**Files**:
- `crates/longtable_language/src/opcode.rs`
- `crates/longtable_language/src/vm.rs`
- `crates/longtable_language/src/compiler.rs`
- `crates/longtable_runtime/src/repl.rs`

**Success Criteria**:
- All vocabulary forms compile to opcodes
- Vocabulary appears in registry
- REPL vocabulary forms removed

**Status**: Not Started

---

## Stage 4: Parser Configuration Opcodes

**Goal**: `type:`, `scope:`, `command:`, `action:` compile to opcodes.

**New Opcodes**:
```rust
/// Register a type constraint: [type_data_map] -> []
RegisterType,

/// Register a scope: [scope_data_map] -> []
RegisterScope,

/// Register a command syntax: [command_data_map] -> []
RegisterCommand,

/// Register an action handler: [action_data_map] -> []
RegisterAction,
```

**Compiler Forms**:
```clojure
(type: weapon :component :weapon)

(scope: inventory :resolver inventory-resolver)

(command: look
  :syntax [verb]
  :action look-action)

(action: look-action
  :match {:verb :look}
  :preconditions [...]
  :handler [...])
```

**Files**: Same as Stage 3

**Success Criteria**:
- All parser/action forms compile to opcodes
- Parser configured correctly
- REPL parser forms removed

**Status**: Not Started

---

## Stage 5: Rule Registration

**Goal**: `rule:` creates rule entities via VM.

**Design Decision**: Rules are entities with `:meta/rule` component. Options:
1. **Dedicated opcode**: `RegisterRule` that spawns entity + compiles rule
2. **Use spawn**: `(spawn {:meta/rule {...}})` with special handling

**Recommendation**: Dedicated opcode because rules need compilation:
```rust
/// Register a rule: [rule_data_map] -> [rule_entity_id]
RegisterRule,
```

**Compiler Form**:
```clojure
(rule: auto-heal
  :when [[?e :health ?h] [?e :regen ?r] (< ?h 100)]
  :then [(set-field ?e :health (+ ?h ?r))]
  :salience 10)
;; Compiles to: build rule data, emit RegisterRule
;; VM: spawns entity with :meta/rule, compiles pattern/action
```

**Files**: Same as above, plus rule engine integration

**Success Criteria**:
- `(rule: ...)` creates entity in World
- Rule entity has `:meta/rule` component
- Tick executor finds and runs rule entities
- REPL `rule:` form removed

**Status**: Not Started

---

## Stage 6: REPL Cleanup

**Goal**: REPL becomes a thin wrapper.

**Remaining REPL Forms** (legitimate session operations):
- `run`, `repl` - mode switching
- `load`, `save!`, `load-world!` - file I/O
- `tick!`, `input!` - session operations
- `inspect` - debugging
- Time travel: `rollback!`, `branch!`, etc.
- Debug: `trace`, `break`, `watch`, etc.

**Removed** (now opcodes):
- `component:`, `relationship:`
- `verb:`, `direction:`, `preposition:`, `pronoun:`, `adverb:`
- `type:`, `scope:`, `command:`, `action:`
- `rule:`
- `query` (becomes opcode or native function?)

**Target**: ~25-30 forms (session/debug operations only)

**Status**: Not Started

---

## Execution Order

1. **Stage 1** - RuntimeContext (unblocks everything)
2. **Stage 2** - Schema opcodes (foundational)
3. **Stage 3** - Vocabulary opcodes
4. **Stage 4** - Parser opcodes
5. **Stage 5** - Rule registration
6. **Stage 6** - Final cleanup

---

## Open Questions

1. **Query**: Should `query` become an opcode or native function? It needs World access but doesn't modify machine structure.

2. **Data format**: How should schema/vocabulary data be represented on the stack? Options:
   - As Value::Map with keyword keys
   - As dedicated Value variants (Value::SchemaDecl, etc.)

3. **Error handling**: What happens if RegisterComponent fails (duplicate name, invalid schema)?

4. **Interning**: Keywords in schemas need to be interned. Should this happen at compile time or execution time?

---

## Architecture After Refactor

```
┌─────────────────────────────────────────────────────────────┐
│                         REPL                                 │
│  - Read input                                                │
│  - Mode switching (run/repl)                                 │
│  - File I/O (load/save)                                      │
│  - Debug UI                                                  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                       Compiler                               │
│  - Language forms (fn, if, let, ...)                        │
│  - Declaration forms (component:, verb:, rule:, ...)        │
│  - Emits bytecode with specialized opcodes                  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    VM (Longtable Machine)                    │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │     ECS      │  │  Vocabulary  │  │    Parser    │       │
│  │   Schemas    │  │   Registry   │  │  + Actions   │       │
│  └──────────────┘  └──────────────┘  └──────────────┘       │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │    World     │  │     Rule     │  │   Bytecode   │       │
│  │   (Data)     │  │    Engine    │  │   Executor   │       │
│  └──────────────┘  └──────────────┘  └──────────────┘       │
│                                                              │
│  Opcodes: RegisterComponent, RegisterVerb, RegisterRule,    │
│           Spawn, Link, GetComponent, Query, ...             │
└─────────────────────────────────────────────────────────────┘
```
