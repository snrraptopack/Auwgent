//! # auwgent-errors
//!
//! Shared error and diagnostic types for the Auwgent compiler pipeline.
//! All crates report errors through these types, enabling uniform
//! error rendering via `ariadne`.

use ariadne::{Color, Label as AriadneLabel, Report, ReportKind, Source};

/// Byte-offset span in source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// Inclusive start byte offset.
    pub start: usize,
    /// Exclusive end byte offset.
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Merge two spans into one covering both.
    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

impl From<Span> for std::ops::Range<usize> {
    fn from(span: Span) -> Self {
        span.start..span.end
    }
}

impl From<std::ops::Range<usize>> for Span {
    fn from(range: std::ops::Range<usize>) -> Self {
        Span {
            start: range.start,
            end: range.end,
        }
    }
}

/// Severity level for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// A compiler diagnostic with source location.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    /// Optional labels pointing to related locations.
    pub labels: Vec<Label>,
    /// Optional help/hint text.
    pub help: Option<String>,
}

/// A labeled span for additional context in diagnostics.
#[derive(Debug, Clone)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            span,
            labels: Vec::new(),
            help: None,
        }
    }

    pub fn warning(message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            span,
            labels: Vec::new(),
            help: None,
        }
    }

    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label {
            span,
            message: message.into(),
        });
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

/// Result type alias for the compiler pipeline.
pub type CompileResult<T> = Result<T, Vec<Diagnostic>>;

// ── Pretty Rendering ─────────────────────────────────────────────────────

/// Render a single diagnostic to stderr using ariadne.
pub fn render_diagnostic(diag: &Diagnostic, filename: &str, source: &str) {
    let kind = match diag.severity {
        Severity::Error => ReportKind::Error,
        Severity::Warning => ReportKind::Warning,
        Severity::Info => ReportKind::Advice,
    };

    let color = match diag.severity {
        Severity::Error => Color::Red,
        Severity::Warning => Color::Yellow,
        Severity::Info => Color::Blue,
    };

    let mut report =
        Report::<(&str, std::ops::Range<usize>)>::build(kind, filename, diag.span.start)
            .with_message(&diag.message);

    // Add the primary label
    report = report.with_label(
        AriadneLabel::new((filename, diag.span.start..diag.span.end))
            .with_message(&diag.message)
            .with_color(color),
    );

    // Add secondary labels
    for label in &diag.labels {
        report = report.with_label(
            AriadneLabel::new((filename, label.span.start..label.span.end))
                .with_message(&label.message)
                .with_color(Color::Cyan),
        );
    }

    if let Some(help) = &diag.help {
        report = report.with_help(help);
    }

    report
        .finish()
        .eprint((filename, Source::from(source)))
        .unwrap();
}

/// Render all diagnostics to stderr. Returns true if any errors were found.
pub fn render_diagnostics(diags: &[Diagnostic], filename: &str, source: &str) -> bool {
    let mut has_errors = false;
    for d in diags {
        if d.severity == Severity::Error {
            has_errors = true;
        }
        render_diagnostic(d, filename, source);
    }
    has_errors
}

/// Render parse errors (plain strings) as diagnostics.
/// Uses contextual help to make messages friendlier.
pub fn render_parse_errors(errors: &[String], filename: &str, source: &str) {
    for err in errors {
        let msg = humanize_parse_error(err);
        let help = get_parse_help(&msg);

        let report =
            Report::<(&str, std::ops::Range<usize>)>::build(ReportKind::Error, filename, 0)
                .with_message(&msg)
                .with_help(help)
                .finish();

        report.eprint((filename, Source::from(source))).unwrap();
    }
}

/// Render lex errors to stderr.
pub fn render_lex_errors(errors: &[Diagnostic], filename: &str, source: &str) {
    for err in errors {
        render_diagnostic(err, filename, source);
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────

/// Make Rust debug token names friendly for users.
fn humanize_parse_error(err: &str) -> String {
    err.replace("Ident", "identifier")
        .replace("LBrace", "'{'")
        .replace("RBrace", "'}'")
        .replace("LParen", "'('")
        .replace("RParen", "')'")
        .replace("LBracket", "'['")
        .replace("RBracket", "']'")
        .replace("Colon", "':'")
        .replace("Comma", "','")
        .replace("Eq", "'='")
        .replace("StringType", "'string'")
        .replace("NumberType", "'number'")
        .replace("BooleanType", "'boolean'")
}

/// Provide contextual help for common parse errors.
fn get_parse_help(msg: &str) -> String {
    if msg.contains("expected '{'") {
        "Every agent, helper, and config block must be wrapped in { }".into()
    } else if msg.contains("expected identifier") {
        "Names must start with a letter or underscore.".into()
    } else if msg.contains("expected ':'") {
        "Properties need a colon between name and type, e.g. `name: string`".into()
    } else if msg.contains("expected string") {
        "Strings must be in double (\") or single (') quotes.".into()
    } else if msg.contains("expected type") {
        "Valid types: string, number, boolean, TypeName, or { prop: type }".into()
    } else if msg.contains("unexpected token") {
        "An agent file can contain: agent, helper, type, prompt, model, import".into()
    } else {
        "Check your .agent file for typos or missing tokens.".into()
    }
}
