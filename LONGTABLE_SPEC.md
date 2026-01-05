# Longtable Specification v0.3

## 1. Overview & Goals

### 1.1 What is Longtable?

Longtable is a rule-based simulation engine combining:

- **LISP-like DSL** for defining components, rules, queries, and logic
- **Archetype-based Entity-Component-System (ECS)** for data organization
- **RETE-style rule engine** for reactive, incremental pattern matching
- **Persistent/functional data structures** enabling time-travel debugging
- **Tick-based discrete simulation** with transactional semantics

### 1.2 Design Philosophy

1. **Everything Is An Entity** - Rules, component schemas, relationships, and even the world itself are entities that can be queried and manipulated.

2. **World As Value** - The world state is immutable. Each tick produces a new world. This enables rollback, time-travel debugging, and "what-if" exploration.

3. **Effects, Not Mutations** - Rules don't mutate state directly. They produce effects that are applied to a transaction overlay, then committed atomically.

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

### 2.2 Tick Overlay (Transaction Model)

During tick execution, an **Overlay** acts as a mutable transaction buffer on top of the immutable base world:

```
┌─────────────────────────────────────────────────────────────┐
│                     TICK EXECUTION                           │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   Base World (immutable)                                     │
│   ════════════════════════                                   │
│   • The committed world state from end of previous tick      │
│   • Used for: rule activation, pattern matching, `prev`      │
│   • Never modified during tick execution                     │
│                                                              │
│   Overlay (mutable transaction buffer)                       │
│   ════════════════════════════════════                       │
│   • Collects all effects from rule execution                 │
│   • Reads: check overlay first, fall back to base world      │
│   • Writes: always go to overlay                             │
│                                                              │
│   On Commit                                                  │
│   ═════════                                                  │
│   • Materialize new World = Base + Overlay diffs             │
│   • New world becomes base for next tick                     │
│   • On error: discard overlay, world unchanged               │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

**Read semantics during tick:**
- `(get entity :component)` reads from overlay if present, else base world
- `(prev entity :component)` always reads from previous tick's committed world

**Write semantics during tick:**
- All effects (`set!`, `spawn!`, `destroy!`, etc.) write to overlay
- Effects are immediately visible to subsequent expressions in the same rule
- Effects are immediately visible to subsequent rules in execution order

### 2.2.1 Evaluation Context Table

Different operations read from different views depending on context:

| Operation | Activation Phase | Execution (`:then`) | Outside Tick (REPL) |
|-----------|------------------|---------------------|---------------------|
| `get` | base | overlay | committed |
| `query` | base | overlay | committed |
| `prev` | prev | prev | prev |
| `get-base` | base | base | committed |
| `query-base` | base | base | committed |
| Pattern matching | base | N/A | committed |
| Negation (`not`) | base | N/A | committed |

**Key insight**: During activation, ALL query pipeline clauses (`:where`, `:let`, `:aggregate`, `:group-by`, `:guard`, `:order-by`, `:limit`) are evaluated against the **base snapshot** to produce a frozen activation record. Only `:then` runs during execution phase with access to the overlay.

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

| Strategy | Use When | Example |
|----------|----------|---------|
| `:field` | Simple connections, no extra data | `:follows`, `:parent`, `:in-room` |
| `:entity` | Relationships need attributes | `:employment`, `:friendship-with-history` |

**Cardinality and Storage Mapping:**

| Cardinality | `:field` Storage | `:entity` Storage |
|-------------|------------------|-------------------|
| `:one-to-one` | Source component holds single EntityRef | One relationship entity per pair |
| `:one-to-many` | Source component holds `Vec<EntityRef>` | One relationship entity per pair |
| `:many-to-one` | Source component holds single EntityRef | One relationship entity per pair |
| `:many-to-many` | Source component holds `Vec<EntityRef>` | One relationship entity per pair |

**Cardinality enforcement:**
- `:one-to-one` - `link!` errors if source already has a target
- `:one-to-many` - No restrictions on source→target
- `:many-to-one` - No restrictions on source→target
- `:many-to-many` - No restrictions

**On-target-delete behaviors:**
- `:remove` - Remove the relationship when target is destroyed
- `:cascade` - Destroy the source entity when target is destroyed
- `:nullify` - Set to `none` (only valid if `:required false`)

Relationships are manipulated with `link!` and `unlink!`:

```clojure
(link! alice :follows bob)
(unlink! alice :follows bob)
```

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

**Cycle Detection**: Cycles in derived component dependencies within a single tick are compile-time errors. Feedback loops across ticks (via regular components and rules) are allowed.

### 2.9 Constraint

A **Constraint** is an invariant checked after rule execution:

```clojure
(constraint: health-bounds
  :where       [[?e :health/current ?hp]
                [?e :health/max ?max]]
  :check       [(>= ?hp 0) (<= ?hp ?max)]
  :on-violation :clamp)
