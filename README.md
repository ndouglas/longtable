# Longtable

A rule-based simulation engine combining a LISP-like DSL with an archetype-based ECS.

## Overview

Longtable is designed for text-based simulation games where complex emergent behavior arises from simple, declarative rules. Think "Zork meets Dwarf Fortress" — rich world simulation driven by pattern-matching rules rather than imperative scripts.

### Key Features

- **Persistent World State** — Immutable snapshots with structural sharing enable time travel, speculation, and deterministic replay
- **Pattern-Matching Rules** — Declarative rules fire when patterns match, with automatic refraction to prevent infinite loops
- **Entity-Component-Relationship** — Archetype-based ECS with first-class relationships and typed schemas
- **LISP-like DSL** — Homoiconic syntax with macros for domain-specific abstractions
- **Derived Components** — Computed values with automatic cache invalidation
- **Constraint Checking** — Invariants validated after each tick with rollback support
- **Time Travel Debugging** — Git-like branching, rollback, and world state diffing
- **Observability** — Tracing, breakpoints, watches, and causal "why" queries
- **125+ Native Functions** — Comprehensive standard library for collections, math, strings, and more

## Status

**Phase 6: Observability Implementation** — Full debugging, tracing, and time travel complete.

- Phases 0-5 complete: Foundation, storage, language, execution engine, interface
- **Observability**: Explain system, tracing, breakpoints, watches, time travel
- Standard library: 125+ native functions (collections, math, strings, predicates, vector math)
- REPL with syntax highlighting, tab completion, and multi-line input
- CLI with debug flags and batch mode
- MessagePack serialization for world state

See [IMPLEMENTATION.md](IMPLEMENTATION.md) for the full development roadmap.

## Quick Start

### Installation

```bash
git clone https://github.com/ndouglas/longtable.git
cd longtable
cargo build --release
```

### Running the REPL

```bash
cargo run --release
```

Or with files:

```bash
cargo run --release -- examples/adventure/_.lt
```

### Example Session

```clojure
;; Basic arithmetic
> (+ 1 2 3)
6

;; Collections
> (map (fn [x] (* x 2)) [1 2 3 4 5])
[2 4 6 8 10]

> (filter (fn [x] (> x 2)) [1 2 3 4 5])
[3 4 5]

> (reduce (fn [acc x] (+ acc x)) 0 [1 2 3 4 5])
15

;; String operations
> (str/upper "hello world")
"HELLO WORLD"

> (str/split "a,b,c" ",")
["a" "b" "c"]

;; Math functions
> (sin (/ pi 2))
1.0

> (sqrt 16)
4.0

;; Vector math (for simulations)
> (vec+ [1 2 3] [4 5 6])
[5.0 7.0 9.0]

> (vec-normalize [3 4])
[0.6 0.8]
```

## CLI Usage

```bash
longtable [OPTIONS] [FILES...]

OPTIONS:
    -h, --help         Print help information
    -V, --version      Print version information
    -b, --batch        Load files and exit (no REPL)

DEBUG OPTIONS:
    --trace            Enable rule tracing output
    --trace-vm         Enable VM instruction tracing
    --trace-match      Enable pattern match tracing
    --max-ticks N      Limit ticks before exit (for testing)
    --dump-world       Dump world state after loading files

EXAMPLES:
    longtable                        Start interactive REPL
    longtable world.lt               Load world.lt, then start REPL
    longtable -b test.lt             Load test.lt and exit
    longtable --trace -b sim.lt      Run with rule tracing
```

## REPL Commands

```clojure
;; Basic commands
(def name value)       ;; Define a session variable
(load "path")          ;; Load a .lt file
(save! "path")         ;; Save world state to file
(load-world! "path")   ;; Load world state from file
(tick!)                ;; Advance simulation by one tick
(inspect entity)       ;; Inspect an entity's details

;; Explain system
(why entity :component)           ;; Why does entity have this value?
(why entity :component :depth 5)  ;; Multi-hop causal chain
(explain-query (query ...))       ;; Explain query execution

;; Debugging
(break :rule foo)                 ;; Breakpoint on rule
(break :entity ?e :component :hp) ;; Breakpoint on component access
(watch (get ?e :health))          ;; Add watch expression
(continue)                        ;; Resume execution
(step-rule)                       ;; Step to next rule

;; Tracing
(trace!)                          ;; Enable tracing
(trace-off!)                      ;; Disable tracing
(get-traces)                      ;; Get trace buffer

;; Time travel
(rollback! 5)                     ;; Go back 5 ticks
(goto-tick! 42)                   ;; Jump to tick 42
(branch! "experiment")            ;; Create branch at current tick
(checkout! "main")                ;; Switch to branch
(branches)                        ;; List all branches
(merge! "experiment")             ;; Merge branch into current
(diff 40 42)                      ;; Compare two ticks
(history)                         ;; Show recent history
(timeline)                        ;; Show timeline status
```

