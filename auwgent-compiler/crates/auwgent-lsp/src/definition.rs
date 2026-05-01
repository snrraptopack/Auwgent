use crate::util::span_to_range;
use auwgent_analysis::DefinitionTarget;
use tower_lsp::lsp_types::{Location, Url};

pub fn analysis_definition_to_lsp(target: DefinitionTarget) -> Option<Location> {
    let uri = Url::from_file_path(&target.path).ok()?;
    Some(Location {
        uri,
        range: span_to_range(target.span, &target.source),
    })
}
