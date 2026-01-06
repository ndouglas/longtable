//! Lexer, parser, compiler, and bytecode VM for Longtable DSL.
//!
//! This crate provides:
//! - [`Span`] - Source location tracking
//! - [`Token`] and [`TokenKind`] - Lexical tokens
//! - [`Lexer`] - Tokenization of Longtable source
//! - [`Ast`] - Abstract syntax tree
//! - [`Parser`] - Parsing tokens into AST
//!
//! # Example
//!
//! ```
//! use longtable_language::{parse, Lexer};
//!
//! let source = "(+ 1 2)";
//!
//! // Tokenize
//! let tokens = Lexer::tokenize_all(source);
//! assert!(tokens.len() > 0);
//!
//! // Parse
//! let ast = parse(source).unwrap();
//! assert_eq!(ast.len(), 1);
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::result_large_err)]
#![allow(clippy::missing_errors_doc)]

pub mod ast;
pub mod compiler;
pub mod declaration;
pub mod lexer;
pub mod opcode;
pub mod parser;
pub mod span;
pub mod token;
pub mod vm;

// Re-exports for convenience
pub use ast::Ast;
pub use compiler::{
    CompiledExpr, CompiledFunction, CompiledProgram, Compiler, compile, compile_expr,
    compile_expression,
};
pub use declaration::{DeclarationAnalyzer, Pattern, PatternClause, PatternValue, RuleDecl};
pub use lexer::Lexer;
pub use opcode::{Bytecode, Opcode};
pub use parser::{Parser, parse, parse_one};
pub use span::Span;
pub use token::{Token, TokenKind};
pub use vm::{Vm, VmContext, VmEffect, WorldContext, eval};
