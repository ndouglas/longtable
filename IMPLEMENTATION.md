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

| Bad At | Why | Don't Try To Fix |
|--------|-----|------------------|
| Twitch real-time games | Ticks can take seconds; no frame budget | Adding "fast mode" |
| Very large worlds (100k+ entities) | Initial impl is O(n) matching | Premature optimization |
| Opaque imperative scripts | Rules are declarative and reactive | Adding "script mode" |
| Non-deterministic behavior | Determinism is a core guarantee | Adding "random mode" |
| Tight Rust integration | DSL defines all domain logic | Typed Rust components |

If someone asks "can Longtable do X?" and X is on this list, the answer is "not well, by design."

### Managing Energy on a Long Roadmap

This is a substantial solo project. Technical discipline is necessary but not sufficient—morale management is equally important.

**Danger zones**:
- Phase 3 (Language): Parser + compiler + VM can induce fatigue
- Phase 4 (Execution): Semantic bugs that don't reproduce drain energy

**Mitigation strategy**: Ship **tiny vertical wins** early:

| Win | When | Why It Helps |
|-----|------|--------------|
| REPL that mutates world (no rules) | Phase 2.5 | Something interactive exists |
| Queries work | Phase 2.5 | You can explore data |
| Adventure world exists (static) | Phase 2.5 | The example is real |
| One rule fires correctly | Phase 4 start | Rules are no longer theoretical |
| Adventure game plays | Phase 4 end | The system is complete |

Use the adventure game example as a **motivational artifact**, not just a test. Seeing "go north" work for the first time is worth more than a dozen passing unit tests.

**Warning: Examples Become Anchors**

"Examples are tests" is philosophically correct but practically dangerous:
- Examples evolve slower than code
- Fixing examples is emotionally heavier than fixing unit tests
- You'll hesitate to refactor because "the adventure game breaks"

Be aware: examples become **anchors**. You'll need discipline to prune or rewrite them when semantics demand it. Don't let a pretty example prevent a necessary change.

### API Stability Tiers

Not all APIs are equal. We distinguish:

| Tier | Stability | Examples | When Frozen |
|------|-----------|----------|-------------|
| **Public Data** | Stable after Phase 1 | `World`, `Value`, `EntityId`, `Error` | API Design phase |
| **DSL Semantics** | Stable after Phase 1.5 | Rule behavior, query semantics, effect visibility | Semantic Spike |
| **DSL Syntax** | Provisional until Phase 3 | Exact syntax forms, keywords, sugar | Language phase |
| **Execution APIs** | Semantic sketches until Phase 4 | `CompiledRule`, `PatternMatcher`, `Opcode`, VM | After Execution works |
| **Private** | Can change anytime | Storage layout, cache structures | Never |

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

```rust
/// Core value type for all Longtable data.
/// Optimized for small values (inline) with Arc for large/shared data.
#[derive(Clone, Debug)]
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(LtString),        // Interned or Arc<str>
    Symbol(SymbolId),        // Interned
    Keyword(KeywordId),      // Interned
    EntityRef(EntityId),
    Vec(LtVec),              // Persistent vector
    Set(LtSet),              // Persistent set
    Map(LtMap),              // Persistent map
    Fn(LtFn),                // Function reference
}

/// Entity identifier with generational index for stale detection.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct EntityId {
    pub index: u64,
    pub generation: u32,
}

/// Type descriptors for schema validation.
#[derive(Clone, Debug, PartialEq)]
pub enum Type {
    Nil,
    Bool,
    Int,
    Float,
    String,
    Symbol,
    Keyword,
    EntityRef,
    Vec(Box<Type>),
    Set(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Option(Box<Type>),
    Any,
}

// Core traits
pub trait LtEq { fn lt_eq(&self, other: &Self) -> bool; }
pub trait LtHash { fn lt_hash(&self, state: &mut impl Hasher); }
pub trait LtDisplay { fn lt_display(&self, f: &mut Formatter) -> fmt::Result; }
```

**Design validation tasks**:
- [x] Verify Value size (target: ≤32 bytes for inline efficiency)
- [x] Verify EntityId can represent 2^64 entities with 2^32 generations
- [x] Verify Type can express all spec type annotations
- [x] Write property tests for Eq/Hash consistency

#### Error System (`lt-foundation::error`)

```rust
/// Unified error type with rich context.
#[derive(Debug)]
pub struct Error {
    pub kind: ErrorKind,
    pub context: ErrorContext,
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

#[derive(Debug, Clone)]
pub enum ErrorKind {
    // Parsing
    LexError { message: String, span: Span },
    ParseError { message: String, span: Span },

    // Type errors
    TypeError { expected: Type, got: Type, span: Option<Span> },
    SchemaViolation { component: KeywordId, field: KeywordId, expected: Type },

    // Runtime
    StaleEntity { entity: EntityId },
    ComponentNotFound { entity: EntityId, component: KeywordId },
    DivisionByZero,
    IndexOutOfBounds { index: i64, len: usize },

    // Rule engine
    ConstraintViolation { constraint: KeywordId, entity: EntityId, message: String },
    InfiniteLoop { rule: KeywordId, iterations: usize },

    // System
    IoError { path: PathBuf, operation: &'static str },
    Internal { message: String },
}

#[derive(Debug, Clone, Default)]
pub struct ErrorContext {
    pub tick: Option<u64>,
    pub rule: Option<KeywordId>,
    pub entity: Option<EntityId>,
    pub expression: Option<String>,
    pub bindings: Option<Vec<(SymbolId, Value)>>,
    pub span: Option<Span>,
    pub file: Option<PathBuf>,
}

pub type Result<T> = std::result::Result<T, Error>;
```

**Design validation tasks**:
- [x] Verify ErrorKind covers all spec error cases
- [x] Verify ErrorContext provides sufficient debugging info
- [x] Test error display formatting

#### Persistent Collections (`lt-foundation::collections`)

Thin wrappers around `im` crate with Longtable-specific behavior:

```rust
/// Persistent vector with structural sharing.
#[derive(Clone, Debug)]
pub struct LtVec(im::Vector<Value>);

/// Persistent hash set.
#[derive(Clone, Debug)]
pub struct LtSet(im::HashSet<Value>);

/// Persistent hash map.
#[derive(Clone, Debug)]
pub struct LtMap(im::HashMap<Value, Value>);

impl LtVec {
    pub fn new() -> Self;
    pub fn len(&self) -> usize;
    pub fn get(&self, index: usize) -> Option<&Value>;
    pub fn push_back(&self, value: Value) -> Self;  // Returns new vec
    pub fn update(&self, index: usize, value: Value) -> Result<Self>;
    pub fn iter(&self) -> impl Iterator<Item = &Value>;
    // ... etc
}
```

