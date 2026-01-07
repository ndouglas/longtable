//! Explanation for query execution.
//!
//! This module provides types for explaining how a query was executed
//! and why certain entities matched or didn't match.
//!
//! # Example
//!
//! ```text
//! (explain-query (query :where [[?e :health ?hp] [?e :tag/enemy]] :return ?e))
//! ;; => {:clauses [{:text "[?e :health ?hp]", :input 100, :output 45}
//! ;;               {:text "[?e :tag/enemy]", :input 45, :output 12}]
//! ;;     :result-count 12}
//!
//! (explain-query (query ...) entity-42)
//! ;; => {:matched false
//! ;;     :failed-at-clause 1
//! ;;     :reason {:missing-component :tag/enemy}}
//! ```

use longtable_foundation::{EntityId, KeywordId, Value};

// =============================================================================
// Match Failure Reason
// =============================================================================

/// Reason why an entity failed to match a query clause.
#[derive(Clone, Debug, PartialEq)]
pub enum MatchFailureReason {
    /// Entity doesn't have the required component.
    MissingComponent {
        /// The component that was expected.
        component: KeywordId,
    },

    /// Component value doesn't match the pattern literal.
    ValueMismatch {
        /// The expected value from the pattern.
        expected: Value,
        /// The actual value found.
        actual: Value,
    },

    /// Variable unification failed (same var bound to different values).
    UnificationFailure {
        /// The variable name.
        var: String,
        /// The previously bound value.
        expected: Value,
        /// The new value that conflicted.
        actual: Value,
    },

    /// Negation clause matched when it shouldn't have.
    NegationMatched {
        /// The component that was found but shouldn't exist.
        component: KeywordId,
    },

    /// Guard expression returned false.
    GuardFailed {
        /// Index of the guard that failed (0-based).
        guard_index: usize,
        /// The guard expression as text (if available).
        guard_text: Option<String>,
    },

    /// Entity doesn't exist.
    EntityNotFound,
}

// =============================================================================
// Clause Match Statistics
// =============================================================================

/// Statistics about a single pattern clause during query execution.
#[derive(Clone, Debug)]
pub struct ClauseMatchStats {
    /// The clause index (0-based).
    pub clause_index: usize,

    /// Human-readable clause description (e.g., "[?e :health ?hp]").
    pub clause_text: String,

    /// Number of binding sets (candidates) before this clause.
    pub input_count: usize,

    /// Number of binding sets that passed this clause.
    pub output_count: usize,

    /// Entities that were filtered out by this clause.
    pub filtered_entities: Vec<EntityId>,

    /// Time spent on this clause in microseconds (if measured).
    pub duration_us: Option<u64>,
}

impl ClauseMatchStats {
    /// Creates new clause statistics.
    #[must_use]
    pub fn new(clause_index: usize, clause_text: impl Into<String>) -> Self {
        Self {
            clause_index,
            clause_text: clause_text.into(),
            input_count: 0,
            output_count: 0,
            filtered_entities: Vec::new(),
            duration_us: None,
        }
    }

    /// Returns the number of candidates filtered out by this clause.
    #[must_use]
    pub fn filtered_count(&self) -> usize {
        self.input_count.saturating_sub(self.output_count)
    }

    /// Returns the pass rate as a percentage (0-100).
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn pass_rate(&self) -> f64 {
        if self.input_count == 0 {
            100.0
        } else {
            (self.output_count as f64 / self.input_count as f64) * 100.0
        }
    }
}

// =============================================================================
// Query Explanation
// =============================================================================

/// Explanation of query execution.
///
/// Shows how the query was processed through its pipeline of clauses.
#[derive(Clone, Debug, Default)]
pub struct QueryExplanation {
    /// Statistics for each clause in order.
    pub clause_stats: Vec<ClauseMatchStats>,

    /// Total binding sets before guard evaluation.
    pub pre_guard_count: usize,

