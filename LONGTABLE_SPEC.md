# Longtable Specification v0.1

## 1. Overview & Goals

### 1.1 What is Longtable?

Longtable is a rule-based simulation engine combining:

- **LISP-like DSL** for defining components, rules, queries, and logic
- **Archetype-based Entity-Component-System (ECS)** for data organization
- **RETE-style rule engine** for reactive, incremental pattern matching
- **Persistent/functional data structures** enabling time-travel debugging
- **Tick-based discrete simulation** with transactional semantics

### 1.2 Design Philosophy

1. **Everything Is An Entity** - Rules, components schemas, relationships, and even the world itself are entities that can be queried and manipulated.

2. **World As Value** - The world state is immutable. Each tick produces a new world. This enables rollback, time-travel debugging, and "what-if" exploration.

3. **Effects, Not Mutations** - Rules don't mutate state directly. They produce effects that are collected, validated, and applied atomically.

4. **Explicit Over Implicit** - Strong typing (nil ≠ false), explicit optionality, declared component schemas.

5. **Observability From The Start** - Change tracking, rule tracing, and debugging primitives are core features, not afterthoughts.

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

### 2.2 Entity

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

### 2.3 Component

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

;; Boolean "tag" components
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

### 2.4 Archetype

An **Archetype** is the set of component types an entity has. Archetypes emerge implicitly from entity composition—they are not declared.

```
Entity A has: [Position, Velocity, Health]  -> Archetype 1
Entity B has: [Position, Velocity, Health]  -> Archetype 1 (same)
Entity C has: [Position, Health]            -> Archetype 2 (different)
```

Entities with the same archetype are stored together in Structure-of-Arrays (SoA) layout for cache efficiency:

```
Archetype 1:
  entity_ids:  [A, B, ...]
  positions:   [{x:0,y:0}, {x:5,y:3}, ...]
  velocities:  [{dx:1,dy:0}, {dx:0,dy:1}, ...]
  healths:     [{cur:100,max:100}, {cur:80,max:100}, ...]
```

When an entity gains or loses components, it moves to a different archetype.

### 2.5 Relationship

A **Relationship** is a typed, directional connection between entities. Relationships are declared with cardinality and cascade behavior:

```clojure
(relationship: follows
  :cardinality :many-to-many
  :on-target-delete :remove)

(relationship: parent
  :cardinality :one-to-many
  :on-target-delete :cascade)   ;; Destroying parent destroys children

(relationship: spouse
  :cardinality :one-to-one
  :on-target-delete :nullify
  :required false)              ;; Can be nil
```

Relationships are manipulated with `link!` and `unlink!`:

```clojure
(link! alice :follows bob)
(unlink! alice :follows bob)
```

**Relationship-as-Component**: Simple relationships are stored as component fields:

```clojure
[alice :follows bob]  ;; alice has a :follows component pointing to bob
```

**Relationship-as-Entity**: Complex relationships with attributes are full entities:

```clojure
(spawn! {:edge/follows true
         :edge/from alice
         :edge/to bob
         :follow/distance 5
         :follow/since tick-42})
```

The system maintains bidirectional indices automatically, so both forward traversal (`who does alice follow?`) and reverse traversal (`who follows bob?`) are O(1).

### 2.6 Rule

A **Rule** is a reactive unit of logic: a pattern (conditions) and a body (effects).

```clojure
(rule: apply-damage
  :salience 50                         ;; Priority (higher = earlier)
  ;; Pattern: match entities with these components
  [?target :health/current ?hp]
  [?target :incoming-damage ?dmg]
  ;; Guards: additional conditions
  [(> ?dmg 0)]
  =>
  ;; Body: effects to apply
  (set! ?target :health/current (- ?hp ?dmg))
  (destroy! (query-one [?d :incoming-damage ?dmg]
                       [?d :damage/target ?target]
                       => ?d)))
```

Rules are themselves entities with components:

```clojure
[rule-entity :rule/name "apply-damage"]
[rule-entity :rule/salience 50]
[rule-entity :rule/enabled true]
[rule-entity :rule/pattern <compiled-pattern>]
[rule-entity :rule/body <compiled-body>]
```

This means rules can be queried, enabled/disabled, and even created by other rules.

### 2.7 Derived Component

A **Derived Component** is a computed value that behaves like a component but is calculated from other components. Derived components are lazily evaluated and aggressively cached.

