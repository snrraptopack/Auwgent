# Discussion 10: `#[quew_builtin]` Proc-Macro

*Proposed as Plan 17 — the single-source-of-truth mechanism for the Quew standard library.*

---

## 1. Current State

The compiler supports `@@rust("id")` metadata on builtin functions (Plan 11) and `extend Type { ... }` for extension methods (Plan 12). The runtime has a clean `NativeRegistry` that maps stable IDs to Rust function pointers (Plan 16).

However, adding a single builtin today requires **four manual edits across three files**:

1. **Rust implementation** — write the function body in `crates/quew-stdlib/src/string.rs`.
2. **Quew prelude declaration** — write `@@rust("std.string.len") !@@function len(value: string): number` in `prelude/native.quew`.
3. **Extension method wrapper** — write `extend string { function len(): number { return len(self) } }` in `prelude/methods.quew`.
4. **Runtime registration** — add the function to `NativeRegistry` in the runtime crate.

This is error-prone (IDs drift, signatures mismatch, wrappers go stale) and slows stdlib iteration.

---

## 2. The Gap

There is **no single source of truth**. The Rust function is the canonical implementation, but three other artifacts must be kept in sync with it by hand.

As the standard library grows — string utilities, array operations, math functions, `fetch()`, JSON parsing, date/time — this overhead becomes unsustainable. A proc-macro that derives the other three artifacts from the Rust function annotation is essential.

---

## 3. Design Goals

1. **One annotation per native leaf** — the Rust function is the only hand-written source.
2. **Three auto-generated artifacts**:
   - Quew prelude declaration (`prelude/native.quew`)
   - Runtime `NativeRegistry` entry (link-time registration)
   - Optional extension method wrapper (`prelude/methods.quew`)
3. **Trust boundary preserved** — `@@rust` remains prelude-only; user code cannot inject native execution.
4. **Build-time generation** — no runtime reflection; everything resolves at compile/link time.
5. **Sync and async support** — native functions can be pure (`fn`) or suspending (`async fn`).

---

## 4. Proposed Architecture

### 4.1 Attribute Syntax

```rust
use quew_macros::quew_builtin;

#[quew_builtin(
    id     = "std.string.len",
    decl   = r#"!@@function len(value: string): number"#,
    extend = "string",
    method = "len"
)]
pub fn string_len(value: &str) -> usize {
    value.len()
}
```

Fields:

| Field | Required | Purpose |
|-------|----------|---------|
| `id` | Yes | Stable dispatch key referenced by `@@rust("...")` |
| `decl` | Yes | Raw Quew declaration emitted into prelude |
| `extend` | No | Receiver type for auto-generated extension method |
| `method` | No | Method name exposed to Quew callers |

Rules:
- `extend` and `method` must appear together or not at all.
- The macro does **not** parse or validate the Quew syntax in `decl`. It is emitted verbatim; the Quew compiler validates it during its own parse phase.
- The Rust function signature must be compatible with the `decl` contract (enforced by convention and tests, not by the macro).

### 4.2 Async Variant

```rust
#[quew_builtin(
    id      = "std.fetch",
    decl    = r#"!@@function fetch(url: string): ToolResult<string>"#,
    r#async = true
)]
pub async fn fetch(url: &str) -> Result<String, reqwest::Error> {
    reqwest::get(url).await?.text().await
}
```

The generated handler becomes `NativeEntry::Async` (or a future-compatible variant) so the graph executor can suspend the node and poll the future.

### 4.3 Generated Artifacts

#### 4.3.1 Prelude Declaration (build-time)

From the example above, the build pipeline emits:

```quew
@@rust("std.string.len")
!@@function len(value: string): number

extend string {
    function len(): number {
        return len(self)
    }
}
```

This lands in `prelude/native.quew` (or `prelude/methods.quew` depending on partitioning). It is a build artifact — never edited by hand.

#### 4.3.2 Runtime Dispatch Entry (link-time)

The macro generates a registration fragment using the `inventory` crate (or an equivalent link-section collector):

