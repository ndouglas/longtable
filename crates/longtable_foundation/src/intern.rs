//! String interning for symbols and keywords.
//!
//! Symbols and keywords are interned to enable fast equality comparison
//! and reduced memory usage for repeated strings.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Interned symbol identifier.
///
/// Symbols are identifiers like `foo`, `bar`, `?entity`.
/// They are interned for fast comparison.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SymbolId(pub(crate) u32);

impl SymbolId {
    /// Returns the raw index of this symbol.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for SymbolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SymbolId({})", self.0)
    }
}

/// Interned keyword identifier.
///
/// Keywords are identifiers prefixed with `:`, like `:health`, `:position/x`.
/// They are interned for fast comparison.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct KeywordId(pub(crate) u32);

impl KeywordId {
    /// Returns the raw index of this keyword.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.0
    }

    // =========================================================================
    // Reserved Keywords
    // =========================================================================
    // These are always interned at startup with fixed indices.

    /// Reserved keyword for relationship type: `:rel/type`
    pub const REL_TYPE: KeywordId = KeywordId(0);

    /// Reserved keyword for relationship source: `:rel/source`
    pub const REL_SOURCE: KeywordId = KeywordId(1);

    /// Reserved keyword for relationship target: `:rel/target`
    pub const REL_TARGET: KeywordId = KeywordId(2);

    /// Reserved keyword for generic value field: `:value`
    pub const VALUE: KeywordId = KeywordId(3);
}

impl fmt::Debug for KeywordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KeywordId({})", self.0)
    }
}

/// Interner for strings, symbols, and keywords.
///
/// This is a simple interner that maps strings to unique IDs and back.
/// It is not thread-safe; use external synchronization if needed.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Interner {
    /// String storage (shared across symbols and keywords).
    strings: Vec<Arc<str>>,
    /// Map from string to index.
    string_to_index: HashMap<Arc<str>, u32>,
    /// Symbol indices (subset of strings that are symbols).
    symbols: Vec<u32>,
    /// Map from symbol string to `SymbolId`.
    symbol_map: HashMap<Arc<str>, SymbolId>,
    /// Keyword indices (subset of strings that are keywords).
    keywords: Vec<u32>,
    /// Map from keyword string to `KeywordId`.
    keyword_map: HashMap<Arc<str>, KeywordId>,
}