```clojure
(derived: combat/effective-damage
  [?e :combat/base-damage ?base]
  [?e :combat/damage-multiplier ?mult]
  => (* ?base ?mult))

(derived: health/percent
  [?e :health/current ?curr]
  [?e :health/max ?max]
  => (/ (* ?curr 100) ?max))
```

Derived components:
- Are read-only (cannot be `set!`)
- Invalidate when dependencies change
- Can be used in rule patterns and queries
- Track dependencies automatically

**Cycle Detection**: Cycles in derived component dependencies within a single tick are compile-time errors. Feedback loops across ticks (via regular components and rules) are allowed and powerful.

### 2.8 Constraint

A **Constraint** is an invariant that is checked after effects are applied. Violations trigger specified behaviors.

```clojure
(constraint: health-non-negative
  [?e :health/current ?hp]
  [(>= ?hp 0)]
  :on-violation :rollback)

(constraint: health-capped
  [?e :health/current ?hp]
  [?e :health/max ?max]
  [(<= ?hp ?max)]
  :on-violation :clamp)        ;; Automatically fix

(constraint: inventory-limit
  [?e :inventory/count ?c]
  [(<= ?c 100)]
  :on-violation :warn)         ;; Log warning, allow
```

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

### 3.2 Composite Types

| Type           | Description                   | Literal Examples            |
| -------------- | ----------------------------- | --------------------------- |
| `:vec<T>`      | Ordered sequence              | `[1 2 3]`, `["a" "b"]`      |
| `:set<T>`      | Unordered unique collection   | `#{1 2 3}`                  |
| `:map<K,V>`    | Key-value mapping             | `{:a 1 :b 2}`               |
| `:option<T>`   | Optional value (Some or None) | `(some 42)`, `none`         |
| `:result<T,E>` | Success or error              | `(ok 42)`, `(err "failed")` |

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
- Rule patterns are type-checked against component schemas

```clojure
(fn: add-ints :- :int [a :- :int, b :- :int]
  (+ a b))
```

Type errors are caught at load time, not runtime.

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

#### Tagged Literals (Extensible)

```clojure
#pos[10 20]         ;; Custom: expands to position
#rgb[255 128 0]     ;; Custom: expands to color
#dice"2d6+3"        ;; Custom: expands to dice roll spec
```

### 4.2 Special Forms

Special forms are built into the compiler and cannot be implemented as functions.

#### Binding & Definition

```clojure
(def name value)                    ;; Bind value to name
(def name :- type value)            ;; With type annotation

(fn: name [args...] body...)        ;; Define function
(fn: name :- ret-type [args...] body...)
(fn: ^:private helper [x] ...)      ;; Private function

(let [name value ...] body...)      ;; Local bindings
(let [{:keys [a b]} map] ...)       ;; Destructuring
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
  success-handler                   ;; (fn [result] ...)
  error-handler)                    ;; (fn [error] ...)
```

#### Quoting

```clojure
(quote x)           ;; Return x unevaluated
'x                  ;; Shorthand for (quote x)
`(a ~b ~@c)         ;; Syntax-quote with unquote/unquote-splicing
```

### 4.3 Declaration Forms

These declare structural elements of the world:

```clojure
;; World metadata
(world:
  :seed 12345
  :name "My Simulation")

;; Component schema
(component: name
  :field1 :type1
  :field2 :type2 :default value
  ...)

;; Relationship declaration
(relationship: name
  :cardinality :one-to-one|:one-to-many|:many-to-many
  :on-target-delete :remove|:cascade|:nullify
  :required true|false)

;; Rule declaration
(rule: name
  :salience number              ;; Optional, default 0
  :enabled true|false           ;; Optional, default true
  [pattern-clause...]           ;; Conditions
  [(guard-expression)]          ;; Guards
  =>
  effect-expressions...)        ;; Body

;; Derived component
(derived: name
  [pattern-clause...]
  => computation-expression)

;; Constraint
(constraint: name
  [pattern-clause...]
  [(guard-expression)]
  :on-violation :rollback|:clamp|:warn)
```

### 4.4 Module System

```clojure
;; File: game/combat.lt
(namespace game.combat
  (:require [game.core :as core]
            [game.utils :refer [distance clamp]]
            [game.items]))

;; Load from file
(load "path/to/file.lt")

;; Load from directory (loads _.lt)
(load "path/to/directory")
```

### 4.5 Standard Macros

Implemented in Longtable itself, shipped with stdlib:

```clojure
;; Conditionals
(when condition body...)            ;; if without else
(when-not condition body...)
(when-let [name expr] body...)      ;; bind + conditional
(if-let [name expr] then else)
(cond clause1 result1 clause2 result2 ... :else default)
(condp pred expr clause1 result1 ...)
(case expr val1 result1 val2 result2 ... default)

