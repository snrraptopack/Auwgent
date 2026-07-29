use std::sync::Arc;

use quew_errors::Severity;
use quew_interner::Interner;
use quew_ir::QuewGraphIR;
use quew_ir::lower::lower;
use quew_source::SourceMap;

pub fn compile_source(source: &str) -> (Arc<Interner>, QuewGraphIR) {
    let interner = Arc::new(Interner::new());
    let source_map = SourceMap::new(Arc::clone(&interner));
    let source_id = source_map.add("test.quew", source.to_string());
    let lex = quew_lexer::lex(source, source_id, &interner);
    assert!(lex.errors.is_empty(), "lex errors: {:?}", lex.errors);
    let parse = quew_parser::parse(&lex, source, &interner);
    assert!(parse.errors.is_empty(), "parse errors: {:?}", parse.errors);
    let check = quew_checker::check(&parse.module, &interner);
    assert!(
        !check
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error),
        "checker errors: {:?}",
        check.diagnostics
    );
    let ir = lower(&parse.module, &check, &interner);
    (interner, ir)
}

pub fn compile_source_with_prelude(source: &str) -> (Arc<Interner>, QuewGraphIR) {
    let interner = Arc::new(Interner::new());
    let source_map = SourceMap::new(Arc::clone(&interner));
    let source_id = source_map.add("test.quew", source.to_string());
    let lex = quew_lexer::lex(source, source_id, &interner);
    assert!(lex.errors.is_empty(), "lex errors: {:?}", lex.errors);
    let parse = quew_parser::parse(&lex, source, &interner);
    assert!(parse.errors.is_empty(), "parse errors: {:?}", parse.errors);
    let prelude = quew_checker::module_with_prelude(&parse.module, &interner);
    let check = quew_checker::check(&prelude.module, &interner);
    assert!(
        !check
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error),
        "checker errors: {:?}",
        check.diagnostics
    );
    let ir = lower(&prelude.module, &check, &interner);
    (interner, ir)
}
