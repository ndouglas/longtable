//! Trace event and record types.
//!
//! This module defines the events that can be traced during simulation execution.

use longtable_foundation::{EntityId, KeywordId, Value};

// =============================================================================
// Tick Phase
// =============================================================================

/// Phase of tick execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickPhase {
    /// Input injection phase.
    Input,
    /// Rule activation finding phase.
    Activation,
    /// Rule firing phase.
    Firing,
    /// Constraint checking phase.
    Constraints,
    /// Derived component evaluation phase.
    Derived,
}

impl std::fmt::Display for TickPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input => write!(f, "input"),
            Self::Activation => write!(f, "activation"),
            Self::Firing => write!(f, "firing"),
            Self::Constraints => write!(f, "constraints"),
            Self::Derived => write!(f, "derived"),
        }
    }
}

// =============================================================================
// Trace Event
// =============================================================================

/// Events that can be traced during simulation execution.
#[derive(Clone, Debug)]
pub enum TraceEvent {
    /// A tick has started.
    TickStart {
        /// The tick number.
        tick: u64,
    },

    /// A tick has ended.
    TickEnd {
        /// The tick number.
        tick: u64,
        /// Whether the tick completed successfully.
        success: bool,
    },

    /// A tick phase has started.
    PhaseStart {
        /// The phase that started.
        phase: TickPhase,
    },

    /// A tick phase has ended.
    PhaseEnd {
        /// The phase that ended.
        phase: TickPhase,
    },

    /// A rule has been activated (pattern matched).
    RuleActivated {
        /// The rule that was activated.
        rule: KeywordId,
        /// The variable bindings from pattern matching.
        bindings: Vec<(String, Value)>,
    },

    /// A rule is about to fire (execute effects).
    RuleFiring {
        /// The rule that is firing.
        rule: KeywordId,
    },

    /// A rule has completed firing.
    RuleComplete {
        /// The rule that completed.
        rule: KeywordId,
    },

    /// A component value was written.
    ComponentWrite {
        /// The entity that was written to.
        entity: EntityId,
        /// The component that was written.
        component: KeywordId,
        /// The previous value (if any).
        old_value: Option<Value>,
        /// The new value.
        new_value: Value,
        /// The rule that performed the write (if any).
        rule: Option<KeywordId>,
    },

    /// An entity was spawned.
    EntitySpawn {
        /// The new entity.
        entity: EntityId,
        /// The rule that spawned it (if any).
        rule: Option<KeywordId>,
    },

    /// An entity was destroyed.
    EntityDestroy {
        /// The entity that was destroyed.
        entity: EntityId,
        /// The rule that destroyed it (if any).
        rule: Option<KeywordId>,
    },

    /// A constraint was checked.
    ConstraintResult {
        /// The constraint name.
        name: KeywordId,
        /// Whether the constraint passed.
        passed: bool,
        /// Violation message if failed.
        message: Option<String>,
    },

    /// A breakpoint was hit (for debugger integration).
    BreakpointHit {
        /// The breakpoint ID.
        breakpoint_id: u64,
    },

    /// A watch expression was evaluated.
    WatchEvaluated {
        /// The watch ID.
        watch_id: u64,
        /// The evaluated value.
        value: Value,
    },

    /// Custom user event.
    Custom {
        /// Event name.
        name: String,
        /// Event data.
        data: Value,
    },
}

impl TraceEvent {
    /// Returns a short name for the event type.
    #[must_use]
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::TickStart { .. } => "tick-start",
            Self::TickEnd { .. } => "tick-end",
            Self::PhaseStart { .. } => "phase-start",
            Self::PhaseEnd { .. } => "phase-end",
            Self::RuleActivated { .. } => "rule-activated",
            Self::RuleFiring { .. } => "rule-firing",
            Self::RuleComplete { .. } => "rule-complete",
            Self::ComponentWrite { .. } => "component-write",
            Self::EntitySpawn { .. } => "entity-spawn",
            Self::EntityDestroy { .. } => "entity-destroy",
            Self::ConstraintResult { .. } => "constraint-result",
            Self::BreakpointHit { .. } => "breakpoint-hit",
            Self::WatchEvaluated { .. } => "watch-evaluated",
            Self::Custom { .. } => "custom",
        }
    }

    /// Returns true if this is a tick boundary event.
    #[must_use]
    pub fn is_tick_boundary(&self) -> bool {
        matches!(self, Self::TickStart { .. } | Self::TickEnd { .. })
    }

    /// Returns true if this is a rule-related event.
    #[must_use]
    pub fn is_rule_event(&self) -> bool {
        matches!(
            self,
            Self::RuleActivated { .. } | Self::RuleFiring { .. } | Self::RuleComplete { .. }
        )
    }

    /// Returns true if this is an entity modification event.
    #[must_use]
    pub fn is_entity_event(&self) -> bool {
        matches!(
            self,
            Self::ComponentWrite { .. } | Self::EntitySpawn { .. } | Self::EntityDestroy { .. }
        )
    }
}

// =============================================================================
// Trace Record
// =============================================================================

/// A timestamped trace record.
#[derive(Clone, Debug)]
pub struct TraceRecord {
    /// Unique record ID within the session.
    pub id: u64,
    /// The tick when this event occurred.
    pub tick: u64,
    /// Timestamp in nanoseconds since session start.
    pub timestamp_ns: u64,
    /// The trace event.
    pub event: TraceEvent,
}

impl TraceRecord {
    /// Creates a new trace record.
    #[must_use]
    pub fn new(id: u64, tick: u64, timestamp_ns: u64, event: TraceEvent) -> Self {
        Self {
            id,
            tick,
            timestamp_ns,
            event,
        }
    }

    /// Returns the event type name.
    #[must_use]
    pub fn event_type(&self) -> &'static str {
        self.event.event_type()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_type_names() {
        use longtable_foundation::Interner;

        let mut interner = Interner::new();
        let rule_id = interner.intern_keyword("test-rule");

        let event = TraceEvent::TickStart { tick: 1 };
        assert_eq!(event.event_type(), "tick-start");

        let event = TraceEvent::RuleFiring { rule: rule_id };
        assert_eq!(event.event_type(), "rule-firing");
    }

    #[test]
    fn event_categories() {
        use longtable_foundation::Interner;

        let mut interner = Interner::new();
        let rule_id = interner.intern_keyword("test-rule");

        let tick_event = TraceEvent::TickStart { tick: 1 };
        assert!(tick_event.is_tick_boundary());
        assert!(!tick_event.is_rule_event());

        let rule_event = TraceEvent::RuleActivated {
            rule: rule_id,
            bindings: vec![],
        };
        assert!(rule_event.is_rule_event());
        assert!(!rule_event.is_tick_boundary());

        let entity_event = TraceEvent::EntitySpawn {
            entity: EntityId::new(1, 0),
            rule: None,
        };
        assert!(entity_event.is_entity_event());
    }

    #[test]
    fn tick_phase_display() {
        assert_eq!(TickPhase::Input.to_string(), "input");
        assert_eq!(TickPhase::Firing.to_string(), "firing");
    }

    #[test]
    fn trace_record_creation() {
        let record = TraceRecord::new(1, 5, 1_000_000, TraceEvent::TickStart { tick: 5 });

        assert_eq!(record.id, 1);
        assert_eq!(record.tick, 5);
        assert_eq!(record.timestamp_ns, 1_000_000);
        assert_eq!(record.event_type(), "tick-start");
    }
}