    /// Total binding sets after guard evaluation.
    pub post_guard_count: usize,

    /// Final result count (after limit/order).
    pub result_count: usize,

    /// Total execution time in microseconds.
    pub total_duration_us: Option<u64>,

    /// Whether grouping/aggregation was used.
    pub used_aggregation: bool,

    /// Whether ordering was applied.
    pub used_ordering: bool,

    /// Limit that was applied (if any).
    pub limit_applied: Option<usize>,
}

impl QueryExplanation {
    /// Creates a new empty query explanation.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds statistics for a clause.
    pub fn add_clause_stats(&mut self, stats: ClauseMatchStats) {
        self.clause_stats.push(stats);
    }

    /// Returns the total number of clauses.
    #[must_use]
    pub fn clause_count(&self) -> usize {
        self.clause_stats.len()
    }

    /// Returns the most selective clause (filtered the most candidates).
    #[must_use]
    pub fn most_selective_clause(&self) -> Option<&ClauseMatchStats> {
        self.clause_stats.iter().max_by_key(|s| s.filtered_count())
    }

    /// Returns the least selective clause (filtered the fewest candidates).
    #[must_use]
    pub fn least_selective_clause(&self) -> Option<&ClauseMatchStats> {
        self.clause_stats.iter().min_by_key(|s| s.filtered_count())
    }

    /// Returns the slowest clause (if timing is available).
    #[must_use]
    pub fn slowest_clause(&self) -> Option<&ClauseMatchStats> {
        self.clause_stats
            .iter()
            .filter(|s| s.duration_us.is_some())
            .max_by_key(|s| s.duration_us.unwrap_or(0))
    }
}

// =============================================================================
// Entity Match Explanation
// =============================================================================

/// Explanation of why a specific entity did or didn't match a query.
#[derive(Clone, Debug)]
pub struct EntityMatchExplanation {
    /// The entity being explained.
    pub entity: EntityId,

    /// Did it match the query?
    pub matched: bool,

    /// Which clause failed (if any) - 0-indexed.
    pub failed_at_clause: Option<usize>,

    /// Why the clause failed (if applicable).
    pub failure_reason: Option<MatchFailureReason>,

    /// Partial bindings at point of failure.
    pub partial_bindings: Vec<(String, Value)>,

    /// Which clauses passed before failure.
    pub passed_clauses: Vec<usize>,
}

impl EntityMatchExplanation {
    /// Creates an explanation for a matched entity.
    #[must_use]
    pub fn matched(entity: EntityId, bindings: Vec<(String, Value)>) -> Self {
        Self {
            entity,
            matched: true,
            failed_at_clause: None,
            failure_reason: None,
            partial_bindings: bindings,
            passed_clauses: Vec::new(),
        }
    }

    /// Creates an explanation for a non-matched entity.
    #[must_use]
    pub fn not_matched(
        entity: EntityId,
        failed_at: usize,
        reason: MatchFailureReason,
        partial_bindings: Vec<(String, Value)>,
    ) -> Self {
        Self {
            entity,
            matched: false,
            failed_at_clause: Some(failed_at),
            failure_reason: Some(reason),
            partial_bindings,
            passed_clauses: (0..failed_at).collect(),
        }
    }

    /// Creates an explanation for an entity that doesn't exist.
    #[must_use]
    pub fn entity_not_found(entity: EntityId) -> Self {
        Self {
            entity,
            matched: false,
            failed_at_clause: Some(0),
            failure_reason: Some(MatchFailureReason::EntityNotFound),
            partial_bindings: Vec::new(),
            passed_clauses: Vec::new(),
        }
    }
}

// =============================================================================
// Query Explanation Builder
// =============================================================================

/// Builder for constructing query explanations during execution.
#[derive(Clone, Debug, Default)]
pub struct QueryExplanationBuilder {
    clause_stats: Vec<ClauseMatchStats>,
    pre_guard_count: usize,
    post_guard_count: usize,
    result_count: usize,
}