;; Boolean
(and a b c ...)                     ;; Short-circuiting
(or a b c ...)
(not x)

;; Threading
(-> x (f a) (g b) (h))              ;; Thread first: (h (g (f x a) b))
(->> x (f a) (g b) (h))             ;; Thread last:  (h (g (f a (f a x)) b))
(as-> x $ (f $ a) (g b $))          ;; Thread with placeholder
(some-> x f g h)                    ;; Thread, short-circuit on nil
(some->> x f g h)
(cond-> x c1 f1 c2 f2)              ;; Conditional threading

;; Iteration
(for [x coll, y other-coll, :when (pred x y)] body)  ;; List comprehension
(doseq [x coll] side-effect-body)   ;; Side-effecting iteration
```

### 4.6 Standard Library Functions

#### Collections

```clojure
;; Basics
(count coll)
(empty? coll)
(first coll) (rest coll) (last coll)
(nth coll index) (nth coll index default)
(get coll key) (get coll key default)
(contains? coll key)
(conj coll item) (cons item coll)
(assoc map key val) (dissoc map key)
(merge map1 map2)

;; Transformations
(map f coll)
(filter pred coll)
(remove pred coll)
(reduce f init coll)
(reduce f coll)                     ;; First element as init
(fold f init coll)                  ;; Alias for reduce

;; Advanced
(take n coll)
(drop n coll)
(take-while pred coll)
(drop-while pred coll)
(partition n coll)
(partition-by f coll)
(group-by f coll)
(sort coll)
(sort-by f coll)
(reverse coll)
(flatten coll)
(distinct coll)
(dedupe coll)
(interleave coll1 coll2)
(interpose sep coll)
(zip coll1 coll2)
(zip-with f coll1 coll2)
(concat coll1 coll2 ...)

;; Predicates
(every? pred coll)
(some pred coll)
(not-any? pred coll)
(not-every? pred coll)

;; Construction
(range end)
(range start end)
(range start end step)
(repeat n x)
(repeatedly n f)
(vec coll)
(set coll)
(into to-coll from-coll)
```

#### Math

```clojure
;; Arithmetic
(+ a b ...) (- a b ...) (* a b ...) (/ a b ...)
(mod a b) (rem a b)
(inc x) (dec x)
(abs x) (neg x)

;; Comparison
(< a b ...) (<= a b ...) (> a b ...) (>= a b ...)
(= a b ...) (!= a b)
(min a b ...) (max a b ...)
(clamp x low high)

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

;; Vector Math (2D/3D)
(vec+ v1 v2) (vec- v1 v2)
(vec* v scalar) (vec-scale v scalar)
(vec-dot v1 v2)
(vec-cross v1 v2)               ;; 3D only
(vec-length v) (vec-length-sq v)
(vec-normalize v)
(vec-distance v1 v2)
(vec-lerp v1 v2 t)
(vec-angle v1 v2)
```

#### Strings

```clojure
(str a b c ...)                 ;; Concatenation
(str/length s)
(str/substring s start end)
(str/split s delimiter)
(str/join delimiter coll)
(str/trim s) (str/trim-left s) (str/trim-right s)
(str/lower s) (str/upper s)
(str/starts-with? s prefix)
(str/ends-with? s suffix)
(str/contains? s substring)
(str/replace s old new)
(str/replace-all s old new)
(str/blank? s)
(format "template {} {}" arg1 arg2)
```

#### Predicates

```clojure
(nil? x) (some? x)              ;; nil checks
(bool? x) (int? x) (float? x) (number? x)
(string? x) (keyword? x) (symbol? x)
(vec? x) (set? x) (map? x) (coll? x)
(fn? x)
(entity? x)
```

---

## 5. Rule Engine

### 5.1 Pattern Syntax

Patterns match entities and bind variables:

```clojure
;; Basic: entity has component
[?e :health/current ?hp]

;; Specific value
[?e :tag/player true]

;; Wildcard (match any value)
[?e :position _]

;; Multiple patterns (implicit join on ?e)
[?e :position ?pos]
[?e :velocity ?vel]
[?e :health/current ?hp]

;; Cross-entity patterns
[?a :follows ?b]
[?b :name ?name]

;; Collection into variable (all matches)
[?enemies <- (collect [?e :tag/enemy])]

