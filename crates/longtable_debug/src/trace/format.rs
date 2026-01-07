//! Trace output formatters.
//!
//! Provides human-readable and JSON formatters for trace records.

use longtable_foundation::Interner;

use super::record::{TraceEvent, TraceRecord};

// =============================================================================
// Trace Formatter Trait
// =============================================================================

/// Trait for formatting trace records.
pub trait TraceFormatter {
    /// Formats a single trace record to a string.
    fn format(&self, record: &TraceRecord, interner: &Interner) -> String;

    /// Formats multiple records.
    fn format_many(&self, records: &[&TraceRecord], interner: &Interner) -> String {
        records
            .iter()
            .map(|r| self.format(r, interner))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// =============================================================================
// Human-Readable Formatter
// =============================================================================

/// Formats trace records in human-readable form.
#[derive(Clone, Debug, Default)]
pub struct HumanFormatter {
    /// Whether to include timestamps.
    pub show_timestamps: bool,
    /// Whether to include record IDs.
    pub show_ids: bool,
}

impl HumanFormatter {
    /// Creates a new human formatter with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder method to show timestamps.
    #[must_use]
    pub fn with_timestamps(mut self) -> Self {
        self.show_timestamps = true;
        self
    }

    /// Builder method to show record IDs.
    #[must_use]
    pub fn with_ids(mut self) -> Self {
        self.show_ids = true;
        self
    }

    /// Formats a keyword ID to its string representation.
    fn keyword_name(id: longtable_foundation::KeywordId, interner: &Interner) -> String {
        interner.get_keyword(id).unwrap_or("?").to_string()
    }

    /// Formats timestamp in microseconds.
    #[allow(clippy::cast_precision_loss)]
    fn format_timestamp(ns: u64) -> String {
        let us = ns / 1000;
        if us >= 1_000_000 {
            format!("{:.3}s", us as f64 / 1_000_000.0)
        } else if us >= 1000 {
            format!("{:.3}ms", us as f64 / 1000.0)
        } else {
            format!("{us}us")
        }
    }
}

impl TraceFormatter for HumanFormatter {
    #[allow(clippy::format_push_string)]
    fn format(&self, record: &TraceRecord, interner: &Interner) -> String {
        use std::fmt::Write;
        let mut prefix = String::new();

        if self.show_ids {
            let _ = write!(prefix, "[{:06}] ", record.id);
        }

        let _ = write!(prefix, "T{:04} ", record.tick);

        if self.show_timestamps {
            let _ = write!(
                prefix,
                "{:>10} ",
                Self::format_timestamp(record.timestamp_ns)
            );
        }

        let event_str = match &record.event {
            TraceEvent::TickStart { tick } => {
                format!("=== TICK {tick} START ===")
            }
            TraceEvent::TickEnd { tick, success } => {
                let status = if *success { "OK" } else { "FAILED" };
                format!("=== TICK {tick} END ({status}) ===")
            }
            TraceEvent::PhaseStart { phase } => {
                format!("  >> {phase}")
            }
            TraceEvent::PhaseEnd { phase } => {
                format!("  << {phase}")
            }
            TraceEvent::RuleActivated { rule, bindings } => {
                let rule_name = Self::keyword_name(*rule, interner);
                if bindings.is_empty() {
                    format!("  ACTIVATED :{rule_name}")
                } else {
                    let bindings_str: Vec<_> =
                        bindings.iter().map(|(k, v)| format!("?{k}={v}")).collect();
                    format!("  ACTIVATED :{rule_name} {{{}}}", bindings_str.join(", "))
                }
            }
            TraceEvent::RuleFiring { rule } => {
                let rule_name = Self::keyword_name(*rule, interner);
                format!("  FIRING :{rule_name}")
            }
            TraceEvent::RuleComplete { rule } => {
                let rule_name = Self::keyword_name(*rule, interner);
                format!("  COMPLETE :{rule_name}")
            }
            TraceEvent::ComponentWrite {
                entity,
                component,
                old_value,
                new_value,
                rule,
            } => {
                let comp_name = Self::keyword_name(*component, interner);
                let rule_str = rule
                    .map(|r| format!(" (:{}) ", Self::keyword_name(r, interner)))
                    .unwrap_or_default();
                match old_value {
                    Some(old) => {
                        format!("    WRITE{rule_str}{entity} :{comp_name} {old} -> {new_value}")
                    }
                    None => {
                        format!("    WRITE{rule_str}{entity} :{comp_name} = {new_value}")
                    }
                }
            }
            TraceEvent::EntitySpawn { entity, rule } => {
                let rule_str = rule
                    .map(|r| format!(" (:{}) ", Self::keyword_name(r, interner)))
                    .unwrap_or_default();
                format!("    SPAWN{rule_str}{entity}")
            }
            TraceEvent::EntityDestroy { entity, rule } => {
                let rule_str = rule
                    .map(|r| format!(" (:{}) ", Self::keyword_name(r, interner)))
                    .unwrap_or_default();
                format!("    DESTROY{rule_str}{entity}")
            }
            TraceEvent::ConstraintResult {
                name,
                passed,
                message,
            } => {
                let name_str = Self::keyword_name(*name, interner);
                let status = if *passed { "PASS" } else { "FAIL" };
                match message {
                    Some(msg) => format!("  CONSTRAINT :{name_str} {status}: {msg}"),
                    None => format!("  CONSTRAINT :{name_str} {status}"),
                }
            }
            TraceEvent::BreakpointHit { breakpoint_id } => {
                format!("  BREAKPOINT #{breakpoint_id}")
            }
            TraceEvent::WatchEvaluated { watch_id, value } => {
                format!("  WATCH #{watch_id} = {value}")
            }
            TraceEvent::Custom { name, data } => {
                format!("  CUSTOM {name}: {data}")
            }
        };

        format!("{prefix}{event_str}")
    }
}

// =============================================================================
// JSON Formatter
// =============================================================================

/// Formats trace records as JSON.
#[derive(Clone, Debug, Default)]
pub struct JsonFormatter {
    /// Whether to pretty-print JSON.
    pub pretty: bool,
}

impl JsonFormatter {
    /// Creates a new JSON formatter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder method for pretty printing.
    #[must_use]
    pub fn pretty(mut self) -> Self {
        self.pretty = true;
        self
    }

    /// Escapes a string for JSON.
    fn escape_string(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    }

    /// Formats a value as JSON.
    fn format_value(value: &longtable_foundation::Value) -> String {
        use longtable_foundation::Value;
        match value {
            Value::Nil => "null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Int(n) => n.to_string(),
            Value::Float(f) => {
                if f.is_nan() {
                    "\"NaN\"".to_string()
                } else if f.is_infinite() {
                    if *f > 0.0 {
                        "\"Infinity\"".to_string()
                    } else {
                        "\"-Infinity\"".to_string()
                    }
                } else {
                    f.to_string()
                }
            }
            Value::String(s) => format!("\"{}\"", Self::escape_string(s)),
            Value::EntityRef(e) => format!("\"{e}\""),
            _ => format!("\"{}\"", Self::escape_string(&value.to_string())),
        }
    }
}

impl TraceFormatter for JsonFormatter {
    #[allow(clippy::too_many_lines)]
    fn format(&self, record: &TraceRecord, interner: &Interner) -> String {
        let event_type = record.event_type();
        let keyword_name =
            |id: longtable_foundation::KeywordId| interner.get_keyword(id).unwrap_or("?");

        let event_data = match &record.event {
            TraceEvent::TickStart { tick } => {
                format!("\"tick\":{tick}")
            }
            TraceEvent::TickEnd { tick, success } => {
                format!("\"tick\":{tick},\"success\":{success}")
            }
            TraceEvent::PhaseStart { phase } | TraceEvent::PhaseEnd { phase } => {
                format!("\"phase\":\"{phase}\"")
            }
            TraceEvent::RuleActivated { rule, bindings } => {
                let bindings_json: Vec<_> = bindings
                    .iter()
                    .map(|(k, v)| format!("\"{}\":{}", k, Self::format_value(v)))
                    .collect();
                format!(
                    "\"rule\":\"{}\",\"bindings\":{{{}}}",
                    keyword_name(*rule),
                    bindings_json.join(",")
                )
            }
            TraceEvent::RuleFiring { rule } | TraceEvent::RuleComplete { rule } => {
                format!("\"rule\":\"{}\"", keyword_name(*rule))
            }
            TraceEvent::ComponentWrite {
                entity,
                component,
                old_value,
                new_value,
                rule,
            } => {
                let old_json = old_value
                    .as_ref()
                    .map_or_else(|| "null".to_string(), Self::format_value);
                let rule_json = rule
                    .map(|r| format!(",\"rule\":\"{}\"", keyword_name(r)))
                    .unwrap_or_default();
                format!(
                    "\"entity\":\"{}\",\"component\":\"{}\",\"old\":{},\"new\":{}{}",
                    entity,
                    keyword_name(*component),
                    old_json,
                    Self::format_value(new_value),
                    rule_json
                )
            }
            TraceEvent::EntitySpawn { entity, rule } => {
                let rule_json = rule
                    .map(|r| format!(",\"rule\":\"{}\"", keyword_name(r)))
                    .unwrap_or_default();
                format!("\"entity\":\"{entity}\"{rule_json}")
            }
            TraceEvent::EntityDestroy { entity, rule } => {
                let rule_json = rule
                    .map(|r| format!(",\"rule\":\"{}\"", keyword_name(r)))
                    .unwrap_or_default();
                format!("\"entity\":\"{entity}\"{rule_json}")
            }
            TraceEvent::ConstraintResult {
                name,
                passed,
                message,
            } => {
                let msg_json = message
                    .as_ref()
                    .map(|m| format!(",\"message\":\"{}\"", Self::escape_string(m)))
                    .unwrap_or_default();
                format!(
                    "\"name\":\"{}\",\"passed\":{passed}{msg_json}",
                    keyword_name(*name)
                )
            }
            TraceEvent::BreakpointHit { breakpoint_id } => {
                format!("\"breakpoint_id\":{breakpoint_id}")
            }
            TraceEvent::WatchEvaluated { watch_id, value } => {
                format!(
                    "\"watch_id\":{watch_id},\"value\":{}",
                    Self::format_value(value)
                )
            }
            TraceEvent::Custom { name, data } => {
                format!(
                    "\"name\":\"{}\",\"data\":{}",
                    Self::escape_string(name),
                    Self::format_value(data)
                )
            }
        };

        let json = format!(
            "{{\"id\":{},\"tick\":{},\"timestamp_ns\":{},\"type\":\"{}\",{}}}",
            record.id, record.tick, record.timestamp_ns, event_type, event_data
        );

        if self.pretty {
            // Simple pretty print - just add newlines and indentation
            // A full implementation would use serde_json
            json
        } else {
            json
        }
    }

    fn format_many(&self, records: &[&TraceRecord], interner: &Interner) -> String {
        let items: Vec<_> = records.iter().map(|r| self.format(r, interner)).collect();
        if self.pretty {
            format!("[\n  {}\n]", items.join(",\n  "))
        } else {
            format!("[{}]", items.join(","))
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use longtable_foundation::EntityId;

    fn setup() -> Interner {
        let mut interner = Interner::new();
        interner.intern_keyword("health");
        interner.intern_keyword("apply-damage");
        interner
    }

    #[test]
    fn human_formatter_tick_events() {
        let interner = setup();
        let formatter = HumanFormatter::new();

        let record = TraceRecord::new(1, 5, 1000, TraceEvent::TickStart { tick: 5 });
        let output = formatter.format(&record, &interner);
        assert!(output.contains("TICK 5 START"));

        let record = TraceRecord::new(
            2,
            5,
            2000,
            TraceEvent::TickEnd {
                tick: 5,
                success: true,
            },
        );
        let output = formatter.format(&record, &interner);
        assert!(output.contains("TICK 5 END"));
        assert!(output.contains("OK"));
    }

    #[test]
    fn human_formatter_with_options() {
        let interner = setup();
        let formatter = HumanFormatter::new().with_timestamps().with_ids();

        let record = TraceRecord::new(42, 5, 1_500_000, TraceEvent::TickStart { tick: 5 });
        let output = formatter.format(&record, &interner);

        assert!(output.contains("[000042]"));
        assert!(output.contains("1.500ms"));
    }

    #[test]
    fn human_formatter_component_write() {
        let mut interner = setup();
        let formatter = HumanFormatter::new();

        let health_id = interner.intern_keyword("health");
        let entity = EntityId::new(1, 0);

        let record = TraceRecord::new(
            1,
            5,
            1000,
            TraceEvent::ComponentWrite {
                entity,
                component: health_id,
                old_value: Some(longtable_foundation::Value::Int(100)),
                new_value: longtable_foundation::Value::Int(75),
                rule: None,
            },
        );

        let output = formatter.format(&record, &interner);
        assert!(output.contains("WRITE"));
        assert!(output.contains(":health"));
        assert!(output.contains("100"));
        assert!(output.contains("75"));
    }

    #[test]
    fn json_formatter_basic() {
        let interner = setup();
        let formatter = JsonFormatter::new();

        let record = TraceRecord::new(1, 5, 1000, TraceEvent::TickStart { tick: 5 });
        let output = formatter.format(&record, &interner);

        assert!(output.starts_with('{'));
        assert!(output.ends_with('}'));
        assert!(output.contains("\"type\":\"tick-start\""));
        assert!(output.contains("\"tick\":5"));
    }

    #[test]
    fn json_formatter_many() {
        let interner = setup();
        let formatter = JsonFormatter::new();

        let r1 = TraceRecord::new(1, 5, 1000, TraceEvent::TickStart { tick: 5 });
        let r2 = TraceRecord::new(
            2,
            5,
            2000,
            TraceEvent::TickEnd {
                tick: 5,
                success: true,
            },
        );

        let output = formatter.format_many(&[&r1, &r2], &interner);
        assert!(output.starts_with('['));
        assert!(output.ends_with(']'));
    }
}
