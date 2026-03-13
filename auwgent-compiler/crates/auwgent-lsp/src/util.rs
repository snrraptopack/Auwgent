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
    let mut utf16_col = 0u32;
    let mut line_start = 0usize;

    for (index, ch) in source.char_indices() {
        if line == position.line {
            if utf16_col >= position.character {
                return index;
            }
        }

        if ch == '\n' {
            if line == position.line {
                return index;
            }
            line += 1;
            utf16_col = 0;
            line_start = index + ch.len_utf8();
            continue;
        }

        if line == position.line {
            utf16_col += ch.len_utf16() as u32;
        }
    }

    if line == position.line {
        return source.len();
    }

    line_start
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
            character += ch.len_utf16() as u32;
        }
    }

    Position { line, character }
}

pub fn span_to_range(span: Span, source: &str) -> Range {
    let bounded_start = span.start.min(source.len());
    let bounded_end = span.end.min(source.len());
    let start = bounded_start.min(bounded_end);
    let mut end = bounded_end.max(start);

    if start == end {
        if let Some(ch) = source[start..].chars().next() {
            if ch != '\n' {
                end = (start + ch.len_utf8()).min(source.len());
            }
        } else if start > 0 {
            if let Some(ch) = source[..start].chars().next_back() {
                let prev_start = start.saturating_sub(ch.len_utf8());
                if ch != '\n' {
                    return Range {
                        start: offset_to_position(source, prev_start),
                        end: offset_to_position(source, start),
                    };
                }
            }
        }
    }

    Range {
        start: offset_to_position(source, start),
        end: offset_to_position(source, end),
    }
}