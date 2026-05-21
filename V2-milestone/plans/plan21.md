# Plan 21: `break`/`continue`, `fetch()`, JSON Builtins, and `any` Type

**Status:** Not started
**Scope:** Add loop control flow, HTTP requests, JSON handling, and a dynamic type to make quew usable for real scripts.

---

## Goals

1. Add `break` and `continue` statements for loop control flow.
2. Add `fetch(url: string, config?: FetchConfig): FetchResponse` builtin using `reqwest::blocking`.
3. Add JSON builtins: `json_parse(text: string): any`, `json_stringify(value: any): string`, `json_get(obj: any, path: string): any`.
4. Add `any` primitive type to the type system.

## Non-Goals

- Async/await in the executor (fetch is blocking only)
- Full HTTP client features (redirects, cookies, proxies — deferred)
- JSON schema validation
- `try`/`catch` error handling for fetch/JSON failures

---

## Architecture

### `break` / `continue`: Control-Flow Errors

```
User code:
    while count < 10 {
        if count == 5 { break }
        count = count + 1
    }

Parser:
    Stmt::Break(Span)
    Stmt::Continue(Span)

Checker:
    Track loop depth. Error if break/continue outside loop.

Lowerer:
    Stmt::Break  → NodeKind::Break
    Stmt::Continue → NodeKind::Continue

Executor:
    NodeKind::Break    → return Err(ExecutionError::Break)
    NodeKind::Continue → return Err(ExecutionError::Continue)

Loop handler:
    match self.run(body_graph, input) {
        Ok(_) => {},
        Err(ExecutionError::Break) => break,
        Err(ExecutionError::Continue) => continue,
        Err(e) => return Err(e),
    }
```

### `fetch()`: Blocking HTTP

```
Rust: reqwest::blocking::Client
IR:   FuncCall → native dispatch → std.net.fetch
```

### JSON: serde_json Bridge

```
Rust: serde_json::Value ↔ quew_runtime::value::Value
IR:   FuncCall → native dispatch → std.json.parse / stringify / get
```

### `any` Type

```
Lexer:    TyAny token ("any")
Parser:   type_name accepts TyAny
Checker:  is_assignable_to: any ↔ anything = true
Runtime:  no representation (same as other values)
```

---

## Implementation Steps

### Step 1: `any` Type

1. **`quew-types`**: Add `PrimTy::Any`, update `Display` impl.
2. **`quew-lexer`**: Add `TyAny` token (`#[token("any")]`).
3. **`quew-parser`**: Add `TokenKind::TyAny` to `type_name` in `common.rs`.
4. **`quew-types`**: Update `is_assignable_to`:
   - If target is `Primitive(Any)`, return true.
   - If self is `Primitive(Any)`, return true.
5. **Tests**: Parse `any` type, verify assignability.

**Files touched:**
- `quew-compiler/crates/quew-types/src/lib.rs`
- `quew-compiler/crates/quew-lexer/src/token.rs`
- `quew-compiler/crates/quew-parser/src/common.rs`

### Step 2: `break` / `continue`

1. **`quew-lexer`**: Add `KwBreak`, `KwContinue` tokens.
2. **`quew-ast`**: Add `Stmt::Break(Span)` and `Stmt::Continue(Span)`.
3. **`quew-parser`**: Parse `break` and `continue` as statements in `parse_stmt.rs`.
4. **`quew-checker`**: Track loop depth in `check_body`. Error if break/continue at depth 0.
5. **`quew-ir`**: Add `NodeKind::Break` and `NodeKind::Continue` to `graph.rs`.
6. **`quew-ir/lower`**: Handle `Stmt::Break` and `Stmt::Continue` in `graph_lower.rs`.
7. **`quew-runtime`**: Handle `Break`/`Continue` nodes in `execution.rs`, catch them in `Loop` and `WhileLoop` handlers.
8. **Tests**: break in for/while, continue in for/while, nested loops, break inside if branch.