;; Aggregation
[(count [?e :tag/enemy]) ?enemy-count]
[(sum [?e :damage/amount]) ?total-damage]
```

### 5.2 Guards

Guards are arbitrary boolean expressions:

```clojure
(rule: low-health-warning
  [?e :health/current ?hp]
  [?e :health/max ?max]
  [(< ?hp (* max 0.2))]         ;; hp < 20% of max
  [(not (get ?e :warned))]      ;; not already warned
  =>
  ...)
```

### 5.3 Negation

Match on absence of components or patterns:

```clojure
(rule: stationary-entities
  [?e :position _]
  (not [?e :velocity _])        ;; Has position but NOT velocity
  =>
  ...)

(rule: lonely-entities
  [?e :social true]
  (not [?other :follows ?e])    ;; Nobody follows this entity
  =>
  ...)
```

### 5.4 Previous Tick Access

Access values from the previous tick:

```clojure
(rule: detect-damage
  [?e :health/current ?hp]
  [(< ?hp (prev ?e :health/current))]  ;; Health decreased
  =>
  (emit! {:event/type :damage-taken
          :event/entity ?e
          :event/amount (- (prev ?e :health/current) ?hp)}))
```

### 5.5 Conflict Resolution

When multiple rules can fire, they are ordered by:

1. **Salience** (higher fires first)
2. **Specificity** (more conditions = more specific)
3. **Declaration order** (earlier in source fires first)

```clojure
(rule: generic-damage
  :salience 0
  [?e :incoming-damage ?d]
  => ...)

(rule: critical-damage
  :salience 10                  ;; Fires before generic
  [?e :incoming-damage ?d]
  [(> ?d 50)]                   ;; More specific
  => ...)
```

### 5.6 Activation & Execution Model

Each tick follows this model:

```
TICK N
├── 1. ACTIVATION PHASE
│   ├── Evaluate all enabled rules against current world state
│   └── Build "activation set" of rules that match
│
├── 2. EXECUTION PHASE (in priority order)
│   ├── For each rule in activation set:
│   │   ├── Re-query matches (sees changes from earlier rules)
│   │   ├── Execute rule body with all current matches
│   │   └── Apply effects to world state immediately
│   │
│   └── Rules NOT in original activation set don't fire,
│       even if changes would now make them match
│
├── 3. CONSTRAINT CHECK
│   ├── Evaluate all constraints
│   └── Handle violations (rollback/clamp/warn)
│
└── 4. COMMIT PHASE
    ├── Flush output buffer
    ├── world_previous = world_current
    └── tick++
```

Key properties:
- **Deterministic**: Same inputs → same outputs
- **No infinite loops**: Activation set is fixed at tick start
- **Immediate visibility**: Effects visible to subsequent rules in same tick
- **Atomic rollback**: Any error rolls back entire tick

### 5.7 Effects

Rules produce effects (they don't mutate directly):

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
(print! (format "Entity {} has {} hp" ?e ?hp))

;; Meta (take effect next tick)
(enable-rule! rule-entity)
(disable-rule! rule-entity)
```

Within a rule's body, effects apply immediately (visible to subsequent expressions):

```clojure
(rule: example
  [?e :value ?v]
  =>
  (set! ?e :value (+ ?v 1))
  (print! (get ?e :value)))     ;; Prints v+1, not v
```

---

## 6. Query System

### 6.1 Query Syntax

Queries use the same pattern syntax as rules:

```clojure
;; Find all enemies with low health
(query
  [?e :tag/enemy]
  [?e :health/current ?hp]
  [(< ?hp 20)]
  => {:entity ?e :hp ?hp})

;; Find single result
(query-one [?e :tag/player] => ?e)

;; Count matches
(query-count [?e :tag/enemy])

;; Check existence
(query-exists? [?e :tag/player])
```

### 6.2 Relationship Traversal

```clojure
;; Forward: who does alice follow?
(query [alice :follows ?target] => ?target)

;; Reverse: who follows alice?
(query [?follower :follows alice] => ?follower)

;; Multi-hop: friends of friends
(query
  [?me :tag/player]
  [?me :follows ?friend]
  [?friend :follows ?fof]
  [(!= ?fof ?me)]
  => ?fof)
```

### 6.3 Aggregations

```clojure
;; Count
(query-count [?e :tag/enemy])

;; Sum
(query
  [(sum [?e :value ?v]) ?total]
  => ?total)

;; Group by
(query
  [?e :faction ?f]
  [?e :power ?p]
  :group-by ?f
  :aggregate {:total-power (sum ?p)
              :count (count)}
  => {:faction ?f :total-power ?total-power :count ?count})
```

