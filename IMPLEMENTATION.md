# Longtable Implementation Plan

## Philosophy

This plan follows an **API-first, contract-driven** approach:

1. **Decompose** the system into major concerns with clear boundaries
2. **Design** stable APIs for each concern that maximize flexibility and power
3. **Validate** that APIs compose correctly and can achieve spec goals
4. **Implement** each subsystem thoroughly without cross-cutting changes
5. **Benchmark** continuously to ensure performance targets are met
6. **Exercise** capabilities through examples that evolve with the system

### Guiding Principles

- **Public APIs are stable; internal execution APIs are provisional** — see API Stability Tiers below
- **TDD throughout** — tests define behavior before implementation
- **Correctness before performance** — get semantics right first, optimize later
- **Examples are tests** — working examples prove the system works
- **Result everywhere** — all fallible operations return `Result<T, Error>` (use `thiserror` for derives)
- **Single-threaded** — simplicity over concurrency (but see caveat below)
- **Ship tiny vertical wins** — morale matters; see below
- **Know what this isn't** — prevent scope creep; see Non-Goals

### The "Single-Threaded" Half-Truth

We say "single-threaded" but we're already thinking in:
- Speculative worlds (branching futures)
- Memoization (content hashing)
- Replay (deterministic re-execution)

That's **concurrency in disguise**. It won't save us from:
- Interior mutability decisions that matter
- Borrow patterns that bite
- Cache invalidation complexity

Don't underestimate this when implementing World and caches. "Single-threaded" means no `Send`/`Sync` constraints, not "simple."

### What This System Is Bad At (Non-Goals)

Explicitly documenting limitations prevents scope creep and keeps us honest:

| Bad At                             | Why                                     | Don't Try To Fix       |
| ---------------------------------- | --------------------------------------- | ---------------------- |
| Twitch real-time games             | Ticks can take seconds; no frame budget | Adding "fast mode"     |
| Very large worlds (100k+ entities) | Initial impl is O(n) matching           | Premature optimization |
| Opaque imperative scripts          | Rules are declarative and reactive      | Adding "script mode"   |
| Non-deterministic behavior         | Determinism is a core guarantee         | Adding "random mode"   |
| Tight Rust integration             | DSL defines all domain logic            | Typed Rust components  |

If someone asks "can Longtable do X?" and X is on this list, the answer is "not well, by design."

### Managing Energy on a Long Roadmap

This is a substantial solo project. Technical discipline is necessary but not sufficient—morale management is equally important.

**Danger zones**:
- Phase 3 (Language): Parser + compiler + VM can induce fatigue
- Phase 4 (Execution): Semantic bugs that don't reproduce drain energy

**Mitigation strategy**: Ship **tiny vertical wins** early:

| Win                                | When          | Why It Helps                    |
| ---------------------------------- | ------------- | ------------------------------- |
| REPL that mutates world (no rules) | Phase 2.5     | Something interactive exists    |
| Queries work                       | Phase 2.5     | You can explore data            |
| Adventure world exists (static)    | Phase 2.5     | The example is real             |
| One rule fires correctly           | Phase 4 start | Rules are no longer theoretical |
| Adventure game plays               | Phase 4 end   | The system is complete          |

Use the adventure game example as a **motivational artifact**, not just a test. Seeing "go north" work for the first time is worth more than a dozen passing unit tests.

**Warning: Examples Become Anchors**

"Examples are tests" is philosophically correct but practically dangerous:
- Examples evolve slower than code
- Fixing examples is emotionally heavier than fixing unit tests
- You'll hesitate to refactor because "the adventure game breaks"

Be aware: examples become **anchors**. You'll need discipline to prune or rewrite them when semantics demand it. Don't let a pretty example prevent a necessary change.

### API Stability Tiers

Not all APIs are equal. We distinguish:

| Tier               | Stability                       | Examples                                          | When Frozen           |
| ------------------ | ------------------------------- | ------------------------------------------------- | --------------------- |
| **Public Data**    | Stable after Phase 1            | `World`, `Value`, `EntityId`, `Error`             | API Design phase      |
| **DSL Semantics**  | Stable after Phase 1.5          | Rule behavior, query semantics, effect visibility | Semantic Spike        |
| **DSL Syntax**     | Provisional until Phase 3       | Exact syntax forms, keywords, sugar               | Language phase        |
| **Execution APIs** | Semantic sketches until Phase 4 | `CompiledRule`, `PatternMatcher`, `Opcode`, VM    | After Execution works |
| **Private**        | Can change anytime              | Storage layout, cache structures                  | Never                 |

**Why this matters**: Rule semantics, pattern matching edge cases, and VM behavior will evolve as we discover issues. Freezing internal execution APIs too early forces contortions or breaks our own rules.

**Critical distinction**: DSL is "semantically stable, syntactically provisional" until Phase 3. If semantics demand syntax changes, change the syntax. Don't let social pressure ("we already documented it") prevent necessary evolution.

**Practically**:
- Mark `Compiled*`, matcher internals, and VM opcodes as `#[doc(hidden)]` or `pub(crate)` until Phase 4 completes
- Treat `World`, `Value`, `EntityId`, `Error` as the true stability boundary
- Layer 3 (Execution) APIs in Phase 1 are **expected to be wrong**—they're semantic sketches, not contracts
- Users should never depend on internal representations

---

## Architectural Decomposition

The system is organized into six layers, each depending only on layers below:

```
┌─────────────────────────────────────────────────────────────────┐
│  Layer 5: OBSERVABILITY                                         │
│  Tracing, Debugging, Time Travel, Explain                       │
├─────────────────────────────────────────────────────────────────┤
│  Layer 4: INTERFACE                                             │
│  REPL, CLI, Serialization, Standard Library                     │
├─────────────────────────────────────────────────────────────────┤
│  Layer 3: EXECUTION                                             │
│  Pattern Matcher, Rule Engine, Query System,                    │
│  Derived Components, Constraints, Effects                       │
├─────────────────────────────────────────────────────────────────┤
│  Layer 2: LANGUAGE                                              │
│  Lexer, Parser, AST, Compiler, Bytecode VM, Module System       │
├─────────────────────────────────────────────────────────────────┤
│  Layer 1: STORAGE                                               │
│  Entity Store, Component Store, Relationships, World            │
├─────────────────────────────────────────────────────────────────┤
│  Layer 0: FOUNDATION                                            │
│  Value Types, Persistent Collections, Error Types               │
└─────────────────────────────────────────────────────────────────┘
```

### Major Crates

```
longtable/
├── crates/
│   ├── lt-foundation/     # Layer 0: Values, collections, errors
│   ├── lt-storage/        # Layer 1: ECS, relationships, world
│   ├── lt-language/       # Layer 2: Parser, compiler, VM
│   ├── lt-engine/         # Layer 3: Rules, queries, constraints
│   ├── lt-stdlib/         # Layer 4: Standard library functions
│   ├── lt-runtime/        # Layer 4: REPL, CLI, serialization
│   └── lt-debug/          # Layer 5: Tracing, debugging, time travel
├── examples/              # Standalone demo projects
├── tests/                 # Integration tests
└── benches/               # Performance benchmarks
```

---

## Phase 0: Project Bootstrap

**Goal**: Establish project structure, tooling, and development workflow.

### Tasks

- [x] Initialize Cargo workspace with crate structure
- [x] Configure CI/CD (test, lint, format, benchmark on PR)
- [x] Set up criterion benchmarks infrastructure
- [x] Create `CHANGELOG.md` with semantic versioning plan
- [x] Establish code style guidelines (rustfmt.toml, clippy.toml)
- [x] Create initial README with project overview

### Success Criteria

- [x] `cargo build` succeeds with empty crates
- [x] `cargo test` runs (no tests yet)
- [x] `cargo bench` infrastructure ready
- [x] CI pipeline runs on push

---

## Phase 1: API Design & Validation

**Goal**: Design all major APIs, implement minimal stubs, validate they compose correctly.

This is the most critical phase. We invest time here to avoid churn later.

### 1.1 Layer 0: Foundation APIs

#### Value System (`lt-foundation::value`)

*(Code example removed - 48 lines - see implementation)*

**Design validation tasks**:
- [x] Verify Value size (target: ≤32 bytes for inline efficiency)
- [x] Verify EntityId can represent 2^64 entities with 2^32 generations
- [x] Verify Type can express all spec type annotations
- [x] Write property tests for Eq/Hash consistency

