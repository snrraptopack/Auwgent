# Plan 8: Prelude Loading And Tool Result Role Consumption

**Status: Complete.**

Plan 8 is the first consumer of the role-binding system from Plan 7.

The goal is to create a real home for Quew-written builtin source, load that
trusted source before user programs, and use the `(tool, value)` role to wrap
direct tool-call return types.

This is the first step where Quew begins defining Quew.

---

## Starting Point

Plan 7 completed:

- `@@type`, `!@@type`, and `@@(keyword, place) type` parsing;
- builtin type metadata in the AST;
- builtin visibility in `quew-scope`;
- role registry in `quew-scope::roles`;
- role preservation in `quew-ir::Definitions`;
- aggressive tests across lexer, parser, scope, checker, and IR.

The missing piece is prelude loading.

Right now a role-bound type only exists if the current source file declares it.
Plan 8 should make selected role-bound types available from compiler-owned Quew
source.

---

## Goals

By the end of Plan 8:

1. `quew-compiler/prelude/` exists as the home for trusted Quew source.
2. The compiler can parse and merge prelude source before user source.
3. The prelude defines `ToolResult<T>` with the `(tool, value)` role.
4. The checker can look up the `(tool, value)` role.
5. Direct tool-call expression types are wrapped through the role-bound type.
6. Tests prove users can use `result.data` from a tool call without declaring
   `ToolResult<T>` themselves.

---

## Non-Goals

Do not implement these in Plan 8:

- `@@function`;
- `!@@function`;
- `#rust("id")`;
- `extend Type { ... }`;
- provider migration;
- `with { ... }` structural validation from `(with, body)`;
- replacing `Text` in `PrimKeys`;
- runtime execution;
- SDK codegen;
- real import resolution.

This is still compiler frontend work.

---

## Source Layout

Create:

```text
quew-compiler/
  prelude/
    tools.quew
```

Initial `tools.quew`:

```quew
@@(tool, value)
type ToolResult<T> = {
    data: T,
    error: string
}
```

Later files can be added without changing the architecture:

```text
prelude/
  core.quew
  tools.quew
  middleware.quew
  with.quew
  providers.quew
```

---

## Prelude Loader

Add a small module that owns trusted prelude loading.

Recommended crate:

```text
quew-parser or new quew-frontend helper?
```

Better first step:

```text
quew-checker/src/prelude.rs
```

Reason: Plan 8 only needs the checker to see prelude declarations before type
checking. The parser should remain a pure parser for one source string.

But if the logic starts to grow, move it into a dedicated crate later:

```text
crates/quew-prelude/
```

Recommended initial API:

```rust
pub fn parse_prelude(interner: &Arc<Interner>) -> PreludeResult
```

Where:

```rust
pub struct PreludeResult {
    pub module: Module,
    pub diagnostics: Vec<Diagnostic>,
}
```

Use `include_str!("../../../prelude/tools.quew")` or an equivalent path from
the owning crate.

---

## Combined Module Flow

Do not mutate parser behavior.

Instead, introduce a checker entry point that can include the prelude:

```rust
pub fn check_with_prelude(
    module: &Module,
    interner: &Arc<Interner>,
) -> CheckResult
```

The existing `check(module, interner)` can keep its current behavior for unit
tests that intentionally build ASTs directly.

Flow:

1. Parse prelude source.
2. Clone/merge prelude items before user items.
3. Build a single symbol table from the combined module.
4. Type-check the combined module or user module with the combined symbol table.

Recommended behavior:

- The symbol table sees prelude and user declarations.
- Diagnostics from prelude parse/check are included.
- User declarations still produce normal duplicate-name errors if they collide
  with prelude declarations.
- User-facing diagnostics should not become noisy from valid prelude items.

---

## CLI Integration

Update CLI commands to use the prelude-aware checker:

```text
quew check <file>
quew compile <file>
```

The CLI should behave as if the prelude is always available.

Existing direct test helpers may continue using `check()` when they need a
prelude-free environment. Integration tests should use the same prelude-aware
path as CLI once it exists.

---

## Tool Result Wrapping

Current behavior:

```quew
tool getName(): string

function demo(): string {
    return getName()
}
```

The call currently behaves as if `getName()` returns `string`.

New behavior for direct tool calls:

```quew
tool getName(): string

function demo(): string {
    let result = getName()
    return result.data
}
```

The checker should infer:

```text
getName() -> ToolResult<string>
result.data -> string
result.error -> string
```

Important rule:

```text
The checker must not hardcode the name ToolResult.
```

