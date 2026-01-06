# Longtable Specification v0.8

## 1. Overview & Goals

### 1.1 What is Longtable?

Longtable is a rule-based simulation engine combining:

- **LISP-like DSL** for defining components, rules, queries, and logic
- **Archetype-based Entity-Component-System (ECS)** for data organization
- **Pattern-matching rule engine** with incremental optimization
- **Persistent/functional data structures** enabling time-travel debugging
- **Tick-based discrete simulation** with transactional semantics

> **Note on RETE**: While Longtable MAY use RETE-like data structures internally for efficient incremental pattern matching, it does NOT support RETE-style rule chaining. Rules activate based on tick-start state only; within-tick effects don't trigger new activations. See Section 5.5 for details.

### 1.2 Design Philosophy

1. **Everything Is An Entity** - Rules, component schemas, relationships, and even the world itself are entities that can be queried and manipulated.

2. **World As Value** - The world state is immutable. Each tick produces a new world. This enables rollback, time-travel debugging, and "what-if" exploration.

3. **Effects, Not Mutations** - Rules produce effects that modify the world within a transaction, committed atomically at tick end.

4. **Explicit Over Implicit** - Strong typing (`nil ≠ false`), explicit optionality, declared component schemas.

5. **Observability From The Start** - Change tracking, rule tracing, and debugging primitives are core features, not afterthoughts.

6. **Internal Consistency** - One unified syntax for pattern matching across rules, queries, derived components, and constraints.

### 1.3 Target Use Cases

- Complex rule-driven simulations
- Exploratory world-building with rich cause-and-effect chains
- Systems where understanding "why" something happened matters
- Text-based simulation games (Zork meets Dwarf Fortress)

### 1.4 Non-Goals

- Real-time performance (60 FPS). Ticks may take seconds.
- Tight Rust integration with typed components. Rust is the runtime; the DSL defines all domain logic.
- Graphical rendering (though the engine can drive one).

---

## 2. Core Concepts

### 2.1 World

The **World** is the complete, immutable state of the simulation at a point in time.

```
World = {
  tick: Int                                 -- Current tick number
  seed: Int                                 -- RNG seed
  entities: Map<EntityId, Entity>           -- All entities
  archetypes: Map<ArchetypeId, Archetype>   -- Component groupings
  indices: Indices                          -- Accelerating structures
  previous: Option<World>                   -- Previous tick (for `prev` access)
}
```

A world is **never mutated**. The `tick` function takes a world and inputs, returning a new world:

```
tick : (World, Vec<Input>) -> Result<World, RollbackError>
```

### 2.2 Transaction Model

Each tick executes as a single transaction against the world:

```
┌─────────────────────────────────────────────────────────────────┐
│                     TICK EXECUTION                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   1. BEGIN TRANSACTION                                          │
│      • Snapshot current world as "previous" (for `prev`)        │
│      • Begin mutable transaction                                │
│                                                                 │
│   2. RULE EXECUTION                                             │
│      • Rules fire until quiescence (see Section 5)              │
│      • All reads see current transaction state                  │
│      • All writes modify current transaction state              │
│                                                                 │
│   3. CONSTRAINT CHECK                                           │
│      • Evaluate constraints against current state               │
│      • :rollback violations → abort transaction                 │
│      • :warn violations → log and continue                      │
│                                                                 │
│   4. COMMIT or ROLLBACK                                         │
│      • On success: transaction becomes new world, tick++        │
│      • On error/violation: discard transaction, world unchanged │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Read/write semantics are uniform:**

| Operation            | What it sees                       |
| -------------------- | ---------------------------------- |
| `get`                | Current transaction state          |
| `query`              | Current transaction state          |
| `prev`               | Previous tick's committed world    |
| `set!`, `update!`    | Modifies current transaction state |
| `spawn!`, `destroy!` | Modifies current transaction state |

There is no distinction between "activation phase" and "execution phase." Rules see the world as it currently is, including changes made by earlier rules in the same tick.

```clojure
;; Simple, uniform semantics:
(rule: example
  :where [[?e :health ?hp]]
  :then [(set! ?e :health 50)
         (print! (get ?e :health))])  ;; Prints 50
```

### 2.3 Entity

An **Entity** is a unique identity that can have components attached. Entities have no inherent data—they are pure identity.

```
EntityId = (index: u64, generation: u32)  -- Generational index
```

The generation counter detects stale references. If entity 42 is destroyed and a new entity reuses index 42, it gets generation+1.

Entities are created with `spawn!` and destroyed with `destroy!`:

```clojure
(let [e (spawn! {:health {:current 100 :max 100}
                 :position {:x 0.0 :y 0.0}})]
  (print! (str "Created entity " e)))

(destroy! some-entity)
```

**Stale reference behavior**: Accessing a destroyed entity (or one from a different generation) is a **runtime error**:

```clojure
(let [enemy (spawn! {:tag/enemy true})]
  (destroy! enemy)
  (get enemy :tag/enemy))  ;; ERROR: stale entity reference

;; Use get? for safe access (returns nil if stale or missing)
(get? enemy :tag/enemy)    ;; => nil
(entity-exists? enemy)     ;; => false
```

This strict behavior catches bugs where entity references outlive their targets. Use `get?` and `entity-exists?` when references may be stale.

### 2.4 Component

A **Component** is a named, typed data structure attached to an entity. Components are declared with schemas:

```clojure
(component: health
  :current :int
  :max :int :default 100
  :regen-rate :float :default 0.0)

(component: position
  :x :float
  :y :float
  :z :float :default 0.0)

;; Boolean "tag" components (single-field shorthand)
(component: tag/player :bool :default true)
(component: tag/enemy :bool :default true)
```

Components are accessed and modified through entities:

```clojure
(get entity :health/current)           ;; Read field
(get entity :health)                   ;; Read whole component
(set! entity :health/current 50)       ;; Set field
(set! entity :health {:current 50 :max 100})  ;; Set whole component
(update! entity :health/current inc)   ;; Apply function to field
```

### 2.5 Archetype

An **Archetype** is the set of component types an entity has. Archetypes emerge implicitly from entity composition—they are not declared.

```
Entity A has: [Position, Velocity, Health]  -> Archetype 1
Entity B has: [Position, Velocity, Health]  -> Archetype 1 (same)
Entity C has: [Position, Health]            -> Archetype 2 (different)
```

When querying `[?e :position _] [?e :velocity _]`:
- Entities A and B match (both have Position AND Velocity)
- Entity C does not match (has Position but no Velocity)

The engine can skip entire archetypes that lack required components, making queries efficient.

**Implementation Note**: The engine may use Structure-of-Arrays (SoA) layout for cache efficiency, but this is an implementation detail not guaranteed by the spec.

### 2.6 Relationship

A **Relationship** is a typed, directional connection between entities. Each relationship declaration specifies **one canonical storage strategy**:

```clojure
;; Field-based relationship (lightweight, stored as component)
(relationship: follows
  :storage :field
  :cardinality :many-to-many
  :on-target-delete :remove)

;; Entity-based relationship (heavyweight, can have attributes)
(relationship: employment
  :storage :entity
  :cardinality :many-to-one
  :on-target-delete :cascade
  :attributes [:start-date :int
               :salary :int
               :title :string])
```

**Storage strategies:**

| Strategy  | Use When                          | Example                                   |
| --------- | --------------------------------- | ----------------------------------------- |
| `:field`  | Simple connections, no extra data | `:follows`, `:parent`, `:in-room`         |
| `:entity` | Relationships need attributes     | `:employment`, `:friendship-with-history` |

**Cardinality as Outgoing/Incoming Limits:**

| Cardinality     | Max Outgoing (source→) | Max Incoming (→target) | `:field` Storage   |
| --------------- | ---------------------- | ---------------------- | ------------------ |
| `:one-to-one`   | 1                      | 1                      | Single `EntityRef` |
| `:one-to-many`  | many                   | 1                      | `Vec<EntityRef>`   |
| `:many-to-one`  | 1                      | many                   | Single `EntityRef` |
| `:many-to-many` | many                   | many                   | `Vec<EntityRef>`   |

**Cardinality enforcement:**

| Violation            | Default Behavior | With `:on-violation :replace` |
| -------------------- | ---------------- | ----------------------------- |
| Exceeds max outgoing | Error            | Unlink old, link new          |
| Exceeds max incoming | Error            | Unlink old, link new          |

```clojure
;; Default: error if player already in a room
(relationship: in-room
  :cardinality :many-to-one)

;; Replace mode: automatically move player to new room
(relationship: in-room
  :cardinality :many-to-one
  :on-violation :replace)
```

**On-target-delete behaviors:**
- `:remove` - Remove the relationship when target is destroyed
- `:cascade` - Destroy the source entity when target is destroyed
- `:nullify` - Set to `none` (only valid if `:required false`)

Relationships are manipulated with `link!` and `unlink!`:

```clojure
(link! alice :follows bob)
(unlink! alice :follows bob)
```

**Link/unlink semantics:**

| Operation                     | Behavior                                       |
| ----------------------------- | ---------------------------------------------- |
| `link!` existing edge         | No-op (idempotent, no duplicate)               |
| `link!` violating cardinality | Error (or replace if `:on-violation :replace`) |
| `unlink!` missing edge        | No-op (no error)                               |

**Ordering**: For `:one-to-many` and `:many-to-many` relationships using `:field` storage, the `Vec<EntityRef>` maintains stable insertion order. This order is deterministic but not semantically meaningful—do not rely on it for game logic.

The system maintains bidirectional indices automatically for O(1) traversal in both directions.

### 2.7 Rule

A **Rule** is a reactive unit of logic with a unified query-like syntax:

```clojure
(rule: apply-damage
  :salience   50
  :where      [[?target :health/current ?hp]
               [?target :incoming-damage ?dmg]]
  :guard      [(> ?dmg 0)]
  :then       [(set! ?target :health/current (- ?hp ?dmg))
               (destroy! ?dmg-source)])
