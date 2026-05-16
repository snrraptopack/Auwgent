# Plan 4: Type System, Scoping, and Semantic Checking

## Where We Are

`quew-parser` is complete and locked. **24 tests, 0 failures.**

**What exists:**
- `quew-errors`, `quew-interner`, `quew-source` — tested foundations
- `quew-lexer` — tokens and lex result (73 tests)
- `quew-ast` — all AST nodes with Span (46 tests)
- `quew-parser` — full grammar, error recovery, chumsky 0.13 (24 tests)

**Stubs ready to be filled:**
- `quew-types` — `Ty` enum (has correct deps: `indexmap`, `quew-interner`)
- `quew-scope` — `SymbolTable` (has correct deps: `quew-ast`, `quew-types`)
- `quew-unify` — unification table (has correct deps: `ena`, `quew-types`)
- `quew-checker` — orchestration pass (has correct deps: all of the above)

**Skipped for now:**
- `quew-resolve` — cross-file import resolution; no `import` syntax yet

---

## Goals of This Plan

By the end:

1. `quew-types` defines every `Ty` variant with correct `Ty::Tool` / `Ty::Agent` / `Ty::Function` distinction
2. `quew-scope` builds a `SymbolTable` for a single file by walking the AST
3. `quew-unify` wraps `ena` for type variable unification
4. `quew-checker` walks the AST, checks names, validates types, returns `Vec<Diagnostic>`
5. All four crates have unit tests; `cargo test -p quew-types -p quew-scope -p quew-unify -p quew-checker` passes with 0 failures

---

## crate: `quew-types`

**Single responsibility:** own the `Ty` enum and its structural operations.
No syntax. No inference. No diagnostics. Just the type algebra.

### Design rules

- No dependency on `quew-ast` — `Ty` must not reference syntax nodes
- All string fields use `InternedStr` — zero-allocation handles
- Record fields use `IndexMap` — deterministic iteration order
- `Ty` is `Clone + PartialEq` — the checker freely compares and copies types
- `ena` is NOT here — unification lives in `quew-unify`

### The `Ty` enum

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    // ── Primitive types ───────────────────────────────────────────────────────
    Primitive(PrimTy),

    // ── Composite types ───────────────────────────────────────────────────────
    /// A named record type: `{ name: string, age: number }`.
    /// Fields are ordered (IndexMap preserves insertion order).
    Record(IndexMap<InternedStr, Ty>),

    /// Union of two or more types: `string | number | bool`.
    Union(Vec<Ty>),

    /// Nullable wrapper: `T?` — sugar for `T | null`.
    Optional(Box<Ty>),

    // ── Callable types ────────────────────────────────────────────────────────
    /// A plain DSL function (`function foo(a: string): bool`).
    /// Params are positional; all are model/caller-provided.
    Function(FunctionTy),

    /// An agent declaration (`agent Name(input: T): R`).
    /// Always exactly one input param. The return type is optional (defaults void).
    /// Agents are not directly callable from DSL — they are entry points.
    Agent(AgentTy),

    /// A host-backed tool callable from the DSL.
    ///
    /// Tools differ from functions in two ways:
    ///   1. They have **bound params** injected from `@tool(name: Type)` context —
    ///      these are filled by the host at call time and never provided by the model.
    ///   2. They have **model params** — the args the model (or DSL caller) must supply.
    ///
    /// At runtime the checker ensures that any `@name: Type` param in the function
    /// signature has a matching name in the tool's `bound_params`.
    Tool(ToolTy),

    // ── Provider type ─────────────────────────────────────────────────────────
    /// A model declaration (`model Name = { model: gemini("..."), config: {...} }`).
    Provider(ProviderKind),

    // ── Inference placeholders ────────────────────────────────────────────────
    /// A type variable — produced during inference, resolved by `quew-unify`.
    Var(TyVar),

    /// Error sentinel — emitted when a name is unresolvable or a type is malformed.
    /// Prevents cascading errors: any operation on `Ty::Error` produces `Ty::Error`.
    Error,
}
```

### Primitive types

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimTy {
    String,
    Number,
    Float,
    Bool,
    Void,
    Null,
}
```

### Callable subtypes

