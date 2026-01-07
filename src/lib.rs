//! Longtable - Rule-based simulation engine
//!
//! This crate re-exports all layers of the Longtable system for convenient access.
//! For detailed documentation, see the individual layer crates.
//!
//! # Architecture
//!
//! ```text
//! Layer 5: longtable_debug     — Tracing, debugging, time travel
//! Layer 4: longtable_runtime   — REPL, CLI, serialization
//!          longtable_stdlib    — Standard library functions
//! Layer 3: longtable_engine    — Rule engine, pattern matching, queries
//! Layer 2: longtable_language  — Lexer, parser, compiler, bytecode VM
//! Layer 1: longtable_storage   — Entity-component storage, relationships
//! Layer 0: longtable_foundation — Core types (Value, EntityId, Error)
//! ```

pub use longtable_debug as debug;
pub use longtable_engine as engine;
pub use longtable_foundation as foundation;
pub use longtable_language as language;
pub use longtable_runtime as runtime;
pub use longtable_stdlib as stdlib;
pub use longtable_storage as storage;