**Files touched:**
- `quew-compiler/crates/quew-lexer/src/token.rs`
- `quew-compiler/crates/quew-ast/src/stmt.rs`
- `quew-compiler/crates/quew-parser/src/parse_stmt.rs`
- `quew-compiler/crates/quew-checker/src/lib.rs`
- `quew-compiler/crates/quew-ir/src/graph.rs`
- `quew-compiler/crates/quew-ir/src/lower/graph_lower.rs`
- `quew-runtime/crates/quew-runtime/src/execution.rs`

### Step 3: `fetch()` Builtin

1. **Dependency**: Add `reqwest = { version = "0.12", features = ["blocking"] }` to `quew-runtime/Cargo.toml` workspace dependencies.
2. **`quew-stdlib`**: Create `src/net.rs` with `#[quew_builtin(id = "std.net.fetch", ...)]`.
3. **`quew-stdlib/src/lib.rs`**: Add `pub mod net;`.
4. **Prelude**: Create `prelude/net.quew` with `FetchConfig`, `FetchResponse`, and `fetch` declaration.
5. **Prelude loader**: Add `NET_PRELUDE` to `prelude.rs`.
6. **Tests**: Test with httpbin.org or a local mock (use manual native registration to avoid network in CI).

**Files touched:**
- `quew-runtime/Cargo.toml`
- `quew-runtime/crates/quew-stdlib/src/net.rs` (new)
- `quew-runtime/crates/quew-stdlib/src/lib.rs`
- `quew-compiler/prelude/net.quew` (new)
- `quew-compiler/crates/quew-checker/src/prelude.rs`

### Step 4: JSON Builtins

1. **Dependency**: `serde_json` is already in the compiler workspace. Add it to `quew-runtime` workspace dependencies.
2. **`quew-stdlib`**: Create `src/json.rs` with three `#[quew_builtin]` functions.
   - `json_parse(text: &str) -> Value`
   - `json_stringify(value: &Value) -> String`
   - `json_get(value: &Value, path: &str) -> Value` (dot-path: `user.name`, `items.0`)
3. **`quew-stdlib/src/lib.rs`**: Add `pub mod json;`.
4. **Prelude**: Create `prelude/json.quew` with declarations.
5. **Prelude loader**: Add `JSON_PRELUDE` to `prelude.rs`.
6. **Tests**: Parse JSON string, stringify Value, get nested field.

**Files touched:**
- `quew-runtime/Cargo.toml`
- `quew-runtime/crates/quew-stdlib/src/json.rs` (new)
- `quew-runtime/crates/quew-stdlib/src/lib.rs`
- `quew-compiler/prelude/json.quew` (new)
- `quew-compiler/crates/quew-checker/src/prelude.rs`

### Step 5: Prelude Test Updates

Update `prelude_registers_native_builtin_functions` test to include `fetch`, `json_parse`, `json_stringify`, `json_get`.

### Step 6: Documentation

- `GAPS.md`: Mark `break`/`continue`, `fetch`, JSON builtins as ✅. Remove from "Biggest Missing Pieces".
- `BUILTIN_PIPELINE.md`: Add `fetch` and JSON examples.

---

## Acceptance Criteria

- [ ] `any` type parses and allows anything to be assigned to it (and vice versa)
- [ ] `break` inside a `while` loop exits the loop
- [ ] `continue` inside a `for` loop skips to the next iteration
- [ ] `break` inside nested `if` inside a loop works correctly
- [ ] `fetch("https://example.com")` compiles and executes (returns `FetchResponse`)
- [ ] `json_parse("{\"name\":\"Alice\"}")` returns an object
- [ ] `json_stringify({ name: "Alice" })` returns a JSON string
- [ ] `json_get(obj, "name")` returns the field value
- [ ] All existing tests continue to pass
- [ ] `GAPS.md` and `BUILTIN_PIPELINE.md` are updated