```rust
/// Plain function type.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionTy {
    /// Positional parameters: (name, type).
    pub params: Vec<(InternedStr, Ty)>,
    /// Return type. `None` is treated as `Ty::Primitive(PrimTy::Void)`.
    pub return_ty: Box<Ty>,
}

/// Agent type — single-input DSL entry point.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentTy {
    /// The single required input parameter.
    pub input_name: InternedStr,
    pub input_ty: Box<Ty>,
    /// Return type (None = void).
    pub return_ty: Box<Ty>,
}

/// Tool type — host-backed callable with bound + model params.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolTy {
    /// Params bound from context via `@tool(name: Type)`.
    /// These are supplied by the host at call time, NOT by the model.
    /// The checker verifies that every `@name: Type` param in the function
    /// body has a matching entry here.
    pub bound_params: Vec<(InternedStr, Ty)>,

    /// Params that the model (or DSL caller) must supply.
    pub model_params: Vec<(InternedStr, Ty)>,

    /// Return type.
    pub return_ty: Box<Ty>,
}
```

### Provider kind

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind { Gemini, OpenAi, Groq }
```

### Type variable (for unification)

```rust
/// Opaque type variable index — used by `quew-unify`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TyVar(pub u32);
```

### Key operations on `Ty`

```rust
impl Ty {
    /// Returns true if `self` is structurally assignable to `target`.
    /// Does NOT unify type variables — that is `quew-unify`'s job.
    pub fn is_assignable_to(&self, target: &Ty) -> bool { ... }

    /// Flatten nested unions: `(A | (B | C)) -> (A | B | C)`.
    pub fn flatten_union(self) -> Ty { ... }

    /// Unwrap optional: `T? -> T`.
    /// Returns `None` if not `Ty::Optional`.
    pub fn inner_optional(&self) -> Option<&Ty> { ... }

    /// True if this type can never cause a cascading error (not `Ty::Error`).
    pub fn is_ok(&self) -> bool { !matches!(self, Ty::Error) }
}
```

---

## crate: `quew-scope`

**Single responsibility:** build a per-file `SymbolTable` by walking a `Module`.

### Key types

```rust
/// A binding registered in the symbol table.
pub struct Symbol {
    pub ty: Ty,
    pub kind: SymbolKind,
    pub def_span: Span,
}

pub enum SymbolKind {
    Let,        // top-level `let name = expr`
    Function,   // `function foo(...)`
    Agent,      // `agent Foo(...)`
    Tool,       // `tool foo(...)` — host-backed
    ToolGroup,  // `tools { ... }`
    Type,       // `type Foo = { ... }`
    Model,      // `model MyModel = { ... }`
    Param,      // function/agent parameter
    Local,      // `let` inside a block
}

/// The output of a single-file scope build pass.
pub struct SymbolTable {
    /// Top-level names declared in the file. Ordered (IndexMap).
    pub globals: IndexMap<InternedStr, Symbol>,
    /// Diagnostics emitted during the pass (duplicates, etc.).
    pub diagnostics: Vec<Diagnostic>,
}
```

### Scope stack (used during building, not in final output)

```rust
/// A single lexical frame.
struct Frame {
    bindings: IndexMap<InternedStr, Symbol>,
}

/// Scope stack — innermost frame is last.
struct Scope {
    frames: Vec<Frame>,
}

impl Scope {
    fn push(&mut self);
    fn pop(&mut self);
    fn define(&mut self, name: InternedStr, sym: Symbol) -> Option<Symbol>; // returns shadowed
    fn lookup(&self, name: InternedStr) -> Option<&Symbol>; // innermost-first
}
```

### Builder entry point

```rust
/// Walk a parsed `Module` and produce a `SymbolTable`.
/// Errors are accumulated — never panics on bad input.
pub fn build_symbol_table(
    module: &Module,
    interner: &Interner,
) -> SymbolTable { ... }
```

### What the builder checks

- **Duplicate top-level names** — two `function foo` declarations in the same file → error
- **Undefined names in expressions** — `let x = foo()` where `foo` is not declared → error
- **Bound param validation** — every `@name: Type` in a `@tool` function must match a `bound_param` in the `@tool(...)` annotation
- Does NOT check types yet — that is `quew-checker`'s job

---

## crate: `quew-unify`

**Single responsibility:** type variable unification using `ena`'s union-find.

### Key types

```rust
/// Thin wrapper around `ena::UnificationTable<TyVar>`.
pub struct UnifyTable {
    inner: ena::unify::InPlaceUnificationTable<TyVar>,
}

impl UnifyTable {
    pub fn new() -> Self;
    /// Allocate a fresh type variable.
    pub fn new_var(&mut self) -> TyVar;
    /// Unify two types. Returns `Err(UnifyError)` on conflict.
    pub fn unify(&mut self, a: &Ty, b: &Ty) -> Result<(), UnifyError>;
    /// Resolve a type variable to its concrete type (if unified).
    pub fn resolve(&self, var: TyVar) -> Option<Ty>;
    /// Fully substitute all type vars in a Ty with their resolved types.
    pub fn apply(&self, ty: Ty) -> Ty;
}