#### Error System (`lt-foundation::error`)

*(Code example removed - 46 lines - see implementation)*

**Design validation tasks**:
- [x] Verify ErrorKind covers all spec error cases
- [x] Verify ErrorContext provides sufficient debugging info
- [x] Test error display formatting

#### Persistent Collections (`lt-foundation::collections`)

Thin wrappers around `im` crate with Longtable-specific behavior:

*(Code example removed - 22 lines - see implementation)*

**Design validation tasks**:
- [x] Benchmark structural sharing efficiency
- [x] Verify O(log n) access times at 100k elements
- [x] Test clone performance (should be O(1))

### 1.2 Layer 1: Storage APIs

#### Entity Store (`lt-storage::entity`)

*(Code example removed - 27 lines - see implementation)*

#### Component Store (`lt-storage::component`)

*(Code example removed - 64 lines - see implementation)*

**Design validation tasks**:
- [x] Benchmark iteration over 10k entities with component filter
- [x] Verify archetype grouping improves cache locality
- [x] Test schema validation catches type mismatches

#### Relationship Store (`lt-storage::relationship`)

*(Code example removed - 52 lines - see implementation)*

**Design validation tasks**:
- [x] Verify bidirectional index consistency
- [x] Test cardinality enforcement
- [x] Benchmark traversal at 10k relationships

#### World (`lt-storage::world`)

*(Code example removed - 45 lines - see implementation)*

**Design validation tasks**:
- [x] Verify World::clone() is O(1)
- [x] Benchmark world fork + small modifications
- [x] Test previous() chain integrity

### 1.3 Layer 2: Language APIs

#### Lexer (`lt-language::lexer`)

*(Code example removed - 45 lines - see implementation)*

#### Parser & AST (`lt-language::parser`, `lt-language::ast`)

*(Code example removed - 42 lines - see implementation)*

#### Compiler (`lt-language::compiler`)

*(Code example removed - 52 lines - see implementation)*

#### Bytecode VM (`lt-language::vm`)

*(Code example removed - 88 lines - see implementation)*

> **Note: The VM is More Than "Just a VM"**
>
> This bytecode VM does world reads, world writes, effect recording, RNG access, rule-local bindings, and error attribution. It's closer to an **effectful interpreter with transactional semantics** than a normal arithmetic VM.
>
> **Implication for evolution**:
> - **Phase 3**: Allow direct mutation via `world_mut()` for simplicity
> - **Phase 4+**: VM may emit **effect intents** instead, applied transactionally by the rule engine
>
> The `world_mut(&mut self) -> &mut World` signature is a semantic commitment that effects are interleaved with evaluation. This works initially but may need revisiting when implementing constraint rollback. Plan for this—don't be surprised when it happens.

> **Implementation Recommendation: Internal Mutation Choke Point**
>
> Even if you keep `world_mut()` initially, route all mutations through a single abstraction:
>
> *(Code example removed - 9 lines - see implementation)*
>
> You don't need intent buffering yet—but you need a **place** to put it later. If you let VM opcodes implicitly assume mutation semantics, you'll bake in assumptions that are painful to uproot.

**Design validation tasks**:
- [x] Span and Token types implemented with proper source tracking
- [x] Lexer tokenizes all spec literals (integers, floats, strings, symbols, keywords, collections)
- [x] Parser builds AST for all expression forms (lists, vectors, sets, maps, quotes, tags)
- [x] 156 unit tests for lexer, parser, compiler, and VM (including closures and recursive functions)
- [x] Round-trip test: source → AST → bytecode → execution → expected value
- [x] Verify all spec expression forms can be represented
- [x] Benchmark VM execution (target: 1M simple ops/sec) — **Achieved: ~2M ops/sec**

### 1.4 Layer 3: Execution APIs

> **These APIs are semantic sketches, not contracts.**
>
> Layer 3 designs will change. The patterns, matcher, rule engine, and VM signatures below are *expected to be wrong*. They capture current thinking about what the interfaces might look like, but they will evolve as Phase 1.5 and Phase 4 reveal semantic realities.
>
> Do not treat these as frozen. Do not optimize for these shapes. They exist to prove composition is *plausible*, not to define final architecture.

#### The Unified Mental Model

Pattern matching, queries, and derived components are **not independent systems**. They are three frontends over the same logical operation:

> **Binding generation over world snapshots**

Even if the code remains separate initially, the mental model must be unified:
- A rule's `:where` clause generates bindings
- A query's `:where` clause generates bindings
- A derived's `:where` clause generates bindings (scoped to `:for`)

The difference is what happens *after* binding generation:
- Rules: execute `:then` body for each binding set
- Queries: evaluate `:return` and collect results
- Derived: evaluate `:value` and cache

This shared core will eventually demand shared implementation. Don't fight it—but also don't force unification before semantics stabilize.

#### Pattern Matcher (`lt-engine::matcher`)

*(Code example removed - 44 lines - see implementation)*

#### Rule Engine (`lt-engine::rules`)

*(Code example removed - 51 lines - see implementation)*

#### Query System (`lt-engine::query`)

*(Code example removed - 45 lines - see implementation)*

#### Derived Components (`lt-engine::derived`)

> **WARNING: High Semantic Risk**
>
> Derived components interact with pattern matching, rule activation, caching, invalidation, AND speculative execution. This five-way intersection is the most dangerous part of the system.
>
> **Initial implementation strategy**: Derived invalidation is **conservative**. The first implementation will invalidate *all* derived caches on *any* world mutation. Fine-grained invalidation (tracking entity-scoped, query-scoped, binding-dependent dependencies) is an **optimization**, not a correctness requirement.
>
> Do not attempt clever invalidation until the naive approach is proven correct.

*(Code example removed - 37 lines - see implementation)*

#### Constraints (`lt-engine::constraint`)

*(Code example removed - 40 lines - see implementation)*

#### Effects & Provenance (`lt-engine::effects`)

*(Code example removed - 43 lines - see implementation)*

### 1.5 Layer 4: Interface APIs

#### Standard Library (`lt-stdlib`)

*(Code example removed - 28 lines - see implementation)*

#### REPL (`lt-runtime::repl`)

*(Code example removed - 36 lines - see implementation)*

#### Serialization (`lt-runtime::serde`)

*(Code example removed - 10 lines - see implementation)*

### 1.6 Layer 5: Observability APIs

#### Tracing (`lt-debug::trace`)

*(Code example removed - 37 lines - see implementation)*

#### Debugging (`lt-debug::debugger`)

*(Code example removed - 33 lines - see implementation)*

#### Time Travel (`lt-debug::timetravel`)

*(Code example removed - 34 lines - see implementation)*

### 1.7 API Validation Phase

Before implementing internals, validate that APIs compose correctly:

#### Validation Tests

*(Code example removed - 41 lines - see implementation)*

#### API Design Review Checklist

For each major API, verify:

- [ ] **Completeness**: Can express all spec requirements?
- [ ] **Composability**: Works well with other APIs?
- [ ] **Testability**: Easy to write unit tests against?
- [ ] **Error handling**: Returns rich, actionable errors?
- [ ] **Performance**: No obvious bottlenecks in the API shape?
- [ ] **Extensibility**: Can add features without breaking changes?

---

## Phase 1.5: Semantic Spike

**Goal**: Validate rule engine semantics before committing to full implementation.

This phase builds a *minimal* end-to-end slice to shake out semantic assumptions early. We're testing *behavior*, not performance.

### Why This Phase Exists

The most dangerous bugs in a rules engine are semantic:
- Refraction logic (when does a rule re-fire?)
- Binding identity (what makes two activations "the same"?)
- Write visibility (when do changes become visible?)
- Error propagation (what happens when a rule fails mid-execution?)

Discovering these in Phase 4 means rewriting Phase 2-3. Discovering them now means adjusting stubs.

### Deliverables

A spike implementation that can:

1. **Parse and compile** one hardcoded rule with one pattern
2. **Match** entities against that pattern
3. **Execute** the rule body with effects (spawn, set, destroy)
4. **Track refraction** to prevent re-firing
5. **Run to quiescence** for a single tick
6. **Report errors** with meaningful context

### Spike Test Cases

*(Code example removed - 26 lines - see implementation)*

### What This Phase Does NOT Do

- Optimize anything
- Implement the full DSL
- Handle complex patterns (joins, negation)
- Support derived components or constraints

### Rules for Phase 1.5

