use crate::completion::analysis_completion_to_lsp;
use crate::definition::analysis_definition_to_lsp;
use crate::diagnostics::{compiler_diagnostic_to_lsp, diagnostics_from_error};
use crate::hover::analysis_hover_to_lsp;
use crate::reference::analysis_reference_to_lsp;
use crate::util::{extract_full_text, path_from_uri, position_to_offset};
use std::collections::HashMap;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverParams, HoverProviderCapability, InitializeParams,
    InitializeResult, InitializedParams, MessageType, OneOf, ReferencesOptions,
    ReferenceParams, ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, Url,
};
use tower_lsp::{Client, LanguageServer};

pub struct Backend {
    client: Client,
    documents: RwLock<HashMap<Url, String>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
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
                completion_provider: Some(CompletionOptions::default()),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Right(ReferencesOptions {
                    work_done_progress_options: Default::default(),
                })),
                ..ServerCapabilities::default()
            },
            ..InitializeResult::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(
                MessageType::INFO,
                "Auwgent Rust LSP initialized with diagnostics, completion, hover, definition, and references support.",
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

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let path = match path_from_uri(&uri) {
            Some(path) => path,
            None => return Ok(None),
        };

        let text = {
            let documents = self.documents.read().await;
            documents.get(&uri).cloned()
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
            let documents = self.documents.read().await;
            documents.get(&uri).cloned()
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
            let documents = self.documents.read().await;
            documents.get(&uri).cloned()
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

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<tower_lsp::lsp_types::Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let path = match path_from_uri(&uri) {
            Some(path) => path,
            None => return Ok(None),
        };

        let text = {
            let documents = self.documents.read().await;
            documents.get(&uri).cloned()
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
}