### 6.4 Query Planning

The engine optimizes queries automatically:

```clojure
(explain-query
  [?e :tag/enemy]
  [?e :position ?pos]
  [(< (:x ?pos) 100)])

;; Output:
;; 1. Scan index :tag/enemy (est. 50 entities)
;; 2. Lookup :position by entity-id (50 lookups)
;; 3. Filter by (:x ?pos) < 100 (est. 25 results)
;; Total estimated cost: 100 operations
```

---

## 7. Tick Lifecycle

### 7.1 Input Processing

Inputs enter the world as entities with special components:

```clojure
;; Before tick, runtime creates:
(spawn! {:input/raw "go north"
         :input/tick current-tick
         :input/source :player})
```

Rules process inputs like any other entity:

```clojure
(rule: parse-command
  :salience 100                 ;; Process early
  [?input :input/raw ?text]
  =>
  (let [parsed (parse-command ?text)]
    (spawn! {:command/type (:type parsed)
             :command/args (:args parsed)
             :command/source ?input})
    (destroy! ?input)))
```

### 7.2 REPL Integration

A low-salience "catch-all" rule can implement REPL evaluation:

```clojure
(rule: repl-eval
  :salience -1000               ;; Very low priority
  [?input :input/raw ?text]
  [(str/starts-with? ?text "(")]  ;; Looks like S-expression
  =>
  (let [result (eval (read-string ?text))]
    (print! (str "=> " result))
    (destroy! ?input)))
```

### 7.3 World Singleton

A singleton entity holds global world state:

```clojure
;; Automatically maintained:
[?world :world/tick ?t]
[?world :world/seed ?seed]
[?world :world/initialized true]

;; User-defined globals:
[?world :game/mode :exploration]
[?world :game/difficulty :hard]
```

### 7.4 Error Handling

```clojure
;; Try/catch within rules
(rule: safe-division
  [?e :dividend ?a]
  [?e :divisor ?b]
  =>
  (try
    (set! ?e :result (/ ?a ?b))
    (fn: [result] (print! (str "Result: " result)))
    (fn: [error] (print! (str "Error: " error)))))

;; Uncaught errors roll back the tick
(rule: dangerous-rule
  [?e :value ?v]
  =>
  (/ ?v 0))   ;; Division by zero → tick rollback
```

Error messages include full context:

```
Error during tick 42:
  Rule: dangerous-rule (defined at game/rules.lt:15)
  Pattern match: {?e: Entity(123), ?v: 100}
  Expression: (/ ?v 0)
  Cause: Division by zero

  Stack trace:
    game/rules.lt:17 (/ ?v 0)
    game/rules.lt:15 (rule: dangerous-rule ...)
```

### 7.5 Constraint Enforcement

After all rules execute, constraints are checked:

```clojure
(constraint: health-bounds
  [?e :health/current ?hp]
  [?e :health/max ?max]
  [(and (>= ?hp 0) (<= ?hp ?max))]
  :on-violation :clamp)         ;; Auto-fix: clamp to [0, max]
```

Violation behaviors:
- `:rollback` - Entire tick fails, world unchanged
- `:clamp` - Automatically adjust to satisfy constraint
- `:warn` - Log warning, allow violation

---

## 8. Observability & Debugging

### 8.1 Change Tracking

Changes are tracked at component-field granularity:

```clojure
;; Query what changed
(query-changes :health/current)
;; => [{:entity E1 :old 100 :new 75} {:entity E2 :old 50 :new 0}]

;; Meaningful change detection: $45 → $45 is NOT a change
;; even if recalculated through complex logic
```

### 8.2 Tracing

```clojure
;; Trace rule firings
(trace :rule/apply-damage)
;; Tick 42: apply-damage fired
;;   Matches: [{?e: Entity(5), ?dmg: 25}]
;;   Effects: (set! Entity(5) :health/current 75)

;; Trace entity changes
(trace-entity entity-42)
;; Tick 42: Entity(42) :position changed {x:0,y:0} → {x:1,y:0}
;; Tick 42: Entity(42) :velocity unchanged

(untrace :rule/apply-damage)
```

### 8.3 Explain

```clojure
;; Explain how a value was computed
(why (get entity-42 :health/current))
;; Value: 75
;; Computed by: rule apply-damage at tick 42
;; Inputs:
;;   :health/current was 100
;;   :incoming-damage was 25
;; Expression: (- ?hp ?dmg) = (- 100 25) = 75

;; Explain derived component
(why (get entity-42 :combat/effective-damage))
;; Value: 50
;; Derived from:
;;   :combat/base-damage = 25
;;   :combat/damage-multiplier = 2.0
;; Expression: (* ?base ?mult) = (* 25 2.0) = 50
```

