use crate::completion::analysis_completion_to_lsp;
use crate::definition::analysis_definition_to_lsp;
use crate::diagnostics::{compiler_diagnostic_to_lsp, diagnostics_from_error};
use crate::hover::analysis_hover_to_lsp;
use crate::reference::analysis_reference_to_lsp;
use crate::rename::analysis_rename_to_lsp;
use crate::util::{apply_content_changes, path_from_uri, position_to_offset};
use auwgent_analysis::AnalysisError;
use auwgent_compile::validate_source_for_compile;
use auwgent_errors::Diagnostic as CompilerDiagnostic;
use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, HoverProviderCapability,
    InitializeParams, InitializeResult, InitializedParams, MessageType, OneOf, ReferenceParams,
    ReferencesOptions, RenameOptions, RenameParams, SaveOptions, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, Url, WorkspaceEdit,
};
use tower_lsp::{Client, LanguageServer};

/// Debounce delay for `did_change` analysis — prevents flickering from
/// transient parse errors while the user is actively typing.
const DEBOUNCE_MS: u64 = 300;

#[derive(Clone)]
struct DocumentState {
    text: String,
    version: i32,
}

/// All shared mutable state, extracted so it can live behind an `Arc` and be
/// cheaply cloned into spawned background tasks.
struct BackendInner {
    client: Client,
    documents: RwLock<HashMap<Url, DocumentState>>,
    /// Monotonic version counter per URI, used to debounce `did_change`.
    change_versions: RwLock<HashMap<Url, u64>>,
}

pub struct Backend {
    inner: Arc<BackendInner>,
}

/// Result produced by the blocking analysis thread.
type AnalysisResult = Vec<(Url, String, Vec<CompilerDiagnostic>)>;

impl BackendInner {
    /// Run analysis off the async runtime (via `spawn_blocking`) and publish
    /// the resulting diagnostics. Panics inside the analysis pipeline are
    /// caught so they never silently kill the handler.
    async fn analyze_and_publish(
        self: &Arc<Self>,
        uri: &Url,
        expected_change_version: Option<u64>,
    ) {
        let Some(path) = path_from_uri(uri) else {
            self.client
                .log_message(MessageType::ERROR, format!("Unsupported URI: {uri}"))
                .await;
            return;
        };

        let (text, _document_version) = {
            let documents = self.documents.read().await;
            let Some(state) = documents.get(uri) else {
                self.client
                    .log_message(
                        MessageType::WARNING,
                        format!("No document content for: {uri}"),
                    )
                    .await;
                return;
            };
            (state.text.clone(), state.version)
        };

        // Clone values for the blocking closure.
        let uri_clone = uri.clone();
        let path_clone = path.clone();
        let text_clone = text.clone();

        let result = tokio::task::spawn_blocking(move || {
            std::panic::catch_unwind(AssertUnwindSafe(|| {
                run_analysis(&uri_clone, &path_clone, &text_clone)
            }))
        })
        .await;

        let publish = match result {
            Ok(Ok(items)) => items,
            Ok(Err(_panic)) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!(
                            "[auwgent-lsp] Analysis panicked for {uri}. Diagnostics may be stale."
                        ),
                    )
                    .await;
                return;
            }
            Err(join_err) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!("[auwgent-lsp] Analysis task failed for {uri}: {join_err}"),
                    )
                    .await;
                return;
            }
        };

        // For did_change-triggered analysis: discard results if a newer
        // keystroke arrived while we were analysing (debounce guard).
        // For did_open/did_save (expected_change_version == None) we always
        // publish — do NOT add a document-version double-check here because
        // VSCode routinely sends did_change immediately after did_open, which
        // would cause every did_open result to be silently dropped.
        if let Some(expected) = expected_change_version {
            let current = {
                let versions = self.change_versions.read().await;
                versions.get(uri).copied().unwrap_or(0)
            };
            if current != expected {
                self.client
                    .log_message(
                        MessageType::LOG,
                        format!("[auwgent-lsp] Dropping stale analysis for {uri} (change {expected} superseded by {current})"),
                    )
                    .await;
                return;
            }
        }

        let diag_count: usize = publish.iter().map(|(_, _, d)| d.len()).sum();
        self.client
            .log_message(
                MessageType::LOG,
                format!("[auwgent-lsp] Publishing {diag_count} diagnostic(s) for {uri}"),
            )
            .await;

        for (diagnostic_uri, source, diagnostics) in publish {
            let publish_uri = diagnostic_uri.clone();
            let lsp_diagnostics: Vec<_> = diagnostics
                .iter()
                .map(|d| compiler_diagnostic_to_lsp(d, &diagnostic_uri, &source))
                .collect();

            for d in &lsp_diagnostics {
                self.client
                    .log_message(
                        MessageType::LOG,
                        format!(
                            "[auwgent-lsp]   diag → uri={} range={}:{}-{}:{} sev={:?} msg=\"{}\"",
                            publish_uri,
                            d.range.start.line,
                            d.range.start.character,
                            d.range.end.line,
                            d.range.end.character,
                            d.severity,
                            d.message.chars().take(120).collect::<String>(),
                        ),
                    )
                    .await;
            }

            // Do not attach a document version to the publish call so VSCode
            // never silently discards diagnostics due to a version mismatch.
            self.client
                .publish_diagnostics(publish_uri, lsp_diagnostics, None)
                .await;
        }
    }
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            inner: Arc::new(BackendInner {
                client,
                documents: RwLock::new(HashMap::new()),
                change_versions: RwLock::new(HashMap::new()),
            }),
        }
    }
}

