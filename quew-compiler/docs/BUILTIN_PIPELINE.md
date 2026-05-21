# Quew Builtin Pipeline: Rust → Quew → IR → Runtime

This document describes the complete data flow for adding a new builtin function to quew — from Rust implementation through Quew source declaration to compiled IR and runtime dispatch.

---

## Architecture Overview

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Rust Function  │────▶│  #[quew_builtin]│────▶│ NativeRegistry  │◀────│   Runtime       │
│  (stdlib)       │     │  proc-macro     │     │  (inventory)    │     │   Executor      │
└─────────────────┘     └─────────────────┘     └─────────────────┘     └─────────────────┘
        │                                               ▲                        │
        │                                               │                        │
        │  decl = "..." (aspirational, not consumed)    │                        │
        ▼                                               │                        │
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐            │
│  Quew Prelude   │────▶│  Parser/Checker │────▶│  IR (FunctionDef│────────────┘
│  (prelude/*.quew)│     │  (Symbol Table) │     │   .native = id) │
└─────────────────┘     └─────────────────┘     └─────────────────┘
```

The pipeline has **two independent input paths** that converge at the runtime:
1. **Rust path**: The Rust function is wrapped by `#[quew_builtin]` and auto-registered via `inventory`.
2. **Quew path**: The function is declared in `prelude/*.quew` with `@@rust("id")`, parsed, checked, and lowered into IR.

The runtime connects them: the IR says "this function is native, its ID is X", and the registry says "ID X maps to this Rust function pointer".

**Important:** The `decl` string in `#[quew_builtin]` is **not consumed by anything today**. It exists for documentation and future codegen, but the compiler parses the `.quew` file independently. The only coordination point is the `id` string. See [§Coordination Problem](#the-coordination-problem) below.

---

## Step-by-Step Walkthrough

### Step 1: Write the Rust Function

**Location:** `quew-runtime/crates/quew-stdlib/src/<module>.rs`

Example (`array.rs`):

```rust
use quew_macros::quew_builtin;

#[quew_builtin(
    id = "std.array.len",
    decl = r#"!@@function array_len(value: number[]): number"#
)]
pub fn array_len(arr: &Value) -> Value {
    match arr {
        Value::Array(a) => Value::Number(a.len() as i64),
        _ => Value::Null,
    }
}
```

**Rules:**
- Use `#[quew_builtin(id = "...", decl = "...")]`.
- `id` is a **stable global identifier** (e.g., `std.array.len`). This string must match the `@@rust("...")` annotation in the prelude exactly.
- `decl` is the Quew function signature as a string literal. It exists for documentation and potential code generation.
- The function takes `&Value` (or `&str`, `i64`, `f64`, `bool`) and returns `Value` (or `Result<Value, NativeError>`).
- The macro generates a wrapper that receives a `&[Value]` slice, extracts arguments by position, calls your function, and returns the result.
- At link time, `inventory::submit!` registers the wrapper in `NativeRegistry`.

---

### Step 2: Export from `quew-stdlib`

**Location:** `quew-runtime/crates/quew-stdlib/src/lib.rs`

```rust
pub mod array;
pub mod string;
pub mod number;
```

Any crate that links `quew-stdlib` will auto-collect its builtins via `inventory`.

---

### Step 3: Declare in the Quew Prelude

**Location:** `quew-compiler/prelude/native.quew`

```quew
@@rust("std.array.len")
!@@function array_len(value: number[]): number
```

**Rules:**
- `@@rust("std.array.len")` attaches a `NativeBinding` to the AST `FunctionDecl`.
- `!@@function` marks it as an **internal builtin** (`BuiltinFunctionMeta::internal()`). This tells the checker that the function has no Quew body — its implementation is provided by the runtime.
- The parameter names and types must match what the Rust function expects.

**Optional: Extension Methods**

You can also add extension methods that call the native free function:

```quew
extend number[] {
    function len(): number {
        return array_len(self)
    }
}
```

Extension methods are **not** native at the IR level. They have a Quew body that calls the native free function. At runtime, the extension method graph dispatches to the native function via `FuncCall`.

---

### Step 4: Prelude Loading (Compile Time)

**Location:** `quew-compiler/crates/quew-checker/src/prelude.rs`

```rust
const NATIVE_PRELUDE: &str = include_str!("../../../prelude/native.quew");
```

**Data flow:**
1. `parse_prelude()` lexes and parses `native.quew` using the standard pipeline.
2. `module_with_prelude()` concatenates prelude items with the user's module.
3. The checker builds a `SymbolTable` from the merged module.
4. In `quew-scope/src/lib.rs`, functions with `@@rust` bindings get a `Symbol` whose `native` field is `Some(interned_id)`.

---

### Step 5: IR Lowering

**Location:** `quew-compiler/crates/quew-ir/src/lower/defs.rs`

The IR `FunctionDef` captures the native binding:

```rust
pub struct FunctionDef {
    pub graph_ref: String,
    pub params: IndexMap<InternedStr, IrType>,
    pub return_ty: IrType,
    pub native: Option<InternedStr>, // Some(id) if @@rust annotated
}
```

**Data flow:**
1. `lower_defs()` iterates over the symbol table.
2. For each function with `sym.native = Some(id)`, it creates a `FunctionDef` with `native: Some(id)`.
3. The function is added to `definitions.functions` under its interned name.

---

### Step 6: Runtime Dispatch

There are **two dispatch sites** — one for graph-level calls, one for inline expression calls.

#### Site A: `NodeKind::FuncCall` (graph nodes)

**Location:** `quew-runtime/crates/quew-runtime/src/execution.rs`

```rust
NodeKind::FuncCall { function, args } => {
    // 1. Check if native
    let is_native = self.ir.definitions.functions.get(function)
        .and_then(|def| def.native)
        .and_then(|native_id| self.natives.get(self.interner.resolve(native_id)));

    if let Some(entry) = is_native {
        // Native path: evaluate args, call handler directly
        let mut arg_values = Vec::with_capacity(args.len());
        for (_slot, data_ref) in args {
            arg_values.push(self.resolve_data_ref(data_ref, &outputs)?);
        }
        let result = match &entry.handler {
            NativeHandler::Sync(f) => f(&arg_values).map_err(|e| ...)?
        };
        outputs.insert(*node_id, result);
        continue;
    }

    // 2. Graph path: package args into object, recurse into subgraph
    let graph_ref = self.resolve_function_graph(*function);
    let mut obj = indexmap::IndexMap::new();
    for (slot, data_ref) in args {
        obj.insert(self.interner.resolve(*slot).to_string(),
                   self.resolve_data_ref(data_ref, &outputs)?);
    }
    let result = self.run(&graph_ref, Value::Object(obj))?;
    outputs.insert(*node_id, result);
}
```

#### Site B: `IrExpr::Call` (inline expressions inside LetBind)

**Location:** `quew-runtime/crates/quew-runtime/src/eval.rs`

```rust
IrExpr::Call { function, args } => {
    let func_name = interner.resolve(*function);

    // Try direct lookup (for manually constructed IR)
    let native_entry = natives.get(func_name)
        // Try via definitions.native mapping (for compiled prelude builtins)
        .or_else(|| {
            ir.definitions.functions.get(function)
                .and_then(|def| def.native)
                .and_then(|native_id| natives.get(interner.resolve(native_id)))
        });

    if let Some(entry) = native_entry {
        // Native path
        let mut arg_values = Vec::with_capacity(args.len());
        for (_name, arg_expr) in args {
            arg_values.push(eval_expr(arg_expr, outputs, interner, natives, ir)?);
        }
        match &entry.handler {
            NativeHandler::Sync(f) => f(&arg_values).map_err(|e| ...)
        }
    } else {
        // Graph path: recurse into subgraph
        let graph_ref = resolve_function_graph(*function, func_name, ir);
        ...
    }
}
```

**Key design point:** Both sites follow the same **native-first, graph-second** pattern. The runtime never hardcodes a list of builtins.

---

## Native Registry Startup

**Location:** `quew-runtime/crates/quew-runtime/src/native.rs`

```rust
impl NativeRegistry {
    pub fn collect() -> Self {
        let mut entries = HashMap::new();
        for entry in inventory::iter::<NativeEntry> {
            entries.insert(entry.id.to_string(), NativeEntry {
                id: entry.id,
                handler: entry.handler,
            });
        }
        NativeRegistry { entries }
    }
}
```

At runtime startup, `NativeRegistry::collect()` iterates over all `inventory::submit!` registrations and builds a `HashMap<String, NativeEntry>`. This happens automatically when `quew-stdlib` is linked.

---

## The Coordination Problem

**Why do we need both `#[quew_builtin]` and `.quew` declarations?**

They serve two different compile-time pipelines:

| Pipeline | When it runs | Input | Output |
|----------|-------------|-------|--------|
| `#[quew_builtin(id="...", decl="...")]` | Rust compile / link time | Rust source | `NativeRegistry` entry (runtime callable) |
| `@@rust("...")` + `!@@function` in `.quew` | Quew compile time | `.quew` text | Symbol table entry + IR `FunctionDef.native` |

The `id` string is the only coordination point. It must match on both sides.

**Why `decl` feels uncoordinated:**

The `decl` attribute in `#[quew_builtin]` is **dead code** — nothing reads it. The compiler parses `prelude/*.quew` independently. If you change the Rust signature but forget to update `.quew`, the `id` will still match but the type checker and runtime may disagree on parameter names, types, or arity.

The `decl` field was designed for a future build script that would scan Rust source for `#[quew_builtin]` attributes and auto-generate the `.quew` prelude, but that script was never written.

**Mitigations (current):**
1. Keep manual coordination via the `id` string.
2. Add verification tests that assert every `@@rust` id in `.quew` has a matching `#[quew_builtin]` registration.
3. Split the prelude into modular files so additions are localized.

**Future fix:**
- Option A: Build script in `quew-stdlib` that scans `.rs` files and writes `.quew` files from `decl` strings.
- Option B: Invert — parse `.quew` and generate Rust trait stubs.

Both require build-system investment that's not justified while the language is still stabilizing.

---

## Adding a New Builtin: Checklist

To add a new builtin (e.g., `std.io.debug`):

1. [ ] **Implement in Rust** (`quew-runtime/crates/quew-stdlib/src/io.rs`)
   ```rust
   #[quew_builtin(id = "std.io.debug", decl = r#"!@@function debug<T>(value: T): T"#)]
   pub fn debug(value: &Value) -> Value {
       eprintln!("{:?}", value);
       value.clone()
   }
   ```

2. [ ] **Declare in prelude** (`quew-compiler/prelude/io.quew`)
   ```quew
   @@rust("std.io.debug")
   !@@function debug<T>(value: T): T
   ```

3. [ ] **Add runtime test** (`quew-runtime/crates/quew-runtime/src/execution.rs`)
   Compile a quew function that calls the builtin and assert the result.

4. [ ] **No compiler changes needed** — the parser, checker, and lowerer already understand `@@rust` and `!@@function`.

---

## Key Files Reference

| Purpose | Path |
|---------|------|
| Proc macro | `quew-runtime/crates/quew-macros/src/lib.rs` |
| Stdlib implementations | `quew-runtime/crates/quew-stdlib/src/*.rs` |
| Native registry | `quew-runtime/crates/quew-runtime/src/native.rs` |
| Graph executor dispatch | `quew-runtime/crates/quew-runtime/src/execution.rs` |
| Expression evaluator dispatch | `quew-runtime/crates/quew-runtime/src/eval.rs` |
| Quew prelude (aggregator) | `quew-compiler/prelude/native.quew` |
| Quew prelude — string | `quew-compiler/prelude/string.quew` |
| Quew prelude — array | `quew-compiler/prelude/array.quew` |
| Quew prelude — number | `quew-compiler/prelude/number.quew` |
| Quew prelude — io | `quew-compiler/prelude/io.quew` |
| Prelude loader | `quew-compiler/crates/quew-checker/src/prelude.rs` |
| IR definitions | `quew-compiler/crates/quew-ir/src/defs.rs` |
| IR lowering | `quew-compiler/crates/quew-ir/src/lower/defs.rs` |

---

## Extension Methods vs Native Functions

| Aspect | Native Free Function | Extension Method |
|--------|---------------------|------------------|
| Declaration | `!@@function foo(x: T): R` | `extend T { function foo(): R { ... } }` |
| IR representation | `FunctionDef { native: Some(id), ... }` | Regular function graph (`FunctionDef { native: None, ... }`) |
| Runtime dispatch | Direct native call (bypasses graph) | Graph call → `FuncCall` → native call |
| Body | None (implemented in Rust) | Quew body (usually calls native free function) |
| Example | `array_len(arr)` | `arr.len()` |

---

## What Can Be Changed?

The pipeline is intentionally decoupled. Each layer only knows about the layer immediately above and below:

- **Rust stdlib** → knows its own `id` string. Does not know about Quew syntax.
- **Quew prelude** → knows the `id` string and the Quew signature. Does not know about Rust types.
- **Compiler** → knows the `id` string is attached to a function. Does not know about the Rust implementation.
- **Runtime** → knows how to look up `id` in `NativeRegistry`. Does not know where the function was defined.

This means you can:
- **Replace the Rust implementation** without changing Quew code (same `id`).
- **Rewrite a Quew extension method** without changing the native function.
- **Add new builtins** without touching the compiler or runtime (just step 1 + 2 above).
