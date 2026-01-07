//! Tracing, debugging, and time travel for Longtable.
//!
//! This crate provides:
//! - [`config::ObservabilityConfig`] - Configuration for observability features
//! - [`explain`] - "Why" queries for understanding how values were computed
//!
//! # Planned Features (Phase 6)
//!
//! - `Tracer` - Rule and entity tracing
//! - `Debugger` - Breakpoints and stepping
//! - `Timeline` - Time travel and branching

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]

pub mod config;
pub mod explain;

pub use config::ObservabilityConfig;
pub use explain::{
    CausalChain, CausalLink, ClauseMatchStats, DerivedDependency, DerivedExplanation,
    DerivedExplanationBuilder, EntityMatchExplanation, MatchFailureReason, QueryExplanation,
    QueryExplanationBuilder, WhyQuery, WhyResult,
};
