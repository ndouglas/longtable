//! Tracing, debugging, and time travel for Longtable.
//!
//! This crate provides:
//! - [`config::ObservabilityConfig`] - Configuration for observability features
//! - [`explain`] - "Why" queries for understanding how values were computed
//! - [`trace`] - Tracing of simulation execution
//!
//! # Planned Features (Phase 6)
//!
//! - `Debugger` - Breakpoints and stepping
//! - `Timeline` - Time travel and branching

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]

pub mod config;
pub mod explain;
pub mod trace;

pub use config::ObservabilityConfig;
pub use explain::{
    CausalChain, CausalLink, ClauseMatchStats, DerivedDependency, DerivedExplanation,
    DerivedExplanationBuilder, EntityMatchExplanation, MatchFailureReason, QueryExplanation,
    QueryExplanationBuilder, WhyQuery, WhyResult,
};
pub use trace::{
    HumanFormatter, JsonFormatter, TickPhase, TraceBuffer, TraceBufferStats, TraceEvent,
    TraceFormatter, TraceOutput, TraceRecord, Tracer, TracerConfig,
};
