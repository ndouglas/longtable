# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial workspace structure with seven crates
- GitHub Actions CI workflow (check, test, format, clippy, docs, bench)
- Project documentation (SPECIFICATION.md, IMPLEMENTATION.md)

### Crates
- `longtable_foundation` - Core types, values, and persistent collections
- `longtable_storage` - Entity-component storage, relationships, and world state
- `longtable_language` - Lexer, parser, compiler, and bytecode VM
- `longtable_engine` - Rule engine, pattern matching, queries, and constraints
- `longtable_stdlib` - Standard library functions
- `longtable_runtime` - REPL, CLI, and serialization
- `longtable_debug` - Tracing, debugging, and time travel
