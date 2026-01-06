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
pub mod gensym;
pub mod lexer;
pub mod macro_def;
pub mod macro_expander;
pub mod macro_registry;
pub mod module_registry;
pub mod namespace;
pub mod opcode;
pub mod parser;
pub mod pretty;
pub mod span;
pub mod stdlib_macros;
pub mod token;
pub mod visitor;
pub mod vm;

#[cfg(test)]
mod fuzz_tests;

// Re-exports for convenience
pub use ast::Ast;
pub use compiler::{
    CompiledExpr, CompiledFunction, CompiledProgram, Compiler, compile, compile_expr,
    compile_expression,
};
pub use declaration::{
    Declaration, DeclarationAnalyzer, Pattern, PatternClause, PatternValue, RuleDecl,
};
pub use gensym::GensymGenerator;
pub use lexer::Lexer;
pub use macro_def::{MacroDef, MacroParam};
pub use macro_expander::MacroExpander;
pub use macro_registry::MacroRegistry;
pub use module_registry::{ModuleRegistry, NamespaceInfo};
pub use namespace::{LoadDecl, NamespaceContext, NamespaceDecl, NamespaceName, RequireSpec};
pub use opcode::{Bytecode, Opcode};
pub use parser::{Parser, parse, parse_one};
pub use span::Span;
pub use stdlib_macros::register_stdlib_macros;
pub use token::{Token, TokenKind};
pub use vm::{Vm, VmContext, VmEffect, WorldContext, eval};