This phase is **allowed to**:
- Violate crate layering
- Be ugly
- Throw away code
- Use hardcoded AST instead of parsing
- Cut every corner that doesn't affect semantics

If you treat it as "clean proto-impl," you'll miss the point. The goal is **semantic confidence**, not code quality.

### Exit Criteria

- [x] All spike tests pass (23 tests in longtable_engine)
- [x] Refraction semantics match spec Section 5.0.2
- [x] Write visibility matches spec Section 2.2
- [x] Errors include rule/binding context
- [x] Team confident in semantic model
- [x] Decision documented: spike validates core rule engine semantics

---

## Phase 2: Foundation Implementation

**Goal**: Implement Layer 0 and Layer 1 with full test coverage.

### 2.1 Value System

- [x] Implement `Value` enum with all variants
- [x] Implement `LtEq`, `LtHash`, `LtDisplay` traits
- [x] Implement interning for symbols and keywords
- [x] Property tests: Eq/Hash consistency, display round-trip
- [x] Benchmark: Value clone, comparison, hashing

### 2.2 Persistent Collections

- [x] Wrap `im` crate types with Longtable semantics
- [x] Implement iteration with spec-compliant ordering
- [x] Property tests: structural sharing, modification
- [x] Benchmark: insert, lookup, iteration at 10k/100k elements

### 2.3 Error System

- [x] Implement `Error` with all `ErrorKind` variants
- [x] Implement `Display` with rich formatting
- [x] Context builders for ergonomic error construction
- [x] Test error messages are actionable

### 2.4 Entity Store

- [x] Implement generational index allocator
- [x] Implement spawn, destroy, exists, validate
- [x] Test stale reference detection
- [x] Benchmark: spawn/destroy at high churn

### 2.5 Component Store

- [x] Implement schema registration and validation
- [x] Implement archetype-based storage
- [x] Implement component set/get/remove
- [x] Implement archetype iteration
- [x] Test type validation
- [x] Benchmark: iteration with component filter

### 2.6 Relationship Store

- [x] Implement relationship schema registration
- [x] Implement bidirectional indices
- [x] Implement link/unlink with cardinality enforcement
- [x] Implement on_target_delete cascade
- [x] Test cardinality violations
- [x] Benchmark: traversal at scale

### 2.7 World

- [x] Implement immutable World with persistent internals
- [x] Implement all mutation methods (returning new World)
- [x] Implement history chain (previous())
- [x] Implement content_hash for speculation
- [x] Test O(1) clone
- [x] Benchmark: fork + small modification

### Example: Entity Lifecycle

*(Code example removed - 27 lines - see implementation)*

---

## Phase 2.5: World Without Rules

**Goal**: Validate storage and query semantics in isolation, before rules add complexity.

This phase delivers a **usable substrate** without the hardest part (rules). It reduces cognitive load when rules arrive and provides an early motivational win.

### Deliverables

A working system that can:

1. **Create worlds** with components and relationships
2. **Query** entities using full query syntax (`:where`, `:aggregate`, `:group-by`, etc.)
3. **Mutate** worlds programmatically (Rust API, not DSL effects)
4. **Serialize/deserialize** world state
5. **REPL** that can inspect and query (but not tick)

### What This Enables

*(Code example removed - 28 lines - see implementation)*

### What This Does NOT Include

- Rules (no `:then` execution)
- Derived components (no caching logic)
- Constraints (no validation)
- VM/bytecode (queries interpreted, not compiled)
- DSL parsing for mutations (Rust API only)

### Why This Phase Exists

Rules are the hardest part. They involve:
- Refraction
- Conflict resolution
- Effect visibility
- Quiescence detection

Don't stack them on untested foundations. Phase 2.5 proves the foundation works.

### Exit Criteria

- [x] All query forms work against static world (QueryCompiler + QueryExecutor with pattern matching, guards, return expressions)
- [x] Aggregation produces correct results (group-by and aggregate collection implemented)
- [x] Relationships traverse correctly (forward and reverse) - link/unlink/targets/sources all implemented
- [x] Serialization round-trips perfectly (MessagePack via rmp-serde, serde support on all types)
- [x] REPL can query and inspect (basic REPL with expression evaluation, syntax highlighting, tab completion)
- [x] Adventure game world can be constructed (examples/adventure/ with components, relationships, world data)

---

## Phase 3: Language Implementation

**Goal**: Implement Layer 2 - complete DSL parsing and bytecode execution.

### 3.1 Lexer

- [x] Implement tokenizer for all spec literals
- [x] Handle comments (`;`, `#_`)
- [x] Handle tagged literals (`#name[...]`)
- [x] Comprehensive span tracking
- [x] Test with spec grammar examples (24 tests)
- [x] Fuzz test for crash resistance (1000+ proptest cases in fuzz_tests.rs)

### 3.2 Parser

- [x] Implement recursive descent parser
- [x] Parse all expression forms
- [x] Parse all declaration forms (component:, rule:, relationship:, derived:, constraint:, query, spawn:, link:)
- [x] Rich error messages with span information
- [x] Test with spec examples (29 tests + 13 spawn/link tests)
- [x] Fuzz test for crash resistance (1000+ proptest cases in fuzz_tests.rs)

### 3.3 AST

- [x] Implement all AST node types
- [x] Implement visitor pattern for traversal (AstVisitor + AstTransform traits in visitor.rs)
- [x] Implement AST pretty-printer (pretty.rs with PrettyConfig)
- [x] Test round-trip: source → AST → pretty-print ≈ source (50+ tests in pretty.rs)

### 3.4 Compiler

- [x] Compile expressions to bytecode
- [x] Compile special forms (if, let, do, def, quote)
- [x] Compile arithmetic operators (+, -, *, /, mod)
- [x] Compile comparison operators (=, !=, <, <=, >, >=)
- [x] Compile logic operators (not)
- [x] Compile collection literals (vector, map, set)
- [x] Constant deduplication via ConstKey
- [x] Local variable slots for let bindings
- [x] Compile expressions with binding variables (compile_expression for queries)
- [x] Compile patterns for rule matching (PatternCompiler in longtable_engine/pattern.rs)
- [x] Compile rule bodies (executed via VM in longtable_engine/rule.rs)
- [x] Compile queries (QueryCompiler in longtable_engine)
- [x] Compile derived components (DerivedCompiler in longtable_engine/derived.rs)
- [x] Compile constraints (ConstraintCompiler in longtable_engine/constraint.rs)
- [x] Macro expansion (MacroExpander with defmacro, gensym hygiene, syntax-quote with namespace qualification)
- [x] Module/namespace resolution (NamespaceContext with aliases, refers; ModuleRegistry with cycle detection)
- [x] Standard library macros (when, when-not, if-not, and, or, ->, ->>, cond, doto, comment)
- [x] Test all expression forms compile correctly (14 tests)

### 3.5 Bytecode VM

- [x] Implement stack-based VM
- [x] Implement stack operations (Const, Pop, Dup)
- [x] Implement arithmetic opcodes (Add, Sub, Mul, Div, Mod, Neg)
- [x] Implement comparison opcodes (Eq, Ne, Lt, Le, Gt, Ge)
- [x] Implement logic opcodes (Not, And, Or)
- [x] Implement control flow (Jump, JumpIf, JumpIfNot)
- [x] Implement local variables (LoadLocal, StoreLocal, LoadBinding)
- [x] Implement collection opcodes (VecNew, VecPush, MapNew, MapInsert, SetNew, SetInsert)
- [x] Implement print opcode
- [x] Implement CallNative with 40+ native functions (predicates, math, collections, strings)
- [x] Implement VmContext trait for World access
- [x] Implement WorldContext wrapper for World integration
- [x] Implement effect opcodes (Spawn, Destroy, SetComponent, SetField, Link, Unlink)
- [x] Implement world access opcodes (GetComponent, GetField)
- [x] Implement VmEffect enum for deferred effect application
- [x] Test: expression evaluation (33 tests)
- [x] Test: native function evaluation (33 tests)
- [x] Implement user-defined function calls (Call opcode)
- [x] Implement CompiledFunction structure for function bytecode
- [x] Implement fn compilation (anonymous functions)
- [x] Test: function calls (11 tests)
- [x] Implement closures (captured variables)
  - [x] LoadCapture opcode for accessing captured variables at runtime
  - [x] MakeClosure opcode for creating closures with captured values
  - [x] PatchCapture opcode for recursive closure support (letrec semantics)
  - [x] CompiledFn uses Arc<Mutex<Vec<Value>>> for mutable captures
  - [x] Compiler tracks outer_locals and captures during fn compilation
  - [x] compile_let uses three-phase approach for recursive bindings
  - [x] Test: closure capture (5 tests) + recursive functions (2 tests)