**Design validation tasks**:
- [x] Benchmark structural sharing efficiency
- [x] Verify O(log n) access times at 100k elements
- [x] Test clone performance (should be O(1))

### 1.2 Layer 1: Storage APIs

#### Entity Store (`lt-storage::entity`)

```rust
/// Manages entity lifecycle and generation tracking.
pub struct EntityStore {
    // Internal details hidden from API
}

impl EntityStore {
    pub fn new() -> Self;

    /// Spawn a new entity, returns its ID.
    pub fn spawn(&mut self) -> EntityId;

    /// Destroy an entity. Returns Ok(()) if existed, Err if stale.
    pub fn destroy(&mut self, id: EntityId) -> Result<()>;

    /// Check if entity exists and is not stale.
    pub fn exists(&self, id: EntityId) -> bool;

    /// Validate entity is live, return error with context if stale.
    pub fn validate(&self, id: EntityId) -> Result<()>;

    /// Total live entity count.
    pub fn len(&self) -> usize;

    /// Iterate all live entity IDs.
    pub fn iter(&self) -> impl Iterator<Item = EntityId> + '_;
}
```

#### Component Store (`lt-storage::component`)

```rust
/// Schema definition for a component type.
#[derive(Clone, Debug)]
pub struct ComponentSchema {
    pub name: KeywordId,
    pub fields: Vec<FieldSchema>,
    pub is_tag: bool,  // Single-field boolean component
}

#[derive(Clone, Debug)]
pub struct FieldSchema {
    pub name: KeywordId,
    pub ty: Type,
    pub default: Option<Value>,
    pub required: bool,
}

/// Stores all component data with archetype optimization.
pub struct ComponentStore {
    // Archetype-based storage for cache efficiency
}

impl ComponentStore {
    pub fn new() -> Self;

    /// Register a component schema. Must be called before using component.
    pub fn register_schema(&mut self, schema: ComponentSchema) -> Result<()>;

    /// Get schema for a component type.
    pub fn schema(&self, component: KeywordId) -> Option<&ComponentSchema>;

    /// Set component on entity. Creates component if not present.
    pub fn set(&mut self, entity: EntityId, component: KeywordId, value: Value) -> Result<()>;

    /// Set a specific field within a component.
    pub fn set_field(&mut self, entity: EntityId, component: KeywordId, field: KeywordId, value: Value) -> Result<()>;

    /// Get component value. Returns None if entity lacks component.
    pub fn get(&self, entity: EntityId, component: KeywordId) -> Option<&Value>;

    /// Get specific field. Returns None if missing.
    pub fn get_field(&self, entity: EntityId, component: KeywordId, field: KeywordId) -> Option<&Value>;

    /// Check if entity has component.
    pub fn has(&self, entity: EntityId, component: KeywordId) -> bool;

    /// Remove component from entity.
    pub fn remove(&mut self, entity: EntityId, component: KeywordId) -> Result<Option<Value>>;

    /// Get archetype for entity (set of component types).
    pub fn archetype(&self, entity: EntityId) -> Option<&Archetype>;

    /// Iterate entities with specific component.
    pub fn with_component(&self, component: KeywordId) -> impl Iterator<Item = EntityId> + '_;

    /// Iterate entities matching archetype (having all listed components).
    pub fn with_archetype(&self, components: &[KeywordId]) -> impl Iterator<Item = EntityId> + '_;
}

/// Represents a set of component types.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Archetype {
    pub components: Vec<KeywordId>,  // Sorted for consistent identity
}
```

**Design validation tasks**:
- [x] Benchmark iteration over 10k entities with component filter
- [x] Verify archetype grouping improves cache locality
- [x] Test schema validation catches type mismatches

#### Relationship Store (`lt-storage::relationship`)

```rust
#[derive(Clone, Debug)]
pub struct RelationshipSchema {
    pub name: KeywordId,
    pub storage: Storage,
    pub cardinality: Cardinality,
    pub on_target_delete: OnDelete,
    pub on_violation: OnViolation,
    pub attributes: Vec<FieldSchema>,  // Only for entity storage
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Storage { Field, Entity }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cardinality { OneToOne, OneToMany, ManyToOne, ManyToMany }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OnDelete { Remove, Cascade, Nullify }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OnViolation { Error, Replace }

pub struct RelationshipStore {
    // Bidirectional indices for O(1) traversal
}

impl RelationshipStore {
    pub fn new() -> Self;

    pub fn register_schema(&mut self, schema: RelationshipSchema) -> Result<()>;

    pub fn schema(&self, relationship: KeywordId) -> Option<&RelationshipSchema>;

    /// Create a relationship edge. Idempotent (linking existing edge is no-op).
    pub fn link(&mut self, source: EntityId, relationship: KeywordId, target: EntityId) -> Result<()>;

    /// Remove a relationship edge. Idempotent (unlinking missing edge is no-op).
    pub fn unlink(&mut self, source: EntityId, relationship: KeywordId, target: EntityId) -> Result<()>;

    /// Get targets of relationship from source (forward traversal).
    pub fn targets(&self, source: EntityId, relationship: KeywordId) -> impl Iterator<Item = EntityId> + '_;

    /// Get sources pointing to target (reverse traversal).
    pub fn sources(&self, target: EntityId, relationship: KeywordId) -> impl Iterator<Item = EntityId> + '_;

    /// Check if specific edge exists.
    pub fn has_edge(&self, source: EntityId, relationship: KeywordId, target: EntityId) -> bool;

    /// Handle entity destruction (process on_target_delete).
    pub fn on_entity_destroyed(&mut self, entity: EntityId) -> Result<Vec<EntityId>>; // Returns cascade victims
}
```

**Design validation tasks**:
- [x] Verify bidirectional index consistency
- [x] Test cardinality enforcement
- [x] Benchmark traversal at 10k relationships

#### World (`lt-storage::world`)

