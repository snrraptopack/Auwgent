//! Type-error tests for `reply(...) with { ... }` field validation.
//!
//! These tests deliberately supply wrong types to with-block fields to verify
//! the checker catches them:
//!   - `model` / `fallback` must be a provider (gemini/openai/groq call or named model)
//!   - `tools` must be an array where every element is a tool/tool-group reference
//!   - `prompt` must be a string
//!   - `retry` / `maxTurn` must be a number
//!   - Bare tool reference with required host params → must pre-bind them

use std::sync::Arc;

use quew_errors::Severity;
use quew_interner::Interner;
use quew_source::SourceMap;
use quew_checker::{check, CheckResult};

fn check_source(src: &str) -> CheckResult {
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
    check(&parsed.module, &interner)
}

// ── model field: wrong types ──────────────────────────────────────────────────

#[test]
fn error_model_is_string_literal() {
    // `model: "gemini-pro"` — a string is not a model
    let r = check_source(r#"
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are helpful."
        model: "gemini-pro"
    }
}
"#);
    assert!(!r.diagnostics.is_empty(), "expected model type error");
    assert!(r.diagnostics.iter().any(|d| {
        d.severity == Severity::Error && d.message.contains("`model`")
    }), "got: {:?}", r.diagnostics);
}

#[test]
fn error_model_is_number() {
    let r = check_source(r#"
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are helpful."
        model: 42
    }
}
"#);
    assert!(!r.diagnostics.is_empty(), "expected model type error");
    assert!(r.diagnostics.iter().any(|d| d.message.contains("`model`")));
}

#[test]
fn error_model_is_bool() {
    let r = check_source(r#"
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are helpful."
        model: true
    }
}
"#);
    assert!(!r.diagnostics.is_empty(), "expected model type error");
    assert!(r.diagnostics.iter().any(|d| d.message.contains("`model`")));
}

#[test]
fn error_model_is_null() {
    let r = check_source(r#"
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are helpful."
        model: null
    }
}
"#);
    assert!(!r.diagnostics.is_empty());
    assert!(r.diagnostics.iter().any(|d| d.message.contains("`model`")));
}

// ── fallback field: wrong types ───────────────────────────────────────────────

#[test]
fn error_fallback_is_string() {
    let r = check_source(r#"
model Gemini = { model: gemini("gemini-pro") }
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are helpful."
        model: Gemini
        fallback: "groq"
    }
}
"#);
    assert!(!r.diagnostics.is_empty(), "expected fallback type error");
    assert!(r.diagnostics.iter().any(|d| d.message.contains("`fallback`")));
}

#[test]
fn error_fallback_is_number() {
    let r = check_source(r#"
model Gemini = { model: gemini("gemini-pro") }
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are helpful."
        model: Gemini
        fallback: 3
    }
}
"#);
    assert!(!r.diagnostics.is_empty());
    assert!(r.diagnostics.iter().any(|d| d.message.contains("`fallback`")));
}

// ── prompt field: wrong types ─────────────────────────────────────────────────

#[test]
fn error_prompt_is_number() {
    let r = check_source(r#"
agent Hello(input: string) {
    reply(input) with {
        prompt: 42
        model: gemini("gemini-pro")
    }
}
"#);
    assert!(!r.diagnostics.is_empty(), "expected prompt type error");
    assert!(r.diagnostics.iter().any(|d| d.message.contains("`prompt`")));
}

#[test]
fn error_prompt_is_bool() {
    let r = check_source(r#"
agent Hello(input: string) {
    reply(input) with {
        prompt: false
        model: gemini("gemini-pro")
    }
}
"#);
    assert!(!r.diagnostics.is_empty());
    assert!(r.diagnostics.iter().any(|d| d.message.contains("`prompt`")));
}

// ── retry / maxTurn: wrong types ──────────────────────────────────────────────

#[test]
fn error_retry_is_string() {
    let r = check_source(r#"
model Gemini = { model: gemini("gemini-pro") }
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are helpful."
        model: Gemini
        retry: "three"
    }
}
"#);
    assert!(!r.diagnostics.is_empty(), "expected retry type error");
    assert!(r.diagnostics.iter().any(|d| d.message.contains("`retry`")));
}

#[test]
fn error_max_turn_is_string() {
    let r = check_source(r#"
model Gemini = { model: gemini("gemini-pro") }
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are helpful."
        model: Gemini
        maxTurn: "unlimited"
    }
}
"#);
    assert!(!r.diagnostics.is_empty(), "expected maxTurn type error");
    assert!(r.diagnostics.iter().any(|d| d.message.contains("`maxTurn`")));
}

#[test]
fn error_max_turn_is_bool() {
    let r = check_source(r#"
model Gemini = { model: gemini("gemini-pro") }
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are helpful."
        model: Gemini
        maxTurn: true
    }
}
"#);
    assert!(!r.diagnostics.is_empty());
    assert!(r.diagnostics.iter().any(|d| d.message.contains("`maxTurn`")));
}

// ── tools field: non-array values ─────────────────────────────────────────────