impl Interner {
    /// Reserved keywords that are pre-interned at startup.
    const RESERVED_KEYWORDS: &'static [&'static str] = &[
        "rel/type",   // KeywordId(0) = REL_TYPE
        "rel/source", // KeywordId(1) = REL_SOURCE
        "rel/target", // KeywordId(2) = REL_TARGET
        "value",      // KeywordId(3) = VALUE
    ];

    /// Creates a new interner with reserved keywords pre-interned.
    #[must_use]
    pub fn new() -> Self {
        let mut interner = Self::default();

        // Pre-intern reserved keywords at fixed indices
        for (i, &kw) in Self::RESERVED_KEYWORDS.iter().enumerate() {
            let id = interner.intern_keyword(kw);
            debug_assert_eq!(
                id.0 as usize, i,
                "Reserved keyword '{}' should have index {}, got {}",
                kw, i, id.0
            );
        }

        interner
    }

    /// Interns a string, returning its index.
    fn intern_string(&mut self, s: &str) -> u32 {
        if let Some(&idx) = self.string_to_index.get(s) {
            return idx;
        }

        let idx = u32::try_from(self.strings.len()).expect("too many interned strings");
        let arc: Arc<str> = s.into();
        self.strings.push(arc.clone());
        self.string_to_index.insert(arc, idx);
        idx
    }

    /// Gets a string by its index.
    #[must_use]
    pub fn get_string(&self, idx: u32) -> Option<&str> {
        self.strings.get(idx as usize).map(AsRef::as_ref)
    }

    /// Interns a symbol, returning its [`SymbolId`].
    ///
    /// # Panics
    ///
    /// Panics if the number of interned symbols exceeds `u32::MAX`.
    pub fn intern_symbol(&mut self, s: &str) -> SymbolId {
        if let Some(&id) = self.symbol_map.get(s) {
            return id;
        }

        let string_idx = self.intern_string(s);
        let symbol_idx = u32::try_from(self.symbols.len()).expect("too many symbols");
        self.symbols.push(string_idx);

        let id = SymbolId(symbol_idx);
        let arc: Arc<str> = s.into();
        self.symbol_map.insert(arc, id);
        id
    }

    /// Gets the string for a symbol.
    #[must_use]
    pub fn get_symbol(&self, id: SymbolId) -> Option<&str> {
        self.symbols
            .get(id.0 as usize)
            .and_then(|&idx| self.get_string(idx))
    }

    /// Interns a keyword, returning its [`KeywordId`].
    ///
    /// The string should NOT include the leading `:`.
    ///
    /// # Panics
    ///
    /// Panics if the number of interned keywords exceeds `u32::MAX`.
    pub fn intern_keyword(&mut self, s: &str) -> KeywordId {
        if let Some(&id) = self.keyword_map.get(s) {
            return id;
        }

        let string_idx = self.intern_string(s);
        let keyword_idx = u32::try_from(self.keywords.len()).expect("too many keywords");
        self.keywords.push(string_idx);

        let id = KeywordId(keyword_idx);
        let arc: Arc<str> = s.into();
        self.keyword_map.insert(arc, id);
        id
    }

    /// Gets the string for a keyword (without the leading `:`).
    #[must_use]
    pub fn get_keyword(&self, id: KeywordId) -> Option<&str> {
        self.keywords
            .get(id.0 as usize)
            .and_then(|&idx| self.get_string(idx))
    }

    /// Returns the number of interned symbols.
    #[must_use]
    pub fn symbol_count(&self) -> usize {
        self.symbols.len()
    }

    /// Returns the number of interned keywords.
    #[must_use]
    pub fn keyword_count(&self) -> usize {
        self.keywords.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_symbol_deduplicates() {
        let mut interner = Interner::new();

        let a = interner.intern_symbol("foo");
        let b = interner.intern_symbol("foo");
        let c = interner.intern_symbol("bar");

        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(interner.symbol_count(), 2);
    }

    #[test]
    fn intern_keyword_deduplicates() {
        let mut interner = Interner::new();
        let reserved_count = Interner::RESERVED_KEYWORDS.len();

        let a = interner.intern_keyword("health");
        let b = interner.intern_keyword("health");
        let c = interner.intern_keyword("position");

        assert_eq!(a, b);
        assert_ne!(a, c);
        // 3 reserved + 2 new = 5
        assert_eq!(interner.keyword_count(), reserved_count + 2);
    }

    #[test]
    fn reserved_keywords_have_fixed_indices() {
        let interner = Interner::new();

        // Reserved keywords should be pre-interned with fixed indices
        assert_eq!(KeywordId::REL_TYPE.index(), 0);
        assert_eq!(KeywordId::REL_SOURCE.index(), 1);
        assert_eq!(KeywordId::REL_TARGET.index(), 2);
        assert_eq!(KeywordId::VALUE.index(), 3);

        // And resolve to the correct strings
        assert_eq!(interner.get_keyword(KeywordId::REL_TYPE), Some("rel/type"));
        assert_eq!(
            interner.get_keyword(KeywordId::REL_SOURCE),
            Some("rel/source")
        );
        assert_eq!(
            interner.get_keyword(KeywordId::REL_TARGET),
            Some("rel/target")
        );
        assert_eq!(interner.get_keyword(KeywordId::VALUE), Some("value"));
    }

    #[test]
    fn re_interning_reserved_keyword_returns_same_id() {
        let mut interner = Interner::new();

        // Re-interning a reserved keyword should return the same ID
        let rel_type = interner.intern_keyword("rel/type");
        assert_eq!(rel_type, KeywordId::REL_TYPE);

        let rel_source = interner.intern_keyword("rel/source");
        assert_eq!(rel_source, KeywordId::REL_SOURCE);

        let rel_target = interner.intern_keyword("rel/target");
        assert_eq!(rel_target, KeywordId::REL_TARGET);

        let value = interner.intern_keyword("value");
        assert_eq!(value, KeywordId::VALUE);
    }

    #[test]
    fn get_symbol_string() {
        let mut interner = Interner::new();

        let id = interner.intern_symbol("my-symbol");
        assert_eq!(interner.get_symbol(id), Some("my-symbol"));
    }

    #[test]
    fn get_keyword_string() {
        let mut interner = Interner::new();

        let id = interner.intern_keyword("health/current");
        assert_eq!(interner.get_keyword(id), Some("health/current"));
    }

    #[test]
    fn symbols_and_keywords_independent() {
        let mut interner = Interner::new();
        #[allow(clippy::cast_possible_truncation)]
        let reserved_count = Interner::RESERVED_KEYWORDS.len() as u32;

        // Same string can be both a symbol and keyword
        let sym = interner.intern_symbol("foo");
        let kw = interner.intern_keyword("foo");

        // Symbols start at 0 (no reserved symbols)
        assert_eq!(sym.0, 0);
        // Keywords start after reserved keywords
        assert_eq!(kw.0, reserved_count);

        // But resolve to same string
        assert_eq!(interner.get_symbol(sym), interner.get_keyword(kw));
    }
}
