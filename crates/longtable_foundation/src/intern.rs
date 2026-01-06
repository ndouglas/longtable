//! String interning for symbols and keywords.
//!
//! Symbols and keywords are interned to enable fast equality comparison
//! and reduced memory usage for repeated strings.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

/// Interned symbol identifier.
///
/// Symbols are identifiers like `foo`, `bar`, `?entity`.
/// They are interned for fast comparison.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
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
pub struct KeywordId(pub(crate) u32);

impl KeywordId {
    /// Returns the raw index of this keyword.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.0
    }
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
    /// Creates a new empty interner.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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

        let a = interner.intern_keyword("health");
        let b = interner.intern_keyword("health");
        let c = interner.intern_keyword("position");

        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(interner.keyword_count(), 2);
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

        // Same string can be both a symbol and keyword
        let sym = interner.intern_symbol("foo");
        let kw = interner.intern_keyword("foo");

        // They have independent ID spaces
        assert_eq!(sym.0, 0);
        assert_eq!(kw.0, 0);

        // But resolve to same string
        assert_eq!(interner.get_symbol(sym), interner.get_keyword(kw));
    }
}
