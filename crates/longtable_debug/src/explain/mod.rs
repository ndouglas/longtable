//! Explanation system for Longtable.
//!
//! Provides "why" queries for understanding how values were computed:
//! - `(why entity :component)` - Single-hop explanation
//! - `(why entity :component :depth N)` - Multi-hop causal chain
//! - `(why entity :derived/component)` - Derived component explanation
//! - `(explain-query query)` - Query execution explanation
//! - `(explain-query query entity)` - Entity-specific query explanation

pub mod derived;
pub mod query;
pub mod why;

pub use derived::{DerivedDependency, DerivedExplanation, DerivedExplanationBuilder};
pub use query::{
    ClauseMatchStats, EntityMatchExplanation, MatchFailureReason, QueryExplanation,
    QueryExplanationBuilder,
};
pub use why::{CausalChain, CausalLink, WhyQuery, WhyResult};
