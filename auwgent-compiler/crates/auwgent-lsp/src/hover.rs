use crate::util::span_to_range;
use auwgent_analysis::HoverInfo;
use tower_lsp::lsp_types::{Hover, HoverContents, MarkedString};

pub fn analysis_hover_to_lsp(hover: HoverInfo, source: &str) -> Hover {
    Hover {
        contents: HoverContents::Scalar(MarkedString::String(hover.contents)),
        range: Some(span_to_range(hover.span, source)),
    }
}