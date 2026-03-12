use std::path::PathBuf;
use auwgent_errors::Span;
use tower_lsp::lsp_types::{Position, Range, TextDocumentContentChangeEvent, Url};

pub fn apply_content_changes(current: &str, changes: &[TextDocumentContentChangeEvent]) -> String {
    let mut text = current.to_string();

    for change in changes {
        if let Some(range) = change.range {
            let start = position_to_offset(&text, range.start);
            let end = position_to_offset(&text, range.end);
            text.replace_range(start..end, &change.text);
        } else {
            text = change.text.clone();
        }
    }

    text
}

pub fn position_to_offset(source: &str, position: Position) -> usize {
    let mut line = 0u32;
    let mut character = 0u32;

    for (index, ch) in source.char_indices() {
        if line == position.line && character == position.character {
            return index;
        }

        if ch == '\n' {
            line += 1;
            character = 0;
            if line > position.line {
                return index;
            }
        } else {
            character += 1;
        }
    }

    source.len()
}

pub fn path_from_uri(uri: &Url) -> Option<PathBuf> {
    uri.to_file_path().ok()
}

pub fn offset_to_position(source: &str, offset: usize) -> Position {
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

pub fn span_to_range(span: Span, source: &str) -> Range {
    Range {
        start: offset_to_position(source, span.start),
        end: offset_to_position(source, span.end),
    }
}