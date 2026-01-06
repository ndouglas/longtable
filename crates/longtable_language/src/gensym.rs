//! Gensym generator for macro hygiene.
//!
//! Generates unique symbols to prevent variable capture in macro expansions.
//!
//! # Example
//!
//! ```
//! use longtable_language::gensym::GensymGenerator;
//!
//! let generator = GensymGenerator::new();
//! let sym1 = generator.gensym("x");  // "x__G__N" (unique suffix)
//! let sym2 = generator.gensym("x");  // "x__G__N+1"
//! let sym3 = generator.gensym("y");  // "y__G__N+2"
//! assert_ne!(sym1, sym2);
//! ```

use std::sync::atomic::{AtomicU64, Ordering};

/// Global counter for generating unique gensym IDs.
static GENSYM_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generator for unique symbols.
///
/// Used by the macro expander to generate hygienically unique
/// symbol names that won't capture user variables.
#[derive(Clone, Debug, Default)]
pub struct GensymGenerator {
    /// Prefix for generated symbols (default: "__G__").
    prefix: String,
}

impl GensymGenerator {
    /// Creates a new gensym generator with the default prefix.
    #[must_use]
    pub fn new() -> Self {
        Self {
            prefix: "__G__".to_string(),
        }
    }

    /// Creates a new gensym generator with a custom prefix.
    #[must_use]
    pub fn with_prefix(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }

    /// Generates a unique symbol based on the given base name.
    ///
    /// The generated symbol has the form `{base}{prefix}{id}`.
    #[must_use]
    pub fn gensym(&self, base: &str) -> String {
        let id = GENSYM_COUNTER.fetch_add(1, Ordering::SeqCst);
        format!("{base}{}{id}", self.prefix)
    }

    /// Expands a gensym pattern (e.g., "x#") to a unique symbol.
    ///
    /// If the name ends with `#`, replaces it with a unique suffix.
    /// Otherwise returns the name unchanged.
    #[must_use]
    pub fn expand_pattern(&self, name: &str) -> String {
        if let Some(base) = name.strip_suffix('#') {
            self.gensym(base)
        } else {
            name.to_string()
        }
    }

    /// Checks if a name is a gensym pattern (ends with `#`).
    #[must_use]
    pub fn is_gensym_pattern(name: &str) -> bool {
        name.ends_with('#')
    }

    /// Resets the global counter (for testing only).
    ///
    /// # Safety
    ///
    /// This should only be called in tests, as it can cause
    /// symbol collisions if used in production code.
    #[cfg(test)]
    pub fn reset_counter() {
        GENSYM_COUNTER.store(0, Ordering::SeqCst);
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gensym_generates_unique_symbols() {
        GensymGenerator::reset_counter();
        let generator = GensymGenerator::new();

        let sym1 = generator.gensym("x");
        let sym2 = generator.gensym("x");
        let sym3 = generator.gensym("y");

        assert_ne!(sym1, sym2);
        assert_ne!(sym2, sym3);
        assert!(sym1.starts_with("x__G__"));
        assert!(sym3.starts_with("y__G__"));
    }

    #[test]
    fn gensym_with_custom_prefix() {
        let generator = GensymGenerator::with_prefix("_gensym_");
        let sym = generator.gensym("foo");
        assert!(sym.starts_with("foo_gensym_"));
    }

    #[test]
    fn expand_pattern_with_hash() {
        GensymGenerator::reset_counter();
        let generator = GensymGenerator::new();

        let expanded = generator.expand_pattern("temp#");
        assert!(expanded.starts_with("temp__G__"));
    }

    #[test]
    fn expand_pattern_without_hash() {
        let generator = GensymGenerator::new();

        let expanded = generator.expand_pattern("normal");
        assert_eq!(expanded, "normal");
    }

    #[test]
    fn is_gensym_pattern_detection() {
        assert!(GensymGenerator::is_gensym_pattern("x#"));
        assert!(GensymGenerator::is_gensym_pattern("temp#"));
        assert!(GensymGenerator::is_gensym_pattern("x##")); // Ends with #
        assert!(!GensymGenerator::is_gensym_pattern("x"));
        assert!(!GensymGenerator::is_gensym_pattern("x#y"));
    }

    #[test]
    fn gensym_increments_counter() {
        GensymGenerator::reset_counter();
        let generator = GensymGenerator::new();

        let sym1 = generator.gensym("a");
        let sym2 = generator.gensym("b");

        // Extract the numeric suffixes
        let num1: u64 = sym1.split("__G__").nth(1).unwrap().parse().unwrap();
        let num2: u64 = sym2.split("__G__").nth(1).unwrap().parse().unwrap();

        assert_eq!(num2, num1 + 1);
    }
}
