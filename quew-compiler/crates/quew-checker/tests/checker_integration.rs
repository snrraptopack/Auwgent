//! End-to-end integration tests for `quew-checker`.
//!
//! Every test parses real quew source text through the full pipeline:
//!   source str → quew_lexer::lex → quew_parser::parse → quew_checker::check
//!
//! Assertions target the checker diagnostics, NOT the parse errors, so each
//! test verifies that the source is syntactically valid first.

use std::sync::Arc;

use quew_checker::{CheckResult, check, check_with_prelude};
use quew_errors::Severity;
use quew_interner::Interner;
use quew_source::SourceMap;

// ── Test harness ──────────────────────────────────────────────────────────────

/// Lex → parse → check.  Panics if the source has parse errors.
fn check_source(src: &str) -> CheckResult {
    let interner = Arc::new(Interner::new());
    let map = SourceMap::new(Arc::clone(&interner));
    let sid = map.add("<test>", src);
    let lex_result = quew_lexer::lex(src, sid, &interner);
    let parse_result = quew_parser::parse(&lex_result, src, &interner);
    assert!(
        parse_result.errors.is_empty(),
        "source has parse errors (fix the test, not the checker):\n{:?}",
        parse_result.errors
    );
    check(&parse_result.module, &interner)
}

/// Lex -> parse -> prelude-aware check. Panics if the user source has parse errors.
fn check_source_with_prelude(src: &str) -> CheckResult {
    let interner = Arc::new(Interner::new());
    let map = SourceMap::new(Arc::clone(&interner));
    let sid = map.add("<test>", src);
    let lex_result = quew_lexer::lex(src, sid, &interner);
    let parse_result = quew_parser::parse(&lex_result, src, &interner);
    assert!(
        parse_result.errors.is_empty(),
        "source has parse errors (fix the test, not the checker):\n{:?}",
        parse_result.errors
    );
    check_with_prelude(&parse_result.module, &interner)
}

// ── Valid programs: zero diagnostics expected ─────────────────────────────────

