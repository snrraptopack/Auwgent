use auwgent_analysis::{CompletionItem, CompletionItemKind};
use tower_lsp::lsp_types::{CompletionItem as LspCompletionItem, CompletionItemKind as LspKind};

pub fn analysis_completion_to_lsp(item: CompletionItem) -> LspCompletionItem {
    LspCompletionItem {
        label: item.label,
        detail: item.detail,
        kind: Some(match item.kind {
            CompletionItemKind::Keyword => LspKind::KEYWORD,
            CompletionItemKind::Variable => LspKind::VARIABLE,
            CompletionItemKind::Field | CompletionItemKind::Context => LspKind::FIELD,
            CompletionItemKind::Tool => LspKind::FUNCTION,
            CompletionItemKind::Helper => LspKind::METHOD,
            CompletionItemKind::Prompt => LspKind::TEXT,
        }),
        ..LspCompletionItem::default()
    }
}
