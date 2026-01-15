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

## Quick Start

### Installation

```bash
git clone https://github.com/ndouglas/longtable.git
cd longtable
cargo build --release
```

### Running the REPL

```bash
./target/release/longtable
```

Or with files:

```bash
./target/release/longtable examples/adventure/_.lt
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

### Sudoku Solver Demo

The `examples/sudoku/` directory contains a complete constraint-propagation Sudoku solver implemented in Longtable's DSL:

```bash
./target/release/longtable examples/sudoku/_.lt
```

```
> (load-puzzle (medium-1))
> (print-grid)
+-------+-------+-------+
| . . . | 2 6 . | 7 . 1 |
| 6 8 . | . 7 . | . 9 . |
| 1 9 . | . . 4 | 5 . . |
+-------+-------+-------+
| 8 2 . | 1 . . | . 4 . |
| . . 4 | 6 . 2 | 9 . . |
| . 5 . | . . 3 | . 2 8 |
+-------+-------+-------+
| . . 9 | 3 . . | . 7 4 |
| . 4 . | . 5 . | . 3 6 |
| 7 . 3 | . 1 8 | . . . |
+-------+-------+-------+

> (solve)
Naked single: R5C1 = 3
Naked single: R6C1 = 9
Naked single: R1C8 = 8
... (45 placements)

+-------+-------+-------+
| 4 3 5 | 2 6 9 | 7 8 1 |
| 6 8 2 | 5 7 1 | 4 9 3 |
| 1 9 7 | 8 3 4 | 5 6 2 |
+-------+-------+-------+
| 8 2 6 | 1 9 5 | 3 4 7 |
| 3 7 4 | 6 8 2 | 9 1 5 |
| 9 5 1 | 7 4 3 | 6 2 8 |
+-------+-------+-------+
| 5 1 9 | 3 2 6 | 8 7 4 |
| 2 4 8 | 9 5 7 | 1 3 6 |
| 7 6 3 | 4 1 8 | 2 5 9 |
+-------+-------+-------+

Puzzle solved
```

The solver demonstrates:
- **Entity-Component architecture**: 81 cells with `:position`, `:value`, `:candidates` components
- **Constraint propagation**: Naked singles, hidden singles, X-Wing, Swordfish, XY-Wing
- **Backtracking with state save/restore**: For puzzles requiring guessing
- **Declarative logic**: Pure functional implementation in ~600 lines of DSL code

### Logic Grid Puzzle Solver

The `examples/logic-grid/` directory contains a constraint-satisfaction solver for logic grid puzzles (the kind where you match items across categories using clues).

**Puzzle credit**: The included "Minnetonka Manatee Company" puzzle is from [PuzzleBaron's Logic Puzzles](https://logic.puzzlebaron.com/).

```bash
./target/release/longtable examples/logic-grid/_.lt
```

```
> (solve-manatee-puzzle!)

============================================================
       MINNETONKA MANATEE COMPANY LOGIC PUZZLE
============================================================
Setting up logic grid...
Grid setup queued (588 cells).

Applying basic exclusion clues...
  Clue 2: Sea Cow != Silver Springs
  Clue 3: Rainbow Reef != Jacobson
  Clue 5: Mellow Mel != 3 manatees
  Clue 8: Samantha != 4, Samantha != Silver Springs

Basic clues applied. Running propagation...

=== Applying deduced constraints ===

Clue 10 -> Hollow Hole = 7 manatees, Benny II = Romero = 9
  Solved: :location/:hollow-hole = :manatees/:7
  Solved: :boat/:benny-ii = :captain/:romero
  ...

============================================================
                        SOLUTION
============================================================

Boat          Captain     Manatees  Location
------------  ----------  --------  --------------
:benny-ii     :romero     :9        :betty-beach
:daily-ray    :espinoza   :6        :yellow-bend
:foxy-roxy    :armstrong  :4        :rainbow-reef
:mellow-mel   :yang       :7        :hollow-hole
:samantha     :jacobson   :5        :treys-tunnel
:sea-cow      :quinn      :8        :arnos-spit
:watery-pete  :preston    :3        :silver-springs

============================================================
```

The solver demonstrates:
- **Grid cell entities**: 588 cells tracking all category pairings (boat×captain, boat×location, etc.)
- **Constraint propagation**: Eliminating possibilities when cells are solved
- **Bidirectional solving**: When boat→captain is solved, captain→boat is automatically solved
- **Declarative clue application**: `(clue-is! :boat :benny-ii :captain :romero)` to assert facts

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

| Operation     | Time        | Notes              |
| ------------- | ----------- | ------------------ |
| VM simple op  | ~500 ns     | 2M ops/sec         |
| Function call | ~1 µs       | Per call overhead  |
| Pattern match | ~100-230 ns | Per pattern        |
| Component get | 66 ns       | O(1) lookup        |
| World clone   | ~50 ns      | Structural sharing |
| Entity spawn  | ~480 ns     | With components    |

See `cargo bench` for full benchmark suite.

## License

This project is released into the public domain under the [Unlicense](LICENSE).
