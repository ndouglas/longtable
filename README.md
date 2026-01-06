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

## Status

**Phase 0: Bootstrap** — Project structure established, implementation not yet started.

See [IMPLEMENTATION.md](IMPLEMENTATION.md) for the development roadmap.

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

## Building

```bash
cargo build
cargo test
cargo doc --open
```

Requires Rust 1.85.0 or later.

## License

This project is released into the public domain under the [Unlicense](LICENSE).