```rust
/// Immutable snapshot of simulation state.
/// Clone is O(1) due to structural sharing.
#[derive(Clone)]
pub struct World {
    // All fields use persistent data structures
}

impl World {
    /// Create empty world with given seed.
    pub fn new(seed: u64) -> Self;

    // Entity operations (return new World)
    pub fn spawn(&self, components: LtMap) -> Result<(World, EntityId)>;
    pub fn destroy(&self, entity: EntityId) -> Result<World>;

    // Component operations
    pub fn get(&self, entity: EntityId, component: KeywordId) -> Result<Option<Value>>;
    pub fn get_field(&self, entity: EntityId, component: KeywordId, field: KeywordId) -> Result<Option<Value>>;
    pub fn set(&self, entity: EntityId, component: KeywordId, value: Value) -> Result<World>;
    pub fn set_field(&self, entity: EntityId, component: KeywordId, field: KeywordId, value: Value) -> Result<World>;

    // Relationship operations
    pub fn link(&self, source: EntityId, relationship: KeywordId, target: EntityId) -> Result<World>;
    pub fn unlink(&self, source: EntityId, relationship: KeywordId, target: EntityId) -> Result<World>;

    // Schema access
    pub fn component_schema(&self, name: KeywordId) -> Option<&ComponentSchema>;
    pub fn relationship_schema(&self, name: KeywordId) -> Option<&RelationshipSchema>;

    // Metadata
    pub fn tick(&self) -> u64;
    pub fn seed(&self) -> u64;
    pub fn entity_count(&self) -> usize;

    // Iteration
    pub fn entities(&self) -> impl Iterator<Item = EntityId> + '_;
    pub fn with_component(&self, component: KeywordId) -> impl Iterator<Item = EntityId> + '_;

    // History
    pub fn previous(&self) -> Option<&World>;

    // Hashing for speculation memoization
    pub fn content_hash(&self) -> u64;
}
```

**Design validation tasks**:
- [x] Verify World::clone() is O(1)
- [x] Benchmark world fork + small modifications
- [x] Test previous() chain integrity

### 1.3 Layer 2: Language APIs

#### Lexer (`lt-language::lexer`)

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    // Delimiters
    LParen, RParen, LBracket, RBracket, LBrace, RBrace,
    HashSet,  // #{

    // Literals
    Nil, True, False,
    Int(i64), Float(f64), String(String),
    Symbol(String), Keyword(String),

    // Special
    Quote, Backtick, Unquote, UnquoteSplice,
    TaggedLiteral(String),  // #name

    // Meta
    Comment, Whitespace, Eof, Error(String),
}

pub struct Lexer<'src> {
    source: &'src str,
    // ...
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str) -> Self;
    pub fn next_token(&mut self) -> Token;
    pub fn tokenize_all(source: &str) -> Result<Vec<Token>>;
}
```

#### Parser & AST (`lt-language::parser`, `lt-language::ast`)

```rust
/// Abstract syntax tree node.
#[derive(Clone, Debug)]
pub enum Ast {
    Nil(Span),
    Bool(bool, Span),
    Int(i64, Span),
    Float(f64, Span),
    String(String, Span),
    Symbol(String, Span),
    Keyword(String, Span),

    List(Vec<Ast>, Span),
    Vector(Vec<Ast>, Span),
    Set(Vec<Ast>, Span),
    Map(Vec<(Ast, Ast)>, Span),

    Quote(Box<Ast>, Span),
    Unquote(Box<Ast>, Span),
    UnquoteSplice(Box<Ast>, Span),
    SyntaxQuote(Box<Ast>, Span),

    Tagged(String, Box<Ast>, Span),
}

impl Ast {
    pub fn span(&self) -> Span;
}

pub struct Parser<'src> {
    lexer: Lexer<'src>,
    // ...
}

impl<'src> Parser<'src> {
    pub fn new(source: &'src str) -> Self;
    pub fn parse(&mut self) -> Result<Ast>;
    pub fn parse_all(&mut self) -> Result<Vec<Ast>>;
}

/// Parse source into AST.
pub fn parse(source: &str) -> Result<Vec<Ast>>;
```

#### Compiler (`lt-language::compiler`)

```rust
/// Compiled program ready for execution.
pub struct Program {
    pub bytecode: Bytecode,
    pub constants: Vec<Value>,
    pub schemas: Vec<ComponentSchema>,
    pub relationships: Vec<RelationshipSchema>,
    pub rules: Vec<CompiledRule>,
    pub constraints: Vec<CompiledConstraint>,
    pub derived: Vec<CompiledDerived>,
    pub functions: Vec<CompiledFunction>,
}

/// Compiled rule ready for matching and execution.
pub struct CompiledRule {
    pub name: KeywordId,
    pub salience: i32,
    pub enabled: bool,
    pub once: bool,
    pub patterns: Vec<CompiledPattern>,
    pub guards: Bytecode,
    pub then_body: Bytecode,
    pub source_location: SourceLocation,
}

pub struct CompiledPattern {
    pub entity_var: SymbolId,
    pub component: KeywordId,
    pub field: Option<KeywordId>,
    pub binding: PatternBinding,
    pub negated: bool,
}

pub enum PatternBinding {
    Variable(SymbolId),
    Literal(Value),
    Wildcard,
}

pub struct Compiler {
    // ...
}

impl Compiler {
    pub fn new() -> Self;

    /// Compile AST into executable program.
    pub fn compile(&mut self, ast: &[Ast]) -> Result<Program>;

    /// Compile a single expression (for REPL).
    pub fn compile_expr(&mut self, ast: &Ast) -> Result<Bytecode>;
}
```

#### Bytecode VM (`lt-language::vm`)

```rust
/// Bytecode instruction set.
#[derive(Clone, Debug)]
pub enum Opcode {
    // Stack
    Nop,
    Push(u16),      // Push constant by index
    Pop,
    Dup,

    // Arithmetic
    Add, Sub, Mul, Div, Mod, Neg,

    // Comparison
    Eq, Ne, Lt, Le, Gt, Ge,

    // Logic
    Not, And, Or,

    // Control flow
    Jump(i16),
    JumpIf(i16),
    JumpIfNot(i16),
    Call(u16),      // Call function by index
    Return,

    // Variables
    LoadLocal(u16),
    StoreLocal(u16),
    LoadBinding(u16),  // Load from pattern bindings

    // Data access
    GetComponent,   // Stack: [entity, component] -> [value]
    GetField,       // Stack: [entity, component, field] -> [value]

    // Effects (during rule execution)
    Spawn,          // Stack: [components_map] -> [entity_id]
    Destroy,        // Stack: [entity] -> []
    Set,            // Stack: [entity, component, value] -> []
    SetField,       // Stack: [entity, component, field, value] -> []
    Link,           // Stack: [source, relationship, target] -> []
    Unlink,         // Stack: [source, relationship, target] -> []
    Print,          // Stack: [value] -> []

