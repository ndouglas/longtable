//! Rule engine, pattern matching, queries, and constraints for Longtable.
//!
//! This crate provides:
//! - `PatternMatcher` - Pattern compilation and matching
//! - `RuleEngine` - Rule activation, refraction, and execution
//! - `QueryExecutor` - Query compilation and execution
//! - `ConstraintChecker` - Constraint validation
//! - `DerivedCache` - Derived component caching

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
// Allow large error types - our Error has rich context
#![allow(clippy::result_large_err)]
// Allow missing error docs for now
#![allow(clippy::missing_errors_doc)]

pub mod constraint;
pub mod derived;
pub mod pattern;
pub mod provenance;
pub mod query;
pub mod rule;
pub mod spike;
pub mod tick;

// Constraints
pub use constraint::{
    CompiledConstraint, ConstraintChecker, ConstraintCompiler, ConstraintResult, ViolationDetails,
};

// Derived components
pub use derived::{CompiledDerived, DerivedCache, DerivedCompiler, DerivedEvaluator};

// Provenance tracking
pub use provenance::{ProvenanceTracker, WriteRecord};

// Tick orchestration
pub use tick::{InputEvent, TickExecutor, TickResult};

// Production pattern matching
pub use pattern::{
    Bindings, CompiledBinding, CompiledClause, CompiledPattern, PatternCompiler, PatternMatcher,
};

// Query system
pub use query::{CompiledQuery, QueryCompiler, QueryExecutor, QueryWarning};

// Production rule engine
pub use rule::{
    Activation, CompiledRule, CompiledRuleBody, EffectRecord, FullCompiledRule,
    ProductionRuleEngine, RuleCompiler,
};

// Spike code (to be replaced)
pub use spike::{
    Activation as SpikeActivation, Pattern as SpikePattern, PatternClause as SpikePatternClause,
    RuleEngine as SpikeRuleEngine, SpikeRule,
};