pub struct UnifyError {
    pub expected: Ty,
    pub found: Ty,
}
```

> **Note:** `ena` requires `TyVar` to implement `ena::unify::UnifyKey`. That implementation lives here.

---

## crate: `quew-checker`

**Single responsibility:** walk the AST with scope + type information and emit diagnostics.

Depends on all previous crates. Orchestrates them.

### Entry point

```rust
pub struct CheckResult {
    pub symbol_table: SymbolTable,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn check(
    module: &Module,
    interner: &Arc<Interner>,
) -> CheckResult { ... }
```

### Checks performed (in order)

1. **Build symbol table** (`quew-scope`) — registers all top-level names
2. **Type-check expressions** — infers types bottom-up, unifies with declared types
3. **Return type validation** — every code path in a function must return the declared type
4. **Tool binding validation** — `@name` params in a `@tool` function must match the annotation
5. **Agent contract** — agent takes exactly one param, returns the declared type or void
6. **Unreachable code** — statement after unconditional `return` → warning

### Bound param binding — detailed

This is the core check for `Ty::Tool`.

**How binding works end-to-end:**

```quew
@tool(id: string)                          // 1. Host contract — what the host will inject
@desc "Delete a user"
function deleteUser(isAdmin: bool, @id: string): string {
    //                             ^^^^^^^^^^^^
    //  2. BoundRef param — brings `id` into scope as a regular variable.
    //     The `@` means: "pull from @tool context, not from the caller."

    let result = delete_user(id)           // 3. Used as plain `id` — no @ here
    ...
}
```

**Three-part rule:**

| Part | Syntax | Role |
|------|--------|------|
| `@tool(id: string)` | annotation | Host contract — names values the host injects |
| `@id: string` in param list | `ParamBinding::BoundRef` | Import — brings `id` into function body scope |
| `id` in body | plain ident | Usage — accessed as a regular local variable |

**Checker rules:**
1. Every `@name: Type` param (`ParamBinding::BoundRef`) must have a matching `name` in `@tool(...)` bound params
2. The types must match — `@id: number` vs `@tool(id: string)` → type mismatch error
3. A function with `@name` params but **no** `@tool` annotation → error
4. A `@tool(...)` param that is NOT imported in the param list → **valid** (host injects it, function just doesn't use it)
5. The bound name is available in the function body exactly like a regular `let` binding — no `@` prefix at usage sites

---

## Implementation Order

```
1. quew-types   ← no deps on the other three; define Ty + PrimTy + callable subtypes
2. quew-scope   ← depends on quew-types + quew-ast
3. quew-unify   ← depends on quew-types (+ ena)
4. quew-checker ← depends on all three above
```

---

## Testing Mandate

### `quew-types`
- Construct every `Ty` variant
- `is_assignable_to`: primitives, record narrowing, union membership
- `flatten_union`: nested union flattening
- `inner_optional`: unwrap T? → T
- `Ty::Error` propagation: any op on Error returns Error

### `quew-scope`
- Single-file: all top-level item kinds register correctly
- Duplicate name: second definition produces a diagnostic
- Bound param match: `@tool(id: string)` + `@id: string` param → ok
- Bound param mismatch: `@tool(id: string)` + `@missing: number` param → error

### `quew-unify`
- Fresh vars unify with concrete types
- Conflicting types produce `UnifyError`
- `apply()` fully substitutes resolved vars

### `quew-checker`
- Valid agent: declared return type matches actual
- Valid tool: bound params match annotation
- Undefined name: error diagnostic, no panic
- Return type mismatch: error diagnostic on each bad path
- Empty body: no return required if return type is `void`

---

## What We Are NOT Doing in This Plan

- Import resolution (`quew-resolve`) — no `import` syntax yet; deferred to a later plan
- IR lowering (`quew-ir`) — plan 5
- Codegen (`quew-codegen`) — plan 6

---

## Definition of Done

- [ ] `Ty` enum defined with all variants including `Tool`, `Agent`, `Function`
- [ ] `ToolTy` carries `bound_params` separate from `model_params`
- [ ] `is_assignable_to` implemented and tested
- [ ] `SymbolTable` built from a `Module` with duplicate-name detection
- [ ] Bound param cross-check implemented in `quew-scope` or `quew-checker`
- [ ] `UnifyTable` wraps `ena` and passes basic unification tests
- [ ] `check()` entry point runs all passes and returns `CheckResult`
- [ ] `cargo test -p quew-types` → 0 failures
- [ ] `cargo test -p quew-scope` → 0 failures
- [ ] `cargo test -p quew-unify` → 0 failures
- [ ] `cargo test -p quew-checker` → 0 failures
