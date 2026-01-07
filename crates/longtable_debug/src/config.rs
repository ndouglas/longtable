//! Configuration for the observability system.

use longtable_engine::provenance::ProvenanceVerbosity;

/// Configuration for the observability system.
///
/// Controls tracing, debugging, and history retention.
#[derive(Clone, Debug)]
pub struct ObservabilityConfig {
    /// Whether observability is enabled (false = zero overhead).
    pub enabled: bool,

    /// Current verbosity level for provenance tracking.
    pub verbosity: ProvenanceVerbosity,

    /// History ring buffer size (number of ticks to retain).
    pub history_size: usize,

    /// Default depth for why-queries.
    pub why_depth: usize,

    /// Output trace to stderr.
    pub trace_to_stderr: bool,

    /// Output format: true for JSON, false for human-readable.
    pub json_output: bool,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            verbosity: ProvenanceVerbosity::Minimal,
            history_size: 100,
            why_depth: 1,
            trace_to_stderr: true,
            json_output: false,
        }
    }
}

impl ObservabilityConfig {
    /// Creates a new configuration with observability enabled.
    #[must_use]
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }

    /// Creates a configuration for development with Standard verbosity.
    #[must_use]
    pub fn development() -> Self {
        Self {
            enabled: true,
            verbosity: ProvenanceVerbosity::Standard,
            history_size: 100,
            why_depth: 3,
            trace_to_stderr: true,
            json_output: false,
        }
    }

    /// Creates a configuration for debugging with Full verbosity.
    #[must_use]
    pub fn debug() -> Self {
        Self {
            enabled: true,
            verbosity: ProvenanceVerbosity::Full,
            history_size: 200,
            why_depth: 10,
            trace_to_stderr: true,
            json_output: false,
        }
    }

    /// Builder method to set enabled state.
    #[must_use]
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Builder method to set verbosity level.
    #[must_use]
    pub fn with_verbosity(mut self, verbosity: ProvenanceVerbosity) -> Self {
        self.verbosity = verbosity;
        self
    }

    /// Builder method to set history size.
    #[must_use]
    pub fn with_history_size(mut self, size: usize) -> Self {
        self.history_size = size;
        self
    }

    /// Builder method to set default why depth.
    #[must_use]
    pub fn with_why_depth(mut self, depth: usize) -> Self {
        self.why_depth = depth;
        self
    }

    /// Builder method to enable/disable stderr tracing.
    #[must_use]
    pub fn with_trace_to_stderr(mut self, trace: bool) -> Self {
        self.trace_to_stderr = trace;
        self
    }

    /// Builder method to enable/disable JSON output.
    #[must_use]
    pub fn with_json_output(mut self, json: bool) -> Self {
        self.json_output = json;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = ObservabilityConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.verbosity, ProvenanceVerbosity::Minimal);
        assert_eq!(config.history_size, 100);
    }

    #[test]
    fn development_config() {
        let config = ObservabilityConfig::development();
        assert!(config.enabled);
        assert_eq!(config.verbosity, ProvenanceVerbosity::Standard);
    }

    #[test]
    fn builder_pattern() {
        let config = ObservabilityConfig::default()
            .with_enabled(true)
            .with_verbosity(ProvenanceVerbosity::Full)
            .with_history_size(200);

        assert!(config.enabled);
        assert_eq!(config.verbosity, ProvenanceVerbosity::Full);
        assert_eq!(config.history_size, 200);
    }
}
