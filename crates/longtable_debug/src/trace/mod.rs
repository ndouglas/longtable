//! Tracing system for Longtable.
//!
//! Provides comprehensive tracing of simulation execution with zero overhead
//! when disabled. Supports both human-readable and JSON output formats.
//!
//! # Example
//!
//! ```text
//! (trace :on)                          ;; Enable tracing
//! (tick!)                              ;; Run a tick (traces will be printed)
//! (trace :off)                         ;; Disable tracing
//! (get-traces :last 10)                ;; Get recent trace records
//! (get-traces :tick 5)                 ;; Get traces for tick 5
//! ```

pub mod buffer;
pub mod format;
pub mod record;

pub use buffer::{TraceBuffer, TraceBufferStats};
pub use format::{HumanFormatter, JsonFormatter, TraceFormatter};
pub use record::{TickPhase, TraceEvent, TraceRecord};

use std::io::{self, Write};
use std::time::Instant;

use longtable_foundation::Interner;

// =============================================================================
// Trace Output
// =============================================================================

/// Where trace output should be sent.
#[derive(Clone, Debug)]
pub enum TraceOutput {
    /// No output (traces still recorded in buffer).
    None,
    /// Write to stderr.
    Stderr,
    /// Write to a custom writer (not clonable, use Stderr or None).
    #[doc(hidden)]
    Custom,
}

impl Default for TraceOutput {
    fn default() -> Self {
        Self::None
    }
}

// =============================================================================
// Tracer Configuration
// =============================================================================

/// Configuration for the tracer.
#[derive(Clone, Debug)]
pub struct TracerConfig {
    /// Whether tracing is enabled.
    pub enabled: bool,
    /// Maximum records to keep in buffer.
    pub buffer_size: usize,
    /// Where to output traces.
    pub output: TraceOutput,
    /// Whether to use JSON format.
    pub json_format: bool,
    /// Filter for specific event types (empty = all).
    pub event_filter: Vec<String>,
}

impl Default for TracerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            buffer_size: 10000,
            output: TraceOutput::None,
            json_format: false,
            event_filter: Vec::new(),
        }
    }
}

impl TracerConfig {
    /// Creates a new tracer configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder method to enable tracing.
    #[must_use]
    pub fn enabled(mut self) -> Self {
        self.enabled = true;
        self
    }

    /// Builder method to set buffer size.
    #[must_use]
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Builder method to output to stderr.
    #[must_use]
    pub fn to_stderr(mut self) -> Self {
        self.output = TraceOutput::Stderr;
        self
    }

    /// Builder method to use JSON format.
    #[must_use]
    pub fn json(mut self) -> Self {
        self.json_format = true;
        self
    }

    /// Builder method to filter event types.
    #[must_use]
    pub fn filter_events(mut self, types: Vec<String>) -> Self {
        self.event_filter = types;
        self
    }
}

// =============================================================================
// Tracer
// =============================================================================

/// The main tracer for recording simulation events.
///
/// Designed for zero overhead when disabled - the `record` method
/// returns immediately if tracing is off.
pub struct Tracer {
    config: TracerConfig,
    buffer: TraceBuffer,
    current_tick: u64,
    start_time: Instant,
    human_formatter: HumanFormatter,
    json_formatter: JsonFormatter,
}

impl Tracer {
    /// Creates a new tracer with the given configuration.
    #[must_use]
    pub fn new(config: TracerConfig) -> Self {
        let buffer_size = config.buffer_size;
        Self {
            config,
            buffer: TraceBuffer::new(buffer_size),
            current_tick: 0,
            start_time: Instant::now(),
            human_formatter: HumanFormatter::new().with_timestamps(),
            json_formatter: JsonFormatter::new(),
        }
    }

    /// Creates a tracer with default configuration (disabled).
    #[must_use]
    pub fn disabled() -> Self {
        Self::new(TracerConfig::default())
    }

    /// Creates an enabled tracer that outputs to stderr.
    #[must_use]
    pub fn to_stderr() -> Self {
        Self::new(TracerConfig::new().enabled().to_stderr())
    }

    /// Returns whether tracing is enabled.
    #[must_use]
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Enables tracing.
    pub fn enable(&mut self) {
        self.config.enabled = true;
    }

    /// Disables tracing.
    pub fn disable(&mut self) {
        self.config.enabled = false;
    }

    /// Sets the current tick number.
    pub fn set_tick(&mut self, tick: u64) {
        self.current_tick = tick;
    }