```rust
#[doc(hidden)]
mod __quew_builtin_string_len {
    use super::*;
    use quew_runtime::{NativeRegistry, NativeEntry, NativeHandler, Value};

    inventory::submit! {
        NativeEntry {
            id: "std.string.len",
            arity: 1,
            handler: NativeHandler::Sync(|args| {
                let s = args[0].as_str()
                    .ok_or_else(|| NativeError::type_mismatch("string", args[0].type_name()))?;
                Ok(Value::Number(string_len(s) as i64))
            }),
        }
    }
}
```

At executable startup the runtime iterates the collected inventory and populates `NativeRegistry::entries`.

### 4.4 Build Pipeline

```text
Rust source files in crates/quew-stdlib/src/
    │
    ▼
#[quew_builtin(...)] attributes
    │
    ├──► proc-macro expands ──► inventory entries (link-time)
    │
    └──► build.rs in quew-prelude crate
            │
            ├──► scans link sections / inventory
            ├──► concatenates all `decl` strings
            ├──► auto-wraps extension methods
            └──► writes prelude/native.quew
                        │
                        ▼
            quew-compiler loads prelude/native.quew
            at startup via include_str! or file path
```

The prelude file is rebuilt whenever `quew-stdlib` changes. In CI the build script fails if the generated prelude drifts from committed state.

---

## 5. Trust Boundary

`#[quew_builtin]` is **prelude-only**. The compiler must reject `@@rust` in user source.

| Layer | Can use `#[quew_builtin]`? | Can write `@@rust` in `.quew`? |
|-------|---------------------------|--------------------------------|
| `quew-stdlib` / compiler team | Yes | Yes (generated) |
| Host developer (server builder) | No | No |
| Agent author (end user) | No | No |

This preserves the sandbox: user Quew code cannot inject arbitrary native Rust execution. Only the compiler-owned standard library carries native binding IDs.

---

## 6. Relationship to Existing Plans

| Plan | What it added | How the macro uses it |
|------|---------------|----------------------|
| Plan 11 | `@@rust("id")` metadata | Macro generates `@@rust` prefix from `id` field |
| Plan 12 | `extend Type { ... }` | Macro generates `extend` block and `self` wiring |
| Plan 10 | `!@@function` declarations | `decl` string carries `@@function` signature |
| Plan 16 | `NativeRegistry` + `NativeEntry` | Macro generates `inventory::submit!` entries |

No parser, checker, or IR changes are required. The macro feeds the existing pipeline with auto-generated text.

---

## 7. Open Questions

1. **Signature inference** — Should a future revision derive the `decl` string from the Rust function signature (`&str` → `string`, `bool` → `bool`) instead of requiring an explicit `decl` string? This reduces duplication but adds complexity when Rust types do not map 1:1 to Quew types.

2. **Monomorphization** — If a builtin is generic in Rust (`fn foo<T>(x: T)`), how is the Quew generic parameter represented? Deferred until Quew generics are exercised in native contexts.

3. **Error mapping** — Should the macro auto-generate `NativeError` conversion from the Rust function's `Result` type, or is manual wrapper code preferred for fine-grained diagnostics?

4. **Module partitioning** — Should extension methods be emitted into `prelude/methods.quew` or co-located with their native declarations in `prelude/native.quew`?

5. **Async runtime coupling** — The `inventory` crate uses `ctor` (constructor functions) which may not work on all targets (e.g. WASM). Do we need a `linkme`-based alternative for `wasm32`?

6. **Arity validation** — Should the macro count Rust params and emit a compile-time assertion that the `decl` arity matches? This catches mismatches early.

---

## 8. Summary

`#[quew_builtin]` lets the language author write one Rust function and receive:
- a Quew prelude declaration,
- a runtime native dispatch entry,
- and optionally an extension method wrapper,

all generated at build time. It is a build convenience for the stdlib author, not an end-user extension mechanism. It does not change the compiler's trust model and does not introduce new syntax beyond the existing `@@rust`, `@@function`, and `extend` constructs.