- [x] Benchmark: 1M ops/sec target — **Achieved: ~2M ops/sec**

#### Benchmark Results (criterion)

Comprehensive benchmarks in `crates/longtable_language/benches/language_benchmarks.rs`:

| Category             | Benchmark          | Time             | Notes                       |
| -------------------- | ------------------ | ---------------- | --------------------------- |
| **Lexer**            | simple_int         | 42 ns            | ~45 MiB/s throughput        |
|                      | expression         | 186 ns           |                             |
|                      | nested             | 1.05 µs          |                             |
|                      | collections        | 1.36 µs          | ~90 MiB/s throughput        |
| **Parser**           | expression         | 297 ns           |                             |
|                      | nested             | 1.69 µs          |                             |
|                      | function           | 1.46 µs          |                             |
| **Compiler**         | simple_add         | 6.1 µs           |                             |
|                      | arithmetic         | 7.2 µs           |                             |
|                      | closure            | 9.6 µs           |                             |
|                      | recursive          | 9.9 µs           |                             |
| **VM Execution**     | constant           | 480 ns           |                             |
|                      | add_simple         | 490 ns           |                             |
|                      | let_binding        | 520 ns           |                             |
|                      | vector_create      | 1.29 µs          |                             |
|                      | map_create         | 1.44 µs          |                             |
| **Function Calls**   | identity           | 1.23 µs          | ~1 µs per call              |
|                      | higher_order       | 2.16 µs          |                             |
|                      | closure_capture    | 1.48 µs          |                             |
| **Recursion**        | factorial_5        | 5.2 µs           |                             |
|                      | factorial_10       | 11.7 µs          |                             |
|                      | fibonacci_10       | 168 µs           | 177 recursive calls         |
|                      | fibonacci_15       | 1.87 ms          | 1,973 recursive calls       |
| **Native Functions** | nil?, int?         | ~505 ns          | Type predicates             |
|                      | count, first, rest | 1.3-2.2 µs       | Collection ops              |
|                      | get, assoc         | 1.5-1.6 µs       | Map ops                     |
| **End-to-End**       | eval_simple        | 7.2 µs           | Full lex→parse→compile→exec |
|                      | eval_factorial     | 21.6 µs          |                             |
| **Throughput**       | simple_op          | **~2M ops/sec**  | Target: 1M ✓                |
|                      | ten_ops            | **~16M ops/sec** | Amortized                   |

**Performance characteristics**:
- Function call overhead: ~1 µs per call (consistent across benchmarks)
- Interpreted VM overhead: ~1,500x vs native for simple ops
- Recursive performance: ~40,000x slower than native C (expected for bytecode VM without JIT)
- Suitable for rule engine DSL where hot paths are native Rust functions

#### Engine & Storage Benchmarks (criterion)

Comprehensive benchmarks for pattern matching, queries, and storage operations:

| Category                | Benchmark                | Time      | Notes              |
| ----------------------- | ------------------------ | --------- | ------------------ |
| **Pattern Compilation** |                          |           |                    |
|                         | simple_component         | 96.7 ns   | Single clause      |
|                         | multi_clause (3 clauses) | 228.4 ns  |                    |
|                         | with_variable_binding    | 137.5 ns  |                    |
| **Throughput**          |                          |           |                    |
|                         | pattern_matches/sec (9K) | 3.84 ms   | ~2.3M matches/sec  |
| **Component Store**     |                          |           |                    |
|                         | get                      | 66.0 ns   |                    |
|                         | get_field                | 112.0 ns  |                    |
|                         | has                      | 32.0 ns   |                    |
|                         | set_tag                  | 104.9 ns  |                    |
|                         | set_structured           | 554.5 ns  |                    |
| **Entity Store**        |                          |           |                    |
|                         | spawn_destroy_cycle      | 54.7 ns   |                    |
| **Relationship Store**  |                          |           |                    |
|                         | link                     | 479.8 ns  |                    |
|                         | has_edge                 | 60.3 ns   |                    |
| **World**               |                          |           |                    |
|                         | spawn_with_components    | 479.6 ns  |                    |
|                         | get                      | 66.1 ns   |                    |
|                         | set                      | 327.3 ns  |                    |
|                         | link                     | 471.2 ns  |                    |
|                         | advance_tick             | 50.8 ns   |                    |
| **Interner**            |                          |           |                    |
|                         | intern_new_symbol        | 296.1 ns  |                    |
|                         | intern_duplicate         | 21.2 ns   |                    |
|                         | get_symbol               | 0.8 ns    |                    |
|                         | intern_1000_unique       | 284.16 µs |                    |
| **Value Operations**    |                          |           |                    |
|                         | clone (int/float)        | ~4 ns     | Inline copy        |
|                         | clone (vec 1000)         | 21 ns     | Structural sharing |
|                         | clone (map 1000)         | 13.3 ns   | Structural sharing |
|                         | compare (int)            | 3.6 ns    |                    |
|                         | compare (vec 1000)       | 5.91 µs   |                    |
|                         | hash (int)               | 16.0 ns   |                    |
|                         | hash (string short)      | 18.5 ns   |                    |
|                         | hash (vec 1000)          | 9.07 µs   |                    |

**Key performance insights**:
- Pattern compilation is very fast (~100-230ns per pattern)
- Pattern matching at scale: ~2.3 million matches/second
- Storage operations are sub-microsecond (component get: 66ns, has: 32ns)
- VM bytecode execution: ~500ns per simple operation
- Persistent data structure cloning is extremely cheap due to structural sharing (vec 1000: 21ns)
- Interning provides fast symbol lookup after first intern (0.8ns vs 296ns)

### Example: Expression Evaluation

*(Code example removed - 12 lines - see implementation)*

---

## Phase 4: Execution Engine Implementation

**Goal**: Implement Layer 3 - rules, queries, and constraints.

### 4.1 Pattern Matcher

- [x] Implement pattern compilation (PatternCompiler in longtable_engine/src/pattern.rs)
- [ ] ~~Implement index-based lookup~~ — **Deferred to Phase 8** (currently naive O(n) iteration; sufficient for MVP, optimization target for 10k+ entities)
- [x] Implement join execution (match_remaining function)
- [x] Implement negation (check_negations function)
- [x] Implement variable unification (try_bind_clause handles bound variables)
- [x] Test with spec pattern examples (spec_damage_rule_pattern, spec_faction_join_pattern, etc.)
- [x] Benchmark: matching at 10k entities (pattern_matching/*/10000 benchmarks)

### 4.2 Rule Engine

- [x] Implement activation finding (ProductionRuleEngine::find_activations in longtable_engine/src/rule.rs)
- [x] Implement refraction (refracted HashSet tracks fired activations)
- [x] Implement conflict resolution (salience, specificity) (sort in find_activations)
- [x] Implement `:once` flag (once_fired HashSet)
- [x] Implement rule execution loop (run_to_quiescence)
- [x] **Implement semantic kill switches** (max_activations, see below)
- [x] Test: quiescence termination (quiescence_termination test)
- [x] Test: deterministic ordering (deterministic_ordering test)
- [x] Test: kill switches trigger correctly (kill_switch_triggers test)
- [x] Benchmark: rules firing per second (throughput/rule_activations_per_sec)

#### Semantic Kill Switches (Hard Ceilings)

Bugs look like malicious rules. Without hard ceilings, semantic bugs become hangs—and hangs are morale killers.

**Required limits** (configurable, with sane defaults):

| Limit                         | Default | What Happens            |
| ----------------------------- | ------- | ----------------------- |
| Max activations per tick      | 10,000  | Error with context      |
| Max effects per tick          | 100,000 | Error with context      |
| Max refires per rule per tick | 1,000   | Error naming the rule   |
| Max derived evaluation depth  | 100     | Error (cycle detection) |
| Max query results             | 100,000 | Truncate with warning   |

These are not performance limits—they're **semantic sanity checks**. A legitimate simulation shouldn't hit them. If it does, something is wrong.

*(Code example removed - 20 lines - see implementation)*

Errors should include full context: which rule, what bindings, how many times it fired. Make debugging possible.

#### Future Consideration: Simple/Declarative Mode

Not all rules are reactive. Some are declarative and should not chain within a tick.

This is **not required for MVP**, but keep it in mind:
- A "snapshot rule" mode where rules see start-of-tick state only
- A "no chaining this tick" flag for specific rules
- A restricted subset for beginners

This matters especially for onboarding. A beginner shouldn't need to understand refraction to write their first rule. Consider this when designing examples and tutorials.

### 4.3 Query System

- [x] Implement query compilation (QueryCompiler in longtable_engine/src/query.rs)
- [x] Implement aggregation functions (basic collection of values per group)
- [x] Implement group-by
- [x] Implement order-by and limit
- [x] Implement query-one, query-count, query-exists?
- [x] **Implement entity ordering warnings** (QueryWarning::EntityOrderingUnstable in query.rs)
- [x] **Pattern variable syntax in return expressions** — `?var` syntax now works in :return, :let, :guard (compile_expression handles `?` prefix)
- [x] Test with spec query examples (spec_query_* tests in query.rs)
- [x] Benchmark: query at scale (query_execution/*/10000 benchmarks)

