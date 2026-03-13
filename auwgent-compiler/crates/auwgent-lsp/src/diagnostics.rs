use auwgent_analysis::AnalysisError;
use auwgent_errors::{Diagnostic as CompilerDiagnostic, Severity, Span};
use std::path::Path;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, Url};
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
            // On Windows, `fs::canonicalize` uppercases the drive letter
            // (`C:\`) while VSCode URIs use lowercase (`c:\`).  Always
            // prefer `root_uri` for the root file so VSCode matches the
            // diagnostics to the open document.
            let is_root = if cfg!(windows) {
                let normalize = |p: &Path| {
                    let s = p.to_string_lossy();
                    if s.starts_with(r"\\?\") {
                        s[4..].to_ascii_lowercase()
                    } else {
                        s.to_ascii_lowercase()
                    }
                };
                normalize(&path) == normalize(root_path)
            } else {
                path == *root_path
            };

            if is_root {
                // Error is in the file the user is editing — publish
                // using the original URI that VSCode gave us.
                vec![(root_uri.clone(), source, diagnostics)]
            } else {
                // Error is in an imported file — publish under its own
                // URI and clear diagnostics for the root file.
                let uri = uri_from_path(&path).unwrap_or_else(|| root_uri.clone());
                vec![
                    (uri, source, diagnostics),
                    (root_uri.clone(), root_source.to_string(), Vec::new()),
                ]
            }
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

pub fn compiler_diagnostic_to_lsp(
    diagnostic: &CompilerDiagnostic,
    uri: &Url,
    source: &str,
) -> Diagnostic {
    let related_information = diagnostic.labels.is_empty().then(Vec::new).unwrap_or_else(|| {
        diagnostic
            .labels
            .iter()
            .map(|label| DiagnosticRelatedInformation {
                location: Location {
                    uri: uri.clone(),
                    range: span_to_range(label.span, source),
                },
                message: label.message.clone(),
            })
            .collect()
    });

    let message = match &diagnostic.help {
        Some(help) if !help.is_empty() => format!("{}\n\nHelp: {help}", diagnostic.message),
        _ => diagnostic.message.clone(),
    };

    Diagnostic {
        range: span_to_range(diagnostic.span, source),
        severity: Some(match diagnostic.severity {
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
            Severity::Info => DiagnosticSeverity::INFORMATION,
        }),
        message,
        related_information: (!related_information.is_empty()).then_some(related_information),
        source: Some("auwgent-rust".to_string()),
        ..Diagnostic::default()
    }
}

fn uri_from_path(path: &Path) -> Option<Url> {
    Url::from_file_path(path).ok()
}