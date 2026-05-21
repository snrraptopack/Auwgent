# Discussion 14: Builtin Pipeline Architecture + `print` + `x is Type`

**Date:** 2026-05-19
**Status:** Discussion complete → Plan 20

---

## Topics

1. **Builtin pipeline coordination** — Why `#[quew_builtin]` and `.quew` declarations feel duplicated
2. **`print` builtin** — Debug output for compiled quew programs
3. **`x is Type` runtime discrimination** — The last non-agent core language gap

---

## 1. Builtin Pipeline: Why Two Sources of Truth?

### Current State

Every builtin requires **two independent declarations**:

**Rust side** (`quew-stdlib/src/string.rs`):
```rust
#[quew_builtin(
    id   = "std.string.len",
    decl = r#"!@@function len(value: string): number"#,
)]
pub fn string_len(value: &str) -> i64 { value.len() as i64 }
```

**Quew side** (`prelude/native.quew`):
```quew
@@rust("std.string.len")
!@@function len(value: string): number
```

### The Problem

The `decl` string in `#[quew_builtin]` is **dead code**. Nothing reads it. The compiler parses `native.quew` independently. If you change the Rust signature but forget to update `.quew`, the `id` might still match but the type checker and runtime will disagree on parameter names, types, or arity. The only coordination point is the `id` string.

### Root Cause: Two Compile Times

| Pipeline | Compile Time | Consumes | Produces |
|----------|-------------|----------|----------|
| `#[quew_builtin]` | Rust compile / link time | Rust source | `NativeRegistry` entry (runtime callable) |
| `.quew` prelude | Quew compile time | `native.quew` text | `SymbolTable` entry + IR `FunctionDef.native` |

These pipelines are **temporally separated**. The Quew compiler doesn't know about Rust `inventory` entries. The Rust linker doesn't know about `.quew` files. `inventory` only works at Rust link time — you can't query it from a proc-macro or build script.

### Design Options

**Option A: Manual coordination (current)**
- Keep both declarations. Human ensures they match.
- Add a **verification test** that iterates all `#[quew_builtin]` `decl` strings, parses them, and asserts they match the corresponding `native.quew` entry.
- Pros: Simple, no build scripts.
- Cons: Still two sources of truth.

**Option B: Auto-generate `.quew` from Rust source**
- Write a `build.rs` in `quew-stdlib` that scans its own `.rs` files for `#[quew_builtin]` attributes, parses the `decl` string, and writes `prelude/native.quew`.
- Pros: Single source of truth (`decl` becomes the canonical declaration).
- Cons: `build.rs` runs before Rust compilation of `quew-stdlib`, but `quew-stdlib` depends on `quew-macros` (the proc-macro crate). Circular build ordering issues. Also, `decl` is just a string — parsing it is fragile.

**Option C: Auto-generate Rust stubs from `.quew`**
- Write a codegen tool that parses `native.quew` and generates Rust trait stubs.
- Pros: `.quew` is the single source of truth.
- Cons: Extra build step. Rust implementations still need to be written manually.

**Option D: Runtime type introspection (future)**
- At runtime, `NativeEntry` carries a `signature: FunctionSignature` field.
- The runtime validates arg count and types before dispatch.
- Pros: Catches mismatches at runtime. `.quew` can be simplified or eliminated.
- Cons: Overhead. Doesn't help the compiler type-check user code.

### Recommendation

**Keep manual coordination for now** (Option A), but add three things:
1. A **compile-time verification test** in `quew-stdlib` that ensures every `#[quew_builtin]` id has a matching `@@rust` entry in `native.quew`, and vice versa.
2. **Split `native.quew` into modules** (`native/string.quew`, `native/array.quew`, `native/io.quew`) for readability. The loader concatenates them.
3. **Document the `decl` field as aspirational** — it's reserved for future codegen, not consumed today.

The real fix (Option B or C) requires a build-script or proc-macro investment that's not justified while the language is still stabilizing.

---

## 2. `print` Builtin

### Motivation

The user wants to write quew code, compile it, and see the actual output to catch errors that automated tests miss. Currently there is **zero side-effectful I/O** in the deterministic executor. You can compute values but you can't observe them without returning them.

### Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Signature | `print<T>(value: T): null` | Generic — accepts any type. Returns `null` because it's a statement-like operation. |
| Side effects | `println!` to stdout | Standard, observable, simple. |
| Return type | `null` | Not `void` (runtime has no `Value::Void`). `null` is the quew idiom for "no value". |
| Module | `std.io.print` | Follows existing naming: `std.string.*`, `std.array.*`, `std.number.*`. |
| Prelude file | `native/io.quew` | Split from `native.quew` for modularity. |

### Why Generic?

`print<T>(value: T)` uses the existing generic inference system. When the user writes `print(42)`, `T` infers to `number`. When they write `print("hello")`, `T` infers to `string`. No `any` type needed.

### Why Not Return the Value (like `dbg!`)?

The user asked for `print`, not `dbg`. A `print` that returns `null` is clearer about its side-effect nature. If we want `dbg` later, we can add `std.io.dbg<T>(value: T): T`.

---

## 3. `x is Type` Runtime Discrimination

### Current State

- **Parser:** ✅ `x is Type` parses into `Expr::Is(IsExpr { value, ty, span })`
- **Checker:** ✅ Returns `Ty::bool()` (blindly — doesn't validate the type exists)
- **IR Lowerer:** ❌ Maps to `IrLit::Null` (stub)
- **Runtime:** ❌ No evaluation

### What's Needed

Add an `IrExpr::Is` variant and evaluate it at runtime.

### Runtime Semantics

At runtime, `Value` carries no type metadata beyond its discriminant. So `is` checks the discriminant against a hardcoded list of primitive type names. For named record types, we can only check `Value::Object` (all records are objects at runtime).

| Quew type | Runtime check |
|-----------|---------------|
| `string` | `matches!(Value::String(_))` |
| `number` | `matches!(Value::Number(_))` |
| `float` | `matches!(Value::Float(_))` |
| `bool` | `matches!(Value::Bool(_))` |
| `null` | `matches!(Value::Null)` |
| `T[]` / `array` | `matches!(Value::Array(_))` |
| Record types (e.g. `Person`) | `matches!(Value::Object(_))` |

**Precision limitation:** `x is Person` and `x is User` both return `true` for any object, because `Value::Object` doesn't carry the record type name. Fixing this requires adding runtime type tags to objects, which is out of scope. We'll document this limitation.

### Why This Is the Last Non-Agent Gap

From `GAPS.md`, the remaining items are:
- Middleware — agent feature
- `break`/`continue` — deferred, needs native loop nodes
- `x is Type` — **this is the last pure language gap**
- Dynamic model in `with` block — agent feature
- HostToolCall / Reply / AgentCall — agent features
- `fetch()` — I/O, deferred
- JSON builtins — I/O, deferred

---

## Performance Considerations

### `print`
- `println!` is a syscall. It's already outside the hot path (debugging only).
- No allocation beyond `Value`'s existing `Display` impl.

### `x is Type`
- Single discriminant match — O(1).
- String comparison on the type name uses interned strings (`InternedStr` → `u32` comparison in the registry lookup, but the evaluator receives a resolved `&str`). We compare `&str` against a small set of literals. Branch predictor will make this effectively free.

### Prelude Modularity
- Splitting `native.quew` into multiple files has **zero runtime cost** — they're `include_str!`ed and concatenated at Rust compile time.
- Slightly slower Quew compile time (parsing more files), but negligible.

---

## Modularity Plan

```
prelude/
  native.quew          → becomes a loader/aggregator
  string.quew          → string builtins + extension methods
  array.quew           → array builtins
  number.quew          → number builtins
  io.quew              → print, future dbg/log
```

The checker prelude loader reads all `.quew` files in `prelude/` and concatenates them before parsing.

---

## Open Questions

1. **Should `print` go to stderr instead of stdout?** — No, stdout is the default for `print`. We can add `eprint` later.
2. **Should `x is Type` support union types?** — No. `x is string | number` is not a common pattern and complicates the runtime check. The user can write `x is string or x is number`.
3. **Should we add runtime type tags to objects?** — Not now. It requires changing `Value::Object` to carry a type name, which touches the entire runtime. Defer until record type precision is actually needed.
