use auwgent_analysis::AnalysisError;
use auwgent_errors::{Diagnostic as CompilerDiagnostic, Severity, Span};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, InitializeParams, InitializeResult, InitializedParams,
    MessageType, Position, Range, ServerCapabilities, TextDocumentContentChangeEvent,
    TextDocumentSyncCapability, TextDocumentSyncKind, Url,
};
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
    documents: RwLock<HashMap<Url, String>>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            documents: RwLock::new(HashMap::new()),
        }
    }

    async fn analyze_and_publish(&self, uri: &Url) {
        let Some(path) = path_from_uri(uri) else {
            self.client
                .log_message(MessageType::ERROR, format!("Unsupported URI: {uri}"))
                .await;
            return;
        };

        let text = {
            let documents = self.documents.read().await;
            documents.get(uri).cloned()
        };

        let Some(text) = text else {
            return;
        };

        let publish = match auwgent_analysis::load_model_from_source_with_imports(&path, &text) {
            Ok(model) => {
                let diagnostics = auwgent_checker::check(&model);
                vec![(uri.clone(), text.clone(), diagnostics)]
            }
            Err(error) => diagnostics_from_error(uri, &path, &text, error),
        };

        for (diagnostic_uri, source, diagnostics) in publish {
            self.client
                .publish_diagnostics(
                    diagnostic_uri,
                    diagnostics
                        .iter()
                        .map(|diagnostic| compiler_diagnostic_to_lsp(diagnostic, &source))
                        .collect(),
                    None,
                )
                .await;
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..ServerCapabilities::default()
            },
            ..InitializeResult::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(
                MessageType::INFO,
                "Auwgent Rust LSP initialized with diagnostics support.",
            )
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.documents.write().await.insert(uri.clone(), text);
        self.analyze_and_publish(&uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(text) = extract_full_text(&params.content_changes) {
            self.documents.write().await.insert(uri.clone(), text);
            self.analyze_and_publish(&uri).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.documents.write().await.remove(&uri);
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}

fn extract_full_text(changes: &[TextDocumentContentChangeEvent]) -> Option<String> {
    changes.last().map(|change| change.text.clone())
}

fn diagnostics_from_error(
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

fn compiler_diagnostic_to_lsp(diagnostic: &CompilerDiagnostic, source: &str) -> Diagnostic {
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

fn span_to_range(span: Span, source: &str) -> Range {
    Range {
        start: offset_to_position(source, span.start),
        end: offset_to_position(source, span.end),
    }
}

fn offset_to_position(source: &str, offset: usize) -> Position {
    let bounded = offset.min(source.len());
    let mut line = 0u32;
    let mut character = 0u32;

    for (byte_index, ch) in source.char_indices() {
        if byte_index >= bounded {
            break;
        }
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += 1;
        }
    }

    Position { line, character }
}

fn path_from_uri(uri: &Url) -> Option<PathBuf> {
    uri.to_file_path().ok()
}

fn uri_from_path(path: &Path) -> Option<Url> {
    Url::from_file_path(path).ok()
}