### 8.4 Breakpoints

```clojure
;; Break when rule fires
(break-on :rule/apply-damage)

;; Break when component changes
(break-on :health/current)

;; Break when entity changes
(break-on entity-42)

;; Conditional breakpoint
(break-on :health/current :when (fn: [old new] (= new 0)))

;; In debugger:
(step)          ;; Execute one rule
(continue)      ;; Run to next breakpoint or tick end
(inspect ?e)    ;; Show entity state
(locals)        ;; Show rule bindings
```

### 8.5 Time Travel

```clojure
;; Navigate history
(rollback! 5)           ;; Go back 5 ticks
(advance! 3)            ;; Go forward 3 ticks (if in history)
(goto-tick! 42)         ;; Jump to specific tick

;; Explore alternatives
(let [alt-world (-> (current-world)
                    (with-hypothetical {:entity-5 :health/current 200})
                    (tick []))]
  (compare-worlds (current-world) alt-world))

;; Timeline branching
(branch! "what-if-no-damage")
;; Now on branch "what-if-no-damage"
;; Changes don't affect main timeline
(switch-branch! "main")
```

### 8.6 REPL Commands

```
> (inspect entity-42)
Entity(42) [Archetype: Position, Velocity, Health]
  :position {:x 10.0 :y 20.0 :z 0.0}
  :velocity {:dx 1.0 :dy 0.0 :dz 0.0}
  :health {:current 75 :max 100 :regen-rate 0.5}

> (inspect :rule/apply-damage)
Rule: apply-damage
  Salience: 50
  Enabled: true
  Pattern: [?target :health/current ?hp] [?target :incoming-damage ?dmg]
  Last fired: tick 42 (3 times)

> (tick!)
Tick 43 completed.
  Rules fired: 12
  Entities changed: 8
  Time: 234ms

> (save! "checkpoint.lt")
World saved to checkpoint.lt

> (load! "checkpoint.lt")
World loaded from checkpoint.lt (tick 43)
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
│  - Input ingestion                                           │
│  - Activation set computation                                │
│  - Rule execution orchestration                              │
│  - Effect application                                        │
│  - Constraint checking                                       │
│  - Output buffer flush                                       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      Rule Engine                             │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────────┐  │
│  │ RETE Network│  │ Query Planner│  │ Derived Component │  │
│  │  (matching) │  │  (optimize)  │  │    Evaluator      │  │
│  └──────┬──────┘  └──────┬───────┘  └────────┬──────────┘  │
│         │                │                   │              │
│         └────────────────┼───────────────────┘              │
│                          ▼                                   │
│              ┌───────────────────┐                          │
│              │   Bytecode VM     │                          │
│              │  (expressions)    │                          │
│              └───────────────────┘                          │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    World (Immutable)                         │
│  ┌────────────────────────────────────────────────────────┐ │
│  │                    Archetypes                           │ │
│  │   [Position, Velocity]    [Position, Velocity, Health] │ │
│  │    SoA storage             SoA storage                 │ │
│  └────────────────────────────────────────────────────────┘ │
│  ┌─────────────────┐ ┌─────────────────┐ ┌───────────────┐ │
│  │  Relationships  │ │     Indices     │ │ Derived Cache │ │
│  └─────────────────┘ └─────────────────┘ └───────────────┘ │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              Persistent Data Structures                      │
│  HAMT (maps), RRB-Vector (sequences), structural sharing    │
└─────────────────────────────────────────────────────────────┘
```

### 9.2 RETE Network

The RETE algorithm provides incremental pattern matching:

**Alpha Network**: Tests individual patterns
- One alpha node per pattern clause
- Filters entities matching the pattern
- Stores matches in alpha memory

**Beta Network**: Joins across patterns
- Beta nodes combine results from alpha nodes
- Performs variable unification
- Stores partial matches in beta memory

**Production Nodes**: Rule firing
- Terminal nodes for complete matches
- Trigger rule activation

**Incremental Updates**:
- When components change, only affected alpha/beta nodes update
- Most ticks only touch a small fraction of the network

### 9.3 Bytecode VM

The VM executes compiled expressions:

**Compilation**: DSL → AST → Bytecode

