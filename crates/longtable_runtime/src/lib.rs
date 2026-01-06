//! REPL, CLI, and serialization for Longtable.
//!
//! This crate provides:
//! - [`Repl`] - Interactive read-eval-print loop
//! - CLI argument parsing and execution
//! - World serialization and deserialization
//!
//! # Example
//!
//! ```no_run
//! use longtable_runtime::Repl;
//!
//! let mut repl = Repl::new().expect("failed to create REPL");
//! repl.run().expect("REPL error");
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
// The Error type is intentionally large for rich error context
#![allow(clippy::result_large_err)]

mod editor;
mod highlight;
mod repl;
mod session;

pub use editor::{LineEditor, RustylineEditor};
pub use repl::Repl;
pub use session::Session;