    /// Returns the current tick number.
    #[must_use]
    pub fn current_tick(&self) -> u64 {
        self.current_tick
    }

    /// Sets whether to use JSON output format.
    pub fn set_json_format(&mut self, json: bool) {
        self.config.json_format = json;
    }

    /// Sets the trace output destination.
    pub fn set_output(&mut self, output: TraceOutput) {
        self.config.output = output;
    }

    /// Records a trace event.
    ///
    /// This is the main entry point for recording events. It's designed
    /// to be as fast as possible when tracing is disabled.
    #[inline]
    pub fn record(&mut self, event: TraceEvent) {
        // Fast path - if disabled, return immediately
        if !self.config.enabled {
            return;
        }

        self.record_internal(event);
    }

    /// Internal recording logic (called when tracing is enabled).
    fn record_internal(&mut self, event: TraceEvent) {
        // Check event filter
        if !self.config.event_filter.is_empty()
            && !self
                .config
                .event_filter
                .contains(&event.event_type().to_string())
        {
            return;
        }

        #[allow(clippy::cast_possible_truncation)]
        let timestamp_ns = self.start_time.elapsed().as_nanos() as u64;
        let id = self.buffer.push(self.current_tick, timestamp_ns, event);

        // Output if configured
        if let TraceOutput::Stderr = self.config.output {
            if let Some(record) = self.buffer.iter().find(|r| r.id == id) {
                Self::output_record(record);
            }
        }
    }

    /// Outputs a record to the configured destination.
    fn output_record(record: &TraceRecord) {
        // We need an interner to format, but we don't have one here.
        // For now, just output the raw event type.
        // In practice, the REPL would call format_record with the interner.
        let line = format!(
            "T{:04} [{:06}] {}",
            record.tick,
            record.id,
            record.event_type()
        );
        let _ = writeln!(io::stderr(), "{line}");
    }

    /// Formats a record using the current format settings.
    #[must_use]
    pub fn format_record(&self, record: &TraceRecord, interner: &Interner) -> String {
        if self.config.json_format {
            self.json_formatter.format(record, interner)
        } else {
            self.human_formatter.format(record, interner)
        }
    }

    /// Formats multiple records.
    #[must_use]
    pub fn format_records(&self, records: &[&TraceRecord], interner: &Interner) -> String {
        if self.config.json_format {
            self.json_formatter.format_many(records, interner)
        } else {
            self.human_formatter.format_many(records, interner)
        }
    }

    /// Returns the trace buffer.
    #[must_use]
    pub fn buffer(&self) -> &TraceBuffer {
        &self.buffer
    }

    /// Clears the trace buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Returns buffer statistics.
    #[must_use]
    pub fn stats(&self) -> TraceBufferStats {
        self.buffer.stats()
    }

    // -------------------------------------------------------------------------
    // Convenience methods for common events
    // -------------------------------------------------------------------------

    /// Records a tick start event.
    #[inline]
    pub fn tick_start(&mut self, tick: u64) {
        self.current_tick = tick;
        self.record(TraceEvent::TickStart { tick });
    }

    /// Records a tick end event.
    #[inline]
    pub fn tick_end(&mut self, tick: u64, success: bool) {
        self.record(TraceEvent::TickEnd { tick, success });
    }

    /// Records a phase start event.
    #[inline]
    pub fn phase_start(&mut self, phase: TickPhase) {
        self.record(TraceEvent::PhaseStart { phase });
    }

    /// Records a phase end event.
    #[inline]
    pub fn phase_end(&mut self, phase: TickPhase) {
        self.record(TraceEvent::PhaseEnd { phase });
    }

    /// Records a rule activated event.
    #[inline]
    pub fn rule_activated(
        &mut self,
        rule: longtable_foundation::KeywordId,
        bindings: Vec<(String, longtable_foundation::Value)>,
    ) {
        self.record(TraceEvent::RuleActivated { rule, bindings });
    }

    /// Records a rule firing event.
    #[inline]
    pub fn rule_firing(&mut self, rule: longtable_foundation::KeywordId) {
        self.record(TraceEvent::RuleFiring { rule });
    }

    /// Records a rule complete event.
    #[inline]
    pub fn rule_complete(&mut self, rule: longtable_foundation::KeywordId) {
        self.record(TraceEvent::RuleComplete { rule });
    }