Keyboard shortcuts:
- `Ctrl+D` — Exit REPL
- `Ctrl+C` — Cancel current input
- `Tab` — Autocomplete keywords

## Crate Structure

```
longtable_foundation  — Core types, values, persistent collections
longtable_storage     — Entity-component storage, relationships, world state
longtable_language    — Lexer, parser, compiler, bytecode VM
longtable_engine      — Rule engine, pattern matching, queries, constraints
longtable_stdlib      — Standard library functions
longtable_runtime     — REPL, CLI, serialization
longtable_debug       — Tracing, debugging, time travel
```

### Layer Dependencies

```
Layer 5: longtable_debug     — Tracing, debugging, time travel
Layer 4: longtable_runtime   — REPL, CLI, serialization
         longtable_stdlib    — Standard library functions
Layer 3: longtable_engine    — Rule engine, pattern matching, queries
Layer 2: longtable_language  — Lexer, parser, compiler, bytecode VM
Layer 1: longtable_storage   — Entity-component storage, world state
Layer 0: longtable_foundation — Core types, persistent collections
```

## Standard Library

### Collections
`map`, `filter`, `reduce`, `first`, `rest`, `last`, `nth`, `count`, `empty?`, `conj`, `cons`, `concat`, `reverse`, `sort`, `sort-by`, `take`, `drop`, `take-while`, `drop-while`, `partition`, `group-by`, `flatten`, `distinct`, `dedupe`, `interleave`, `interpose`, `zip`, `zip-with`, `repeat`, `range`, `into`, `vec`, `set`, `keys`, `vals`, `get`, `assoc`, `dissoc`, `merge`, `contains?`, `every?`, `some`, `not-any?`, `not-every?`, `remove`

### Math
`+`, `-`, `*`, `/`, `mod`, `rem`, `abs`, `neg`, `inc`, `dec`, `min`, `max`, `clamp`, `floor`, `ceil`, `round`, `trunc`, `sqrt`, `cbrt`, `pow`, `exp`, `log`, `log10`, `log2`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `sinh`, `cosh`, `tanh`, `pi`, `e`, `rand`, `rand-int`

### Vector Math
`vec+`, `vec-`, `vec*`, `vec-scale`, `vec-dot`, `vec-cross`, `vec-length`, `vec-length-sq`, `vec-normalize`, `vec-distance`, `vec-lerp`, `vec-angle`

### Strings
`str`, `str/len`, `str/upper`, `str/lower`, `str/trim`, `str/trim-left`, `str/trim-right`, `str/split`, `str/join`, `str/replace`, `str/replace-all`, `str/starts-with?`, `str/ends-with?`, `str/contains?`, `str/blank?`, `str/substring`, `format`

### Predicates
`nil?`, `some?`, `int?`, `float?`, `string?`, `keyword?`, `symbol?`, `bool?`, `number?`, `list?`, `vector?`, `map?`, `set?`, `coll?`, `fn?`, `entity?`, `type`

### Logic
`=`, `!=`, `<`, `<=`, `>`, `>=`, `not`, `and`, `or`, `if`, `when`, `cond`

## Building

```bash
cargo build                           # Build all crates
cargo test                            # Run all tests (~1000 tests)
cargo bench                           # Run benchmarks
cargo clippy --all-targets            # Lint
cargo +nightly fmt --all              # Format (requires nightly)
cargo doc --no-deps --open            # Generate documentation
```

Requires Rust 1.85.0 or later (Edition 2024).

## Performance

Benchmark highlights (M1 Mac):

| Operation | Time | Notes |
|-----------|------|-------|
| VM simple op | ~500 ns | 2M ops/sec |
| Function call | ~1 µs | Per call overhead |
| Pattern match | ~100-230 ns | Per pattern |
| Component get | 66 ns | O(1) lookup |
| World clone | ~50 ns | Structural sharing |
| Entity spawn | ~480 ns | With components |

See `cargo bench` for full benchmark suite.

## License

This project is released into the public domain under the [Unlicense](LICENSE).