```

**Violation behaviors:**
- `:rollback` - Entire tick fails, world unchanged
- `:clamp` - Automatically adjust to satisfy constraint (traced as system effect)
- `:warn` - Log warning, allow violation

When `:clamp` is triggered, the adjustment is recorded as a **system-generated effect** in the trace, showing which constraint fired and what it changed.

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
| `:entity-ref` | Reference to an entity     | `#entity[42.3]`        |

**Important**: `nil ≠ false`. They are distinct values of distinct types.

**NaN Debug Mode**: Float operations can produce NaN, which propagates silently. For debugging, enable NaN detection:

```clojure
(set-option! :fail-on-nan true)  ;; Any operation producing NaN throws an error
```

When enabled, operations like `(/ 0.0 0.0)` or `(sqrt -1.0)` will throw an error with a stack trace, making it easier to find the source of NaN propagation.

### 3.2 Composite Types

| Type           | Description                   | Literal Examples            |
| -------------- | ----------------------------- | --------------------------- |
| `:vec<T>`      | Ordered sequence              | `[1 2 3]`, `["a" "b"]`      |
| `:set<T>`      | Unordered unique collection   | `#{1 2 3}`                  |
| `:map<K,V>`    | Key-value mapping             | `{:a 1 :b 2}`               |
| `:option<T>`   | Optional value (Some or None) | `(some 42)`, `none`         |

### 3.3 Type Modifiers in Component Schemas

```clojure
(component: example
  :required-int :int                      ;; Must have a value
  :optional-int :option<:int>             ;; Can be none
  :defaulted-int :int :default 0          ;; Has default value
  :computed-default :int :default (tick)) ;; Default from expression
```

### 3.4 Type Checking

Longtable is **statically typed at the schema level**:
- Component field types are declared and enforced
- Function parameter/return types can be annotated (optional)
- Patterns are type-checked against component schemas

```clojure
(fn: add-ints :- :int [a :- :int, b :- :int]
  (+ a b))
```

Type errors are caught at load time, not runtime.

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
#entity[42.3]       ;; entity reference (index.generation)
```

#### Tagged Literals

Tagged literals provide custom syntax that expands at read time:

```clojure
;; Built-in
#entity[42.3]       ;; Entity reference

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
  :on-violation :rollback|:clamp|:warn)
```

### 4.6 Query Clause Reference

All query-like forms (rules, queries, derived, constraints) support these clauses:

| Clause | Purpose | Available In |
|--------|---------|--------------|
| `:where` | Pattern matching | All |
| `:let` | Per-match computed values | All |
| `:aggregate` | Aggregate functions | All |
| `:group-by` | Partition results | Rule, Query, Constraint |
| `:guard` | Filter on computed/aggregate values | All |
| `:order-by` | Sort results | Rule, Query |
| `:limit` | Cap result count | Rule, Query |
| `:for` | Entity being computed | Derived only |
| `:return` | Output shape | Query only |
| `:then` | Effects to execute | Rule only |
| `:value` | Computed value | Derived only |
| `:check` | Invariant conditions | Constraint only |

**Execution order:**

```
:where      → Find all pattern matches against base snapshot
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

**Negation semantics**: Evaluated against the **base snapshot** (not the overlay). This ensures deterministic, order-independent evaluation.

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

### 5.4 Conflict Resolution

When multiple rules can fire, they are ordered by:

1. **Salience** (higher fires first)
2. **Specificity** (more pattern clauses = more specific)
3. **Declaration order** (earlier in source fires first)

### 5.5 Activation & Execution Model (Snapshot Agenda)

