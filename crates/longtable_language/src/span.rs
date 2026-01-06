//! Source location tracking.
//!
//! `Span` tracks the position of tokens and AST nodes in source code
//! for error reporting and debugging.

/// A span of source text.
///
/// Tracks byte offsets and line/column positions for error reporting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Span {
    /// Byte offset where this span starts.
    pub start: usize,
    /// Byte offset where this span ends (exclusive).
    pub end: usize,
    /// 1-based line number where this span starts.
    pub line: u32,
    /// 1-based column number where this span starts.
    pub column: u32,
}

impl Span {
    /// Creates a new span.
    #[must_use]
    pub const fn new(start: usize, end: usize, line: u32, column: u32) -> Self {
        Self {
            start,
            end,
            line,
            column,
        }
    }

    /// Creates a span at the start of input.
    #[must_use]
    pub const fn at_start() -> Self {
        Self {
            start: 0,
            end: 0,
            line: 1,
            column: 1,
        }
    }

    /// Creates a span covering the range from this span to another.
    #[must_use]
    pub fn to(self, other: Self) -> Self {
        Self {
            start: self.start,
            end: other.end,
            line: self.line,
            column: self.column,
        }
    }

    /// Returns the length of this span in bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end - self.start
    }

    /// Returns true if this span is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Returns the text this span covers in the given source.
    #[must_use]
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start..self.end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_at_start() {
        let span = Span::at_start();
        assert_eq!(span.start, 0);
        assert_eq!(span.end, 0);
        assert_eq!(span.line, 1);
        assert_eq!(span.column, 1);
    }

    #[test]
    fn span_new() {
        let span = Span::new(5, 10, 2, 3);
        assert_eq!(span.start, 5);
        assert_eq!(span.end, 10);
        assert_eq!(span.line, 2);
        assert_eq!(span.column, 3);
    }

    #[test]
    fn span_to() {
        let a = Span::new(0, 5, 1, 1);
        let b = Span::new(5, 10, 1, 6);
        let combined = a.to(b);
        assert_eq!(combined.start, 0);
        assert_eq!(combined.end, 10);
        assert_eq!(combined.line, 1);
        assert_eq!(combined.column, 1);
    }

    #[test]
    fn span_len() {
        let span = Span::new(5, 10, 1, 1);
        assert_eq!(span.len(), 5);
    }

    #[test]
    fn span_text() {
        let source = "hello world";
        let span = Span::new(0, 5, 1, 1);
        assert_eq!(span.text(source), "hello");
    }
}