    /// Records a component write event.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub fn component_write(
        &mut self,
        entity: longtable_foundation::EntityId,
        component: longtable_foundation::KeywordId,
        old_value: Option<longtable_foundation::Value>,
        new_value: longtable_foundation::Value,
        rule: Option<longtable_foundation::KeywordId>,
    ) {
        self.record(TraceEvent::ComponentWrite {
            entity,
            component,
            old_value,
            new_value,
            rule,
        });
    }

    /// Records an entity spawn event.
    #[inline]
    pub fn entity_spawn(
        &mut self,
        entity: longtable_foundation::EntityId,
        rule: Option<longtable_foundation::KeywordId>,
    ) {
        self.record(TraceEvent::EntitySpawn { entity, rule });
    }

    /// Records an entity destroy event.
    #[inline]
    pub fn entity_destroy(
        &mut self,
        entity: longtable_foundation::EntityId,
        rule: Option<longtable_foundation::KeywordId>,
    ) {
        self.record(TraceEvent::EntityDestroy { entity, rule });
    }
}

impl Default for Tracer {
    fn default() -> Self {
        Self::disabled()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracer_disabled_by_default() {
        let tracer = Tracer::default();
        assert!(!tracer.is_enabled());
    }

    #[test]
    fn tracer_enable_disable() {
        let mut tracer = Tracer::default();
        assert!(!tracer.is_enabled());

        tracer.enable();
        assert!(tracer.is_enabled());

        tracer.disable();
        assert!(!tracer.is_enabled());
    }

    #[test]
    fn tracer_records_when_enabled() {
        let config = TracerConfig::new().enabled().with_buffer_size(100);
        let mut tracer = Tracer::new(config);

        tracer.tick_start(1);
        tracer.tick_end(1, true);

        assert_eq!(tracer.buffer().len(), 2);
    }

    #[test]
    fn tracer_ignores_when_disabled() {
        let mut tracer = Tracer::disabled();

        tracer.tick_start(1);
        tracer.tick_end(1, true);

        assert!(tracer.buffer().is_empty());
    }

    #[test]
    fn tracer_tracks_current_tick() {
        let config = TracerConfig::new().enabled();
        let mut tracer = Tracer::new(config);

        assert_eq!(tracer.current_tick(), 0);

        tracer.tick_start(5);
        assert_eq!(tracer.current_tick(), 5);
    }

    #[test]
    fn tracer_event_filter() {
        let config = TracerConfig::new()
            .enabled()
            .filter_events(vec!["tick-start".to_string()]);
        let mut tracer = Tracer::new(config);

        tracer.tick_start(1);
        tracer.tick_end(1, true); // This should be filtered out

        assert_eq!(tracer.buffer().len(), 1);
        assert_eq!(
            tracer.buffer().iter().next().unwrap().event_type(),
            "tick-start"
        );
    }

    #[test]
    fn tracer_convenience_methods() {
        let config = TracerConfig::new().enabled();
        let mut tracer = Tracer::new(config);

        let mut interner = longtable_foundation::Interner::new();
        let rule = interner.intern_keyword("test-rule");
        let health = interner.intern_keyword("health");
        let entity = longtable_foundation::EntityId::new(1, 0);

        tracer.tick_start(1);
        tracer.phase_start(TickPhase::Activation);
        tracer.rule_activated(rule, vec![]);
        tracer.rule_firing(rule);
        tracer.component_write(
            entity,
            health,
            None,
            longtable_foundation::Value::Int(100),
            Some(rule),
        );
        tracer.rule_complete(rule);
        tracer.phase_end(TickPhase::Activation);
        tracer.tick_end(1, true);

        assert_eq!(tracer.buffer().len(), 8);
    }

    #[test]
    fn tracer_clear() {
        let config = TracerConfig::new().enabled();
        let mut tracer = Tracer::new(config);

        tracer.tick_start(1);
        tracer.tick_end(1, true);
        assert_eq!(tracer.buffer().len(), 2);

        tracer.clear();
        assert!(tracer.buffer().is_empty());
    }

    #[test]
    fn tracer_stats() {
        let config = TracerConfig::new().enabled();
        let mut tracer = Tracer::new(config);

        tracer.tick_start(1);
        tracer.tick_end(1, true);

        let stats = tracer.stats();
        assert_eq!(stats.record_count, 2);
        assert_eq!(stats.oldest_tick, Some(1));
        assert_eq!(stats.newest_tick, Some(1));
    }
}
