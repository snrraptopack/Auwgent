# Auwgent Runtime Testing — Language-by-Language Verification

This directory contains **realistic runtime tests** for every Auwgent target language. Unlike the deterministic unit tests in `ir-runtime/tests/` and `targets/rust/rust-verfication/`, these tests exercise the **full FFI layer + real LLM calls** so we can observe end-to-end behavior in each language.

## Philosophy

> Deterministic tests give us ~90% confidence that the FFI layer is correct. These runtime tests give us the remaining 10% by hitting real provider APIs and observing how each language binding behaves under actual load.

Each test scenario:
1. Uses a **real LLM provider** (Groq by default — fast and cheap)
2. Exercises a **specific runtime feature** (tools, helpers, middleware, etc.)
3. Prints **detailed, human-readable output** for manual review
4. Is **independent** — failures in one scenario do not cascade

## Language Order

1. **TypeScript** — Most mature FFI target (napi-rs). Serves as the reference.
2. **Python** — PyO3 bindings. Second most used target.
3. **Dart** — FFI bindings. Mobile/desktop focus.
4. **Rust** — Native runtime. Formalize the existing ad-hoc live tests.

## Test Scenarios (Canonical Matrix)

Every language must pass all scenarios before we move on.

| # | Scenario | What We Verify |
|---|----------|----------------|
| 1 | **Basic Chat** | `response_text` intent fires; session turns are recorded |
| 2 | **Tool Call (no args)** | `tool_call` + `tool_result` for `get_location`; correct result passed back to LLM |
| 3 | **Tool Call (with args)** | `tool_call` + `tool_result` for `get_marks`; args deserialized correctly |
| 4 | **Tool Error** | Unknown tool emits `tool_error`; runtime does not crash |
| 5 | **Workflow Execution** | `workflow_call` runs body; tools invoked inside workflow; `workflow_result` emitted |
| 6 | **Helper (Return)** | `helper_call` → sub-engine → `helper_result`; stack is clean |
| 7 | **Helper (User Handoff)** | Helper streams to user; parent resumes after helper completes |
| 8 | **Structured Output** | `response_schema` emitted; JSON matches declared output type |
| 9 | **Custom Intent** | User-defined intent (`Loud`) parsed and emitted with correct fields |
| 10 | **Middleware Lifecycle** | All hooks fire in order: `run_start` → `llm_start` → `intent` → `llm_end` → `run_complete` |
| 11 | **Session Export/Import** | `exportSession()` → `importSession()` → conversation continues with context preserved |
| 12 | **Error Swallowing** | Middleware `onError` returns `{ swallow: true }`; run continues gracefully |
| 13 | **Streaming / Partial Intents** | `onIntentPartial` fires during streaming; deltas accumulate correctly |

## Canonical Agent

All scenarios use `canonical.agent` (compiled to `canonical.agent.json`) so results are comparable across languages.

## How to Run

```bash
# TypeScript
cd runtime-tests/typescript
bun install  # or npm install
bun test     # runs all scenarios sequentially

# Python
cd runtime-tests/python
uv run python test_runner.py

# Dart
cd runtime-tests/dart
dart run test_runner.dart

# Rust
cd runtime-tests/rust
cargo run
```

## Manual Review Checklist

After running a scenario, verify:
- [ ] Intents fired in the expected order
- [ ] Tool arguments match what the LLM should have sent
- [ ] Tool results were passed back to the LLM correctly
- [ ] Session state has the expected number of turns
- [ ] No crashes, panics, or uncaught exceptions
- [ ] Streaming output is smooth (no duplicate chunks)
- [ ] Memory usage is stable (no leaks between scenarios)

## Sign-off

| Language | Status | Date | Signed off by |
|----------|--------|------|---------------|
| TypeScript | ⏳ In Progress | | |
| Python | ⏳ Pending | | |
| Dart | ⏳ Pending | | |
| Rust | ⏳ Pending | | |