**Instruction Categories**:
- Stack operations: `PUSH`, `POP`, `DUP`
- Arithmetic: `ADD`, `SUB`, `MUL`, `DIV`, `MOD`
- Comparison: `LT`, `LE`, `GT`, `GE`, `EQ`, `NE`
- Control flow: `JUMP`, `JUMP_IF`, `CALL`, `RETURN`
- Data access: `GET_COMPONENT`, `GET_FIELD`, `GET_LOCAL`
- Effects: `SPAWN`, `DESTROY`, `SET`, `UPDATE`

**Optimization**:
- Constant folding
- Dead code elimination
- Inline caching for component access

### 9.4 Persistent Data Structures

**HAMT (Hash Array Mapped Trie)**: For maps
- O(log32 n) ≈ O(1) for practical sizes
- Structural sharing on updates
- Used for: entity→components, component→entities, world state

**RRB-Vector (Relaxed Radix Balanced)**: For sequences
- O(log n) random access
- O(1) amortized append
- Efficient concatenation and slicing
- Used for: entity lists in archetypes, event queues

**Structural Sharing**:
```
world_tick_41 ──┬── entities ─── shared ───┐
                │                          │
world_tick_42 ──┴── entities ─── [new] ────┘
                                   │
                         (only changed entities)
```

### 9.5 Storage Layout

**Archetype SoA Storage**:
```
Archetype [Position, Velocity, Health]:
  entity_ids: [E1, E2, E3, E4, ...]      // Dense array
  positions:  [P1, P2, P3, P4, ...]      // Parallel array
  velocities: [V1, V2, V3, V4, ...]      // Parallel array
  healths:    [H1, H2, H3, H4, ...]      // Parallel array
```

Benefits:
- Cache-friendly iteration over single component type
- Good for rules that process many entities with same components

