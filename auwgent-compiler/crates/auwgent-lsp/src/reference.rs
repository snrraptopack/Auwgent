use crate::util::span_to_range;
use auwgent_analysis::ReferenceTarget;
use tower_lsp::lsp_types::{Location, Url};

pub fn analysis_reference_to_lsp(target: ReferenceTarget) -> Option<Location> {
    let uri = Url::from_file_path(&target.path).ok()?;
    Some(Location {
        uri,
        range: span_to_range(target.span, &target.source),
    })
}
