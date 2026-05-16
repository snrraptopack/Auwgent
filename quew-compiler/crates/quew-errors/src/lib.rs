//! # quew-errors
//!
//! Foundation crate for the quew compiler diagnostic system.
//!
//! ## Responsibilities
//!
//! - [`Span`]: a byte-range `(start, end)` into a source file. Every AST node
//!   carries one. Used for precise error highlighting.
//! - [`Diagnostic`]: a structured error/warning/info message with a severity,
//!   primary label, optional secondary labels, and a help note.
//! - [`render`]: renders a [`Diagnostic`] to the terminal using `ariadne`.
//!
//! ## Design rules
//!
//! 1. This crate has **no dependency on any other quew crate** — it is the
//!    absolute base of the dependency graph.
//! 2. All types in this crate are `Send + Sync` so they can be collected across
//!    threads and surfaced to the user from any compilation context.
//! 3. `Span` is `Copy` — pass it everywhere without thinking about ownership.

/// A half-open byte range `[start, end)` into a source file.
///
/// `start` and `end` are byte offsets, not character or line indices.
/// Use `ariadne`'s `Source` to convert offsets to line/column for display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    /// Create a new span from a half-open byte range.
    #[inline]
    pub fn new(start: usize, end: usize) -> Self {
        debug_assert!(start <= end, "Span::new: start > end");
        Self { start, end }
    }

    /// A zero-length span at a given offset. Useful for synthetic nodes.
    #[inline]
    pub fn at(offset: usize) -> Self {
        Self { start: offset, end: offset }
    }

    /// Merge two spans into the smallest span that covers both.
    #[inline]
    pub fn cover(self, other: Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Number of bytes this span covers.
    #[inline]
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// True when the span covers no bytes.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// Convert a `logos`-style `Range<usize>` (which is what `logos` returns per
/// token) into a [`Span`].
impl From<std::ops::Range<usize>> for Span {
    fn from(r: std::ops::Range<usize>) -> Self {
        Span::new(r.start, r.end)
    }
}

/// Diagnostic severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// A single compiler diagnostic ready to be rendered.
///
/// Construct one and then call [`render`] to print it.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Severity determines color and the exit code of the compiler.
    pub severity: Severity,
    /// Short one-line message. Shown in the header.
    pub message: String,
    /// The primary span that caused this diagnostic.
    pub primary_span: Span,
    /// A label attached to the primary span (e.g., "defined here").
    pub primary_label: Option<String>,
    /// Optional additional spans with labels (e.g., "previously used here").
    pub secondary: Vec<(Span, String)>,
    /// A free-text hint shown after the diagnostic (e.g., "try removing…").
    pub help: Option<String>,
    /// A stable machine-readable error code (e.g., `"E001"`).
    pub code: Option<String>,
}

impl Diagnostic {
    /// Create a minimal error diagnostic.
    pub fn error(message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            primary_span: span,
            primary_label: None,
            secondary: Vec::new(),
            help: None,
            code: None,
        }
    }

    /// Create a minimal warning diagnostic.
    pub fn warning(message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            primary_span: span,
            primary_label: None,
            secondary: Vec::new(),
            help: None,
            code: None,
        }
    }

    /// Attach a label to the primary span.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.primary_label = Some(label.into());
        self
    }

    /// Attach a secondary span with a label.
    pub fn with_secondary(mut self, span: Span, label: impl Into<String>) -> Self {
        self.secondary.push((span, label.into()));
        self
    }

    /// Attach a help note shown below the diagnostic.
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Attach a stable error code (e.g., `"E001"`).
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Returns `true` if this diagnostic will cause a compilation failure.
    pub fn is_fatal(&self) -> bool {
        self.severity == Severity::Error
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_new_and_cover() {
        let a = Span::new(0, 5);
        let b = Span::new(3, 10);
        let covered = a.cover(b);
        assert_eq!(covered.start, 0);
        assert_eq!(covered.end, 10);
    }

    #[test]
    fn span_len_and_empty() {
        let s = Span::new(4, 4);
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);

        let s2 = Span::new(2, 7);
        assert!(!s2.is_empty());
        assert_eq!(s2.len(), 5);
    }

    #[test]
    fn span_from_range() {
        let r = 3..9;
        let s = Span::from(r);
        assert_eq!(s.start, 3);
        assert_eq!(s.end, 9);
    }

    #[test]
    fn diagnostic_builder_chain() {
        let d = Diagnostic::error("undefined variable `x`", Span::new(10, 11))
            .with_label("used here")
            .with_help("did you mean `y`?")
            .with_code("E001");
        assert!(d.is_fatal());
        assert_eq!(d.code.as_deref(), Some("E001"));
        assert!(d.help.is_some());
        assert_eq!(d.secondary.len(), 0);
    }

    #[test]
    fn warning_is_not_fatal() {
        let d = Diagnostic::warning("unused variable", Span::new(0, 3));
        assert!(!d.is_fatal());
    }
}
