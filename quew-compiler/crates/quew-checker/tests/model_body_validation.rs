//! Role-backed validation for `model Name = { ... }` declarations.
//!
//! These tests exercise the Plan 10 migration path: the parser and IR still use
//! a dedicated provider-backed `ModelDecl`, but the checker should read the
//! expected declaration body shape from the `(model, body)` role.

use std::sync::Arc;

use quew_checker::{CheckResult, check, check_with_prelude};
use quew_errors::Severity;
use quew_interner::Interner;
use quew_source::SourceMap;

fn parse_and_check(src: &str, with_prelude: bool) -> CheckResult {
    let interner = Arc::new(Interner::new());
    let map = SourceMap::new(Arc::clone(&interner));
    let sid = map.add("<test>", src);
    let lex = quew_lexer::lex(src, sid, &interner);
    let parsed = quew_parser::parse(&lex, src, &interner);
    assert!(
        parsed.errors.is_empty(),
        "source has parse errors (fix the test fixture):\n{:?}",
        parsed.errors
    );

    if with_prelude {
        check_with_prelude(&parsed.module, &interner)
    } else {
        check(&parsed.module, &interner)
    }
}

fn check_source(src: &str) -> CheckResult {
    parse_and_check(src, false)
}

fn check_source_with_prelude(src: &str) -> CheckResult {
    parse_and_check(src, true)
}

#[test]
fn valid_model_body_uses_prelude_model_contract() {
    let r = check_source_with_prelude(
        r#"
model Gemini = { model: gemini("gemini-pro") }
"#,
    );

    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_model_body_allows_config_record_from_prelude_contract() {
    let r = check_source_with_prelude(
        r#"
model Gemini = {
    model: gemini("gemini-pro")
    config: {
        temperature: 1
        label: "fast"
    }
}
"#,
    );

    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_model_body_role_does_not_require_model_body_name() {
    let r = check_source(
        r#"
@@type Model = {
    provider: string
    name: string
}

@@(model, body)
type CustomModelContract = {
    model: Model
}

model Gemini = { model: gemini("gemini-pro") }
"#,
    );

    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn invalid_model_config_is_checked_against_role_contract() {
    let r = check_source(
        r#"
@@type Model = {
    provider: string
    name: string
}

@@(model, body)
type StrictModelContract = {
    model: Model
    config?: string
}

model Gemini = {
    model: gemini("gemini-pro")
    config: {
        temperature: 1
    }
}
"#,
    );

    assert!(
        r.diagnostics
            .iter()
            .any(|d| { d.severity == Severity::Error && d.message.contains("`config` must be") }),
        "expected config role-contract diagnostic, got {:?}",
        r.diagnostics
    );
}