impl QueryExplanationBuilder {
    /// Creates a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records statistics for a clause.
    pub fn record_clause(
        &mut self,
        clause_index: usize,
        clause_text: impl Into<String>,
        input_count: usize,
        output_count: usize,
    ) {
        let mut stats = ClauseMatchStats::new(clause_index, clause_text);
        stats.input_count = input_count;
        stats.output_count = output_count;
        self.clause_stats.push(stats);
    }

    /// Records the pre-guard count.
    pub fn record_pre_guard_count(&mut self, count: usize) {
        self.pre_guard_count = count;
    }

    /// Records the post-guard count.
    pub fn record_post_guard_count(&mut self, count: usize) {
        self.post_guard_count = count;
    }

    /// Records the final result count.
    pub fn record_result_count(&mut self, count: usize) {
        self.result_count = count;
    }

    /// Builds the final explanation.
    #[must_use]
    pub fn build(self) -> QueryExplanation {
        QueryExplanation {
            clause_stats: self.clause_stats,
            pre_guard_count: self.pre_guard_count,
            post_guard_count: self.post_guard_count,
            result_count: self.result_count,
            total_duration_us: None,
            used_aggregation: false,
            used_ordering: false,
            limit_applied: None,
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use longtable_foundation::Interner;

    #[test]
    fn clause_match_stats() {
        let mut stats = ClauseMatchStats::new(0, "[?e :health ?hp]");
        stats.input_count = 100;
        stats.output_count = 45;

        assert_eq!(stats.filtered_count(), 55);
        assert!((stats.pass_rate() - 45.0).abs() < 0.001);
    }

    #[test]
    fn query_explanation_basics() {
        let mut explanation = QueryExplanation::new();

        let mut stats1 = ClauseMatchStats::new(0, "[?e :health ?hp]");
        stats1.input_count = 100;
        stats1.output_count = 45;
        explanation.add_clause_stats(stats1);

        let mut stats2 = ClauseMatchStats::new(1, "[?e :tag/enemy]");
        stats2.input_count = 45;
        stats2.output_count = 12;
        explanation.add_clause_stats(stats2);

        assert_eq!(explanation.clause_count(), 2);

        let most_selective = explanation.most_selective_clause().unwrap();
        assert_eq!(most_selective.clause_index, 0);
        assert_eq!(most_selective.filtered_count(), 55);
    }

    #[test]
    fn entity_match_explanation() {
        let entity = EntityId::new(42, 0);

        // Matched entity
        let matched = EntityMatchExplanation::matched(
            entity,
            vec![("?e".to_string(), Value::EntityRef(entity))],
        );
        assert!(matched.matched);
        assert!(matched.failure_reason.is_none());

        // Not matched - missing component
        let mut interner = Interner::new();
        let health = interner.intern_keyword("health");

        let not_matched = EntityMatchExplanation::not_matched(
            entity,
            0,
            MatchFailureReason::MissingComponent { component: health },
            Vec::new(),
        );
        assert!(!not_matched.matched);
        assert_eq!(not_matched.failed_at_clause, Some(0));
        assert!(matches!(
            not_matched.failure_reason,
            Some(MatchFailureReason::MissingComponent { .. })
        ));
    }

    #[test]
    fn builder_pattern() {
        let mut builder = QueryExplanationBuilder::new();
        builder.record_clause(0, "[?e :health ?hp]", 100, 45);
        builder.record_clause(1, "[?e :tag/enemy]", 45, 12);
        builder.record_pre_guard_count(12);
        builder.record_post_guard_count(10);
        builder.record_result_count(10);

        let explanation = builder.build();

        assert_eq!(explanation.clause_count(), 2);
        assert_eq!(explanation.pre_guard_count, 12);
        assert_eq!(explanation.post_guard_count, 10);
        assert_eq!(explanation.result_count, 10);
    }
}