    // Collections
    VecNew,
    VecPush,
    VecGet,
    MapNew,
    MapInsert,
    MapGet,
    SetNew,
    SetInsert,
    SetContains,
}

#[derive(Clone, Debug, Default)]
pub struct Bytecode {
    pub opcodes: Vec<Opcode>,
}

/// Virtual machine for bytecode execution.
pub struct Vm {
    stack: Vec<Value>,
    locals: Vec<Value>,
    bindings: Vec<Value>,
    // ...
}

/// Context for VM execution (provides world access, effect handling).
pub trait VmContext {
    fn get_constant(&self, index: u16) -> &Value;
    fn get_function(&self, index: u16) -> &CompiledFunction;
    fn world(&self) -> &World;
    fn world_mut(&mut self) -> &mut World;  // For effects
    fn rng(&mut self) -> &mut impl Rng;
    fn print(&mut self, value: &Value);
}

impl Vm {
    pub fn new() -> Self;

    /// Execute bytecode with given context and return result.
    pub fn execute(&mut self, bytecode: &Bytecode, ctx: &mut impl VmContext) -> Result<Value>;

    /// Set pattern bindings for rule execution.
    pub fn set_bindings(&mut self, bindings: Vec<Value>);
}
```

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
> ```rust
> enum EffectMode { Direct, Buffered }
>
> fn apply_effect(&mut self, effect: Effect, mode: EffectMode) -> Result<()> {
>     match mode {
>         EffectMode::Direct => self.world.apply(effect),
>         EffectMode::Buffered => self.pending_effects.push(effect),
>     }
> }
> ```
>
> You don't need intent buffering yet—but you need a **place** to put it later. If you let VM opcodes implicitly assume mutation semantics, you'll bake in assumptions that are painful to uproot.

**Design validation tasks**:
- [x] Span and Token types implemented with proper source tracking
- [x] Lexer tokenizes all spec literals (integers, floats, strings, symbols, keywords, collections)
- [x] Parser builds AST for all expression forms (lists, vectors, sets, maps, quotes, tags)
- [x] 62 unit tests for lexer and parser
- [ ] Round-trip test: source → AST → bytecode → execution → expected value
- [ ] Verify all spec expression forms can be represented
- [ ] Benchmark VM execution (target: 1M simple ops/sec)

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

```rust
/// A set of variable bindings from pattern matching.
#[derive(Clone, Debug)]
pub struct Bindings {
    values: Vec<(SymbolId, Value)>,
}

impl Bindings {
    pub fn get(&self, var: SymbolId) -> Option<&Value>;
    pub fn iter(&self) -> impl Iterator<Item = (SymbolId, &Value)>;
    pub fn to_vec(&self) -> Vec<Value>;  // For VM binding array
}

/// Compiled patterns optimized for matching.
pub struct PatternMatcher {
    // Query plan, index usage strategy, etc.
}

impl PatternMatcher {
    /// Compile patterns into optimized matcher.
    pub fn compile(patterns: &[CompiledPattern], world: &World) -> Result<Self>;

    /// Find all binding sets that satisfy the patterns.
    pub fn find_matches(&self, world: &World) -> impl Iterator<Item = Bindings> + '_;

    /// Check if any matches exist (early exit).
    pub fn has_matches(&self, world: &World) -> bool;

    /// Count matches without allocating all bindings.
    pub fn count_matches(&self, world: &World) -> usize;
}

/// Explain how a query would be executed.
pub fn explain_query(patterns: &[CompiledPattern], world: &World) -> QueryPlan;

pub struct QueryPlan {
    pub steps: Vec<QueryStep>,
    pub estimated_cost: usize,
}

pub struct QueryStep {
    pub operation: String,
    pub estimated_rows: usize,
}
```

#### Rule Engine (`lt-engine::rules`)

```rust
/// Refraction key: identifies a unique rule activation.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ActivationKey {
    pub rule: KeywordId,
    pub binding_hash: u64,
}

/// A rule activation ready to fire.
pub struct Activation {
    pub rule: KeywordId,
    pub bindings: Bindings,
    pub salience: i32,
    pub specificity: usize,
}

/// Manages rule execution within a tick.
pub struct RuleEngine {
    refracted: HashSet<ActivationKey>,
    once_fired: HashSet<KeywordId>,
    // ...
}

impl RuleEngine {
    pub fn new() -> Self;

    /// Reset refraction set for new tick.
    pub fn begin_tick(&mut self);

    /// Find all current activations, respecting refraction.
    pub fn find_activations(&self, rules: &[CompiledRule], world: &World) -> Vec<Activation>;

    /// Execute until quiescence. Returns final world and effect log.
    pub fn run_to_quiescence(
        &mut self,
        rules: &[CompiledRule],
        world: World,
        vm: &mut Vm,
        ctx: &mut impl VmContext,
    ) -> Result<(World, Vec<EffectRecord>)>;

    /// Execute a single rule (for debugging/stepping).
    pub fn fire_one(
        &mut self,
        activation: &Activation,
        rule: &CompiledRule,
        world: World,
        vm: &mut Vm,
        ctx: &mut impl VmContext,
    ) -> Result<(World, Vec<EffectRecord>)>;
}
```

#### Query System (`lt-engine::query`)

```rust
/// Compiled query ready for execution.
pub struct CompiledQuery {
    pub patterns: Vec<CompiledPattern>,
    pub let_bindings: Vec<(SymbolId, Bytecode)>,
    pub aggregates: Vec<(SymbolId, Aggregate)>,
    pub group_by: Vec<SymbolId>,
    pub guards: Bytecode,
    pub order_by: Vec<(SymbolId, Ordering)>,
    pub limit: Option<usize>,
    pub return_expr: Bytecode,
}

#[derive(Clone, Debug)]
pub enum Aggregate {
    Count(SymbolId),
    Sum(SymbolId),
    Min(SymbolId),
    Max(SymbolId),
    Avg(SymbolId),
    MinBy(SymbolId, SymbolId),
    MaxBy(SymbolId, SymbolId),
    Collect(SymbolId),
    CollectSet(SymbolId),
}

pub struct QueryExecutor {
    // ...
}

impl QueryExecutor {
    pub fn new() -> Self;

    /// Execute query and return results.
    pub fn execute(&self, query: &CompiledQuery, world: &World, vm: &mut Vm) -> Result<Vec<Value>>;

    /// Execute and return first result.
    pub fn execute_one(&self, query: &CompiledQuery, world: &World, vm: &mut Vm) -> Result<Option<Value>>;

    /// Check if any results exist.
    pub fn execute_exists(&self, query: &CompiledQuery, world: &World) -> Result<bool>;

