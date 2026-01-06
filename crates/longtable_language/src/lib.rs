//! Lexer, parser, compiler, and bytecode VM for Longtable DSL.
//!
//! This crate will provide:
//! - `Lexer` - Tokenization of Longtable source
//! - `Parser` - Parsing tokens into AST
//! - `Compiler` - Compiling AST to bytecode
//! - `Vm` - Stack-based bytecode interpreter

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