**Indices**:
- `EntityId → ArchetypeId, row` (find entity's data)
- `ComponentType → Set<EntityId>` (find entities with component)
- `Relationship → (forward: Map, reverse: Map)` (bidirectional traversal)
- Optional: field value → entities (for filtered queries)

### 9.6 RNG Design

```
World Seed
    │
    ├── Tick N Seed = hash(world_seed, N)
    │   │
    │   ├── Rule "foo" Seed = hash(tick_seed, "foo")
    │   │   │
    │   │   └── Entity E Seed = hash(rule_seed, E.id)
    │   │
    │   └── Rule "bar" Seed = hash(tick_seed, "bar")
    │
    └── Tick N+1 Seed = hash(world_seed, N+1)
```

Properties:
- Deterministic within a run
- Rules don't affect each other's RNG
- Replayable from any tick with same seed

### 9.7 Rust API

```rust
/// Opaque world handle
pub struct World { /* ... */ }

/// Generational entity identifier
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct EntityId {
    index: u64,
    generation: u32,
}

/// Dynamic value type
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(Arc<str>),
    Symbol(SymbolId),
    Keyword(KeywordId),
    EntityRef(EntityId),
    Vec(Arc<Vector<Value>>),
    Set(Arc<HashSet<Value>>),
    Map(Arc<HashMap<Value, Value>>),
    Option(Option<Box<Value>>),
    Result(Result<Box<Value>, Box<Value>>),
}

impl World {
    /// Create a new world with the given seed
    pub fn new(seed: u64) -> Result<World, Error>;

    /// Load and execute DSL source code
    pub fn load(&mut self, source: &str) -> Result<(), Error>;

    /// Load from file
    pub fn load_file(&mut self, path: &Path) -> Result<(), Error>;

    /// Execute one tick with the given inputs
    pub fn tick(&self, inputs: Vec<Value>) -> Result<World, TickError>;

    /// Execute a query, returning results
    pub fn query(&self, query: &str) -> Result<Vec<Value>, Error>;

    /// Get a component value from an entity
    pub fn get(&self, entity: EntityId, component: &str) -> Result<Value, Error>;

    /// Get current tick number
    pub fn current_tick(&self) -> u64;

    /// Access previous world state (for time travel)
    pub fn previous(&self) -> Option<&World>;

    /// Serialize world to bytes
    pub fn save(&self) -> Result<Vec<u8>, Error>;

    /// Deserialize world from bytes
    pub fn restore(bytes: &[u8]) -> Result<World, Error>;
}

/// Register a native function callable from DSL
#[longtable::native_fn]
fn my_pathfind(from: Value, to: Value) -> Result<Value, Error> {
    // Implementation
}
```

### 9.8 File Format

World saves use MessagePack with this structure:

```
{
  "version": "0.1",
  "tick": 42,
  "seed": 12345,
  "entities": {
    "42.3": {                    // EntityId as string
      "position": {"x": 10.0, "y": 20.0},
      "health": {"current": 75, "max": 100},
      ...
    },
    ...
  },
  "relationships": {
    "follows": {
      "forward": {"42.3": ["55.1", "60.2"]},
      "reverse": {"55.1": ["42.3"], "60.2": ["42.3"]}
    },
    ...
  },
  "rules": { ... },             // Serialized rule definitions
  "world_entity": "1.1"         // ID of world singleton
}
```

---

## Appendix A: Grammar (EBNF)

```ebnf
program     = form* ;

form        = literal
            | symbol
            | keyword
            | list
            | vector
            | set
            | map
            | tagged ;

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

comment     = ";" [^\n]* "\n"
            | "#_" form ;
```

---

## Appendix B: Reserved Symbols

The following symbols have special meaning and cannot be redefined:

```
;; Special forms
def fn: let if do match loop recur try quote

;; Declaration forms
world: component: relationship: derived: rule: constraint:

;; Module system
namespace load

;; Constants
nil true false none

;; Built-in operators
=> <- _
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

;; Core components
(component: position
  :x :int
  :y :int)

(component: description
  :short :string
  :long :string :default "")

(component: name
  :value :string)

;; Tags
(component: tag/player :bool :default true)
(component: tag/room :bool :default true)
(component: tag/item :bool :default true)
(component: tag/npc :bool :default true)

;; Relationships
(relationship: in-room
  :cardinality :many-to-one
  :on-target-delete :cascade)

(relationship: contains
  :cardinality :one-to-many
  :on-target-delete :remove)

;; Game state
(component: game/turns :int :default 0)
(component: game/score :int :default 0)


;; === File: rules.lt ===
(namespace adventure.rules
  (:require [adventure.components]))

;; Process movement commands
(rule: handle-go
  :salience 100
  [?cmd :command/type :go]
  [?cmd :command/direction ?dir]
  [?player :tag/player]
  [?player :in-room ?current-room]
  [?current-room :exits ?exits]
  =>
  (if-let [next-room (get ?exits ?dir)]
    (do
      (unlink! ?player :in-room ?current-room)
      (link! ?player :in-room next-room)
      (print! (get next-room :description/short)))
    (print! "You can't go that way."))
  (destroy! ?cmd))

;; Look command
(rule: handle-look
  :salience 100
  [?cmd :command/type :look]
  [?player :tag/player]
  [?player :in-room ?room]
  =>
  (print! (get ?room :description/long))
  (let [items (query [?i :tag/item] [?i :in-room ?room] => ?i)]
    (when (not (empty? items))
      (print! "You see:")
      (doseq [item items]
        (print! (str "  - " (get item :name/value))))))
  (destroy! ?cmd))

;; Increment turn counter
(rule: tick-turn
  :salience -100
  [?world :game/turns ?t]
  [_ :command/type _]           ;; Any command was processed
  =>
  (update! ?world :game/turns inc))


;; === File: data/initial-world.lt ===
(namespace adventure.data)

;; Bootstrap rule - runs once
(rule: bootstrap
  :salience 1000
  (not [_ :world/initialized])
  =>
  ;; Create rooms
  (let [entrance (spawn! {:tag/room true
                          :name/value "Cave Entrance"
                          :description/short "A dark cave entrance."
                          :description/long "You stand at the mouth of a dark cave. Cold air flows from within."
                          :exits {:north nil}})  ;; Will link to next room

        main-hall (spawn! {:tag/room true
                           :name/value "Main Hall"
                           :description/short "A vast underground hall."
                           :description/long "An enormous cavern stretches before you. Stalactites hang from the ceiling."
                           :exits {:south nil}})]

    ;; Link rooms
    (set! entrance :exits {:north main-hall})
    (set! main-hall :exits {:south entrance})

    ;; Create player
    (let [player (spawn! {:tag/player true
                          :name/value "Adventurer"})]
      (link! player :in-room entrance))

    ;; Create an item
    (let [lantern (spawn! {:tag/item true
                           :name/value "brass lantern"
                           :description/short "A battered brass lantern."})]
      (link! lantern :in-room entrance))

    ;; Mark initialized
    (spawn! {:world/initialized true
             :game/turns 0
             :game/score 0})

    (print! "Welcome to THE DARK CAVE")
    (print! "")
    (print! (get entrance :description/long))))
```

---

*End of Specification*