    /// Count results without full execution.
    pub fn execute_count(&self, query: &CompiledQuery, world: &World) -> Result<usize>;
}
```

#### Derived Components (`lt-engine::derived`)

> **WARNING: High Semantic Risk**
>
> Derived components interact with pattern matching, rule activation, caching, invalidation, AND speculative execution. This five-way intersection is the most dangerous part of the system.
>
> **Initial implementation strategy**: Derived invalidation is **conservative**. The first implementation will invalidate *all* derived caches on *any* world mutation. Fine-grained invalidation (tracking entity-scoped, query-scoped, binding-dependent dependencies) is an **optimization**, not a correctness requirement.
>
> Do not attempt clever invalidation until the naive approach is proven correct.

```rust
pub struct CompiledDerived {
    pub name: KeywordId,
    pub for_var: SymbolId,
    pub patterns: Vec<CompiledPattern>,
    pub aggregates: Vec<(SymbolId, Aggregate)>,
    pub value_expr: Bytecode,
    // NOTE: dependencies field intentionally simple for now.
    // True dependencies are entity-scoped, query-scoped, and binding-dependent.
    // Initial impl ignores this and invalidates everything conservatively.
    pub dependencies: Vec<KeywordId>,
}

/// Cache for derived component values.
pub struct DerivedCache {
    // entity -> component -> cached value + validity
}

impl DerivedCache {
    pub fn new() -> Self;

    /// Get derived value, computing if necessary.
    pub fn get(
        &mut self,
        entity: EntityId,
        derived: &CompiledDerived,
        world: &World,
        vm: &mut Vm,
    ) -> Result<Value>;

    /// Invalidate caches for changed component.
    /// INITIAL IMPL: May invalidate everything conservatively.
    pub fn invalidate(&mut self, entity: EntityId, component: KeywordId);

    /// Invalidate all caches (e.g., on tick boundary or any mutation).
    pub fn invalidate_all(&mut self);
}
```

#### Constraints (`lt-engine::constraint`)

```rust
pub struct CompiledConstraint {
    pub name: KeywordId,
    pub salience: i32,
    pub patterns: Vec<CompiledPattern>,
    pub guards: Bytecode,
    pub checks: Vec<Bytecode>,
    pub on_violation: OnViolation,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OnViolation {
    Rollback,
    Warn,
}

pub struct ConstraintChecker {
    // ...
}

impl ConstraintChecker {
    pub fn new() -> Self;

    /// Check all constraints against world.
    /// Returns violations found.
    pub fn check_all(
        &self,
        constraints: &[CompiledConstraint],
        world: &World,
        vm: &mut Vm,
    ) -> Vec<ConstraintViolation>;
}

pub struct ConstraintViolation {
    pub constraint: KeywordId,
    pub entity: EntityId,
    pub check_index: usize,
    pub message: String,
    pub on_violation: OnViolation,
}
```

#### Effects & Provenance (`lt-engine::effects`)

```rust
/// Record of a single effect for provenance tracking.
#[derive(Clone, Debug)]
pub struct EffectRecord {
    pub tick: u64,
    pub entity: EntityId,
    pub kind: EffectKind,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub source: EffectSource,
}

#[derive(Clone, Debug)]
pub enum EffectKind {
    Spawn,
    Destroy,
    SetComponent(KeywordId),
    SetField(KeywordId, KeywordId),
    Link(KeywordId, EntityId),
    Unlink(KeywordId, EntityId),
}

#[derive(Clone, Debug)]
pub enum EffectSource {
    Rule(KeywordId, Option<Bindings>),
    Constraint(KeywordId),
    External,
}

/// Tracks effects for debugging and provenance.
pub struct EffectLog {
    records: Vec<EffectRecord>,
}

impl EffectLog {
    pub fn new() -> Self;
    pub fn record(&mut self, effect: EffectRecord);
    pub fn clear(&mut self);
    pub fn iter(&self) -> impl Iterator<Item = &EffectRecord>;

    /// Find what set a particular field to its current value.
    pub fn why(&self, entity: EntityId, component: KeywordId, field: Option<KeywordId>) -> Option<&EffectRecord>;
}
```

### 1.5 Layer 4: Interface APIs

#### Standard Library (`lt-stdlib`)

```rust
/// Registry of built-in functions.
pub struct StdLib {
    functions: HashMap<KeywordId, NativeFunction>,
}

pub struct NativeFunction {
    pub name: KeywordId,
    pub arity: Arity,
    pub implementation: fn(&[Value], &mut impl VmContext) -> Result<Value>,
}

pub enum Arity {
    Exact(usize),
    Range(usize, usize),
    Variadic(usize),  // Minimum args
}

impl StdLib {
    /// Create stdlib with all built-in functions.
    pub fn new() -> Self;

    /// Get function by name.
    pub fn get(&self, name: KeywordId) -> Option<&NativeFunction>;

    /// Register additional native function.
    pub fn register(&mut self, func: NativeFunction);
}
```

#### REPL (`lt-runtime::repl`)

```rust
pub struct Repl {
    world: World,
    program: Program,
    history: Vec<String>,
    // ...
}

impl Repl {
    pub fn new(program: Program, world: World) -> Self;

    /// Evaluate input and return output.
    pub fn eval(&mut self, input: &str) -> Result<ReplOutput>;

    /// Execute a tick with given inputs.
    pub fn tick(&mut self, inputs: Vec<Value>) -> Result<TickResult>;