/// Case-aware path comparison: case-insensitive on Windows, exact on other
/// platforms. This is needed because `std::fs::canonicalize` on Windows
/// uppercases the drive letter (e.g. `C:\`) while VSCode URIs use lowercase
/// (`c:\`), causing direct `==` to fail.
fn paths_match(a: &Path, b: &Path) -> bool {
    fn normalize(p: &Path) -> String {
        let s = p.to_string_lossy();
        if s.starts_with(r"\\?\") {
            s[4..].to_ascii_lowercase()
        } else {
            s.to_ascii_lowercase()
        }
    }

    if cfg!(windows) {
        normalize(a) == normalize(b)
    } else {
        a == b
    }
}

/// Pure, blocking analysis that can safely run on a worker thread.
///
/// **Important**: all diagnostics targeting the root file MUST be published
/// under the original `uri` that VSCode sent us, NOT a URI reconstructed from
/// the canonicalized path — those differ on Windows and VSCode will silently
/// ignore diagnostics for unknown URIs.
fn run_analysis(uri: &Url, path: &PathBuf, text: &str) -> AnalysisResult {
    match validate_source_for_compile(path, text) {
        Ok(validation) => vec![(uri.clone(), text.to_string(), validation.diagnostics)],
        Err(AnalysisError::Lex {
            path: error_path, ..
        })
        | Err(AnalysisError::Parse {
            path: error_path, ..
        }) if paths_match(&error_path, path) => {
            match auwgent_analysis::best_effort_model_from_source_with_imports(path, text) {
                Ok((model, mut diagnostics)) => {
                    diagnostics.extend(auwgent_checker::check(&model));
                    vec![(uri.clone(), text.to_string(), diagnostics)]
                }
                Err(error) => diagnostics_from_error(uri, path, text, error),
            }
        }
        Err(error) => diagnostics_from_error(uri, path, text, error),
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        will_save: None,
                        will_save_wait_until: None,
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                    },
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        ".".to_string(),
                        "@".to_string(),
                        ":".to_string(),
                    ]),
                    ..CompletionOptions::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Right(ReferencesOptions {
                    work_done_progress_options: Default::default(),
                })),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(false),
                    work_done_progress_options: Default::default(),
                })),
                ..ServerCapabilities::default()
            },
            ..InitializeResult::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.inner
            .client
            .log_message(
                MessageType::INFO,
                "Auwgent Rust LSP initialized with real-time diagnostics, completion, hover, definition, references, and rename support.",
            )
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        let version = params.text_document.version;
        self.inner
            .documents
            .write()
            .await
            .insert(uri.clone(), DocumentState { text, version });

        let inner = Arc::clone(&self.inner);
        tokio::spawn(async move {
            inner.analyze_and_publish(&uri, None).await;
        });
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let new_text = {
            let mut documents = self.inner.documents.write().await;
            let Some(document) = documents.get_mut(&uri) else {
                return;
            };

            document.text = apply_content_changes(&document.text, &params.content_changes);
            document.version = params.text_document.version;
            document.text.clone()
        };

        if new_text.is_empty() && params.content_changes.is_empty() {
            return;
        }

        let version = {
            let mut versions = self.inner.change_versions.write().await;
            let v = versions.entry(uri.clone()).or_insert(0);
            *v += 1;
            *v
        };

        // Spawn a background task so this handler returns immediately and the
        // LSP can process other requests (hover, completion, etc.) while the
        // debounce timer and analysis are running.
        let inner = Arc::clone(&self.inner);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(DEBOUNCE_MS)).await;

            let current = {
                let versions = inner.change_versions.read().await;
                versions.get(&uri).copied().unwrap_or(0)
            };
            if version == current {
                inner.analyze_and_publish(&uri, Some(version)).await;
            }
        });
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        // If the save notification includes text, update the in-memory copy.
        if let Some(text) = params.text {
            if let Some(document) = self.inner.documents.write().await.get_mut(&uri) {
                document.text = text;
            }
        }
        // Re-analyze on save as a reliability fallback.
        self.inner.analyze_and_publish(&uri, None).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.inner.documents.write().await.remove(&uri);
        self.inner
            .client
            .publish_diagnostics(uri, Vec::new(), None)
            .await;
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let path = match path_from_uri(&uri) {
            Some(path) => path,
            None => return Ok(None),
        };

        let text = {
            let documents = self.inner.documents.read().await;
            documents.get(&uri).map(|document| document.text.clone())
        };
        let Some(text) = text else {
            return Ok(None);
        };

        let offset = position_to_offset(&text, params.text_document_position.position);
        let items = auwgent_analysis::completions_for_source(&path, &text, offset)
            .into_iter()
            .map(analysis_completion_to_lsp)
            .collect();

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let path = match path_from_uri(&uri) {
            Some(path) => path,
            None => return Ok(None),
        };

        let text = {
            let documents = self.inner.documents.read().await;
            documents.get(&uri).map(|document| document.text.clone())
        };
        let Some(text) = text else {
            return Ok(None);
        };

        let offset = position_to_offset(&text, params.text_document_position_params.position);
        let hover = auwgent_analysis::hover_for_source(&path, &text, offset)
            .map(|hover| analysis_hover_to_lsp(hover, &text));

        Ok(hover)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let path = match path_from_uri(&uri) {
            Some(path) => path,
            None => return Ok(None),
        };

        let text = {
            let documents = self.inner.documents.read().await;
            documents.get(&uri).map(|document| document.text.clone())
        };
        let Some(text) = text else {
            return Ok(None);
        };

        let offset = position_to_offset(&text, params.text_document_position_params.position);
        let definition = auwgent_analysis::definition_for_source(&path, &text, offset)
            .and_then(analysis_definition_to_lsp)
            .map(GotoDefinitionResponse::Scalar);

        Ok(definition)
    }

    async fn references(
        &self,
        params: ReferenceParams,
    ) -> Result<Option<Vec<tower_lsp::lsp_types::Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let path = match path_from_uri(&uri) {
            Some(path) => path,
            None => return Ok(None),
        };

        let text = {
            let documents = self.inner.documents.read().await;
            documents.get(&uri).map(|document| document.text.clone())
        };
        let Some(text) = text else {
            return Ok(None);
        };

        let offset = position_to_offset(&text, params.text_document_position.position);
        let references = auwgent_analysis::references_for_source(&path, &text, offset)
            .into_iter()
            .filter_map(analysis_reference_to_lsp)
            .collect::<Vec<_>>();

        Ok(Some(references))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let path = match path_from_uri(&uri) {
            Some(path) => path,
            None => return Ok(None),
        };

        let text = {
            let documents = self.inner.documents.read().await;
            documents.get(&uri).map(|document| document.text.clone())
        };
        let Some(text) = text else {
            return Ok(None);
        };

        let offset = position_to_offset(&text, params.text_document_position.position);
        let edit = auwgent_analysis::rename_for_source(&path, &text, offset, &params.new_name);

        Ok(analysis_rename_to_lsp(edit))
    }
}