#[test]
fn valid_type_declaration() {
    let r = check_source("type User = { name: string, age: number }");
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_type_optional_field() {
    let r = check_source("type Profile = { name: string, bio?: string }");
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_model_declaration() {
    let r = check_source(r#"model MyModel = { model: gemini("gemini-pro") }"#);
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_agent_no_body() {
    let r = check_source(
        r#"
agent Chat(input: string) {
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_function_no_params_no_return() {
    let r = check_source(
        r#"
function greet() {
    let msg = "hello"
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_function_with_return() {
    let r = check_source(
        r#"
function getMessage(): string {
    return "hello world"
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_generic_type_and_function_identity() {
    let r = check_source(
        r#"
type Box<T> = { value: T }

function identity<T>(value: T): T {
    return value
}

function demo(input: string): string {
    return identity(input)
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_role_bound_generic_type() {
    let r = check_source(
        r#"
@@(tool, value)
type ToolResult<T> = {
    data: T
    error: string
}

function unwrap(result: ToolResult<string>): string {
    return result.data
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_prelude_type_is_available_without_user_declaration() {
    let r = check_source_with_prelude(
        r#"
function unwrap(result: ToolResult<string>): string {
    return result.data
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_direct_tool_call_value_uses_prelude_wrapper_data() {
    let r = check_source_with_prelude(
        r#"
tool getName(): string

function demo(): string {
    let result = getName()
    return result.data
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_direct_tool_call_value_uses_prelude_wrapper_error() {
    let r = check_source_with_prelude(
        r#"
tool getName(): string

function demo(): string {
    let result = getName()
    return result.error
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_direct_tool_call_number_data_preserves_return_type() {
    let r = check_source_with_prelude(
        r#"
tool getScore(): number

function demo(): number {
    return getScore().data
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_tools_list_remains_compatible_with_prelude() {
    let r = check_source_with_prelude(
        r#"
tool getName(): string

agent Hello(input: string) {
    reply(input) with {
        prompt: "You are helpful."
        model: gemini("gemini-pro")
        tools: [getName]
    }
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_direct_tool_call_stays_raw_without_prelude_for_isolated_tests() {
    let r = check_source(
        r#"
tool getName(): string

function demo(): string {
    return getName()
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn invalid_direct_tool_call_raw_value_mismatches_with_prelude() {
    let r = check_source_with_prelude(
        r#"
tool getName(): string

function demo(): string {
    return getName()
}
"#,
    );
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.message.contains("return type mismatch")),
        "expected return type mismatch, got {:?}",
        r.diagnostics
    );
}

#[test]
fn invalid_user_tool_result_collides_with_prelude_type() {
    let r = check_source_with_prelude(
        r#"
type ToolResult<T> = { data: T }
"#,
    );
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.message.contains("duplicate definition")),
        "expected duplicate symbol diagnostic, got {:?}",
        r.diagnostics
    );
}

#[test]
fn invalid_duplicate_role_binding_flows_through_check() {
    let r = check_source(
        r#"
@@(tool, value)
type ToolResult<T> = { data: T }

@@(tool, value)
type OtherToolResult<T> = { data: T }
"#,
    );
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.message.contains("duplicate role binding")),
        "expected duplicate role binding diagnostic, got {:?}",
        r.diagnostics
    );
}

#[test]
fn invalid_unknown_role_key_flows_through_check() {
    let r = check_source(
        r#"
@@(unknown, elsewhere)
type BadRole = { value: string }
"#,
    );
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.message.contains("unknown role keyword")),
        "expected unknown role keyword diagnostic, got {:?}",
        r.diagnostics
    );
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.message.contains("unknown role place")),
        "expected unknown role place diagnostic, got {:?}",
        r.diagnostics
    );
}

#[test]
fn valid_generic_record_field_substitution() {
    let r = check_source(
        r#"
type Pair<A, B> = {
    first: A
    second: B
}

function first<A, B>(pair: Pair<A, B>): A {
    return pair.first
}

function demo(pair: Pair<string, bool>): string {
    return first(pair)
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_nested_generic_stress_case() {
    let r = check_source(
        r#"
type Box<T> = { value: T }
type Pair<A, B> = { first: A, second: B }

function unbox<T>(box: Box<T>): T {
    return box.value
}

function first<A, B>(pair: Pair<A, B>): A {
    return pair.first
}

function demo(input: Box<Pair<string, bool>>): string {
    let pair = unbox(input)
    return first(pair)
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn invalid_generic_type_arity_errors() {
    let r = check_source(
        r#"
type Box<T> = { value: T }

function bad(value: Box<string, number>) {
}
"#,
    );
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.message.contains("expects 1 type argument")),
        "expected arity diagnostic, got: {:?}",
        r.diagnostics
    );
}

#[test]
fn invalid_generic_function_conflicting_inference_errors() {
    let r = check_source(
        r#"
function same<T>(left: T, right: T): T {
    return left
}

function demo(input: string): string {
    return same(input, 1)
}
"#,
    );
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.message.contains("conflicting inference")),
        "expected conflicting inference diagnostic, got: {:?}",
        r.diagnostics
    );
}

#[test]
fn valid_function_with_params() {
    let r = check_source(
        r#"
function add(a: number, b: number): number {
    return a
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_let_in_body() {
    let r = check_source(
        r#"
function compute() {
    let x = 42
    let y = "hello"
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_if_in_body() {
    let r = check_source(
        r#"
function check(x: bool) {
    if x {
        let msg = "yes"
    }
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_shadowing_inner_block() {
    // Shadowing `x` in an inner block is legal
    let r = check_source(
        r#"
function shadow() {
    let x = 1
    if true {
        let x = "inner"
    }
}
"#,
    );
    assert!(
        r.diagnostics.is_empty(),
        "shadowing outer let should be ok: {:?}",
        r.diagnostics
    );
}

#[test]
fn valid_tool_no_bound_params() {
    let r = check_source(
        r#"
@desc "Fetch a user by id"
function getUser(id: string): string {
    return id
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_tool_with_bound_params() {
    // @tool(id: string) declares the host binding
    // @id: string in the param list imports it as a local
    let r = check_source(
        r#"
@tool(id: string)
@desc "Delete a user"
function deleteUser(isAdmin: bool, @id: string): string {
    return id
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_multiple_distinct_items() {
    let r = check_source(
        r#"
type User = { name: string, age: number }
type Post = { title: string, body: string }
function getName(u: User): string {
    return "ok"
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

// ── Collision: top-level name collisions ──────────────────────────────────────

#[test]
fn error_duplicate_function_names() {
    let r = check_source(
        r#"
function foo(): string {
    return "first"
}
function foo(): string {
    return "second"
}
"#,
    );
    assert!(!r.diagnostics.is_empty(), "expected duplicate name error");
    assert!(
        r.diagnostics
            .iter()
            .any(|d| { d.severity == Severity::Error && d.message.contains("duplicate") }),
        "expected 'duplicate' diagnostic, got: {:?}",
        r.diagnostics
    );
}

#[test]
fn error_type_and_function_same_name() {
    let r = check_source(
        r#"
type Response = { code: number }
function Response(): string {
    return "oops"
}
"#,
    );
    assert!(!r.diagnostics.is_empty(), "expected duplicate name error");
    assert!(r.diagnostics.iter().any(|d| d.severity == Severity::Error));
}

#[test]
fn error_two_type_declarations_same_name() {
    let r = check_source(
        r#"
type Config = { host: string }
type Config = { port: number }
"#,
    );
    assert!(!r.diagnostics.is_empty(), "expected duplicate type error");
    assert!(
        r.diagnostics
            .iter()
            .any(|d| { d.severity == Severity::Error && d.message.contains("duplicate") })
    );
}

// ── Collision: param name collisions ─────────────────────────────────────────

#[test]
fn error_duplicate_param_names() {
    let r = check_source(
        r#"
function bad(x: string, x: number): string {
    return x
}
"#,
    );
    assert!(!r.diagnostics.is_empty(), "expected duplicate param error");
    assert!(
        r.diagnostics
            .iter()
            .any(|d| { d.severity == Severity::Error && d.message.contains("already declared") }),
        "expected 'already declared' diagnostic, got: {:?}",
        r.diagnostics
    );
}

// ── Collision: duplicate let in same block ────────────────────────────────────

#[test]
fn error_duplicate_let_same_block() {
    let r = check_source(
        r#"
function dupe() {
    let x = 1
    let x = 2
}
"#,
    );
    assert!(!r.diagnostics.is_empty(), "expected duplicate let error");
    assert!(
        r.diagnostics
            .iter()
            .any(|d| { d.severity == Severity::Error && d.message.contains("already declared") }),
        "got: {:?}",
        r.diagnostics
    );
}

// ── Unreachable code ──────────────────────────────────────────────────────────

#[test]
fn error_unreachable_after_return() {
    let r = check_source(
        r#"
function earlyReturn(): string {
    return "done"
    let dead = "unreachable"
}
"#,
    );
    assert!(!r.diagnostics.is_empty(), "expected unreachable code error");
    assert!(
        r.diagnostics
            .iter()
            .any(|d| { d.severity == Severity::Error && d.message.contains("unreachable") }),
        "got: {:?}",
        r.diagnostics
    );
}

#[test]
fn error_unreachable_multiple_stmts_after_return() {
    let r = check_source(
        r#"
function twoDeadStmts() {
    return 1
    let a = 2
    let b = 3
}
"#,
    );
    // At least one unreachable diagnostic
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.message.contains("unreachable")),
        "got: {:?}",
        r.diagnostics
    );
}

// ── Bound param: missing from @tool annotation ────────────────────────────────

#[test]
fn error_bound_param_not_in_tool_annotation() {
    // @ghost is a BoundRef but not declared in @tool(id: string)
    let r = check_source(
        r#"
@tool(id: string)
@desc "Do something"
function doThing(isAdmin: bool, @ghost: string): string {
    return "ok"
}
"#,
    );
    assert!(
        !r.diagnostics.is_empty(),
        "expected unmatched bound param error"
    );
    assert!(
        r.diagnostics.iter().any(|d| d.severity == Severity::Error),
        "got: {:?}",
        r.diagnostics
    );
}

// ── Inference: literal type propagation ───────────────────────────────────────

#[test]
fn let_binding_is_tracked_as_local() {
    // After `let name = "alice"`, using `name` should resolve (no undefined error)
    let r = check_source(
        r#"
function test() {
    let name = "alice"
    let greeting = name
}
"#,
    );
    assert!(
        r.diagnostics.is_empty(),
        "local lookup failed: {:?}",
        r.diagnostics
    );
}

#[test]
fn param_is_tracked_as_local() {
    // Function params must be accessible inside the body
    let r = check_source(
        r#"
function greet(name: string): string {
    return name
}
"#,
    );
    assert!(
        r.diagnostics.is_empty(),
        "param not in scope: {:?}",
        r.diagnostics
    );
}

// ── Symbol table presence ─────────────────────────────────────────────────────

#[test]
fn symbol_table_contains_declared_type() {
    let r = check_source("type Payload = { ok: bool, msg: string }");
    // The interner from check_source is separate, so we can only assert the
    // count is right — we can't look up by name without the interner.
    assert_eq!(r.symbol_table.globals.len(), 1);
}

#[test]
fn symbol_table_contains_all_top_level_items() {
    let r = check_source(
        r#"
type User = { name: string }
function greet(): string { return "hi" }
model M = { model: gemini("g-pro") }
"#,
    );
    assert_eq!(
        r.symbol_table.globals.len(),
        3,
        "expected 3 globals, got {:?}",
        r.symbol_table.globals.len()
    );
}

// ── For loop variable scoping ─────────────────────────────────────────────────

#[test]
fn for_loop_variable_available_in_body() {
    let r = check_source(
        r#"
function iter() {
    let items = "dummy"
    for item in items {
        let x = item
    }
}
"#,
    );
    assert!(
        r.diagnostics.is_empty(),
        "for var not in scope: {:?}",
        r.diagnostics
    );
}

// ── Regression: panic-free on malformed but parsed input ─────────────────────

#[test]
fn check_does_not_panic_on_empty_source() {
    let r = check_source("");
    assert!(r.diagnostics.is_empty());
}

#[test]
fn check_does_not_panic_on_only_model() {
    let r = check_source(r#"model M = { model: openai("gpt-4") }"#);
    assert!(r.diagnostics.is_empty());
}

// ── Agent: reply with — core DSL constructs ───────────────────────────────────
// These tests mirror the examples from not.txt.
// The checker today validates: agent body statements, name scoping, collisions.
// with-block field semantics (prompt type, model reference) are deferred to plan 5.

#[test]
fn valid_agent_with_prompt_and_model() {
    // not.txt lines 9–15: basic agent
    let r = check_source(
        r#"
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are a helpful assistant."
        model: gemini("gemini-pro")
    }
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_agent_with_tools_list() {
    // not.txt lines 44–51: agent + tools array
    let r = check_source(
        r#"
tool getWeather(): string
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are a helpful assistant."
        model: gemini("gemini-pro")
        tools: [getWeather]
    }
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_agent_with_builtin_field() {
    // not.txt lines 54–62: builtin tools
    let r = check_source(
        r#"
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are helpful."
        model: gemini("gemini-pro")
        builtin: ["code_execution"]
    }
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_agent_with_fallback_and_retry() {
    // not.txt lines 645–654: fallback model + retry count
    let r = check_source(
        r#"
model Gemini = { model: gemini("gemini-pro") }
model Groq = { model: groq("llama-3") }
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are helpful."
        model: Gemini
        fallback: Groq
        retry: 3
    }
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_agent_with_max_turn() {
    // not.txt lines 656–664: maxTurn field
    let r = check_source(
        r#"
model Gemini = { model: gemini("gemini-pro") }
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are helpful."
        model: Gemini
        maxTurn: 3
    }
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_agent_input_preprocessing_before_reply() {
    // not.txt lines 73–100: preprocess input then reply
    let r = check_source(
        r#"
function sanitize(input: string): string {
    return input
}
agent Hello(input: string) {
    let user_input = sanitize(input)
    reply(user_input) with {
        prompt: "You are helpful."
        model: gemini("gemini-pro")
    }
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_agent_conditional_tools_selection() {
    // not.txt lines 218–227: tools selected based on runtime flag
    let r = check_source(
        r#"
tool getWeather(): string
tool deleteUser(): string
agent Hello(input: string) {
    let isAdmin = false
    let selected = [getWeather, deleteUser] if isAdmin else [getWeather]
    reply(input) with {
        prompt: "You are helpful."
        model: gemini("gemini-pro")
        tools: selected
    }
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_agent_calling_another_agent() {
    // not.txt lines 276–292: agent composition
    let r = check_source(
        r#"
agent Analyze(input: string) {
    reply(input) with {
        prompt: "Analyze this."
        model: gemini("gemini-pro")
    }
}
agent Main(input: string) {
    let result = Analyze(input)
    reply(result) with {
        prompt: "You are helpful."
        model: gemini("gemini-pro")
    }
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_agent_early_return_to_sub_agent() {
    // not.txt lines 297–306: `return Agent(input)` handoff pattern
    let r = check_source(
        r#"
agent One(input: string) {
    reply(input) with { prompt: "One.", model: gemini("gemini-pro") }
}
agent Two(input: string) {
    reply(input) with { prompt: "Two.", model: gemini("gemini-pro") }
}
agent Main(input: string) {
    let flag = true
    if flag {
        return One(input)
    }
    return Two(input)
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_agent_named_model_alias() {
    // not.txt lines 489–497: `model: Gemini` using a model declaration
    let r = check_source(
        r#"
model Gemini = {
    model: gemini("gemini-pro")
    config: { temperature: 0.7 }
}
agent Hello(input: string) {
    reply(input) with {
        prompt: "You are helpful."
        model: Gemini
    }
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_agent_structured_output_return_type() {
    // not.txt lines 416–431: agent with named return type
    let r = check_source(
        r#"
type Response = {
    userName: string
    age: number
    location: string
}
agent Hello(input: string): Response {
    reply(input) with {
        prompt: "Extract user info."
        model: gemini("gemini-pro")
    }
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn valid_agent_with_tool_binding_gating_pattern() {
    // not.txt lines 131-162: @tool gating — `isAdmin` pre-bound by caller, `id` seen by model
    // Pre-binding: delete_person(true) binds isAdmin=true at the tools list
    let r = check_source(
        r#"
tool delete_user(id: string): bool

@tool(id: string)
@desc "use this to delete a user"
function delete_person(isAdmin: bool, @id: string): string {
    if not isAdmin {
        return "user is not an admin"
    }
    return "deleted"
}

agent Admin(input: string) {
    reply(input) with {
        prompt: "You manage users."
        model: gemini("gemini-pro")
        tools: [delete_person(true)]
    }
}
"#,
    );
    assert!(r.diagnostics.is_empty(), "unexpected: {:?}", r.diagnostics);
}

#[test]
fn error_tool_with_required_host_params_used_as_bare_ref() {
    // delete_person has isAdmin: bool as a required host-binding param.
    // Using it as a bare reference WITHOUT pre-binding must error.
    let r = check_source(
        r#"
@tool(id: string)
@desc "use this to delete a user"
function delete_person(isAdmin: bool, @id: string): string {
    return "deleted"
}

agent Admin(input: string) {
    reply(input) with {
        prompt: "You manage users."
        model: gemini("gemini-pro")
        tools: [delete_person]
    }
}
"#,
    );
    assert!(
        !r.diagnostics.is_empty(),
        "expected error: isAdmin must be pre-bound"
    );
    assert!(
        r.diagnostics.iter().any(|d| {
            d.severity == Severity::Error && d.message.contains("host-binding parameters")
        }),
        "got: {:?}",
        r.diagnostics
    );
}

// ── Agent: collision errors spanning agent + function + type namespace ─────────

#[test]
fn error_agent_and_function_same_name() {
    let r = check_source(
        r#"
agent Foo(input: string) {
    reply(input) with { prompt: "p", model: gemini("g") }
}
function Foo(): string {
    return "oops"
}
"#,
    );
    assert!(!r.diagnostics.is_empty(), "expected duplicate name error");
    assert!(r.diagnostics.iter().any(|d| d.severity == Severity::Error));
}

#[test]
fn error_agent_and_type_same_name() {
    let r = check_source(
        r#"
type Request = { text: string }
agent Request(input: string) {
    reply(input) with { prompt: "p", model: gemini("g") }
}
"#,
    );
    assert!(!r.diagnostics.is_empty(), "expected duplicate name error");
    assert!(r.diagnostics.iter().any(|d| d.severity == Severity::Error));
}

#[test]
fn error_duplicate_agent_names() {
    let r = check_source(
        r#"
agent Bot(input: string) {
    reply(input) with { prompt: "first", model: gemini("g") }
}
agent Bot(input: string) {
    reply(input) with { prompt: "second", model: gemini("g") }
}
"#,
    );
    assert!(
        !r.diagnostics.is_empty(),
        "expected duplicate agent name error"
    );
    assert!(
        r.diagnostics
            .iter()
            .any(|d| { d.severity == Severity::Error && d.message.contains("duplicate") })
    );
}

#[test]
fn valid_agent_let_collision_same_block_errors() {
    // Duplicate `let` inside an agent body is still caught
    let r = check_source(
        r#"
agent Hello(input: string) {
    let x = "first"
    let x = "second"
    reply(x) with { prompt: "p", model: gemini("g") }
}
"#,
    );
    assert!(
        !r.diagnostics.is_empty(),
        "expected duplicate let in agent body"
    );
    assert!(
        r.diagnostics
            .iter()
            .any(|d| { d.severity == Severity::Error && d.message.contains("already declared") })
    );
}