    /// Get current world.
    pub fn world(&self) -> &World;
}

pub enum ReplOutput {
    Value(Value),
    TickComplete(TickResult),
    Command(CommandResult),
    Error(Error),
}

pub struct TickResult {
    pub tick: u64,
    pub rules_fired: usize,
    pub entities_changed: usize,
    pub elapsed: Duration,
    pub output: Vec<String>,
    pub warnings: Vec<String>,
}
```

#### Serialization (`lt-runtime::serde`)

```rust
/// Save world state to bytes.
pub fn save(world: &World) -> Result<Vec<u8>>;

/// Restore world state from bytes.
/// Requires program to be loaded for recompilation.
pub fn restore(bytes: &[u8], program: &Program) -> Result<World>;

/// Save format version for compatibility checking.
pub const FORMAT_VERSION: &str = "0.1";
```

### 1.6 Layer 5: Observability APIs

#### Tracing (`lt-debug::trace`)

```rust
pub struct Tracer {
    active_traces: HashSet<TraceTarget>,
    output: Vec<TraceEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TraceTarget {
    Rule(KeywordId),
    Entity(EntityId),
    Component(KeywordId),
}

#[derive(Clone, Debug)]
pub struct TraceEvent {
    pub tick: u64,
    pub target: TraceTarget,
    pub kind: TraceKind,
    pub details: String,
}

#[derive(Clone, Debug)]
pub enum TraceKind {
    RuleFired { bindings: Vec<(String, Value)> },
    ComponentChanged { old: Value, new: Value },
    EntitySpawned,
    EntityDestroyed,
}

impl Tracer {
    pub fn new() -> Self;
    pub fn trace(&mut self, target: TraceTarget);
    pub fn untrace(&mut self, target: TraceTarget);
    pub fn record(&mut self, event: TraceEvent);
    pub fn events(&self) -> &[TraceEvent];
    pub fn clear(&mut self);
}
```

#### Debugging (`lt-debug::debugger`)

```rust
pub struct Debugger {
    breakpoints: HashSet<Breakpoint>,
    state: DebugState,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Breakpoint {
    Rule(KeywordId),
    Component(KeywordId),
    Entity(EntityId),
    Condition(KeywordId, Bytecode),  // Component + condition
}

pub enum DebugState {
    Running,
    Paused { activation: Activation, world: World },
}

pub enum DebugCommand {
    Step,       // Execute one rule
    Continue,   // Run to next breakpoint
    Inspect(EntityId),
    Locals,
    Quit,
}

impl Debugger {
    pub fn new() -> Self;
    pub fn set_breakpoint(&mut self, bp: Breakpoint);
    pub fn clear_breakpoint(&mut self, bp: &Breakpoint);
    pub fn execute(&mut self, cmd: DebugCommand, engine: &mut RuleEngine) -> Result<DebugOutput>;
}
```

#### Time Travel (`lt-debug::timetravel`)

```rust
pub struct Timeline {
    worlds: Vec<World>,  // World at each tick
    branches: HashMap<String, Vec<World>>,
    current_branch: String,
}

impl Timeline {
    pub fn new(initial: World) -> Self;

    /// Record a tick's result.
    pub fn record(&mut self, world: World);

    /// Go back n ticks.
    pub fn rollback(&mut self, n: usize) -> Option<&World>;

    /// Jump to specific tick.
    pub fn goto(&mut self, tick: u64) -> Option<&World>;

    /// Create named branch from current position.
    pub fn branch(&mut self, name: &str);

    /// Switch to named branch.
    pub fn switch(&mut self, name: &str) -> Result<&World>;

