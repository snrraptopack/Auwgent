# Plan 17: `#[quew_builtin]` Proc-Macro

**Status:** Completed. ✅

**Scope:** Build the `quew-macros` proc-macro crate, integrate `inventory` for link-time native registration, and create a `quew-stdlib` crate that uses the macro to declare the first builtins. The prelude build script auto-generates `.quew` files from the annotated Rust sources.

---

## Where We Are

Plan 16 completed the runtime's `NativeRegistry` — a clean container that gets populated at runtime startup. But:
- There is **no mechanism** to populate it automatically from Rust function definitions.
- There is **no stdlib crate** — builtins are either hardcoded (removed) or test-only.
- The prelude is **hand-written** — every new builtin requires manual edits to `.quew` files.

## Goals

1. Create `quew-macros` — a proc-macro crate exporting `#[quew_builtin]`.
2. Create `quew-stdlib` — a crate that uses `#[quew_builtin]` to declare the first native builtins (string, number, array utilities).
3. Integrate `inventory` (or equivalent) for link-time collection of `NativeEntry` registrations.
4. Create a prelude build script that scans `quew-stdlib` sources and auto-generates `prelude/native.quew` + `prelude/methods.quew`.
5. Wire the generated prelude into the compiler's existing prelude loader.
6. Ensure the trust boundary holds: `@@rust` remains prelude-only.

## Non-Goals

- `#[quew_host_tool]` or `#[quew_plugin]` — host-developer extensibility is a separate RFC.
- Signature inference from Rust types — `decl` remains an explicit string for now.
- Generic Rust builtins — deferred until Quew generics are exercised natively.
- WASM-compatible registration — `inventory` may need a fallback; addressed only if it blocks.
- Async native handlers — the `NativeEntry` enum gains an `Async` variant, but no executor support yet.

## Architecture

### Crate Layout

```
quew-compiler/crates/
  quew-macros/          # NEW: proc-macro crate
    Cargo.toml
    src/lib.rs

  quew-stdlib/          # NEW: standard library implementations
    Cargo.toml
    src/lib.rs
    src/string.rs
    src/number.rs
    src/array.rs

  quew-runtime/         # EXISTS: modified to integrate inventory
    src/native.rs       # add inventory iteration

  quew-checker/         # EXISTS: prelude loader reads generated files
    src/prelude.rs

prelude/                # EXISTS: build script writes here
  native.quew           # GENERATED
  methods.quew          # GENERATED
```

### `quew-macros` — Proc-Macro

```rust
// crates/quew-macros/src/lib.rs
use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn quew_builtin(args: TokenStream, input: TokenStream) -> TokenStream {
    // 1. Parse attribute arguments: id, decl, extend?, method?, async?
    // 2. Parse the annotated function (sync or async)
    // 3. Emit:
    //    a) The original function unchanged
    //    b) An inventory::submit! block wrapping the function for NativeRegistry
    //    c) A build-script-visible metadata struct for prelude generation
}
```

### `inventory` Integration in `quew-runtime`

Add to `native.rs`:

```rust
pub fn collect_inventory() -> NativeRegistry {
    let mut reg = NativeRegistry::new();
    for entry in inventory::iter::<NativeEntry> {
        reg.register(entry.id, entry.clone());
    }
    reg
}
```

`NativeEntry` must implement `inventory::Collect`.

### Prelude Build Script

```rust
// crates/quew-stdlib/build.rs (or a separate quew-prelude crate)
fn main() {
    // Scan src/ for #[quew_builtin(...)] attributes
    // Extract id, decl, extend, method from each
    // Write prelude/native.quew:
    //   - Concatenate all decl strings
    //   - Auto-wrap extension methods
    // Write prelude/methods.quew (optional partitioning)
}
```

### Generated Prelude Example