#[test]
fn error_tools_is_string() {
    let r = check_source(r#"
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are helpful."
        model: gemini("gemini-pro")
        tools: "getWeather"
    }
}
"#);
    assert!(!r.diagnostics.is_empty(), "expected tools type error");
    assert!(r.diagnostics.iter().any(|d| d.message.contains("`tools`")));
}

#[test]
fn error_tools_is_number() {
    let r = check_source(r#"
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are helpful."
        model: gemini("gemini-pro")
        tools: 1
    }
}
"#);
    assert!(!r.diagnostics.is_empty());
    assert!(r.diagnostics.iter().any(|d| d.message.contains("`tools`")));
}

// ── tools array: non-tool elements ───────────────────────────────────────────

#[test]
fn error_tools_array_contains_plain_function() {
    // A plain function (no @tool) must not appear in tools array
    let r = check_source(r#"
function greet(name: string): string {
    return name
}
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are helpful."
        model: gemini("gemini-pro")
        tools: [greet]
    }
}
"#);
    assert!(!r.diagnostics.is_empty(), "expected: function is not a tool");
    assert!(r.diagnostics.iter().any(|d| {
        d.severity == Severity::Error && d.message.contains("not a tool")
    }), "got: {:?}", r.diagnostics);
}

#[test]
fn error_tools_array_contains_string_literal() {
    let r = check_source(r#"
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are helpful."
        model: gemini("gemini-pro")
        tools: ["getWeather"]
    }
}
"#);
    assert!(!r.diagnostics.is_empty(), "string literal is not a tool reference");
    assert!(r.diagnostics.iter().any(|d| d.severity == Severity::Error));
}

#[test]
fn error_tools_array_contains_number_literal() {
    let r = check_source(r#"
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are helpful."
        model: gemini("gemini-pro")
        tools: [42]
    }
}
"#);
    assert!(!r.diagnostics.is_empty());
    assert!(r.diagnostics.iter().any(|d| d.severity == Severity::Error));
}

// ── tools: bare tool ref with required host params ────────────────────────────

#[test]
fn error_bare_tool_ref_requires_host_param_prebinding() {
    // delete_person(isAdmin: bool, @id: string) with @tool(id: string).
    // `isAdmin` is a required host param → bare `[delete_person]` must error.
    let r = check_source(r#"
@tool(id: string)
@desc "Delete a user"
function delete_person(isAdmin: bool, @id: string): string {
    return "ok"
}
agent Admin(input: string) {
    reply(input) with {
        prompt: "You manage users."
        model: gemini("gemini-pro")
        tools: [delete_person]
    }
}
"#);
    assert!(!r.diagnostics.is_empty(), "expected host param error");
    assert!(r.diagnostics.iter().any(|d| {
        d.severity == Severity::Error && d.message.contains("host-binding parameters")
    }), "got: {:?}", r.diagnostics);
}

#[test]
fn valid_tool_ref_with_host_params_prebound() {
    // Same as above but isAdmin is pre-bound → ok
    let r = check_source(r#"
@tool(id: string)
@desc "Delete a user"
function delete_person(isAdmin: bool, @id: string): string {
    return "ok"
}
agent Admin(input: string) {
    reply(input) with {
        prompt: "You manage users."
        model: gemini("gemini-pro")
        tools: [delete_person(true)]
    }
}
"#);
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn error_tool_prebinding_wrong_arg_count() {
    // delete_person has 1 host param; passing 2 args must error
    let r = check_source(r#"
@tool(id: string)
function delete_person(isAdmin: bool, @id: string): string {
    return "ok"
}
agent Admin(input: string) {
    reply(input) with {
        prompt: "p"
        model: gemini("gemini-pro")
        tools: [delete_person(true, false)]
    }
}
"#);
    assert!(!r.diagnostics.is_empty(), "expected arg count mismatch");
    assert!(r.diagnostics.iter().any(|d| d.message.contains("expected 1 host argument")));
}

// ── @context: ctx field access type-checks against context type ───────────────

#[test]
fn valid_agent_with_context_annotation() {
    let r = check_source(r#"
type Context = { isAdmin: bool, userId: string }
@context(Context)
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are helpful."
        model: gemini("gemini-pro")
    }
}
"#);
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_agent_context_field_used_in_body() {
    let r = check_source(r#"
type Context = { isAdmin: bool, userId: string }
@context(Context)
agent Hello(input: string) {
    let admin = ctx.isAdmin
    let uid = ctx.userId
    reply(input) with {
        prompt: "You are helpful."
        model: gemini("gemini-pro")
    }
}
"#);
    assert!(r.diagnostics.is_empty(), "ctx fields not in scope: {:?}", r.diagnostics);
}

#[test]
fn valid_agent_context_used_for_tool_prebinding() {
    // not.txt pattern: delete_person(ctx.isAdmin) in tools list
    let r = check_source(r#"
type Context = { isAdmin: bool, userId: string }

@tool(id: string)
@desc "Delete a user"
function delete_person(isAdmin: bool, @id: string): string {
    return "ok"
}

@context(Context)
agent Admin(input: string) {
    reply(input) with {
        prompt: "You manage users."
        model: gemini("gemini-pro")
        tools: [delete_person(ctx.isAdmin)]
    }
}
"#);
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}