    /// Compare two worlds.
    pub fn diff(&self, a: &World, b: &World) -> WorldDiff;
}

pub struct WorldDiff {
    pub added_entities: Vec<EntityId>,
    pub removed_entities: Vec<EntityId>,
    pub changed_components: Vec<(EntityId, KeywordId, Value, Value)>,
}
```

### 1.7 API Validation Phase

Before implementing internals, validate that APIs compose correctly:

#### Validation Tests

```rust
// test_api_composition.rs

#[test]
fn test_world_spawn_set_query() {
    // Spawn entity, set component, query it back
    let world = World::new(42);
    let (world, id) = world.spawn(/* health component */).unwrap();
    let world = world.set(id, kw!(health), /* value */).unwrap();

    // Compile and execute query
    let query = compile_query("...");
    let results = execute_query(&query, &world);
    assert_eq!(results.len(), 1);
}

#[test]
fn test_rule_engine_basic_cycle() {
    // Define rule, create matching world, run engine
    let rule = compile_rule("...");
    let world = /* world with matching entities */;
    let mut engine = RuleEngine::new();

    engine.begin_tick();
    let (world, effects) = engine.run_to_quiescence(&[rule], world, &mut vm, &mut ctx)?;

    assert!(!effects.is_empty());
}

#[test]
fn test_full_tick_cycle() {
    // Parse -> Compile -> Create world -> Run tick -> Check constraints
    let source = include_str!("../examples/simple.lt");
    let ast = parse(source)?;
    let program = compile(&ast)?;
    let world = World::new(42);

    let result = tick(&program, world, vec![])?;

    assert!(result.constraints_passed);
}
```

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

```rust
#[test]
fn spike_rule_fires_once_per_match() {
    // Rule: when entity has :counter, increment it
    // Expected: fires once per entity, then stops (refraction)
}

#[test]
fn spike_changes_visible_to_later_rules() {
    // Rule A sets :flag true
    // Rule B matches on :flag true
    // Expected: Rule B fires in same tick after Rule A
}

#[test]
fn spike_refraction_uses_binding_identity() {
    // Rule matches [?e :value ?v]
    // After firing, change ?v to different value
    // Expected: rule does NOT re-fire (same ?e binding)
}

#[test]
fn spike_error_includes_context() {
    // Rule that divides by zero
    // Expected: error includes rule name, bindings, expression
}
```

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

- [ ] All spike tests pass
- [ ] Refraction semantics match spec Section 5.0.2
- [ ] Write visibility matches spec Section 2.2
- [ ] Errors include rule/binding context
- [ ] Team confident in semantic model
- [ ] Decision documented: any spec clarifications discovered

---

## Phase 2: Foundation Implementation

**Goal**: Implement Layer 0 and Layer 1 with full test coverage.

### 2.1 Value System

- [ ] Implement `Value` enum with all variants
- [ ] Implement `LtEq`, `LtHash`, `LtDisplay` traits
- [ ] Implement interning for symbols and keywords
- [ ] Property tests: Eq/Hash consistency, display round-trip
- [ ] Benchmark: Value clone, comparison, hashing

### 2.2 Persistent Collections

- [ ] Wrap `im` crate types with Longtable semantics
- [ ] Implement iteration with spec-compliant ordering
- [ ] Property tests: structural sharing, modification
- [ ] Benchmark: insert, lookup, iteration at 10k/100k elements

### 2.3 Error System

- [ ] Implement `Error` with all `ErrorKind` variants
- [ ] Implement `Display` with rich formatting
- [ ] Context builders for ergonomic error construction
- [ ] Test error messages are actionable

### 2.4 Entity Store

- [ ] Implement generational index allocator
- [ ] Implement spawn, destroy, exists, validate
- [ ] Test stale reference detection
- [ ] Benchmark: spawn/destroy at high churn

### 2.5 Component Store

- [ ] Implement schema registration and validation
- [ ] Implement archetype-based storage
- [ ] Implement component set/get/remove
- [ ] Implement archetype iteration
- [ ] Test type validation
- [ ] Benchmark: iteration with component filter

### 2.6 Relationship Store

- [ ] Implement relationship schema registration
- [ ] Implement bidirectional indices
- [ ] Implement link/unlink with cardinality enforcement
- [ ] Implement on_target_delete cascade
- [ ] Test cardinality violations
- [ ] Benchmark: traversal at scale

### 2.7 World

- [ ] Implement immutable World with persistent internals
- [ ] Implement all mutation methods (returning new World)
- [ ] Implement history chain (previous())
- [ ] Implement content_hash for speculation
- [ ] Test O(1) clone
- [ ] Benchmark: fork + small modification

### Example: Entity Lifecycle

```rust
#[test]
fn example_entity_lifecycle() {
    let mut world = World::new(42);

    // Spawn entity with components
    let (world, player) = world.spawn(map! {
        kw!(tag/player) => Value::Bool(true),
        kw!(health) => map! {
            kw!(current) => Value::Int(100),
            kw!(max) => Value::Int(100),
        },
    })?;

    // Modify component
    let world = world.set_field(player, kw!(health), kw!(current), Value::Int(75))?;

    // Query
    assert_eq!(
        world.get_field(player, kw!(health), kw!(current))?,
        Some(Value::Int(75))
    );

    // Destroy
    let world = world.destroy(player)?;
    assert!(!world.entity_exists(player));
}
```

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

```rust
// This should work after Phase 2.5:
let world = World::new(42);
let (world, player) = world.spawn(components! {
    tag/player: true,
    health: { current: 100, max: 100 },
    position: { x: 0.0, y: 0.0 },
})?;

let (world, room) = world.spawn(components! {
    tag/room: true,
    name: "Cave Entrance",
})?;

let world = world.link(player, kw!(in_room), room)?;

// Queries work:
let result = query(&world, r#"
    :where [[?p :tag/player]
            [?p :in-room ?r]
            [?r :name ?name]]
    :return ?name
"#)?;
assert_eq!(result, vec![Value::String("Cave Entrance".into())]);

// Serialization works:
let bytes = save(&world)?;
let restored = restore(&bytes, &schema)?;
```

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

- [ ] All query forms work against static world
- [ ] Aggregation produces correct results
- [ ] Relationships traverse correctly (forward and reverse)
- [ ] Serialization round-trips perfectly
- [ ] REPL can query and inspect
- [ ] Adventure game world can be constructed (no rules yet)

---

## Phase 3: Language Implementation

**Goal**: Implement Layer 2 - complete DSL parsing and bytecode execution.

### 3.1 Lexer

- [x] Implement tokenizer for all spec literals
- [x] Handle comments (`;`, `#_`)
- [x] Handle tagged literals (`#name[...]`)
- [x] Comprehensive span tracking
- [x] Test with spec grammar examples (24 tests)
- [ ] Fuzz test for crash resistance

### 3.2 Parser

- [x] Implement recursive descent parser
- [x] Parse all expression forms
- [ ] Parse all declaration forms (component:, rule:, etc.)
- [x] Rich error messages with span information
- [x] Test with spec examples (29 tests)
- [ ] Fuzz test for crash resistance

### 3.3 AST

- [x] Implement all AST node types
- [ ] Implement visitor pattern for traversal
- [ ] Implement AST pretty-printer
- [ ] Test round-trip: source → AST → pretty-print ≈ source

### 3.4 Compiler

- [ ] Compile expressions to bytecode
- [ ] Compile patterns for rule matching
- [ ] Compile rule bodies
- [ ] Compile queries
- [ ] Compile derived components
- [ ] Compile constraints
- [ ] Macro expansion
- [ ] Module/namespace resolution
- [ ] Test all spec forms compile correctly

### 3.5 Bytecode VM

- [ ] Implement stack-based VM
- [ ] Implement all opcodes
- [ ] Implement function calls
- [ ] Implement effect opcodes
- [ ] Test: expression evaluation
- [ ] Benchmark: 1M ops/sec target

### Example: Expression Evaluation

```rust
#[test]
fn example_expression_eval() {
    let source = "(+ (* 3 4) (- 10 5))";  // Should be 17
    let ast = parse(source)?;
    let bytecode = compile_expr(&ast[0])?;

    let mut vm = Vm::new();
    let result = vm.execute(&bytecode, &mut TestContext::new())?;

    assert_eq!(result, Value::Int(17));
}
```

---

## Phase 4: Execution Engine Implementation

**Goal**: Implement Layer 3 - rules, queries, and constraints.

### 4.1 Pattern Matcher

- [ ] Implement pattern compilation
- [ ] Implement index-based lookup
- [ ] Implement join execution
- [ ] Implement negation
- [ ] Implement variable unification
- [ ] Test with spec pattern examples
- [ ] Benchmark: matching at 10k entities

### 4.2 Rule Engine

- [ ] Implement activation finding
- [ ] Implement refraction
- [ ] Implement conflict resolution (salience, specificity)
- [ ] Implement `:once` flag
- [ ] Implement rule execution loop
- [ ] **Implement semantic kill switches** (see below)
- [ ] Test: quiescence termination
- [ ] Test: deterministic ordering
- [ ] Test: kill switches trigger correctly
- [ ] Benchmark: rules firing per second

#### Semantic Kill Switches (Hard Ceilings)

Bugs look like malicious rules. Without hard ceilings, semantic bugs become hangs—and hangs are morale killers.

**Required limits** (configurable, with sane defaults):

| Limit | Default | What Happens |
|-------|---------|--------------|
| Max activations per tick | 10,000 | Error with context |
| Max effects per tick | 100,000 | Error with context |
| Max refires per rule per tick | 1,000 | Error naming the rule |
| Max derived evaluation depth | 100 | Error (cycle detection) |
| Max query results | 100,000 | Truncate with warning |

These are not performance limits—they're **semantic sanity checks**. A legitimate simulation shouldn't hit them. If it does, something is wrong.

```rust
pub struct TickLimits {
    pub max_activations: usize,
    pub max_effects: usize,
    pub max_refires_per_rule: usize,
    pub max_derived_depth: usize,
    pub max_query_results: usize,
}

impl Default for TickLimits {
    fn default() -> Self {
        Self {
            max_activations: 10_000,
            max_effects: 100_000,
            max_refires_per_rule: 1_000,
            max_derived_depth: 100,
            max_query_results: 100_000,
        }
    }
}
```

Errors should include full context: which rule, what bindings, how many times it fired. Make debugging possible.

#### Future Consideration: Simple/Declarative Mode

Not all rules are reactive. Some are declarative and should not chain within a tick.

This is **not required for MVP**, but keep it in mind:
- A "snapshot rule" mode where rules see start-of-tick state only
- A "no chaining this tick" flag for specific rules
- A restricted subset for beginners

This matters especially for onboarding. A beginner shouldn't need to understand refraction to write their first rule. Consider this when designing examples and tutorials.

### 4.3 Query System

- [ ] Implement query compilation
- [ ] Implement aggregation functions
- [ ] Implement group-by
- [ ] Implement order-by and limit
- [ ] Implement query-one, query-count, query-exists?
- [ ] **Implement entity ordering warnings** (see below)
- [ ] Test with spec query examples
- [ ] Benchmark: query at scale

#### Entity Ordering Is a Footgun

We allow ordered queries and entity iteration, but ordering on `EntityId` is almost always a bug waiting to happen.

**The problem**: Users will rely on allocation order even though it's unspecified. Someone's game logic will silently depend on "player spawned first."

**Mitigation**:
- Emit a **warning** when ordering by entity ID in queries
- Consider requiring explicit opt-in: `:order-by [?e :entity-id]` vs implicit ordering
- Document heavily that entity order is not stable across serialization/deserialization
- In examples, always use explicit ordering criteria (`:order-by [?e :name/value]`)

### 4.4 Derived Components

- [ ] Implement derived compilation
- [ ] Implement dependency tracking
- [ ] Implement lazy evaluation
- [ ] Implement cache invalidation
- [ ] Test: cycle detection
- [ ] Benchmark: cache hit rate

### 4.5 Constraints

- [ ] Implement constraint compilation
- [ ] Implement check evaluation
- [ ] Implement rollback vs warn
- [ ] Test: constraint violation handling
- [ ] Test: constraint ordering

### 4.6 Effects & Provenance (Minimal)

> **Note**: Phase 4 implements *minimal* effect logging—enough for basic `why` queries and error context. Full tracing, debugger integration, and time travel remain in Phase 6.

- [ ] Implement effect recording (what, where, source)
- [ ] Implement basic provenance tracking (last-writer per field)
- [ ] Implement basic `why` query (who set this value)
- [ ] Test: effect log accuracy
- [ ] Do NOT implement: full history, bindings capture, expression IDs (Phase 6)

### 4.7 Tick Orchestration

- [ ] Implement full tick cycle
- [ ] Input injection
- [ ] Rule execution to quiescence
- [ ] Constraint checking
- [ ] Commit or rollback
- [ ] Test: atomicity
- [ ] Benchmark: full tick at scale

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
- [ ] Decision documented: VM mutation model (direct vs intent-based)
- [ ] If direct mutation chosen: document what features this limits
- [ ] If intent-based chosen: implement before Phase 5

### Example: Rule Execution

```rust
#[test]
fn example_damage_rule() {
    let source = r#"
        (component: health :current :int :max :int)
        (component: incoming-damage :amount :int)

        (rule: apply-damage
          :where [[?e :health/current ?hp]
                  [?e :incoming-damage ?dmg]]
          :then [(set! ?e :health/current (- ?hp (:amount ?dmg)))
                 (destroy! (query-one :where [[?d :incoming-damage _]] :return ?d))])
    "#;

    let program = compile(parse(source)?)?;
    let world = World::new(42);

    // Spawn entity with health and damage
    let (world, e) = world.spawn(map! {
        kw!(health) => map! { kw!(current) => 100, kw!(max) => 100 },
    })?;
    let (world, dmg) = world.spawn(map! {
        kw!(incoming_damage) => map! { kw!(amount) => 25 },
    })?;

    // Run tick
    let world = tick(&program, world, vec![])?;

    // Verify damage applied
    assert_eq!(world.get_field(e, kw!(health), kw!(current))?, Some(Value::Int(75)));
    // Verify damage entity destroyed
    assert!(!world.entity_exists(dmg));
}
```

---

## Phase 5: Interface Implementation

**Goal**: Implement Layer 4 - standard library, REPL, serialization.

### 5.1 Standard Library

Implement all spec functions organized by category:

- [ ] Collection functions (map, filter, reduce, etc.)
- [ ] Math functions (arithmetic, trig, vector math)
- [ ] String functions (str/*, format)
- [ ] Predicate functions (nil?, some?, type checks)
- [ ] Test each function with spec examples
- [ ] Document each function

### 5.2 REPL

- [ ] Command parsing
- [ ] Expression evaluation
- [ ] Tick execution
- [ ] History and line editing
- [ ] Special commands (inspect, tick!, save!, load!)
- [ ] Test: interactive scenarios
- [ ] Test: error recovery

### 5.3 CLI

- [ ] File loading and execution
- [ ] REPL mode
- [ ] Batch mode
- [ ] Debug mode flags
- [ ] Test: CLI argument parsing
- [ ] Test: file execution

### 5.4 Serialization

- [ ] Implement save format (MessagePack)
- [ ] Implement world serialization
- [ ] Implement world deserialization
- [ ] Test: round-trip correctness
- [ ] Test: version compatibility checking

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

| Operation | Target | Scale | Purpose |
|-----------|--------|-------|---------|
| Value clone | <100ns | - | Detect disasters |
| Entity spawn | <10μs | - | Detect disasters |
| Full tick | <1s | 1k entities | Usable for development |
| Query | <100ms | 1k entities | Usable for development |

**1.0 Targets** (Phase 8 complete):

| Operation | Target | Scale |
|-----------|--------|-------|
| Value clone | <10ns | - |
| Entity spawn | <1μs | - |
| Component get | <100ns | - |
| World clone | <100ns | - |
| Pattern match | <1ms | 10k entities |
| Rule fire | <10μs | per rule |
| Full tick | <100ms | 10k entities, 100 rules |
| Query | <10ms | 10k entities |

**Post-1.0 Targets** (incremental matching):

| Operation | Target | Scale |
|-----------|--------|-------|
| Full tick | <10ms | 10k entities (steady state) |
| Pattern update | <1ms | per changed entity |

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

| Category | Frequency | Purpose |
|----------|-----------|---------|
| Micro | Every PR | Catch regressions in hot paths |
| Integration | Daily | End-to-end tick performance |
| Scale | Weekly | Behavior at large entity counts |
| Memory | Weekly | Allocation patterns, memory usage |

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

- [ ] REPL can create and query worlds
- [ ] Adventure game world exists (static, no rules)
- [ ] Serialization round-trips work
- [ ] Aggregation and relationships work
- [ ] You can play with data, even without rules

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