```

Rules are themselves entities with components, meaning they can be queried, enabled/disabled, and even created by other rules (changes take effect next tick).

### 2.8 Derived Component

A **Derived Component** is a computed value that behaves like a component but is calculated from other data:

```clojure
(derived: health/percent
  :for   ?self
  :where [[?self :health/current ?curr]
          [?self :health/max ?max]]
  :value (/ (* ?curr 100) ?max))

;; With aggregation across entities
(derived: faction/total-power
  :for       ?faction
  :where     [[?faction :tag/faction]
              [?member :faction ?faction]
              [?member :power ?p]]
  :aggregate {:total (sum ?p)}
  :value     ?total)
```

Derived components:
- Are read-only (cannot be `set!`)
- Invalidate when dependencies change
- Can be used in rule patterns and queries
- Track dependencies automatically

**Dependency tracking and invalidation**:

Derived values track dependencies at a coarse granularity:
- **Field dependencies**: Any component/field read during evaluation
- **Query membership**: The set of entities matching `:where` patterns
- **Other derived values**: Transitive dependencies through derived→derived references

A derived cache invalidates when:
- Any tracked field changes on any tracked entity
- An entity enters or leaves a tracked query's match set
- Any upstream derived value invalidates

**Evaluation**: Derived values always see the current world state. They are recomputed when their dependencies change (or served from cache if dependencies are unchanged).

**Cycle Detection**:
- **Static**: The compiler detects cycles in explicit derived→derived references (derived A references derived B by name)
- **Runtime**: A guard stack detects evaluation recursion within a tick (guaranteed to catch all cycles)
- **Across ticks**: Feedback loops via regular components and rules are allowed and expected

### 2.9 Constraint

A **Constraint** is an invariant checked after rule execution:

```clojure
(constraint: health-bounds
  :where        [[?e :health/current ?hp]
                 [?e :health/max ?max]]
  :check        [(>= ?hp 0) (<= ?hp ?max)]
  :on-violation :rollback)
```

**Violation behaviors:**
- `:rollback` - Entire tick fails, world unchanged (default)
- `:warn` - Log warning, allow violation

**No automatic clamping**: Constraints detect violations but don't fix them. If you need boundary enforcement, write an explicit rule:

```clojure
;; Instead of :clamp, use explicit rules for clarity
(rule: clamp-health
  :salience -100                          ;; Run after combat rules
  :where [[?e :health/current ?hp]
          [?e :health/max ?max]]
  :then [(when (< ?hp 0)
           (set! ?e :health/current 0))
         (when (> ?hp ?max)
           (set! ?e :health/current ?max))])
```

**Why no `:clamp`:**
- Explicit rules are clearer and more debuggable
- Rule effects appear in normal traces with provenance
- No hidden complexity around constraint ordering or iteration
- Users control exactly when and how clamping happens via salience

**Constraint ordering**: When multiple constraints trigger violations, they are checked in order by:
1. **Salience** (higher first, default 0)
2. **Declaration order** (earlier first)

### 2.10 Meta-Entities (Everything Is An Entity)

Longtable commits to the principle that **engine concepts are queryable entities**. This enables powerful introspection and meta-programming.

**Schema entities**: Each `component:` or `relationship:` declaration creates an entity:

```clojure
;; Query all component schemas
(query
  :where [[?schema :meta/type :component]
          [?schema :meta/name ?name]]
  :return ?name)
;; => [:health, :position, :tag/player, ...]

;; Inspect a schema's fields
(query
  :where [[?schema :meta/name :health]
          [?schema :meta/fields ?fields]]
  :return ?fields)
