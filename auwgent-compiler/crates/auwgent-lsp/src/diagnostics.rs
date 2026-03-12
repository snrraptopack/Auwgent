use auwgent_analysis::AnalysisError;
use auwgent_errors::{Diagnostic as CompilerDiagnostic, Severity, Span};
use std::path::Path;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Url};
use crate::util::span_to_range;

pub fn diagnostics_from_error(
    root_uri: &Url,
    root_path: &Path,
    root_source: &str,
    error: AnalysisError,
) -> Vec<(Url, String, Vec<CompilerDiagnostic>)> {
    match error {
        AnalysisError::Lex {
            path,
            source,
            diagnostics,
        }
        | AnalysisError::Parse {
            path,
            source,
            diagnostics,
        } => {
            let uri = uri_from_path(&path).unwrap_or_else(|| root_uri.clone());
            let mut published = vec![(uri, source, diagnostics)];
            if path != root_path {
                published.push((root_uri.clone(), root_source.to_string(), Vec::new()));
            }
            published
        }
        AnalysisError::ResolveImport {
            import_path,
            message,
            span,
            ..
        } => vec![(
            root_uri.clone(),
            root_source.to_string(),
            vec![CompilerDiagnostic {
                severity: Severity::Error,
                message: format!("Could not resolve import '{import_path}': {message}"),
                span: span.unwrap_or(Span::new(0, 0)),
                labels: Vec::new(),
                help: None,
            }],
        )],
        AnalysisError::CanonicalizeRoot { message, .. }
        | AnalysisError::ReadFile { message, .. } => vec![(
            root_uri.clone(),
            root_source.to_string(),
            vec![CompilerDiagnostic {
                severity: Severity::Error,
                message,
                span: Span::new(0, 0),
                labels: Vec::new(),
                help: None,
            }],
        )],
    }
}

pub fn compiler_diagnostic_to_lsp(diagnostic: &CompilerDiagnostic, source: &str) -> Diagnostic {
    Diagnostic {
        range: span_to_range(diagnostic.span, source),
        severity: Some(match diagnostic.severity {
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
            Severity::Info => DiagnosticSeverity::INFORMATION,
        }),
        message: diagnostic.message.clone(),
        source: Some("auwgent-rust".to_string()),
        ..Diagnostic::default()
    }
}

fn uri_from_path(path: &Path) -> Option<Url> {
    Url::from_file_path(path).ok()
}