#### Entity Ordering Is a Footgun

We allow ordered queries and entity iteration, but ordering on `EntityId` is almost always a bug waiting to happen.

**The problem**: Users will rely on allocation order even though it's unspecified. Someone's game logic will silently depend on "player spawned first."

**Mitigation**:
- Emit a **warning** when ordering by entity ID in queries
- Consider requiring explicit opt-in: `:order-by [?e :entity-id]` vs implicit ordering
- Document heavily that entity order is not stable across serialization/deserialization
- In examples, always use explicit ordering criteria (`:order-by [?e :name/value]`)

#### Known Limitation: Relationship Queries

Pattern matching currently only supports component queries, not relationship queries. A pattern like `[?e :exit/north ?target]` is interpreted as a component lookup, not a relationship traversal.

**Workaround**: Use native functions to query relationships:
- `(targets world entity :relationship)` — get relationship targets
- `(sources world entity :relationship)` — get reverse relationship sources

**Future work**: Extend PatternMatcher to recognize relationship keywords and emit relationship traversal code. This requires:
1. Schema awareness in pattern compiler (distinguish components vs relationships)
2. New pattern clause type for relationships
3. Cross-entity join logic in pattern matcher

### 4.4 Derived Components

- [x] Implement derived compilation (DerivedCompiler in longtable_engine/src/derived.rs)
- [x] Implement dependency tracking (dependencies HashSet in CompiledDerived)
- [x] Implement lazy evaluation (DerivedEvaluator::get with caching)
- [x] Implement cache invalidation (DerivedCache::invalidate_by_component)
- [x] Test: cycle detection (max_depth limit in evaluator)
- [x] Benchmark: cache operations (derived_components/* benchmarks)

### 4.5 Constraints

- [x] Implement constraint compilation (ConstraintCompiler in longtable_engine/src/constraint.rs)
- [x] Implement check evaluation (ConstraintChecker::check_all, placeholder evaluation)
- [x] Implement rollback vs warn (ConstraintResult enum with Rollback/Warn variants)
- [x] Test: constraint violation handling (check_passes_with_no_constraints, check_with_constraints_no_matches)
- [x] Test: constraint ordering (constraints_preserve_declaration_order test)

### 4.6 Effects & Provenance (Minimal)

> **Note**: Phase 4 implements *minimal* effect logging—enough for basic `why` queries and error context. Full tracing, debugger integration, and time travel remain in Phase 6.

- [x] Implement effect recording (EffectRecord in rule.rs)
- [x] Implement basic provenance tracking (ProvenanceTracker in provenance.rs)
- [x] Implement basic `why` query (ProvenanceTracker::why, last_writer)
- [x] Test: effect log accuracy (provenance tests)
- [x] Do NOT implement: full history, bindings capture, expression IDs (Phase 6)

### 4.7 Tick Orchestration

- [x] Implement full tick cycle (TickExecutor in longtable_engine/src/tick.rs)
- [x] Input injection (inject_inputs with InputEvent variants)
- [x] Rule execution to quiescence (via ProductionRuleEngine::run_to_quiescence)
- [x] Constraint checking (via ConstraintChecker::check_all)
- [x] Commit or rollback (TickResult with success flag, original world saved for rollback)
- [x] Test: atomicity (tick_with_no_rules, tick_runs_rules_to_quiescence tests)
- [x] Benchmark: full tick at scale (tick_orchestration/* benchmarks)

### 4.8 Phase 4 Exit Gate: The Mutation Model Decision

> **This is a semantic fork, not an implementation detail.**

Before exiting Phase 4, make an explicit decision:

**Option A: VM Mutates World Directly**
- `world_mut(&mut self) -> &mut World` remains
- Rollback requires snapshotting before rule execution
- Simpler initial implementation
- Constraint rollback discards entire transaction

**Option B: VM Emits Effect Intents**
- VM returns `Vec<EffectIntent>` instead of mutating
- Rule engine applies effects transactionally
- Enables fine-grained rollback
- Required for "what would happen if" debugging

You can start Phase 4 with Option A—but **document it as a temporary semantic compromise**. If you defer this decision, you'll discover it painfully during constraint implementation or speculation.

**Exit criteria addendum**:
- [x] Decision documented: VM mutation model (direct vs intent-based)
- [x] If direct mutation chosen: document what features this limits
- [x] If intent-based chosen: implement before Phase 5 — **N/A** (Option A chosen)

#### Decision: Option A (Direct Mutation with Snapshot)

**Chosen**: Option A - VM mutates World directly with snapshot-based rollback.

**Rationale**:
1. Simpler initial implementation - gets MVP working faster
2. Sufficient for current use cases - constraint rollback works via snapshot
3. Performance - avoids allocating effect intent vectors per rule
4. World immutability - World's persistent data structures already enable efficient cloning

**Implementation details** (in `tick.rs`):
- `TickExecutor::tick()` clones the world before rule execution: `let original_world = world.clone()`
- Rules modify world directly through the execution closure
- On constraint violation (`ConstraintResult::Rollback`), restore `original_world`
- On success, keep the modified world

**Limitations this creates**:
1. **No fine-grained rollback**: Cannot undo individual rule effects; must rollback entire tick
2. **No "what would happen if" without execution**: Speculation requires actual execution then discard
3. **No effect replay**: Cannot replay individual effects for debugging without re-executing rules
4. **All-or-nothing constraints**: Constraint violations rollback ALL changes, not just offending ones

**Migration path to Option B (if needed)**:
1. Change `run_to_quiescence` executor closure to return `Vec<EffectIntent>` instead of modified `World`
2. Accumulate effects, apply at end of quiescence
3. Enable selective effect rollback in constraint handler
4. Add effect log for replay/debugging

This migration can be done in Phase 6 (Observability) if debugging features require it. For MVP, Option A is sufficient.

### Example: Rule Execution

*(Code example removed - 33 lines - see implementation)*

---

## Phase 5: Interface Implementation

**Goal**: Implement Layer 4 - standard library, REPL, serialization.

### 5.1 Standard Library

Implement all spec functions organized by category:

- [x] Collection functions (map, filter, reduce, every, some, take-while, drop-while, remove, group-by, zip-with, etc.) — 125+ native functions in vm/native/*.rs
- [x] Math functions (arithmetic, trig, hyperbolic, vector math) — sin, cos, tan, sinh, cosh, tanh, vec+, vec-, vec*, vec-scale, vec-dot, vec-cross, vec-length, vec-normalize, vec-lerp, vec-angle, etc.
- [x] String functions (str/*, format) — split, join, trim, replace, starts-with?, ends-with?, contains?, blank?, substring, upper, lower, format
- [x] Predicate functions (nil?, some?, type checks) — nil?, int?, float?, string?, keyword?, symbol?, vector?, list?, map?, set?, bool?, number?, coll?, fn?, entity?
- [x] Higher-order function opcodes — Map, Filter, Reduce, Every, Some, TakeWhile, DropWhile, Remove, GroupBy, ZipWith, Repeatedly
- [x] Test each function with spec examples (164+ native function tests across collection.rs, math.rs, predicates.rs, string.rs + 22 HOF opcode tests in vm/tests.rs)
- [ ] Document each function

### 5.2 REPL

- [x] Command parsing (try_special_form handles def, load, save!, load-world!, tick!, inspect)
- [x] Expression evaluation (basic eval loop with parse → compile → execute)
- [x] Tick execution (tick! command with TickExecutor integration)
- [x] History and line editing (rustyline integration with LineEditor trait)
- [x] Session variables (def) stored in session (VM globals pending)
- [x] File loading (load) with relative path resolution
- [x] Multi-line input with bracket validation
- [x] Syntax highlighting for DSL
- [x] Tab completion for keywords
- [x] Special commands (inspect, tick!, save!, load-world!) — save world state, load world state, advance tick, inspect entity
- [x] **DSL declaration execution** (component:, relationship:, spawn:, link:) — enables world construction via DSL
- [x] Entity name registry for spawn:/link: resolution
- [x] **Query execution** (query form) — compiles and executes queries, returns results as vectors
- [x] Test: basic evaluation tests (7 tests)
- [x] Test: interactive scenarios (7 tests for arithmetic chains, def/use, collections, strings, HOFs, math)
- [x] Test: error recovery (4 tests for syntax errors, undefined vars, division by zero, type errors)
- [x] Test: spawn and link execution (7 tests for entity creation, linking, error handling)

### 5.3 CLI

- [x] File loading and execution (longtable binary with file arguments)
- [x] REPL mode (default when no files specified)
- [x] Batch mode (--batch flag for non-interactive execution)
- [x] Debug mode flags (--trace, --trace-vm, --trace-match, --max-ticks N, --dump-world)
- [x] CLI argument parsing (--help, --version, --batch, debug flags)
- [x] Test: CLI argument parsing (18 tests for help, version, batch, files, trace flags, max-ticks, combined options, error cases)
- [x] Test: file execution (4 tests for help, version, nonexistent file, batch mode)

### 5.4 Serialization

- [x] Implement save format (MessagePack via rmp-serde)
- [x] Implement world serialization (custom serde impl for World, skips history)
- [x] Implement world deserialization
- [x] Test: round-trip correctness (5 tests in serialize module)
- [x] Test: version compatibility checking (comprehensive version_compatibility test with multiple components, entities, relationships)

### Example: REPL Session

```
$ longtable examples/adventure/_.lt

Welcome to THE DARK CAVE

Longtable REPL v0.1
> (query :where [[?e :tag/player]] :return ?e)
[Entity(1)]

> (get Entity(1) :name/value)
"Adventurer"

> (tick! [{:input/raw "go north"}])
Tick 1 completed. Rules: 3, Time: 12ms
You enter the Main Hall.

> (rollback! 1)
Rolled back to tick 0.

> (save! "checkpoint.lt")
Saved to checkpoint.lt
```

---

## Phase 5.5: Relationship Reification

**Goal**: Unify relationships with entities for consistent query semantics.

> **Context**: Pattern matching currently only supports component queries. Relationship queries like `[?e :exit/north ?target]` fail because the pattern matcher doesn't recognize relationships. Rather than add special-case code for relationships, we reify relationships as entities—making them queryable like any other data.
>
> **Priority**: This is being implemented NOW before Phase 6, as it's blocking the "adventure game runs end-to-end" MVP criterion. Relationship queries are essential for navigation rules.

### Motivation

Current state:
- Relationships stored separately from components
- Pattern matcher only iterates entities and checks components
- `[?e :exit/north ?target]` interpreted as component lookup, fails

After reification:
- Every relationship is an entity with `:rel/type`, `:rel/source`, `:rel/target` components
- Pattern matching "just works"—relationships are data like everything else
- Can attach metadata to relationships (weight, locked?, bidirectional?)
- Uniform query model, no special cases

### Design Decisions

**D1: Relationships are visible entities**
Users can query `:rel/source` directly. Transparency over magic.

**D2: Cardinality enforced at mutation time**
`link:` checks cardinality BEFORE creating relationship entity. Synchronous errors, same UX as current system. The `relationship:` declaration registers cardinality rules that `link:` enforces.

**D3: Garbage collection for orphaned relationships**
When source or target entity is deleted, relationship entities must be cleaned up. Implemented via deletion hooks or constraints.

**D4: Syntax sugar for relationship patterns**
`[?e :exit/north ?t]` desugars to `[?r :rel/type :exit/north] [?r :rel/source ?e] [?r :rel/target ?t]` when `:exit/north` is a registered relationship.

### 5.5.1 Infrastructure

- [x] Add reserved components: `:rel/type`, `:rel/source`, `:rel/target` — plus `:value` field keyword
- [x] Add `World::spawn_relationship(rel, source, target) -> Result<(World, EntityId)>`
- [x] Add `World::find_relationships(rel_type?, source?, target?) -> Vec<EntityId>`
- [x] Add `World::has_outgoing(source, rel) -> bool` (O(n) initially)
- [x] Add `World::has_incoming(target, rel) -> bool` (O(n) initially)
- [x] Test: infrastructure helpers work correctly (6 new tests in world.rs)

### 5.5.2 Cardinality Enforcement

- [x] Add `World::create_relationship(rel_type, source, target)` with cardinality enforcement
- [x] OneToOne: check no existing outgoing OR incoming
- [x] ManyToOne: check no existing outgoing
- [x] OneToMany: check no existing incoming
- [x] ManyToMany: no check needed
- [x] Support `OnViolation::Error` and `OnViolation::Replace` strategies
- [x] Test: cardinality violations return errors at link time (12 new tests)

### 5.5.3 Dual-Write Migration

- [x] `World::link()` now creates BOTH old-style relationship AND new relationship entity
- [x] Idempotent: skips creating relationship entity if it already exists
- [x] Both storages stay in sync
- [x] Test: all 878 existing tests still pass (updated 2 tests for new entity counts)

### 5.5.4 Query Migration

- [x] Modified `PatternMatcher` to detect relationship keywords via `world.relationship_schema()`
- [x] Relationship patterns like `[?e :in-room ?r]` now match against relationship entities
- [x] Extracts `:rel/source` as entity_var, `:rel/target` as binding variable
- [x] Works for relationships as first clause or subsequent clauses
- [x] Test: 3 new tests for relationship pattern matching (881 total tests)

### 5.5.5 Read Migration

- [x] Switched `World::targets()` to read from relationship entities via `find_relationships`
- [x] Switched `World::sources()` to read from relationship entities via `find_relationships`
- [x] Old `RelationshipStore` now write-only (dual-write still active)
- [x] All 883 tests pass (existing tests verify reads return same results)

### 5.5.6 Orphan Cleanup

- [ ] On entity deletion, find and delete relationship entities where source OR target matches
- [ ] Respect `on-target-delete` semantics (cascade vs remove)
- [ ] Test: deleting entity cleans up relationships

### 5.5.7 Remove Old Storage

- [ ] Delete `RelationshipStore`
- [ ] Remove dual-write from `link:`
- [ ] Clean up unused code
- [ ] Test: all tests pass with only new storage

### Exit Criteria

- [ ] Relationship queries work in pattern matcher
- [ ] Cardinality enforced at `link:` time (synchronous errors)
- [ ] Adventure game navigation queries work
- [ ] No regression in existing relationship functionality
- [ ] Old `RelationshipStore` deleted

---

## Phase 5.6: Component Value Indexing (Future)

**Goal**: Efficient lookup of entities by component value.

> **Note**: This phase is NOT required for MVP. It's documented here for future reference when O(n) scans become a bottleneck. Implement when relationship queries or cardinality checks are measurably slow (likely at 10k+ entities with many relationships).

### Motivation

Without indexes, queries like "find all relationships from Entity(5)" require O(n) scan over all entities. With indexes, this becomes O(1) lookup + O(k) iteration where k = number of matches.

### 5.6.1 Single-Column Indexes

Inverted index: `(component, value) → Set<EntityId>`

```rust
struct ComponentIndex {
    index: HashMap<(KeywordId, Value), HashSet<EntityId>>,
}
```

- [ ] Add `ComponentIndex` to World
- [ ] Update `set()` to maintain index
- [ ] Update `remove_component()` to maintain index
- [ ] Add `World::find_by_value(component, value) -> Vec<EntityId>`
- [ ] Index `:rel/type`, `:rel/source`, `:rel/target` by default
- [ ] Benchmark: cardinality check performance improvement

### 5.6.2 Composite Indexes

For multi-column lookups: `(source, rel_type) → Set<EntityId>`

```rust
struct CompositeIndex {
    // (source_entity, rel_type) -> relationship entities
    by_source_and_type: HashMap<(EntityId, KeywordId), HashSet<EntityId>>,
    // (target_entity, rel_type) -> relationship entities
    by_target_and_type: HashMap<(EntityId, KeywordId), HashSet<EntityId>>,
}
```

- [ ] Add composite index for relationship queries
- [ ] `has_outgoing(source, rel)` becomes O(1)
- [ ] `has_incoming(target, rel)` becomes O(1)
- [ ] Benchmark: relationship query performance at 100k entities

### 5.6.3 User-Defined Indexes (Aspirational)

```clojure
;; Explicit index declaration
(index: :faction)  ; index entities by faction value

;; Query uses index automatically
(query :where [[?e :faction :rebels]] :return ?e)  ; O(1) + O(k)
```

- [ ] Add `index:` declaration form
- [ ] Pattern compiler uses indexes when available
- [ ] Document index trade-offs (memory vs query speed)

### Performance Targets

| Operation | Without Index | With Single Index | With Composite |
|-----------|---------------|-------------------|----------------|
| `has_outgoing(e, rel)` | O(n) | O(k) | O(1) |
| `find_relationships(type, source, _)` | O(n) | O(k) | O(1) |
| Cardinality check | O(n) | O(k) | O(1) |

Where n = total entities, k = relationships from/to specific entity.

---

## Phase 6: Observability Implementation

**Goal**: Implement Layer 5 - full debugging, tracing, and time travel.

> **Note**: Phase 4 delivered *minimal* effect logging. Phase 6 builds on that foundation to provide rich observability:
> - Full effect history (not just last-writer)
> - Bindings capture in effect records
> - Expression IDs for step-through debugging
> - Time travel infrastructure
>
> **Ordering rationale**: Observability is a *consumer* of execution correctness. Building rich debugging before semantics are stable couples observability to unstable internals. Now that Phase 4 proved semantics correct, we can instrument safely.

### 6.1 Tracing

- [ ] Implement trace targets
- [ ] Implement trace event recording
- [ ] Implement trace output formatting
- [ ] REPL integration
- [ ] Test: trace accuracy

### 6.2 Debugging

- [ ] Implement breakpoints
- [ ] Implement step execution
- [ ] Implement state inspection
- [ ] REPL integration
- [ ] Test: debugger commands

### 6.3 Time Travel

- [ ] Implement timeline recording
- [ ] Implement rollback
- [ ] Implement branching
- [ ] Implement world diff
- [ ] REPL integration
- [ ] Test: timeline integrity
- [ ] Benchmark: memory usage with history

### 6.4 Explain

- [ ] Implement `why` for components
- [ ] Implement `why` for derived values
- [ ] Implement `explain-query`
- [ ] Test: explanation accuracy

### Example: Debugging

```
> (break-on :rule/apply-damage)
Breakpoint set on rule apply-damage.

> (tick! [])
BREAK: Rule apply-damage about to fire
  Bindings: {?e: Entity(5), ?hp: 100, ?dmg: 25}

debug> (inspect Entity(5))
Entity(5) [health, tag/enemy]
  :health {:current 100, :max 100}
  :tag/enemy true

debug> (step)
Rule apply-damage fired.
Effects: (set! Entity(5) :health/current 75)

debug> (continue)
Tick 1 completed.
```

---

## Phase 7: Examples & Documentation

**Goal**: Build comprehensive examples that exercise all features.

### 7.1 Integration Test Suite

Small, focused tests for each feature:

```
tests/
├── foundation/
│   ├── values.rs
│   ├── collections.rs
│   └── errors.rs
├── storage/
│   ├── entities.rs
│   ├── components.rs
│   ├── relationships.rs
│   └── world.rs
├── language/
│   ├── lexer.rs
│   ├── parser.rs
│   └── vm.rs
├── engine/
│   ├── patterns.rs
│   ├── rules.rs
│   ├── queries.rs
│   └── constraints.rs
└── integration/
    ├── tick_cycle.rs
    ├── speculation.rs
    └── time_travel.rs
```

### 7.2 Example: Counter Machine (Deliberately Ugly)

Before the adventure game, build a **brutally mechanical** example that exposes semantics without hiding them behind narrative:

```
examples/counter/
├── _.lt
└── tests/
    └── counter_test.rs
```

**Why this matters**: Adventure games hide complexity behind narrative. A dumb, brutal system exposes semantics fast.

```clojure
;; counter/_.lt
;; Deliberately ugly. Tests refraction, chaining, and termination.

(component: counter :value :int)
(component: increment-request)
(component: done)

;; Rule 1: Increment counter when requested
(rule: do-increment
  :where [[?e :counter/value ?v]
          [?e :increment-request]]
  :then [(set! ?e :counter/value (+ ?v 1))
         (remove! ?e :increment-request)])

;; Rule 2: Request another increment if under limit
(rule: maybe-again
  :where [[?e :counter/value ?v]
          (not [?e :increment-request])
          (not [?e :done])]
  :when [(< ?v 10)]
  :then [(set! ?e :increment-request true)])

;; Rule 3: Mark done when limit reached
(rule: finish
  :where [[?e :counter/value ?v]
          (not [?e :done])]
  :when [(>= ?v 10)]
  :then [(set! ?e :done true)])
```

**Test cases**:
- Counter reaches exactly 10, not 11
- Rules fire in deterministic order
- Refraction prevents infinite increment loop
- Kill switch triggers if limit removed

This example should feel boring. That's the point.

### 7.3 Example: Adventure Game

Full implementation of spec's adventure game example:

```
examples/adventure/
├── _.lt              # Entry point
├── components.lt     # Component schemas
├── relationships.lt  # Relationship schemas
├── rules/
│   ├── commands.lt   # Command parsing
│   ├── movement.lt   # Go command
│   ├── items.lt      # Take, drop, use
│   └── combat.lt     # Fight command
├── data/
│   └── world.lt      # Initial world state
└── tests/
    └── adventure_test.rs  # Rust integration tests
```

### 7.4 Example: Combat Simulation

Demonstrates rules, damage, death, and AI:

```
examples/combat/
├── _.lt
├── components.lt
├── rules/
│   ├── damage.lt
│   ├── death.lt
│   ├── ai.lt
│   └── initiative.lt
└── tests/
    └── combat_test.rs
```

### 7.5 Example: Economy Simulation

Demonstrates derived components, constraints, aggregates:

```
examples/economy/
├── _.lt
├── components.lt
├── derived.lt        # Total wealth, faction power
├── constraints.lt    # No negative money
├── rules/
│   ├── trade.lt
│   └── production.lt
└── tests/
    └── economy_test.rs
```

### 7.6 Example: AI Planning

Demonstrates speculation and GOAP:

```
examples/ai-planning/
├── _.lt
├── components.lt
├── rules/
│   ├── actions.lt
│   └── planning.lt   # World simulation, goal evaluation
└── tests/
    └── planning_test.rs
```

### 7.7 Documentation

- [ ] API documentation (rustdoc)
- [ ] Language reference (DSL syntax and semantics)
- [ ] Tutorial (building the adventure game)
- [ ] Architecture overview
- [ ] Performance tuning guide

---

## Phase 8: Performance Optimization

**Goal**: Meet performance targets at scale.

### 8.1 Benchmarking Infrastructure

```
benches/
├── foundation/
│   ├── values.rs        # Value operations
│   └── collections.rs   # Persistent collection ops
├── storage/
│   ├── entities.rs      # Spawn/destroy throughput
│   └── iteration.rs     # Component iteration
├── engine/
│   ├── matching.rs      # Pattern matching
│   ├── rules.rs         # Rule firing rate
│   └── queries.rs       # Query execution
└── integration/
    ├── tick.rs          # Full tick throughput
    └── scale.rs         # Behavior at 10k/100k entities
```

### 8.2 Performance Targets

> **Philosophy**: Correctness and determinism first. Benchmarks detect disasters, not optimize.
>
> For MVP, drop 10k entity guarantees. Get semantics right. You can always optimize once semantics are frozen. You cannot easily fix semantics once users rely on them.

**MVP Targets** (Phase 4 complete):

| Operation    | Target | Scale       | Purpose                |
| ------------ | ------ | ----------- | ---------------------- |
| Value clone  | <100ns | -           | Detect disasters       |
| Entity spawn | <10μs  | -           | Detect disasters       |
| Full tick    | <1s    | 1k entities | Usable for development |
| Query        | <100ms | 1k entities | Usable for development |

**1.0 Targets** (Phase 8 complete):

| Operation     | Target | Scale                   |
| ------------- | ------ | ----------------------- |
| Value clone   | <10ns  | -                       |
| Entity spawn  | <1μs   | -                       |
| Component get | <100ns | -                       |
| World clone   | <100ns | -                       |
| Pattern match | <1ms   | 10k entities            |
| Rule fire     | <10μs  | per rule                |
| Full tick     | <100ms | 10k entities, 100 rules |
| Query         | <10ms  | 10k entities            |

**Post-1.0 Targets** (incremental matching):

| Operation      | Target | Scale                       |
| -------------- | ------ | --------------------------- |
| Full tick      | <10ms  | 10k entities (steady state) |
| Pattern update | <1ms   | per changed entity          |

### 8.3 Optimization Priorities

1. **Pattern Matching** - Most critical for rule engine performance
   - [ ] Index usage optimization
   - [ ] Join order optimization
   - [ ] Archetype pruning

2. **VM Execution** - Bytecode interpretation overhead
   - [ ] Opcode dispatch optimization
   - [ ] Inline caching
   - [ ] Constant folding

3. **Memory Layout** - Cache efficiency
   - [ ] Component storage layout
   - [ ] Archetype grouping effectiveness
   - [ ] Reduce allocations in hot paths

4. **Incremental Matching** (Future)
   - [ ] Track world changes
   - [ ] Update match sets incrementally
   - [ ] RETE-style optimization

### 8.4 Profiling

- [ ] CPU profiling with flamegraph
- [ ] Memory profiling with heaptrack
- [ ] Cache analysis with cachegrind
- [ ] Allocation tracking

---

## Phase 9: Maturity & Stability

**Goal**: Production-ready quality.

### 9.1 Robustness

- [ ] Comprehensive error handling
- [ ] Graceful degradation
- [ ] Input validation at all boundaries
- [ ] Fuzz testing for parser and VM
- [ ] Property-based testing for core algorithms

### 9.2 Stability

- [ ] API freeze (1.0 commitment)
- [ ] Semantic versioning
- [ ] Deprecation policy
- [ ] Migration guides for breaking changes

### 9.3 Ecosystem

- [ ] Package registry (crates.io)
- [ ] Example repository
- [ ] Community guidelines
- [ ] Issue templates
- [ ] Contributing guide

### 9.4 Tooling

- [ ] Language server (LSP) for IDE integration
- [ ] Syntax highlighting for editors
- [ ] Formatter
- [ ] Linter

---

## Benchmarking Strategy

### Continuous Benchmarking

Every PR runs benchmarks and compares against main:

```yaml
# .github/workflows/bench.yml
on:
  pull_request:
benchmark:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v3
    - name: Run benchmarks
      run: cargo bench -- --save-baseline pr
    - name: Compare with main
      run: cargo bench -- --baseline main --compare
```

### Before/After Protocol

For major changes:

1. **Baseline**: Run full benchmark suite, record results
2. **Implement**: Make the change
3. **Measure**: Run benchmarks again
4. **Document**: Record delta in PR description
5. **Investigate**: If regression >5%, investigate or justify

### Benchmark Categories

| Category    | Frequency | Purpose                           |
| ----------- | --------- | --------------------------------- |
| Micro       | Every PR  | Catch regressions in hot paths    |
| Integration | Daily     | End-to-end tick performance       |
| Scale       | Weekly    | Behavior at large entity counts   |
| Memory      | Weekly    | Allocation patterns, memory usage |

---

## Example Maintenance Strategy

### Living Examples

Examples are:
1. **Tested**: Each example has integration tests that run in CI
2. **Versioned**: Examples specify which Longtable version they target
3. **Documented**: Each example has README explaining concepts demonstrated
4. **Exercised**: CI runs each example as part of the test suite

### Update Protocol

When making changes that affect examples:

1. **Check**: Run `cargo test --examples` before starting
2. **Update**: Modify examples alongside implementation changes
3. **Test**: Verify examples still pass
4. **Document**: Update example READMEs if behavior changes

### Example Test Structure

```rust
// examples/adventure/tests/adventure_test.rs

#[test]
fn test_adventure_starts_correctly() {
    let world = load_and_tick("examples/adventure/_.lt", vec![])?;

    // Player exists
    let player = query_one(&world, ":where [[?e :tag/player]] :return ?e")?;
    assert!(player.is_some());

    // Starting room is cave entrance
    let room = query_one(&world, ":where [[?p :tag/player] [?p :in-room ?r]] :return ?r")?;
    let name = get(&world, room, kw!(name/value))?;
    assert_eq!(name, Value::String("Cave Entrance".into()));
}

#[test]
fn test_go_north_changes_room() {
    let world = load_and_tick("examples/adventure/_.lt", vec![])?;
    let world = tick(&world, vec![input("go north")])?;

    let room = query_one(&world, /* get player's room */)?;
    let name = get(&world, room, kw!(name/value))?;
    assert_eq!(name, Value::String("Main Hall".into()));
}
```

---

## Timeline Overview

This is not a time estimate, but a logical ordering of work:

```
┌─────────────────────────────────────────────────────────────┐
│  CORE SEMANTICS (The Real Project)                          │
├─────────────────────────────────────────────────────────────┤
│  Phase 0: Bootstrap            ████                         │
│  Phase 1: API Design           ████████████████             │
│  Phase 1.5: Semantic Spike     ████████                     │
│  Phase 2: Foundation           ████████                     │
│  Phase 2.5: World Without Rules████████  ← First REPL       │
│  Phase 3: Language             ████████████                 │
│  Phase 4: Execution            ████████████████ ← It works! │
├─────────────────────────────────────────────────────────────┤
│  USABILITY (Make It Livable)                                │
├─────────────────────────────────────────────────────────────┤
│  Phase 5: Interface            ████████                     │
│  Phase 6: Observability        ████████                     │
├─────────────────────────────────────────────────────────────┤
│  POST-MVP (Only If It Earns It) — ASPIRATIONAL              │
├─────────────────────────────────────────────────────────────┤
│  Phase 7: Examples             ████████████                 │
│  Phase 8: Optimization         ████████████                 │
│  Phase 9: Maturity             ████████████████             │
└─────────────────────────────────────────────────────────────┘
```

**Critical path**: Phases 0 → 1 → 1.5 → 2 → 2.5 → 3 → 4 are sequential.

**Key milestones**:
- Phase 1.5: Semantic confidence (can throw away code)
- Phase 2.5: First interactive REPL, queries work, adventure world exists (major morale win)
- Phase 4: Adventure game plays end-to-end (system is complete)

**Phases 7-9 are aspirational**. They have equal narrative weight in this document but NOT equal commitment. Do not feel obligated to implement LSP, incremental matching, or community infrastructure just because they're "in the plan." The system is **complete** at Phase 6.

Phases 5-6 can partially parallelize after Phase 4.

---

## Success Criteria

### Phase 2.5 Milestone: "World Without Rules"

**Primary goal**: Something interactive exists. Queries work. You can explore.

- [x] REPL can create and query worlds
- [x] Adventure game world exists (static, no rules)
- [x] Serialization round-trips work
- [x] Aggregation and relationships work
- [x] You can play with data, even without rules

This is a **major morale checkpoint**. If Phase 2.5 feels like a real system, you have the energy for Phase 3-4.

### Minimum Viable Product (Phase 4 + basic Phase 5)

**Primary goal**: Correct semantics, deterministic behavior, usable development experience.

- [ ] Adventure game example runs end-to-end
- [ ] All spec sections have basic implementation
- [ ] REPL is usable for development
- [ ] Semantic spike tests all pass
- [ ] VM mutation model decision documented
- [ ] Performance usable at 1k entities (not optimized)
- [ ] Errors include full context (rule, bindings, expression)

**Explicitly NOT required for MVP**:
- 10k entity performance
- Full observability (tracing, debugger, time travel)
- Fine-grained derived invalidation
- Incremental pattern matching

### Version 1.0 (Phase 8 complete)

- [ ] All spec features fully implemented
- [ ] Comprehensive test coverage (>80%)
- [ ] Performance meets 1.0 targets at 10k entities
- [ ] Documentation complete
- [ ] Public APIs stable (no breaking changes planned)
- [ ] Internal APIs stable (internal tier frozen)

### Mature Product (Post-1.0)

- [ ] Performance optimized (incremental matching)
- [ ] Fine-grained derived invalidation
- [ ] Rich tooling (LSP, formatter, linter)
- [ ] Community examples and extensions
- [ ] Production use cases validated

---

*This plan is a living document. Update it as implementation reveals new insights.*
