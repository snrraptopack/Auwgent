# Discussion 15: `break`/`continue`, `fetch()`, and JSON Builtins

**Date:** 2026-05-19
**Status:** Discussion complete → Plan 21

---

## Topics

1. **`break` / `continue`** — Loop control flow
2. **`fetch()` builtin** — HTTP requests via reqwest
3. **JSON builtins** — `json_parse`, `json_stringify`, `json_get`
4. **`any` type** — Needed for JSON return values

---

## 1. `break` / `continue`

### Current State

Loops are native IR nodes (`Loop` for `for`, `WhileLoop` for `while`). The body of each loop is a separate subgraph. The executor runs the body graph once per iteration.

### Design Decision: Control-Flow Errors

Add `ExecutionError::Break` and `ExecutionError::Continue`. When the executor encounters a `Break` or `Continue` node inside a body graph, it returns these "errors" from `run()`. The loop executor catches them and translates them into actual loop control.

```rust
// In the executor
NodeKind::Break => return Err(ExecutionError::Break),
NodeKind::Continue => return Err(ExecutionError::Continue),

// In the Loop handler
match self.run(body_graph, input) {
    Ok(_) => {}
    Err(ExecutionError::Break) => break,
    Err(ExecutionError::Continue) => continue,
    Err(e) => return Err(e),
}
```

**Why this works:**
- `break` / `continue` are only valid inside loops (enforced by checker).
- Nested loops: the inner loop catches the error first.
- Branches: if `break` is in an untaken branch, the `Break` node is unreachable (marked by `mark_unreachable`), so it never fires.
- Functions called from loops cannot contain `break`/`continue` (checker enforces this), so the error never escapes a function boundary.

**Lowerer strategy:**
- `Stmt::Break` → `NodeKind::Break` (produces no value, immediately exits the graph)
- `Stmt::Continue` → `NodeKind::Continue` (same)

No changes needed to the body graph return structure.

---

## 2. `fetch()` Builtin

### Design

```quew
type FetchConfig = {
    method?: string
    headers?: object
    body?: string
}

type FetchResponse = {
    status: number
    body: string
}

@@rust("std.net.fetch")
!@@function fetch(url: string, config?: FetchConfig): FetchResponse
```

### Implementation

Use `reqwest::blocking` because the deterministic executor is synchronous. Adding full async to the executor is out of scope for this plan.

```rust
#[quew_builtin(
    id = "std.net.fetch",
    decl = r#"!@@function fetch(url: string, config?: FetchConfig): FetchResponse"#,
)]
pub fn fetch(url: &str, config: &Value) -> Value {
    let client = reqwest::blocking::Client::new();
    let mut req = client.get(url);
    // parse method, headers, body from config...
    let resp = req.send()?;
    let status = resp.status().as_u16() as i64;
    let body = resp.text()?;
    Value::Object(indexmap! {
        "status" => Value::Number(status),
        "body" => Value::String(body),
    })
}
```

### Dependency

Add `reqwest = { version = "0.12", features = ["blocking"] }` to `quew-runtime` workspace dependencies.

---

## 3. JSON Builtins

### The Problem

`json_parse` returns a dynamically-typed value. The quew type system is static. We need a way to represent "any JSON value" in the type system.

### Solution: `any` Type

Add `any` as a primitive type. Semantics:
- Any type is assignable to `any`.
- `any` is assignable to any type. (Yes, this is unsound, but it's pragmatic for a DSL that interops with dynamic JSON.)
- `any` has no runtime representation — it's just a type-system escape hatch.

```quew
@@rust("std.json.parse")
!@@function json_parse(text: string): any

@@rust("std.json.stringify")
!@@function json_stringify(value: any): string

@@rust("std.json.get")
!@@function json_get(obj: any, path: string): any
```

### Runtime Implementation

`serde_json` is already a workspace dependency. Use it:

```rust
#[quew_builtin(id = "std.json.parse", ...)]
pub fn json_parse(text: &str) -> Value {
    let json: serde_json::Value = serde_json::from_str(text).unwrap_or(serde_json::Value::Null);
    serde_to_quew(json)
}

#[quew_builtin(id = "std.json.stringify", ...)]
pub fn json_stringify(value: &Value) -> String {
    let json = quew_to_serde(value);
    json.to_string()
}

#[quew_builtin(id = "std.json.get", ...)]
pub fn json_get(value: &Value, path: &str) -> Value {
    // Simple dot-path: "user.name" or "items.0"
    ...
}
```

### Type System Changes

- `PrimTy::Any` in `quew-types`
- `TyAny` token in `quew-lexer`
- `any` in `type_name` parser
- `is_assignable_to`: `any` ↔ anything is always true

---

## Performance Considerations

- **`break`/`continue`**: Zero overhead when not used. The error variants are only constructed when encountered.
- **`fetch`**: Blocking I/O. Suitable for scripts and tests. For production async use, a future plan will wrap the executor in an async runtime.
- **JSON**: `serde_json` is heavily optimized. Parsing is allocation-heavy but unavoidable for JSON.

---

## Modularity

- `fetch` → `prelude/net.quew`
- JSON → `prelude/json.quew`
- `break`/`continue` → no prelude changes (language features)
- `any` type → no prelude changes (type system)