Input Rust:
```rust
#[quew_builtin(
    id     = "std.string.len",
    decl   = r#"!@@function len(value: string): number"#,
    extend = "string",
    method = "len"
)]
pub fn string_len(value: &str) -> usize { value.len() }

#[quew_builtin(
    id   = "std.number.abs",
    decl = r#"!@@function abs(value: number | float): number | float"#
)]
pub fn number_abs(value: &Value) -> Value { ... }
```

Generated `prelude/native.quew`:
```quew
@@rust("std.string.len")
!@@function len(value: string): number

@@rust("std.number.abs")
!@@function abs(value: number | float): number | float

extend string {
    function len(): number {
        return len(self)
    }
}
```

## Implementation Steps

### Step 1: Create `quew-macros` crate

- Add to workspace `Cargo.toml`
- Implement `#[quew_builtin]` attribute parser
- Parse `id = "..."`, `decl = "..."`, `extend = "..."`, `method = "..."`, `r#async = true`
- Emit the original function plus an `inventory::submit!` block
- Add compile-time assertions (e.g. `extend` and `method` appear together)

### Step 2: Make `NativeEntry` inventory-compatible

- Add `inventory` dependency to `quew-runtime`
- Implement `inventory::Collect` for `NativeEntry`
- Add `collect_inventory()` function
- Keep `NativeRegistry::new()` for test injection

### Step 3: Create `quew-stdlib` crate

- New crate depending on `quew-macros` and `quew-runtime`
- `src/string.rs` — first builtins: `len`, `is_empty`, `contains`, `starts_with`
- `src/number.rs` — `abs`, `clamp`, `min`, `max`
- `src/array.rs` — `len`, `is_empty`, `contains`, `push` (returns new array)
- Each function annotated with `#[quew_builtin]`

### Step 4: Prelude build script

- New `crates/quew-prelude/build.rs` or inline in `quew-stdlib`
- Scan `.rs` files for `#[quew_builtin(...)]` using regex or syn
- Extract `decl` and extension-method wrappers
- Write to `prelude/native.quew` and `prelude/methods.quew`
- Fail build if generated content drifts from committed files (CI guard)

### Step 5: Wire generated prelude into compiler

- Update `quew-checker/src/prelude.rs` to read generated files
- Ensure `module_with_prelude()` loads both `native.quew` and `methods.quew`
- Remove hand-written duplicates from existing prelude files

### Step 6: Integration tests

- Test that `quew-stdlib` builtins are available in compiled Quew programs
- Test extension methods work through the auto-generated wrappers
- Test that user code using `@@rust` is rejected by the checker

## Test Plan

### Unit tests (`quew-macros`)

- Parse valid `#[quew_builtin(...)]` attributes
- Reject invalid combinations (e.g. `extend` without `method`)
- Emit correct `inventory::submit!` tokens

### Unit tests (`quew-runtime`)

- `collect_inventory()` populates registry from linked `quew-stdlib`
- Registry contains expected IDs (`std.string.len`, etc.)

### Integration tests

- Compile a `.quew` that calls `std.string.len` → executes correctly
- Compile a `.quew` that uses `"hello".len()` extension method → executes correctly
- Compile a `.quew` with a user-declared `@@rust` → checker error

### Existing tests

- Full workspace `cargo test` must still pass
- Generated prelude must parse without errors

## Acceptance Criteria

- [x] `quew-macros` crate exists and exports `#[quew_builtin]`
- [x] `#[quew_builtin(id, decl)]` on a sync function generates a `NativeEntry` registration
- [x] `#[quew_builtin(id, decl, extend, method)]` generates an extension method wrapper
- [x] `quew-stdlib` crate exists with 4 string builtins and 2 number builtins
- [x] `inventory` integration populates `NativeRegistry` at executable startup
- [ ] Prelude build script auto-generates `prelude/native.quew` from annotated Rust sources *(deferred — prelude updated by hand for now; build script in Plan 18)*
- [x] The compiler loads the updated prelude and builtins are available to user code
- [x] The checker rejects `@@rust` in user-written `.quew` source *(already implemented in `quew-scope`)*
- [x] All existing tests pass (381+ tests)