```
┌─────────────────────────────────────────────────────────────┐
│                         TICK N                               │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│ 1. ACTIVATION PHASE                                          │
│    • Take snapshot of base world                             │
│    • For each rule, evaluate ALL query pipeline clauses:     │
│      :where → :let → :aggregate → :group-by → :guard →       │
│      :order-by → :limit                                      │
│    • Produces (rule, frozen-bindings) pairs                  │
│    • This is the "activation set" - ALL BINDINGS FROZEN      │
│                                                              │
│ 2. EXECUTION PHASE                                           │
│    • Sort activations by salience/specificity/order          │
│    • For each (rule, bindings) in sorted order:              │
│      - Bindings are FROZEN from activation phase             │
│      - Rule body CAN READ from overlay (sees earlier effects)│
│      - Rule body WRITES to overlay                           │
│    • Rules NOT in activation set do NOT fire this tick,      │
│      even if overlay changes would now make them match       │
│                                                              │
│ 3. CONSTRAINT PHASE                                          │
│    • Evaluate all constraints against overlay                │
│    • :rollback → discard overlay, tick fails                 │
│    • :clamp → apply fix, record as system effect             │
│    • :warn → log warning, continue                           │
│                                                              │
│ 4. COMMIT PHASE                                              │
│    • Materialize new World = base + overlay                  │
│    • new_world.previous = base_world                         │
│    • Flush output buffer                                     │
│    • tick++                                                  │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

**Key properties:**
- **Deterministic**: Same inputs → same outputs
- **No infinite loops**: Activation set is fixed at tick start
- **Immediate visibility**: Rule bodies see overlay changes from earlier rules
- **Frozen bindings**: Pattern match bindings come from base snapshot
- **Atomic rollback**: Any error discards overlay, world unchanged

### 5.6 Group-By Semantics in Rules

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

### 5.7 Effects

Effects modify the overlay (visible immediately within the tick):

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

;; Output (buffered until commit)
(print! "message")

;; Meta (take effect next tick)
(enable-rule! rule-entity)
(disable-rule! rule-entity)
```

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

Queries follow the same read semantics as `get`:

| Context | View |
|---------|------|
| During activation phase | Base snapshot |
| During rule `:then` execution | Overlay (sees earlier effects) |
| Outside tick (REPL, external) | Latest committed world |

To explicitly query the base snapshot during `:then` execution, use `query-base`:

```clojure
(rule: check-original-state
  :where [[?e :tag/enemy]]
  :then  [(let [current-count (query-count :where [[_ :tag/enemy]])
                original-count (query-base-count :where [[_ :tag/enemy]])]
            (when (!= current-count original-count)
              (print! "Enemy count changed this tick")))])
```

**Note**: Pattern matching in `:where` clauses always uses the base snapshot (frozen during activation). Use `query` within `:then` when you need to see overlay changes.

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
(tick)          ;; => Current tick number
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

### 7.4 Constraint Tracing

When constraints apply `:clamp` fixes, they are traced as system effects:

```
Tick 42 constraint enforcement:
  Constraint: health-bounds
  Entity: Entity(42)
  Violation: :health/current was -5, must be >= 0
  Action: clamped to 0
  Traced as: (system-effect :clamp Entity(42) :health/current -5 0)
```

This allows debugging and `(why ...)` to explain constraint-applied changes.

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

For v0.1, pattern matching uses **indices + join planning**:

1. **Component indices**: `ComponentType → Set<EntityId>`
2. **Join order optimization**: Start with most selective pattern
3. **Filter application**: Apply guards after structural matching

Full RETE network implementation may be added in future versions for incremental matching across ticks.

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

| Flags | Allowed In |
|-------|-----------|
| `pure` + `deterministic` | Everywhere (queries, guards, derived, rules) |
| `pure` + `nondeterministic` | Rule `:then` only (must use provided RNG) |
| `effectful` | Rule `:then` only |

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
    Option(Option<Box<Value>>),
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

### 9.8 File Format

MessagePack with structure:

```
{
  "version": "0.2",
  "tick": 42,
  "seed": 12345,
  "entities": { "42.3": { "position": {...}, ... }, ... },
  "relationships": { "follows": { "forward": {...}, "reverse": {...} }, ... },
  "rules": { ... },
  "world_entity": "1.1"
}
```

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

## Appendix B: Reserved Symbols

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

*End of Specification v0.3*