;; => [{:name :current, :type :int, :default nil}
;;     {:name :max, :type :int, :default 100}
;;     {:name :regen-rate, :type :float, :default 0.0}]
```

**Rule entities**: Each `rule:` declaration creates an entity (as already mentioned in Section 2.7):

```clojure
;; Find all rules matching a pattern
(query
  :where [[?rule :meta/type :rule]
          [?rule :meta/patterns ?patterns]]
  :guard [(some #(mentions? % :health) ?patterns)]
  :return ?rule)

;; Disable a rule dynamically
(disable-rule! (query-one :where [[?r :meta/name :apply-damage]] :return ?r))
```

**Constraint and derived entities**: Similarly queryable:

```clojure
(query :where [[?c :meta/type :constraint]] :return ?c)
(query :where [[?d :meta/type :derived]] :return ?d)
```

**Meta-entity components** (read-only, in `:meta/*` namespace):

| Component               | Description                                                       |
| ----------------------- | ----------------------------------------------------------------- |
| `:meta/type`            | `:component`, `:relationship`, `:rule`, `:constraint`, `:derived` |
| `:meta/name`            | The declared name (keyword)                                       |
| `:meta/namespace`       | Defining namespace                                                |
| `:meta/source-location` | File and line number                                              |
| `:meta/enabled`         | For rules: currently enabled?                                     |
| `:meta/salience`        | For rules/constraints: priority                                   |
| `:meta/patterns`        | For rules: the `:where` patterns                                  |
| `:meta/fields`          | For components: field definitions                                 |
| `:meta/cardinality`     | For relationships: cardinality info                               |

This makes Longtable self-describing and enables tooling like schema browsers, rule visualizers, and dynamic rule management.

**Meta-entity phase boundary**: Changes to meta-entities take effect at **tick boundaries**, not during tick execution:

| Operation                         | Effect Timing                                 |
| --------------------------------- | --------------------------------------------- |
| `(disable-rule! r)`               | Rule excluded from **next** tick's activation |
| `(enable-rule! r)`                | Rule included in **next** tick's activation   |
| `(set! rule :meta/salience n)`    | New salience used **next** tick               |
| `(spawn! {:meta/type :rule ...})` | New rule activates **next** tick              |

This ensures the activation set is stable throughout a tick. A rule cannot disable itself mid-tick to prevent its own effects—once activated, it runs to completion.

---

## 3. Type System

### 3.1 Primitive Types

| Type          | Description                | Literal Examples       |
| ------------- | -------------------------- | ---------------------- |
| `:nil`        | The absence of a value     | `nil`                  |
| `:bool`       | Boolean                    | `true`, `false`        |
| `:int`        | 64-bit signed integer      | `42`, `-17`, `0`       |
| `:float`      | 64-bit IEEE float          | `3.14`, `-0.5`, `1.0`  |
| `:string`     | UTF-8 string               | `"hello"`, `"world\n"` |
| `:symbol`     | Interned identifier        | `'foo`, `'bar/baz`     |
| `:keyword`    | Self-evaluating identifier | `:foo`, `:bar/baz`     |
| `:entity-ref` | Reference to an entity     | `#entity[3.42]`        |

**Important**: `nil ≠ false`. They are distinct values of distinct types.

**NaN Debug Mode**: Float operations can produce NaN, which propagates silently. For debugging, enable NaN detection:

```clojure
(set-option! :fail-on-nan true)  ;; Any operation producing NaN throws an error
```

When enabled, operations like `(/ 0.0 0.0)` or `(sqrt -1.0)` will throw an error with a stack trace, making it easier to find the source of NaN propagation.

### 3.2 Composite Types

| Type        | Description                 | Literal Examples       |
| ----------- | --------------------------- | ---------------------- |
| `:vec<T>`   | Ordered sequence            | `[1 2 3]`, `["a" "b"]` |
| `:set<T>`   | Unordered unique collection | `#{1 2 3}`             |
| `:map<K,V>` | Key-value mapping           | `{:a 1 :b 2}`          |

### 3.3 Nullability

Longtable uses `nil` directly for absent values—there is no wrapped Option type at runtime.

**Schema annotation**: `:option<T>` indicates a field may contain `nil`:

```clojure
(component: example
  :required-int :int                      ;; Must have a value, nil is error
  :optional-int :option<:int>             ;; Can be nil
  :defaulted-int :int :default 0          ;; Has default, never nil
  :computed-default :int :default (current-tick)) ;; Default from expression
```

**Runtime behavior**: Values are either the value or `nil`. No unwrapping needed:

```clojure
;; Check for presence
(nil? x)       ;; true if x is nil
(some? x)      ;; true if x is not nil

;; Idiomatic patterns
(when-let [hp (get ?e :health/current)]
  (use hp))

(if (some? target)
  (attack target)
  (wander))

;; Default values
(or (get ?e :nickname) (get ?e :name))
```

**Schema enforcement**: Assigning `nil` to a non-optional field is a load-time or runtime error depending on context.

### 3.4 Type Checking

Longtable has a **hybrid type system**:

**Schema-typed (load-time checking):**
- Component field types are declared and enforced
- Pattern matching is type-checked against component schemas
- Relationship cardinality and storage are validated

**Dynamically typed (runtime):**
- General expression evaluation
- Collection operations
- Function arguments (unless annotated)

```clojure
;; Optional type annotations for functions
(fn: add-ints :- :int [a :- :int, b :- :int]
  (+ a b))

;; This compiles but fails at runtime:
(map inc ["a" "b" "c"])  ;; Runtime error: inc expects number
```

**Practical implication**: Component access and pattern matching catch type errors at load time. General expressions may produce runtime type errors. Use optional annotations for critical functions.

### 3.5 Equality and Hashing

For collections, indices, and deduplication, equality and hashing follow these rules:

**Primitives:**
- `nil`, `true`, `false` - identity equality
- `:int` - numeric equality
- `:float` - IEEE equality with exceptions:
  - `NaN ≠ NaN` (IEEE behavior)
  - `-0.0 = +0.0`
  - For hashing: all NaN values hash identically
- `:string` - byte-wise equality
- `:symbol`, `:keyword` - interned identity comparison (fast)
- `:entity-ref` - equality by `(index, generation)` pair

**Composites:**
- `:vec`, `:set`, `:map` - deep structural equality
- Order matters for `:vec`, not for `:set` or `:map`

**Hashing:**
- Must be consistent with equality
- Floats: hash based on bit representation (NaN hashes to fixed value)
- Collections: combine element hashes

### 3.6 Determinism

Longtable guarantees **RNG determinism**: given the same world seed and inputs, random operations produce identical results.

```clojure
;; Deterministic: same seed → same random sequence
(random)        ;; Seeded by world + tick + rule + entity
(random-int n)  ;; Same
```

**Collection iteration order is NOT guaranteed** across runs, platforms, or implementations:

| Collection | Iteration Order                      |
| ---------- | ------------------------------------ |
| `:vec`     | Index order (0, 1, 2, ...) — stable  |
| `:set`     | Unspecified (implementation-defined) |
| `:map`     | Unspecified (implementation-defined) |

**Practical implications:**

```clojure
;; If order matters, sort explicitly:
(query
  :where [[?e :tag/enemy]]
  :order-by [[?e :asc]]          ;; Explicit ordering
  :return (collect ?e))

;; Without order-by, result order is implementation-defined
(query
  :where [[?e :tag/enemy]]
  :return (collect ?e))           ;; Order may vary between runs
```

**Why this simplification:**
- Sorted iteration has significant runtime cost
- Most game logic doesn't depend on iteration order
- When order matters, explicit `:order-by` makes intent clear
- RNG determinism (what players actually care about) is preserved

---

## 4. DSL Specification

### 4.1 Lexical Structure

#### Comments

```clojure
;; Line comment (preferred)
; Also a line comment

#_ (ignored form)   ;; Ignore next form (useful for debugging)
```

#### Literals

```clojure
;; Primitives
42                  ;; int
3.14                ;; float
true false          ;; bool
nil                 ;; nil
"string"            ;; string
:keyword            ;; keyword
:namespaced/keyword ;; namespaced keyword
'symbol             ;; quoted symbol
symbol              ;; symbol (evaluated as variable reference)

;; Collections
[1 2 3]             ;; vector
#{1 2 3}            ;; set
{:a 1 :b 2}         ;; map
'(1 2 3)            ;; quoted list

;; Special literals
#entity[3.42]       ;; entity reference (generation.index)
```

#### Tagged Literals

Tagged literals provide custom syntax that expands at read time:

```clojure
;; Built-in
#entity[3.42]       ;; Entity reference (generation.index)

;; User-defined (must be declared before use)
#pos[10 20]         ;; Custom position literal
#rgb[255 128 0]     ;; Custom color literal
```

### 4.2 Tagged Literal Definitions

Define custom tagged literals with `literal:`:

```clojure
(literal: pos [x :- :int, y :- :int]
  {:x x :y y})

(literal: rgb [r :- :int, g :- :int, b :- :int]
  {:r r :g g :b b :a 255})

(literal: dice [spec :- :string]
  (parse-dice-notation spec))

;; Usage (after definition):
#pos[10 20]         ;; => {:x 10 :y 20}
#rgb[255 128 0]     ;; => {:r 255 :g 128 :b 0 :a 255}
#dice"2d6+3"        ;; => {:count 2 :sides 6 :modifier 3}
```

Tagged literals are expanded at **read time** (before compilation), so they must be defined in a file that loads before first use.

### 4.3 Special Forms

Special forms are built into the compiler and cannot be implemented as functions.

#### Binding & Definition

```clojure
(def name value)                    ;; Bind value to name
(def name :- type value)            ;; With type annotation

(fn: name [args...] body...)        ;; Define function
(fn: name :- ret-type [args...] body...)
(fn: ^:private helper [x] ...)      ;; Private function

(let [name value ...] body...)      ;; Local bindings
(let [{:keys [a b]} map] ...)       ;; Map destructuring
(let [[x & rest] vec] ...)          ;; Sequence destructuring
```

#### Control Flow

```clojure
(if condition then-expr else-expr)
(do expr1 expr2 ... exprN)          ;; Sequence, returns last

(match value
  pattern1 result1
  pattern2 result2
  _ default-result)                 ;; Pattern matching

(loop [name init-val ...] body...)  ;; Loop with bindings
(recur new-val ...)                 ;; Tail-recursive jump to loop

(try expr
  (fn: [result] success-body)       ;; Called on success
  (fn: [error] error-body))         ;; Called on error
```

#### Quoting

```clojure
(quote x)           ;; Return x unevaluated
'x                  ;; Shorthand for (quote x)
`(a ~b ~@c)         ;; Syntax-quote with unquote/unquote-splicing
```

### 4.4 Pattern Matching (`match`)

The `match` form provides structural pattern matching on values:

```clojure
(match value
  ;; Literal patterns
  nil "was nil"
  42 "was forty-two"
  :keyword "was keyword"

  ;; Variable binding
  ?x (str "bound to " ?x)

  ;; Wildcard
  _ "matched anything"

  ;; Vector patterns
  [] "empty vector"
  [?head & ?tail] (str "head: " ?head)
  [?a ?b ?c] (str "three elements")

  ;; Map patterns
  {:type :point :x ?x :y ?y} (str "point at " ?x "," ?y)
  {:keys [name age]} (str name " is " age)

  ;; With guard
  (?n :when (> ?n 0)) "positive number"

  ;; Or pattern
  (:or :yes :true 1) "truthy value")
```

**Pattern semantics:**
- First matching pattern wins
- A binding name appearing multiple times must unify (equal values)
- Map patterns ignore unspecified keys (open matching)
- Guards (`:when`) evaluate after structural match

### 4.5 Declaration Forms (Unified Syntax)

All declaration forms share a common pattern-matching syntax with `:where` clauses.

#### World Metadata

```clojure
(world:
  :seed 12345
  :name "My Simulation")
```

#### Component Schema

```clojure
(component: name
  :field1 :type1
  :field2 :type2 :default value
  ...)

;; Single-field shorthand for tags
(component: tag/player :bool :default true)
```

#### Relationship Declaration

```clojure
(relationship: name
  :storage :field|:entity
  :cardinality :one-to-one|:one-to-many|:many-to-one|:many-to-many
  :on-target-delete :remove|:cascade|:nullify
  :required true|false
  :attributes [...])  ;; Only for :entity storage
```

#### Rule Declaration

```clojure
(rule: name
  ;; Metadata
  :salience   number              ;; Priority, default 0
  :enabled    true|false          ;; Default true
  :once       true|false          ;; Fire at most once per tick, default false

  ;; Query pipeline
  :where      [[?e :component ?val] ...]
  :let        [computed (expr) ...]
  :aggregate  {:name (agg-fn ?var) ...}
  :group-by   [?var ...]
  :guard      [(condition) ...]
  :order-by   [[?var :asc|:desc] ...]
  :limit      n

  ;; Effects
  :then       [(effect!) ...])
```

#### Query

```clojure
(query
  :where      [[?e :component ?val] ...]
  :let        [computed (expr) ...]
  :aggregate  {:name (agg-fn ?var) ...}
  :group-by   [?var ...]
  :guard      [(condition) ...]
  :order-by   [[?var :asc|:desc] ...]
  :limit      n
  :return     expr)
```

#### Derived Component

```clojure
(derived: name
  :for        ?self              ;; Entity this is computed for
  :where      [[?self :dep ?val] ...]
  :let        [...]
  :aggregate  {...}
  :value      expr)
```

#### Constraint

```clojure
(constraint: name
  :where        [[?e :component ?val] ...]
  :let          [...]
  :aggregate    {...}
  :guard        [...]
  :check        [(invariant) ...]
  :on-violation :rollback|:warn)
```

### 4.6 Query Clause Reference

All query-like forms (rules, queries, derived, constraints) support these clauses:

| Clause       | Purpose                             | Available In            |
| ------------ | ----------------------------------- | ----------------------- |
| `:where`     | Pattern matching                    | All                     |
| `:let`       | Per-match computed values           | All                     |
| `:aggregate` | Aggregate functions                 | All                     |
| `:group-by`  | Partition results                   | Rule, Query, Constraint |
| `:guard`     | Filter on computed/aggregate values | All                     |
| `:order-by`  | Sort results                        | Rule, Query             |
| `:limit`     | Cap result count                    | Rule, Query             |
| `:for`       | Entity being computed               | Derived only            |
| `:return`    | Output shape                        | Query only              |
| `:then`      | Effects to execute                  | Rule only               |
| `:value`     | Computed value                      | Derived only            |
| `:check`     | Invariant conditions                | Constraint only         |

**Execution order:**

```
:where      → Find all pattern matches against current world
:let        → Compute per-match values
:aggregate  → Compute aggregates (creates new bindings)
:group-by   → Partition by grouping variables
:guard      → Filter based on aggregates/computed values
:order-by   → Sort within groups (or overall)
:limit      → Cap count per group (or overall)
:return/:then/:value/:check → Terminal action
```

### 4.7 Aggregate Functions

Available in `:aggregate` clauses:

```clojure
:aggregate {
  :cnt      (count ?var)          ;; Count of matches
  :total    (sum ?var)            ;; Sum of values
  :minimum  (min ?var)            ;; Minimum value
  :maximum  (max ?var)            ;; Maximum value
  :average  (avg ?var)            ;; Average value
  :smallest (min-by ?sort ?ret)   ;; Entity with minimum sort value
  :largest  (max-by ?sort ?ret)   ;; Entity with maximum sort value
  :all      (collect ?var)        ;; Vector of all values
  :unique   (collect-set ?var)    ;; Set of unique values
}
```

Aggregates ignore `nil` values by default. To error on `nil`, use explicit guards.

### 4.8 Module System

```clojure
;; Declare namespace at top of file
(namespace game.combat
  (:require [game.core :as core]
            [game.utils :refer [distance clamp]]
            [game.items]))

;; Load another file
(load "path/to/file.lt")

;; Load from directory (loads _.lt as entry point)
(load "path/to/directory")
```

**Compilation pipeline**:

```
1. READ PHASE
   • Parse source text into forms
   • Expand tagged literals (require pre-declared readers)
   • Process (load ...) directives recursively

2. MACRO EXPANSION PHASE
   • Expand user macros
   • Expand standard macros (when, cond, ->, etc.)

3. DECLARATION COMPILATION
   • Compile component schemas
   • Compile relationship declarations
   • Compile derived component definitions
   • Compile constraint definitions
   • Compile rule definitions

4. EXPRESSION COMPILATION
   • Compile function bodies to bytecode
   • Compile rule :then bodies to bytecode
   • Compile derived :value expressions to bytecode

5. INSTALLATION
   • Register schemas with world
   • Create rule/constraint entities
   • Populate indices
```

**Load order**: Files are loaded in declaration order. Cyclic loads (A loads B loads A) are a **compile-time error**. All dependencies must form a DAG.

**Tagged literal constraint**: Tagged literals (like `#pos[10 20]`) must be defined before first use. This may require careful file ordering or putting literals in a shared prelude.

### 4.9 Macros

User-defined macros expand at compile time:

```clojure
(defmacro name [args...] body...)
```

**Hygiene model** (Clojure-style):

1. **Locals introduced by macros are hygienic** - automatically renamed to avoid capture
2. **Syntax-quote (`) qualifies symbols** to the macro's namespace
3. **Unquote (~) and unquote-splicing (~@)** splice caller expressions
4. **Gensym (#)** creates unique symbols: `x#`

```clojure
(defmacro when [pred & body]
  `(if ~pred (do ~@body) nil))

(defmacro with-retry [n & body]
  (let [attempts# (gensym "attempts")]
    `(loop [~attempts# ~n]
       (try (do ~@body)
         (fn: [result] result)
         (fn: [error]
           (if (> ~attempts# 1)
             (recur (dec ~attempts#))
             (throw error)))))))
```

### 4.10 Standard Macros

Shipped with stdlib:

```clojure
;; Conditionals
(when condition body...)
(when-not condition body...)
(when-let [name expr] body...)
(if-let [name expr] then else)
(cond clause1 result1 clause2 result2 ... :else default)
(condp pred expr clause1 result1 ...)
(case expr val1 result1 val2 result2 ... default)

;; Boolean (short-circuiting)
(and a b c ...)
(or a b c ...)
(not x)

;; Threading
(-> x (f a) (g b) (h))
(->> x (f a) (g b) (h))
(as-> x $ (f $ a) (g b $))
(some-> x f g h)
(some->> x f g h)
(cond-> x c1 f1 c2 f2)

;; Iteration
(for [x coll, y other :when (pred x y)] body)
(doseq [x coll] side-effect-body)
```

### 4.11 Standard Library Functions

#### Collections

```clojure
;; Basics
(count coll) (empty? coll)
(first coll) (rest coll) (last coll)
(nth coll index) (nth coll index default)
(get coll key) (get coll key default)
(contains? coll key)
(conj coll item) (cons item coll)
(assoc map key val) (dissoc map key)
(merge map1 map2)

;; Transformations
(map f coll) (filter pred coll) (remove pred coll)
(reduce f init coll) (reduce f coll)
(fold f init coll)

;; Advanced
(take n coll) (drop n coll)
(take-while pred coll) (drop-while pred coll)
(partition n coll) (partition-by f coll)
(group-by f coll)
(sort coll) (sort-by f coll)
(reverse coll) (flatten coll)
(distinct coll) (dedupe coll)
(interleave coll1 coll2) (interpose sep coll)
(zip coll1 coll2) (zip-with f coll1 coll2)
(concat coll1 coll2 ...)

;; Predicates
(every? pred coll) (some pred coll)
(not-any? pred coll) (not-every? pred coll)

;; Construction
(range end) (range start end) (range start end step)
(repeat n x) (repeatedly n f)
(vec coll) (set coll) (into to from)
```

#### Math

```clojure
;; Arithmetic
(+ a b ...) (- a b ...) (* a b ...) (/ a b ...)
(mod a b) (rem a b)
(inc x) (dec x) (abs x) (neg x)

;; Comparison
(< a b ...) (<= a b ...) (> a b ...) (>= a b ...)
(= a b ...) (!= a b)
(min a b ...) (max a b ...) (clamp x low high)

;; Rounding
(floor x) (ceil x) (round x) (trunc x)

;; Powers & Roots
(pow base exp) (sqrt x) (cbrt x)
(exp x) (log x) (log10 x) (log2 x)

;; Trigonometry
(sin x) (cos x) (tan x)
(asin x) (acos x) (atan x) (atan2 y x)
(sinh x) (cosh x) (tanh x)

;; Constants
pi e

;; Vector Math
(vec+ v1 v2) (vec- v1 v2)
(vec* v scalar) (vec-scale v scalar)
(vec-dot v1 v2) (vec-cross v1 v2)
(vec-length v) (vec-length-sq v)
(vec-normalize v) (vec-distance v1 v2)
(vec-lerp v1 v2 t) (vec-angle v1 v2)
```

#### Strings

```clojure
(str a b c ...)
(str/length s) (str/substring s start end)
(str/split s delimiter) (str/join delimiter coll)
(str/trim s) (str/trim-left s) (str/trim-right s)
(str/lower s) (str/upper s)
(str/starts-with? s prefix) (str/ends-with? s suffix)
(str/contains? s substring)
(str/replace s old new) (str/replace-all s old new)
(str/blank? s)
(format "template {} {}" arg1 arg2)
```

#### Predicates

```clojure
(nil? x) (some? x)
(bool? x) (int? x) (float? x) (number? x)
(string? x) (keyword? x) (symbol? x)
(vec? x) (set? x) (map? x) (coll? x)
(fn? x) (entity? x)
```

---

## 5. Rule Engine

### 5.0 Rule Execution Semantics

Longtable uses a **forward-chaining rule engine** inspired by CLIPS/OPS5. Rules react to the current world state, and changes are visible immediately to subsequent rules within the same tick.

#### 5.0.1 The Basic Cycle

The rule engine runs a recognize-act cycle:

1. **Match**: Find all rules whose `:where` patterns match the current world state
2. **Select**: Choose the highest-priority matching rule that hasn't been refracted
3. **Fire**: Execute the rule's `:then` block; effects apply immediately
4. **Repeat**: Go to step 1 until no rules can fire

This continues until **quiescence**—the state where no un-refracted rules match.

#### 5.0.2 Refraction

**Refraction** prevents infinite loops by ensuring a rule cannot fire twice on the same binding tuple within a single tick.

A **binding tuple** is the specific set of variable bindings that satisfied a rule's `:where` clause. For example:

```clojure
(rule: greet
  :where [[?person :name ?n]]
  :then [(print! (str "Hello, " ?n))])
```

If the world contains `{Alice :name "Alice"}` and `{Bob :name "Bob"}`:
- First firing: `{?person: Alice, ?n: "Alice"}` → prints "Hello, Alice"
- Second firing: `{?person: Bob, ?n: "Bob"}` → prints "Hello, Bob"
- No more firings: both tuples are now refracted

Even if the rule's `:then` block modifies Alice or Bob, the rule will not re-fire on them this tick because those binding tuples have already fired.

**Refraction resets each tick.** A rule that fired on Alice this tick can fire on Alice again next tick (if she still matches).

#### 5.0.3 The `:once` Flag

For rules that should fire at most once per tick globally (regardless of how many binding tuples match), use `:once true`:

```clojure
(rule: bootstrap
  :once true
  :where [(not [_ :world/initialized])]
  :then [(spawn! {:world/initialized true})
         (print! "World initialized!")])

(rule: daily-report
  :once true
  :where [[?e :tag/employee]]
  :aggregate {:count (count ?e)}
  :then [(print! (format "Employee count: {}" ?count))])
```

With `:once true`:
- The rule fires on the first matching binding tuple
- After firing once, it is excluded from the activation set for the remainder of the tick
- Like refraction, this resets each tick

This is useful for:
- Bootstrap/initialization rules that should run exactly once
- Aggregate reports that summarize across all matches
- Global state transitions that shouldn't repeat

#### 5.0.4 Why Refraction Works

Refraction prevents the most common infinite loops:

```clojure
;; WITHOUT refraction, this would loop forever:
(rule: increment-forever
  :where [[?e :counter ?n]]
  :then [(set! ?e :counter (+ ?n 1))])

;; WITH refraction:
;; - Fires once: {?e: E, ?n: 5} → sets counter to 6
;; - Won't fire again on E this tick, even though E still has :counter
```

Refraction tracks the binding tuple at match time, not the resulting values. The rule fired on "entity E with counter," so it won't fire on "entity E with counter" again—regardless of what the counter's value becomes.

#### 5.0.5 Cascading Effects

Because changes are visible immediately, rules naturally chain:

```clojure
(rule: apply-damage
  :salience 100
  :where [[?e :incoming-damage ?dmg]
          [?e :health ?hp]]
  :then [(set! ?e :health (- ?hp ?dmg))
         (destroy! ?dmg)])

(rule: check-death
  :salience 50
  :where [[?e :health ?hp]]
  :guard [(<= ?hp 0)]
  :then [(spawn! {:event/death :entity ?e})])

(rule: on-death-drop-loot
  :salience 40
  :where [[?event :event/death ?corpse]
          [?corpse :inventory ?items]]
  :then [(doseq [item ?items]
           (move-to-room! item (get ?corpse :location)))
         (destroy! ?event)])
```

When damage reduces health below zero:
1. `apply-damage` fires, sets health to -5
2. `check-death` now matches (health ≤ 0), fires, spawns death event
3. `on-death-drop-loot` now matches (death event exists), fires, drops items

All within one tick. No multi-tick workarounds needed.

#### 5.0.6 Conflict Resolution

When multiple rules can fire, they are ordered by:

1. **Salience** (higher fires first, default 0)
2. **Specificity** (more constraints = more specific, fires first)
3. **Declaration order** (earlier in source fires first)

**Specificity calculation:**
- Each `:where` pattern clause: +1
- Each `:guard` condition: +1
- Each negation (`not`): +1
- `:let` bindings don't count (computed values, not constraints)

The highest-priority rule fires, then the engine re-evaluates what rules now match (since the world may have changed).

#### 5.0.7 Determinism

Given the same world state and rule definitions, the rule engine produces identical results:

- **Rule ordering** is deterministic (salience → specificity → declaration order)
- **Refraction** is deterministic (same matches → same refraction set)
- **RNG** is seeded deterministically (same seed → same random sequence)

What is NOT guaranteed:
- Bit-exact replay across platforms (floating point, hash iteration)
- Performance characteristics

### 5.1 Pattern Syntax

Patterns match entities and bind variables:

```clojure
;; Basic: entity has component with value bound to variable
[?e :health/current ?hp]

;; Specific value required
[?e :tag/player true]

;; Wildcard (match any value, don't bind)
[?e :position _]

;; Multiple patterns (implicit join on shared variables)
[?e :position ?pos]
[?e :velocity ?vel]

;; Cross-entity patterns
[?a :follows ?b]
[?b :name ?name]
```

### 5.2 Negation

Match on absence of patterns:

```clojure
(rule: stationary-entities
  :where [[?e :position _]
          (not [?e :velocity _])]     ;; Has position but NOT velocity
  :then  [...])

(rule: lonely-entities
  :where [[?e :social true]
          (not [?other :follows ?e])] ;; Nobody follows this entity
  :then  [...])
```

**Negation semantics**: Evaluated against the current world state. If an earlier rule in the same tick created or destroyed entities, negations see those changes.

**Safety rule**: All variables in a negated pattern must be bound by earlier positive patterns. This prevents ambiguous "global negation":

```clojure
;; VALID: ?e is bound before negation
:where [[?e :position _]
        (not [?e :velocity _])]

;; INVALID: ?e is unbound (compile-time error)
:where [(not [?e :tag/enemy])]

;; For "no enemies exist", use query-exists? in a guard:
:where [[?world :world/singleton]]
:guard [(not (query-exists? :where [[_ :tag/enemy]]))]
```

This safety rule ensures negation has clear, unambiguous semantics: "for this bound entity, no matching pattern exists."

### 5.3 Previous Tick Access

Access values from the committed previous tick:

```clojure
(rule: detect-damage
  :where [[?e :health/current ?hp]]
  :let   [prev-hp (prev ?e :health/current)
          damage (- prev-hp ?hp)]
  :guard [(< ?hp prev-hp)]
  :then  [(spawn! {:event/type :damage-taken
                   :event/entity ?e
                   :event/amount damage})])
```

### 5.4 Tick Lifecycle

```
tick(world, inputs) -> Result<World, Error>

┌──────────────────────────────────────────────────────────────┐
│                         TICK N                               │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│ 1. SETUP                                                     │
│    • Store current world as "previous" (for `prev` access)   │
│    • Inject input entities into world                        │
│    • Clear refraction set from previous tick                 │
│                                                              │
│ 2. RULE EXECUTION LOOP                                       │
│    ┌────────────────────────────────────────────────────┐    │
│    │ while true:                                        │    │
│    │   matches = find_all_matching_rules(world)         │    │
│    │   candidates = matches - refracted_this_tick       │    │
│    │                                                    │    │
│    │   if candidates.is_empty():                        │    │
│    │     break  // Quiescence reached                   │    │
│    │                                                    │    │
│    │   rule, bindings = highest_priority(candidates)    │    │
│    │   execute(rule.then, bindings)  // Mutates world   │    │
│    │   refracted_this_tick.add((rule, bindings))        │    │
│    └────────────────────────────────────────────────────┘    │
│                                                              │
│ 3. CONSTRAINT CHECKING                                       │
│    • For each constraint (ordered by salience, then decl):   │
│      - Evaluate :check predicates against current world      │
│      - :rollback violation → return Error, world unchanged   │
│      - :warn violation → log warning, continue               │
│                                                              │
│ 4. COMMIT                                                    │
│    • Flush output buffer (print! calls)                      │
│    • Return new world with tick incremented                  │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

**Key properties:**

| Property    | Guarantee                                                      |
| ----------- | -------------------------------------------------------------- |
| Termination | Refraction ensures finite rule firings per tick                |
| Atomicity   | Entire tick succeeds or fails; no partial commits              |
| Causality   | Effects chain naturally; rule A creates entity → rule B reacts |
| Determinism | Same inputs + same world → same outputs                        |

**Error handling:**

Any unhandled error during rule execution aborts the tick. The transaction is discarded and the world remains unchanged. Errors include full context:

```
Error during tick 42:
  Rule: calculate-damage (defined at game/combat.lt:15)
  Bindings: {?attacker: Entity(5), ?target: Entity(12)}
  Expression: (/ ?damage ?armor)
  Cause: Division by zero
```

Rules can catch errors explicitly with `try`.

### 5.5 Group-By Semantics in Rules

When `:group-by` is present, `:then` executes **once per group**:

```clojure
(rule: faction-bonus
  :where     [[?e :faction ?f]
              [?e :power ?p]]
  :group-by  [?f]
  :aggregate {:total-power (sum ?p)
              :count (count ?e)}
  :guard     [(> ?count 5)]
  :then      [(print! (format "Faction {} has {} members with {} total power"
                              ?f ?count ?total-power))])
```

This fires once per faction that has more than 5 members.

### 5.6 Effects

Effects modify the world (visible immediately):

```clojure
;; Entity lifecycle
(spawn! {:component value ...})     ;; Returns new entity ID
(destroy! entity)

;; Component mutation
(set! entity :component value)
(set! entity :component/field value)
(update! entity :component f)
(update! entity :component/field f)

;; Relationships
(link! source :relationship target)
(unlink! source :relationship target)

;; Output (buffered until tick commit)
(print! "message")

;; Meta (take effect next tick)
(enable-rule! rule-entity)
(disable-rule! rule-entity)
```

**Bindings vs current state**: Bindings are captured when a rule matches. Within the `:then` body, you can use bindings (the values that caused the rule to fire) or query current state:

```clojure
(rule: example
  :where [[?e :hp ?hp]]           ;; ?hp bound at match time
  :then [(set! ?e :hp 0)
         (print! ?hp)             ;; Prints the hp that triggered the match
         (print! (get ?e :hp))])  ;; Prints 0 (current value)
```

Both are useful: bindings tell you "why this rule fired," while `get`/`query` tell you "what's true right now."

**Stale entity references**: If an earlier rule destroyed an entity that a later rule matched:

```clojure
(rule: example-stale
  :where [[?e :tag/enemy] [?e :target ?victim]]
  :then [(destroy! ?victim)                    ;; ?victim is now stale
         (set! ?victim :hp 0)])               ;; ERROR: stale entity reference
```

- `destroy!` on the same entity multiple times is **idempotent** (no error)
- `set!`/`update!`/`get` on a destroyed entity is a **runtime error**
- Use `entity-exists?` or `get?` for safe conditional access:

```clojure
(rule: safe-example
  :where [[?e :tag/enemy] [?e :target ?victim]]
  :then [(when (entity-exists? ?victim)
           (set! ?victim :hp 0))
         (destroy! ?victim)])                  ;; Safe, idempotent
```

This strict behavior catches bugs where multiple rules might act on the same entity without coordination.

### 5.7 Write Conflict Semantics

When multiple rules write to the same entity/field within a tick, **last-write-wins**:

```clojure
;; Rule A (salience 10) fires first:
(set! ?e :health/current 50)

;; Rule B (salience 5) fires second:
(set! ?e :health/current 75)

;; Result: :health/current is 75
```

**Rationale**: With deterministic rule ordering (salience → specificity → declaration order), the write order is predictable. Last-write-wins is simple and matches the mental model of "rules execute in order."

**Debugging**: The provenance system tracks all writes, so `(why (get ?e :health/current))` shows the full write history, not just the final value.

**Future consideration**: Per-field `:conflict :error` annotation may be added if use cases emerge where accidental overwrites need detection.

---

## 6. Query System

### 6.1 Query Syntax

Queries use the same pattern syntax as rules:

```clojure
;; Basic query
(query
  :where  [[?e :tag/enemy]
           [?e :health/current ?hp]]
  :guard  [(< ?hp 20)]
  :return {:entity ?e :hp ?hp})

;; Shorthand queries
(query-one
  :where [[?e :tag/player]]
  :return ?e)

(query-count
  :where [[?e :tag/enemy]])

(query-exists?
  :where [[?e :tag/player]])
```

### 6.2 Query Evaluation Context

Queries always see the current world state:

| Context               | View                                             |
| --------------------- | ------------------------------------------------ |
| During rule execution | Current transaction state (sees earlier effects) |
| Outside tick (REPL)   | Latest committed world                           |

To compare against the previous tick, use `prev`:

```clojure
(rule: check-enemy-change
  :where [[?e :tag/enemy]]
  :then [(let [current-count (query-count :where [[_ :tag/enemy]])
               ;; For previous tick comparison, query against prev values
               ]
            (print! (str "Current enemies: " current-count)))])
```

### 6.3 Aggregation Examples

```clojure
;; Total gold across all players
(query
  :where     [[?p :tag/player]
              [?p :gold ?g]]
  :aggregate {:total (sum ?g)}
  :return    ?total)

;; Per-faction statistics
(query
  :where     [[?e :faction ?f]
              [?e :power ?p]]
  :group-by  [?f]
  :aggregate {:total-power (sum ?p)
              :member-count (count ?e)
              :avg-power (avg ?p)}
  :return    {:faction ?f
              :total ?total-power
              :count ?member-count
              :avg ?avg-power})

;; Top 5 strongest enemies
(query
  :where    [[?e :tag/enemy]
             [?e :power ?p]]
  :order-by [[?p :desc]]
  :limit    5
  :return   ?e)
```

### 6.4 Relationship Traversal

```clojure
;; Forward: who does alice follow?
(query
  :where [[alice :follows ?target]]
  :return ?target)

;; Reverse: who follows alice?
(query
  :where [[?follower :follows alice]]
  :return ?follower)

;; Multi-hop: friends of friends
(query
  :where [[?me :tag/player]
          [?me :follows ?friend]
          [?friend :follows ?fof]
          (!= ?fof ?me)]
  :return ?fof)
```

### 6.5 Query Planning

The engine optimizes queries automatically:

```clojure
(explain-query
  :where [[?e :tag/enemy]
          [?e :position ?pos]]
  :guard [(< (:x ?pos) 100)])

;; Output:
;; 1. Scan index :tag/enemy (est. 50 entities)
;; 2. Lookup :position by entity-id (50 lookups)
;; 3. Filter by (:x ?pos) < 100 (est. 25 results)
;; Total estimated cost: 100 operations
```

---

## 7. Tick Lifecycle

### 7.1 Input Processing

Inputs enter the world as entities:

```clojure
;; Before tick, runtime creates:
(spawn! {:input/raw "go north"
         :input/tick current-tick
         :input/source :player})
```

Rules process inputs like any other entity:

```clojure
(rule: parse-command
  :salience 100
  :where   [[?input :input/raw ?text]]
  :then    [(let [parsed (parse-command ?text)]
              (spawn! {:command/type (:type parsed)
                       :command/args (:args parsed)
                       :command/source ?input})
              (destroy! ?input))])
```

### 7.2 World Singleton

A singleton entity holds **user-defined** global state. Runtime metadata is accessed via functions, not components:

```clojure
;; Runtime values via functions (NOT queryable components):
(current-tick)  ;; => Current tick number
(world-seed)    ;; => World RNG seed
(random)        ;; => Random float in [0, 1)
(random-int n)  ;; => Random int in [0, n)

;; World entity for USER-DEFINED globals only:
[?world :world/singleton]          ;; Tag to find the world entity
[?world :game/mode :exploration]
[?world :game/difficulty :hard]
[?world :game/turns 42]
```

**Why this separation:**
- Runtime values (tick, seed) are read-only and shouldn't be in the ECS
- User globals (game mode, difficulty) are mutable game state
- Prevents users from accidentally modifying runtime invariants

```clojure
;; Find and use world entity
(rule: increment-turns
  :where [[?world :world/singleton]
          [?cmd :command/type _]]
  :then  [(update! ?world :game/turns inc)])
```

### 7.3 Error Handling

```clojure
;; Try/catch within rules
(rule: safe-division
  :where [[?e :dividend ?a]
          [?e :divisor ?b]]
  :then  [(try (/ ?a ?b)
            (fn: [result] (set! ?e :result result))
            (fn: [error] (print! (str "Error: " error))))])

;; Uncaught errors roll back the entire tick
(rule: dangerous
  :where [[?e :value ?v]]
  :then  [(/ ?v 0)])   ;; → tick rollback
```

Error messages include full context:

```
Error during tick 42:
  Rule: dangerous (defined at game/rules.lt:15)
  Activation bindings: {?e: Entity(123), ?v: 100}
  Expression: (/ ?v 0)
  Cause: Division by zero

  Stack trace:
    game/rules.lt:17 (/ ?v 0)
    game/rules.lt:15 (rule: dangerous ...)
```

### 7.4 Constraint Violations

When constraints detect violations, they report them clearly:

```
Tick 42 constraint violation:
  Constraint: health-bounds
  Entity: Entity(42)
  Check failed: (>= ?hp 0) where ?hp = -5
  Action: rollback (tick discarded)
```

For `:warn` constraints:

```
Tick 42 constraint warning:
  Constraint: optional-bounds
  Entity: Entity(42)
  Check failed: (<= ?debt 1000) where ?debt = 1500
  Action: warning logged, tick continues
```

This allows debugging and supports the `(why ...)` introspection when investigating failed ticks.

### 7.5 Speculative Execution & Planning

Longtable's "world as value" design enables powerful AI planning. Since `simulate` is a pure function and worlds are immutable, agents can simulate futures without affecting the real world.

#### 7.5.1 Core API

```clojure
;; Get the base world as an immutable value
(world-snapshot)  ;; => World value (the committed state before this tick)

;; Simulate a tick (pure function, no side effects escape)
(simulate world inputs)  ;; => New world value

;; Query a specific world value (for speculation)
(world-get world entity :component)
(world-get world entity :component/field)
(world-query world :where [[?e :tag/enemy]] :return ?e)

;; Modify a world value (returns new world, original unchanged)
(world-assoc world entity :component value)
(world-dissoc world entity :component)

;; Hash a world for memoization/visited-set tracking
(world-hash world)  ;; => Deterministic hash of world state
```

#### 7.5.2 Basic Speculation

```clojure
(rule: ai-choose-action
  :where [[?ai :tag/ai-controlled]
          [?ai :possible-actions ?actions]]
  :then [(let [base (world-snapshot)
               scored (for [action ?actions]
                        {:action action
                         :outcome (simulate base [{:actor ?ai :action action}])})
               best (max-by #(evaluate (:outcome %) ?ai) scored)]
           (set! ?ai :chosen-action (:action best)))])
```

#### 7.5.3 Multi-Perspective Planning (Theory of Mind)

For complex AI that reasons about other agents' responses:

```clojure
;; "He's guarding the door. If I approach, he'll close it.
;;  Better to go for the window."

(fn: evaluate-action [world actor action]
  (let [;; Simulate my action
        after-my-action (simulate world [{:actor actor :action action}])

        ;; Simulate each other agent's response
        other-agents (world-query after-my-action
                       :where [[?a :tag/ai-controlled]
                               (!= ?a actor)]
                       :return ?a)

        after-responses (reduce
                          (fn [w other]
                            (let [their-action (predict-action w other)]
                              (simulate w [{:actor other :action their-action}])))
                          after-my-action
                          other-agents)]

    ;; Evaluate the resulting world from my perspective
    (score-world after-responses actor)))

(fn: predict-action [world agent]
  ;; Simulate what this agent would choose (using their AI)
  ;; This is recursive - they might reason about me too!
  (let [their-options (world-get world agent :possible-actions)
        their-best (max-by #(score-world
                              (simulate world [{:actor agent :action %}])
                              agent)
                           their-options)]
    their-best))

(rule: smart-ai-planning
  :where [[?ai :tag/smart-ai]
          [?ai :possible-actions ?actions]]
  :then [(let [base (world-snapshot)
               scores (for [a ?actions]
                        {:action a
                         :score (evaluate-action base ?ai a)})
               best (max-by :score scores)]
           (set! ?ai :chosen-action (:action best)))])
```

#### 7.5.4 GOAP-Style Planning

Goal-Oriented Action Planning searches for action sequences:

```clojure
(fn: plan-goap [world actor goal-fn max-depth]
  "Find a sequence of actions that achieves goal-fn."
  (let [actions (world-get world actor :available-actions)]
    (loop [frontier [{:world world :plan [] :cost 0}]
           visited #{}
           iterations 0]
      (cond
        ;; No solution found
        (empty? frontier) nil

        ;; Safety limit
        (> iterations 10000) nil

        :else
        (let [{:keys [world plan cost]} (first frontier)
              state-hash (world-hash world)]
          (cond
            ;; Goal achieved!
            (goal-fn world actor)
            plan

            ;; Already visited this state
            (contains? visited state-hash)
            (recur (rest frontier) visited (inc iterations))

            ;; Max depth reached
            (>= (count plan) max-depth)
            (recur (rest frontier) visited (inc iterations))

            ;; Expand this node
            :else
            (let [children (for [action actions
                                 :let [next-world (simulate world [{:actor actor :action action}])
                                       action-cost (action-cost action)]
                                 :when (valid-action? world actor action)]
                             {:world next-world
                              :plan (conj plan action)
                              :cost (+ cost action-cost)})]
              (recur (into (rest frontier) children)
                     (conj visited state-hash)
                     (inc iterations)))))))))

;; A* variant with heuristic
(fn: plan-astar [world actor goal-fn heuristic-fn max-depth]
  (let [actions (world-get world actor :available-actions)
        compare-fn (fn [a b] (< (+ (:cost a) (:heuristic a))
                                (+ (:cost b) (:heuristic b))))]
    (loop [frontier (sorted-set-by compare-fn
                      {:world world :plan [] :cost 0
                       :heuristic (heuristic-fn world actor)})
           visited #{}]
      ;; ... similar to above, but frontier is priority queue
      )))
```

#### 7.5.5 The Door-and-Window Example

```clojure
;; Components
(component: position :x :int :y :int)
(component: blocking :target :entity-ref)  ;; "I'm blocking this"
(component: intention :action :keyword :target :entity-ref)

;; The guard's reactive behavior
(rule: guard-blocks-door
  :where [[?guard :blocking ?door]
          [?intruder :intention {:action :approach :target ?door}]]
  :then [(set! ?guard :intention {:action :close :target ?door})])

;; The intruder's planning
(fn: evaluate-entry-plan [world intruder]
  (let [door (world-query-one world :where [[?d :tag/door]] :return ?d)
        window (world-query-one world :where [[?w :tag/window]] :return ?w)
        guard (world-query-one world :where [[?g :blocking door]] :return ?g)

        ;; Plan A: Go for door
        door-plan [(simulate world [{:actor intruder :action :approach :target door}])]
        ;; Simulate guard's response
        door-after-guard (if guard
                           (simulate (first door-plan)
                                 [{:actor guard :action :close :target door}])
                           (first door-plan))
        door-success? (world-get door-after-guard door :open)

        ;; Plan B: Go for window
        window-plan [(simulate world [{:actor intruder :action :break :target window}])]
        window-success? true  ;; Windows can't be blocked
        window-cost 10]       ;; But it's noisy/costly

    (if (and door-success? (not guard))
      {:plan :door :expected-success true :cost 0}
      {:plan :window :expected-success true :cost window-cost})))

(rule: intruder-plans-entry
  :where [[?intruder :goal :enter-building]
          [?intruder :tag/smart-ai]]
  :then [(let [plan (evaluate-entry-plan (world-snapshot) ?intruder)]
           (set! ?intruder :chosen-plan (:plan plan))
           (print! (format "{} decides: {}" ?intruder (:plan plan))))])
```

#### 7.5.6 Output Handling in Speculation

Speculative ticks buffer their outputs:

| Output Type | During Speculation           | After Real Commit        |
| ----------- | ---------------------------- | ------------------------ |
| `print!`    | Attached to world value      | Flushed to output        |
| `trace!`    | Always live (debug channel)  | Always live              |
| Effects     | Applied to speculative world | Applied to current world |

```clojure
;; Inspect what a speculative tick would have printed
(let [future (simulate (world-snapshot) [{:action :attack}])]
  (world-get future :runtime/output-buffer))
;; => ["You attack the goblin!" "The goblin dies!"]
```

#### 7.5.7 Performance Considerations

**Structural sharing**: Worlds share structure, so forking is O(changes), not O(world-size).

**Memoization**: Use `(world-hash world)` for visited-set tracking in search algorithms.

**Depth limits**: Always bound search depth to prevent runaway computation.

**Lazy evaluation**: Consider evaluating branches lazily or with iterative deepening.

```clojure
;; Iterative deepening for anytime planning
(fn: plan-iterative [world actor goal-fn]
  (loop [depth 1
         best-plan nil]
    (if (> depth 20)
      best-plan
      (let [plan (plan-goap world actor goal-fn depth)]
        (recur (inc depth) (or plan best-plan))))))
```

**Parallel search** (if supported):

```clojure
;; Evaluate top-level branches in parallel
(let [branches (for [action actions]
                 (future (evaluate-action base actor action)))]
  (max-by :score (map deref branches)))
```

---

## 8. Observability & Debugging

### 8.1 Change Tracking

Changes are tracked at component-field granularity:

```clojure
(query-changes :health/current)
;; => [{:entity E1 :old 100 :new 75 :source :rule/apply-damage}
;;     {:entity E2 :old 50 :new 0 :source :rule/apply-damage}]

;; Meaningful change detection: $45 → $45 is NOT a change
```

### 8.2 Tracing

```clojure
(trace :rule/apply-damage)
;; Tick 42: apply-damage fired
;;   Bindings: {?target: Entity(5), ?hp: 100, ?dmg: 25}
;;   Effects: (set! Entity(5) :health/current 75)

(trace-entity entity-42)
(untrace :rule/apply-damage)
```

### 8.3 Explain

```clojure
(why (get entity-42 :health/current))
;; Value: 75
;; Set by: rule apply-damage at tick 42
;; Expression: (- ?hp ?dmg) where ?hp=100, ?dmg=25

(why (get entity-42 :health/percent))
;; Value: 75
;; Derived component: health/percent
;; Dependencies: :health/current=75, :health/max=100
;; Expression: (/ (* ?curr 100) ?max)
```

### 8.4 Breakpoints

```clojure
(break-on :rule/apply-damage)
(break-on :health/current)
(break-on entity-42)
(break-on :health/current :when (fn: [old new] (= new 0)))

;; Debugger commands:
(step)        ;; Execute one rule
(continue)    ;; Run to next breakpoint
(inspect ?e)  ;; Show entity
(locals)      ;; Show bindings
```

### 8.5 Time Travel

```clojure
(rollback! 5)           ;; Go back 5 ticks
(advance! 3)            ;; Forward 3 ticks (if in history)
(goto-tick! 42)         ;; Jump to specific tick

;; Explore alternatives
(let [alt (-> (current-world)
              (with-hypothetical {:entity-5 :health/current 200})
              (tick []))]
  (compare-worlds (current-world) alt))

;; Branching
(branch! "what-if")
(switch-branch! "main")
```

### 8.6 REPL Commands

```
> (inspect entity-42)
Entity(42) [Archetype: Position, Velocity, Health]
  :position {:x 10.0 :y 20.0}
  :velocity {:dx 1.0 :dy 0.0}
  :health {:current 75 :max 100}

> (tick!)
Tick 43 completed. Rules: 12, Entities changed: 8, Time: 234ms

> (save! "checkpoint.lt")
> (load! "checkpoint.lt")
```

### 8.7 Provenance Model

For `(why ...)` and debugging to work, the engine maintains an **effect log**:

```
EffectRecord = {
  tick:        Int              ;; When this happened
  entity:      EntityId         ;; What entity was affected
  field:       Keyword          ;; Which component/field
  old_value:   Value            ;; Previous value (or nil if new)
  new_value:   Value            ;; New value (or nil if destroyed)
  source:      EffectSource     ;; Who caused this
  expression:  Option<ExprId>   ;; What expression (debug mode)
  bindings:    Option<Map>      ;; Activation bindings (debug mode)
}

EffectSource = :rule/<name> | :constraint/<name> | :system | :external
```

**Storage policy:**

| Mode        | Stored                      | Use Case                            |
| ----------- | --------------------------- | ----------------------------------- |
| Production  | Last-writer index per field | Minimal overhead, basic `(why ...)` |
| Development | Full effect log per tick    | Complete history, rich debugging    |
| Debug       | + expression IDs + bindings | Step-through, full provenance       |

The `(why ...)` function traverses the effect log to explain how a value was computed, including intermediate writes that were later overwritten.

### 8.8 Event Entity Pattern

Event entities are a useful pattern for modeling discrete occurrences that multiple rules should react to. While rules chain naturally (a death causes loot to drop), explicit event entities add structure and debuggability.

**When to use event entities:**
- Multiple independent reactions to the same occurrence
- You want to query "what happened this tick" after the fact
- Cross-tick effects (damage over time, delayed reactions)
- Historical logging for debugging or replay

**Basic pattern:**

```clojure
(component: event/type :keyword)
(component: event/tick :int)
(component: event/source :entity-ref)
(component: event/data :map)

;; Emit an event when something notable happens
(rule: emit-death-event
  :salience 50
  :where [[?e :health/current ?hp]]
  :guard [(<= ?hp 0)]
  :then [(spawn! {:event/type :entity-died
                  :event/tick (current-tick)
                  :event/source ?e})])

;; Multiple rules can react to the same event type
(rule: on-death-drop-loot
  :salience 40
  :where [[?event :event/type :entity-died]
          [?event :event/source ?corpse]
          [?corpse :inventory ?items]]
  :then [(doseq [item ?items]
           (spawn! {:item/type item
                    :position (get ?corpse :position)}))])

;; Cleanup at end of tick
(rule: cleanup-events
  :salience -1000
  :where [[?event :event/type _]]
  :then [(destroy! ?event)])
```

**Cross-tick events**: For effects that span ticks (damage over time, cooldowns):

```clojure
(component: event/expires-at :int)

(rule: expire-events
  :salience -1000
  :where [[?event :event/expires-at ?expires]]
  :guard [(<= ?expires (current-tick))]
  :then [(destroy! ?event)])
```

Note that event entities are optional—rules chain naturally without them. Use events when the added structure benefits your game design or debugging.

---

## 9. Implementation Notes

### 9.1 Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                      User Interface                          │
│  REPL, Game Loop, Tests, Debugger, Time-Travel Inspector    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     Tick Executor                            │
│  • Input ingestion         • Constraint checking             │
│  • Activation computation  • Output buffer flush             │
│  • Rule orchestration      • Overlay commit                  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      Rule Engine                             │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────────┐  │
│  │   Pattern   │  │    Query     │  │     Derived       │  │
│  │   Matcher   │  │   Planner    │  │    Evaluator      │  │
│  └──────┬──────┘  └──────┬───────┘  └────────┬──────────┘  │
│         └────────────────┼───────────────────┘              │
│                          ▼                                   │
│              ┌───────────────────┐                          │
│              │   Bytecode VM     │                          │
│              └───────────────────┘                          │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│               World (Immutable) + Overlay                    │
│  ┌─────────────────┐ ┌─────────────────┐ ┌───────────────┐ │
│  │    Entities     │ │     Indices     │ │ Derived Cache │ │
│  └─────────────────┘ └─────────────────┘ └───────────────┘ │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              Persistent Data Structures                      │
│  HAMT (maps), RRB-Vector (sequences), structural sharing    │
└─────────────────────────────────────────────────────────────┘
```

### 9.2 Pattern Matching Implementation

The engine computes activation sets using indexed pattern matching:

1. **Component indices**: `ComponentType → Set<EntityId>` enables O(1) lookup
2. **Join order optimization**: Start with most selective pattern
3. **Archetype pruning**: Skip entity groups that cannot match
4. **Filter application**: Apply guards after structural matching

**Incremental optimization**: The engine MAY cache partial match results across ticks and update them incrementally based on world diffs. This is semantically equivalent to full re-evaluation but more efficient for large worlds with small per-tick changes.

The specification does not mandate a particular matching algorithm. Implementations must produce identical activation sets regardless of strategy used.

**Implementation roadmap**: The initial implementation uses snapshot re-evaluation each tick. Full incremental match maintenance (RETE-style) is a priority goal to be implemented as soon as the core system is functioning correctly. This optimization does not change observable behavior—only performance characteristics.

### 9.3 Bytecode VM

The VM executes compiled expressions:

**Compilation**: DSL → AST → Bytecode

**Instruction categories**:
- Stack: `PUSH`, `POP`, `DUP`
- Arithmetic: `ADD`, `SUB`, `MUL`, `DIV`, `MOD`
- Comparison: `LT`, `LE`, `GT`, `GE`, `EQ`, `NE`
- Control: `JUMP`, `JUMP_IF`, `CALL`, `RETURN`
- Data: `GET_COMPONENT`, `GET_FIELD`, `GET_LOCAL`
- Effects: `SPAWN`, `DESTROY`, `SET`, `UPDATE`

### 9.4 Persistent Data Structures

**HAMT**: O(log32 n) maps with structural sharing
**RRB-Vector**: O(log n) vectors with efficient slice/concat

These enable cheap world snapshots for time travel.

### 9.5 RNG Design

```
World Seed
├── Tick N Seed = hash(world_seed, N)
│   ├── Rule "foo" Seed = hash(tick_seed, "foo")
│   │   └── Entity E Seed = hash(rule_seed, E.id)
│   └── Rule "bar" Seed = hash(tick_seed, "bar")
└── Tick N+1 Seed = hash(world_seed, N+1)
```

Deterministic within a run, independent per-rule.

### 9.6 Native Function Registration

Native functions (Rust-implemented, callable from DSL) declare metadata:

```rust
#[longtable::native_fn(
    name = "math/pathfind",
    signature = "(:vec<:int>, :vec<:int>) -> :vec<:vec<:int>>",
    effect = "pure",           // pure | effectful
    determinism = "deterministic"  // deterministic | nondeterministic
)]
fn pathfind(from: Value, to: Value) -> Result<Value, Error> {
    // Implementation
}
```

**Effect/determinism rules:**

| Flags                       | Allowed In                                   |
| --------------------------- | -------------------------------------------- |
| `pure` + `deterministic`    | Everywhere (queries, guards, derived, rules) |
| `pure` + `nondeterministic` | Rule `:then` only (must use provided RNG)    |
| `effectful`                 | Rule `:then` only                            |

Pure/deterministic functions are optimizable; others are not.

### 9.7 Rust API

```rust
pub struct World { /* opaque */ }

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct EntityId { index: u64, generation: u32 }

pub enum Value {
    Nil, Bool(bool), Int(i64), Float(f64),
    String(Arc<str>), Symbol(SymbolId), Keyword(KeywordId),
    EntityRef(EntityId),
    Vec(Arc<Vector<Value>>), Set(Arc<HashSet<Value>>),
    Map(Arc<HashMap<Value, Value>>),
}

impl World {
    pub fn new(seed: u64) -> Result<World, Error>;
    pub fn load(&mut self, source: &str) -> Result<(), Error>;
    pub fn load_file(&mut self, path: &Path) -> Result<(), Error>;
    pub fn tick(&self, inputs: Vec<Value>) -> Result<World, TickError>;
    pub fn query(&self, query: &str) -> Result<Vec<Value>, Error>;
    pub fn get(&self, entity: EntityId, component: &str) -> Result<Value, Error>;
    pub fn current_tick(&self) -> u64;
    pub fn previous(&self) -> Option<&World>;
    pub fn save(&self) -> Result<Vec<u8>, Error>;
    pub fn restore(bytes: &[u8]) -> Result<World, Error>;
}
```

### 9.8 File Format & Serialization

**What gets saved:**

| Content          | Saved? | Notes                         |
| ---------------- | ------ | ----------------------------- |
| Entity data      | Yes    | All components, relationships |
| World metadata   | Yes    | Tick, seed, world entity      |
| Rule activations | No     | Recomputed on load            |
| Derived caches   | No     | Recomputed on demand          |
| Bytecode         | No     | Recompiled from source        |

**Save/restore serializes world state, not compiled code**. Rules and constraints are stored as their meta-entity data, not bytecode. On restore, the engine recompiles from the original source files.

**Implication**: Save files are **not portable** across DSL versions that change compilation. The saved world assumes the same source files are available at restore time.

**Format**: MessagePack with structure:

```
{
  "version": "0.2",
  "tick": 42,
  "seed": 12345,
  "entities": { "3.42": { "position": {...}, ... }, ... },
  "relationships": { "follows": { "forward": {...}, "reverse": {...} }, ... },
  "meta_entities": { "rule/apply-damage": {...}, ... },
  "world_entity": "1.1"
}
```

**Restore process:**
1. Load entity data from save file
2. Reload and recompile source DSL files
3. Verify meta-entity consistency (warn if rules differ)
4. Rebuild indices and derived caches

---

## Appendix A: Grammar (EBNF)

```ebnf
program     = form* ;

form        = literal | symbol | keyword | list | vector | set | map | tagged ;

literal     = nil | bool | int | float | string ;
nil         = "nil" ;
bool        = "true" | "false" ;
int         = ["-"] digit+ ;
float       = ["-"] digit+ "." digit+ ;
string      = '"' (char | escape)* '"' ;

symbol      = symbol_start symbol_char* ;
symbol_start= letter | "_" | "+" | "-" | "*" | "/" | "!" | "?" | "<" | ">" | "=" ;
symbol_char = symbol_start | digit | ":" | "." ;

keyword     = ":" symbol ;

list        = "(" form* ")" ;
vector      = "[" form* "]" ;
set         = "#{" form* "}" ;
map         = "{" (form form)* "}" ;

tagged      = "#" symbol form ;

comment     = ";" [^\n]* "\n" | "#_" form ;
```

---

## Appendix B: Reserved Symbols and Namespaces

### Reserved Symbols

```
;; Special forms
def fn: let if do match loop recur try quote defmacro

;; Declaration forms
world: component: relationship: derived: rule: constraint: literal:

;; Module system
namespace load

;; Constants
nil true false none

;; Query clauses
:where :let :aggregate :group-by :guard :order-by :limit
:for :return :then :value :check :on-violation
:salience :enabled :storage :cardinality :required :attributes
```

### Reserved Namespaces

The following keyword namespaces are reserved for engine use. User code **cannot** declare components, relationships, or other entities in these namespaces:

| Namespace     | Purpose                  | Example                             |
| ------------- | ------------------------ | ----------------------------------- |
| `:runtime/*`  | Engine runtime values    | `:runtime/tick`, `:runtime/seed`    |
| `:system/*`   | System-generated effects | `:system/rollback`, `:system/error` |
| `:meta/*`     | Entity metadata          | `:meta/rule`, `:meta/component`     |
| `:internal/*` | Implementation details   | `:internal/archetype-id`            |

Attempting to `(set! entity :runtime/tick 42)` is a compile-time or load-time error.

User-defined namespaces should use project-specific prefixes (e.g., `:game/*`, `:myproject/*`).

---

## Appendix C: Example Program

```clojure
;; === File: _.lt (entry point) ===
(namespace adventure)

(load "components")
(load "rules")
(load "data/initial-world")

(world:
  :seed 42
  :name "The Dark Cave")


;; === File: components.lt ===
(namespace adventure.components)

(component: position :x :int :y :int)
(component: description :short :string :long :string :default "")
(component: name :value :string)

(component: tag/player :bool :default true)
(component: tag/room :bool :default true)
(component: tag/item :bool :default true)

(relationship: in-room
  :storage :field
  :cardinality :many-to-one
  :on-target-delete :cascade)

(component: game/turns :int :default 0)


;; === File: rules.lt ===
(namespace adventure.rules
  (:require [adventure.components]))

(rule: handle-go
  :salience 100
  :where   [[?cmd :command/type :go]
            [?cmd :command/direction ?dir]
            [?player :tag/player]
            [?player :in-room ?current-room]
            [?current-room :exits ?exits]]
  :then    [(if-let [next-room (get ?exits ?dir)]
              (do
                (unlink! ?player :in-room ?current-room)
                (link! ?player :in-room next-room)
                (print! (get next-room :description/short)))
              (print! "You can't go that way."))
            (destroy! ?cmd)])

(rule: handle-look
  :salience 100
  :where   [[?cmd :command/type :look]
            [?player :tag/player]
            [?player :in-room ?room]]
  :then    [(print! (get ?room :description/long))
            (let [items (query
                          :where [[?i :tag/item] [?i :in-room ?room]]
                          :return ?i)]
              (when (not (empty? items))
                (print! "You see:")
                (doseq [item items]
                  (print! (str "  - " (get item :name/value))))))
            (destroy! ?cmd)])

(rule: tick-turn
  :salience -100
  :where   [[?world :game/turns ?t]
            [_ :command/type _]]
  :then    [(update! ?world :game/turns inc)])


;; === File: data/initial-world.lt ===
(namespace adventure.data)

(rule: bootstrap
  :once    true
  :salience 1000
  :where   [(not [_ :world/initialized])]
  :then    [(let [entrance (spawn! {:tag/room true
                                    :name/value "Cave Entrance"
                                    :description/short "A dark cave entrance."
                                    :description/long "You stand at the mouth of a dark cave."
                                    :exits {:north nil}})
                  main-hall (spawn! {:tag/room true
                                     :name/value "Main Hall"
                                     :description/short "A vast underground hall."
                                     :description/long "An enormous cavern stretches before you."
                                     :exits {:south nil}})]
              (set! entrance :exits {:north main-hall})
              (set! main-hall :exits {:south entrance})

              (let [player (spawn! {:tag/player true :name/value "Adventurer"})]
                (link! player :in-room entrance))

              (let [lantern (spawn! {:tag/item true
                                     :name/value "brass lantern"
                                     :description/short "A battered brass lantern."})]
                (link! lantern :in-room entrance))

              (spawn! {:world/initialized true :game/turns 0})

              (print! "Welcome to THE DARK CAVE")
              (print! "")
              (print! (get entrance :description/long)))])
```

---

*End of Specification v0.8*