Instead:

1. Look up role key `(tool, value)` in `SymbolTable::roles`.
2. Read the bound type name from the role binding.
3. Instantiate that generic type with the tool's declared return type.
4. Resolve the instantiated type through existing generic substitution.

Suggested helper:

```rust
fn wrap_role_value(
    role: RoleKey,
    value_ty: Ty,
    table: &SymbolTable,
    prim: &PrimKeys,
    diags: &mut Vec<Diagnostic>,
    span: Span,
) -> Ty
```

This should live in a focused checker module, not inside a large expression
match arm.

Possible module:

```text
quew-checker/src/roles.rs
```

Keep `quew-scope/src/roles.rs` as the registry/validation owner.
`quew-checker/src/roles.rs` should own semantic use of roles.

---

## Which Tool Calls Are Wrapped

Plan 8 should wrap direct DSL expression calls to tools:

```quew
let r = getName()
let r = deleteUser("123")
return getName().data
```

Do not change `tools: [getName]` exposure behavior yet. Tool exposure in
`reply(...) with { tools: [...] }` is a model-facing declaration, not a direct
runtime call expression.

Do not change host callback result envelopes in runtime yet. This is checker
typing and IR type-contract work only.

---

## IR Impact

The IR should preserve enough information for downstream runtime work.

Minimum:

- `Definitions.roles` already stores `(tool, value) -> ToolResult`.
- Existing function/tool definitions continue to store declared return types.

Optional if small:

- Add a checker/lowering test showing a direct `HostToolCall` node output type
  is considered wrapped where type information is available.

If current graph nodes do not carry output types, do not force that into Plan 8.
Keep Plan 8 focused on checker semantics.

---

## Tests

Testing should be aggressive and layered.

### Prelude Tests

- Prelude file parses cleanly.
- Prelude module contains `ToolResult<T>`.
- Prelude symbol table contains `ToolResult`.
- Prelude role registry contains `(tool, value)`.
- User source can reference `ToolResult<string>` without declaring it.
- User source colliding with `ToolResult` produces duplicate definition
  diagnostics.

### Checker Role Tests

- Direct host tool call returns wrapped type.
- `let r = getName(); return r.data` type-checks.
- `let r = getName(); return r.error` type-checks as `string`.
- Returning raw `getName()` from a `string` function errors.
- Returning `getName().data` from a `string` function passes.
- Generic substitution works for non-string tool returns, such as `number`.

### Compatibility Tests

- `tools: [getName]` still validates as before.
- Tool prebinding in `tools: [deleteUser(ctx.isAdmin)]` still validates.
- Existing checker and IR tests still pass.
- `check()` without prelude still works for isolated unit tests.
- CLI `check` and `compile` use prelude-aware checking.

### Failure Tests

- Missing `(tool, value)` role in a prelude-free check reports a clear internal
  diagnostic if wrapping is attempted.
- Role type with wrong generic arity reports a useful diagnostic.
- Role binding to a non-type reports a useful diagnostic if representable.

---

## Implementation Order

1. Add `quew-compiler/prelude/tools.quew`.
2. Add a small prelude loader module with tests.
3. Add `check_with_prelude()`.
4. Update CLI to use `check_with_prelude()`.
5. Add checker role-consumption helper module.
6. Wrap direct tool-call return types using `(tool, value)`.
7. Add checker integration tests for `result.data`, `result.error`, and raw
   result mismatch.
8. Run `cargo test --workspace`.

---

## Definition Of Done

- [x] `quew-compiler/prelude/tools.quew` exists.
- [x] Prelude source defines `ToolResult<T>` with `(tool, value)`.
- [x] Prelude source parses in tests.
- [x] `check_with_prelude()` exists.
- [x] CLI `check` uses prelude-aware checking.
- [x] CLI `compile` uses prelude-aware checking.
- [x] Direct tool calls are typed as the role-bound wrapper.
- [x] Tool call wrapper uses role lookup, not the name `ToolResult`.
- [x] `result.data` and `result.error` type-check.
- [x] Raw wrapped tool result mismatches plain declared returns.
- [x] Existing `tools: [...]` behavior remains compatible.
- [x] `cargo test --workspace` passes.

---

## Deferred After Plan 8

Plan 9 should move `with { ... }` shape validation toward `(with, body)`.

Plan 10 should add builtin functions and begin provider migration.

Plan 11 should introduce `#rust("id")` native builtin leaves.

Plan 12 should add `extend Type { ... }`, implicit `self`, and method lookup.
