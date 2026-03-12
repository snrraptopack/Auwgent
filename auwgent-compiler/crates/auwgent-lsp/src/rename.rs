use crate::util::span_to_range;
use auwgent_analysis::RenameEdit;
use std::collections::HashMap;
use tower_lsp::lsp_types::{TextEdit, Url, WorkspaceEdit};

pub fn analysis_rename_to_lsp(edits: Vec<RenameEdit>) -> Option<WorkspaceEdit> {
    if edits.is_empty() {
        return None;
    }

    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    for edit in edits {
        let uri = Url::from_file_path(&edit.path).ok()?;
        changes.entry(uri).or_default().push(TextEdit {
            range: span_to_range(edit.span, &edit.source),
            new_text: edit.new_text,
        });
    }

    for edits in changes.values_mut() {
        edits.sort_by(|left, right| {
            left.range
                .start
                .line
                .cmp(&right.range.start.line)
                .then(left.range.start.character.cmp(&right.range.start.character))
                .then(left.range.end.line.cmp(&right.range.end.line))
                .then(left.range.end.character.cmp(&right.range.end.character))
        });
    }

    Some(WorkspaceEdit {
        changes: Some(changes),
        ..WorkspaceEdit::default()
    })
}