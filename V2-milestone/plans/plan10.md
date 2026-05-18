# Plan 10: Builtin Functions And Model Body Contract

**Status: In Progress.**

Plan 10 starts moving hardcoded model/provider construction into compiler-owned
Quew source without introducing a separate provider-role system.

Discussion 5 already gives the intended direction:

- builtin functions exist through `@@function` and `!@@function`;
- provider builders such as `gemini`, `openai`, and `groq` become builtin
  functions;
- those functions return a Quew-defined builtin `Model` type;
- model declaration bodies can be described by a role-bound type such as
  `(model, body)`.

The important shift is that the compiler should not ask:

```text
Is this expression a hardcoded ProviderCall?
```

It should move toward asking:

```text
Which function creates a Model?
Which type defines the shape of a `model Name = { ... }` body?
```

This keeps the language surface in Quew while the compiler still owns checking
and lowering.

---

## Starting Point

Current model/provider behavior is still hardcoded:

- `quew-lexer` emits provider keyword tokens for `gemini`, `openai`, and `groq`.
- `quew-parser` parses those tokens into `Expr::Provider(ProviderCall)`.
- `quew-checker` infers provider expressions as `Ty::Provider(ProviderKind)`.
- `quew-ir` lowers model declarations from hardcoded `ProviderCall` data.
- `model Name = { model: gemini("...") }` is represented by a dedicated
  `ModelDecl` shape, not by a role-bound body contract.
- `reply(...) with { model: ... }` still validates `model` and `fallback`
  through specialized checker behavior.

Current prelude behavior is type-only:

- `prelude/tools.quew` defines `ToolResult<T>` for `(tool, value)`.
- `prelude/with.quew` defines `WithBlock` for `(with, body)`.
- No prelude function declarations are compiler-owned yet.
- No prelude model contract exists yet.

---

## Goals

By the end of Plan 10:

1. Builtin function metadata exists in the AST.
2. Public/internal builtin function prefixes parse for `function` declarations.
3. Prelude source can declare builtin provider-builder functions.
4. A public builtin `Model` type exists in prelude source.
5. A role-bound `(model, body)` type describes the shape of model declaration
   bodies.
6. The prelude loader includes a model/provider prelude file.
7. The checker can resolve `(model, body)` without hardcoding the bound type
   name.
8. `model Gemini = { model: gemini("gemini-pro") }` remains valid.
9. `gemini`, `openai`, and `groq` begin migrating toward ordinary builtin
   function calls returning `Model`.
10. Existing provider syntax remains source-compatible while the migration is in
    progress.
11. IR lowering preserves the provider/model metadata the runtime currently
    expects.
12. Tests prove the new builtin-function and model-body contract behavior.

---

## Non-Goals

Do not implement these in Plan 10:

- a separate `(provider, ...)` role system;
- string-based provider role keys;
- `#rust("id")` native builtin leaves;
- full removal of `Expr::Provider` if that makes the plan too large;
- runtime provider driver changes;
- SDK changes;
- generalized native function bodies;
- imports or user-defined modules;
- `extend Type { ... }`, implicit `self`, or method lookup;
- replacing all `with { model, fallback }` specialized validation;
- adding full collection/list/map types.

Plan 10 may keep compatibility shims. The objective is to establish builtin
function declarations and model-body contracts, not to finish the entire
provider cleanup.

---

## Builtin Function Syntax

Mirror builtin type declarations:

```quew
@@function name(arg: Type): ReturnType
!@@function internalName(arg: Type): ReturnType
```

Provider builders become ordinary public builtin functions in the prelude:

```quew
@@function gemini(name: string): Model
@@function openai(name: string): Model
@@function groq(name: string): Model
```

`gemini`, `openai`, and `groq` are currently keyword tokens. During migration,
the parser may need compatibility handling so those tokens can appear where a
function declaration name or call callee is expected. Long term, these names
should stop being hardcoded provider keywords and become ordinary identifiers.

Recommended metadata shape:

```rust
pub enum BuiltinFunctionMeta {
    User,
    Builtin {
        visibility: BuiltinVisibility,
    },
}
```

This can live beside `BuiltinTypeMeta` in `quew-ast::builtin`.

Do not add provider-role metadata to builtin functions in this plan. The return
type and model-body role are enough for this migration slice.

---

## Model Prelude Contract

Create a new prelude file, or a clearly named extension of the current prelude
set:

```text
quew-compiler/prelude/models.quew
```

Initial source should be close to Discussion 5:

```quew
@@type Model = {
    provider: string,
    name: string
}

@@type ModelConfig = {}

@@(model, body)
type ModelBody = {
    model: Model,
    config?: ModelConfig
}

@@function gemini(name: string): Model
@@function openai(name: string): Model
@@function groq(name: string): Model
```

Design notes:

- `Model` is the user-visible Quew type for model builder results.
- `ModelBody` is the compiler contract for `model Name = { ... }` bodies.
- `(model, body)` is the role the checker should consume.
- `ModelConfig = {}` is transitional until richer config typing exists.
- The contract can expand later with known config fields, unions, or richer
  record support.
- The compiler should not hardcode the name `ModelBody`; it should resolve the
  type bound to `(model, body)`.

---

## Role Registry Extension

The current role registry accepts role keywords such as `tool`, `with`, and
`middleware`, with places such as `value`, `args`, and `body`.

Plan 10 should extend role validation to accept:

```text
keyword: model
place: body
```

This keeps the existing role design from Discussion 5:

```quew
@@(model, body)
type ModelBody = { ... }
```

No provider role is needed.

Rules:

- one binding per `(model, body)`;
- duplicate `(model, body)` bindings are errors;
- checker code looks up `(model, body)` through the role registry;
- existing roles such as `(tool, value)` and `(with, body)` keep working.

---

## Model Declaration Validation

Today source looks like this:

```quew
model Gemini = { model: gemini("gemini-pro") }
```

Plan 10 should move validation toward the role-bound body type:

1. Resolve the type bound to `(model, body)`.
2. Treat the fields inside `model Name = { ... }` as a record-like body.
3. Validate expressible fields against the role-bound record.
4. Preserve specialized behavior where the current AST/IR still requires it.

Initial target behavior:

```quew
model Gemini = { model: gemini("gemini-pro") }
model OpenAI = { model: openai("gpt-4o") }
model Groq = { model: groq("llama-3") }
```

should all remain valid.

Invalid examples should produce checker diagnostics:

```quew
model Bad = { model: "gemini-pro" }
model BadConfig = { model: gemini("gemini-pro"), config: false }
```

If full `config` typing is too limited because `ModelConfig = {}` is
transitional, Plan 10 can keep config permissive while still validating the
`model` field through the role-bound contract.

---

## Provider Builder Migration Strategy

Use a compatibility-first migration.

### Phase 1: Builtin function declarations exist

- Parse `@@function` and `!@@function`.
- Add builtin metadata to `FunctionDecl`.
- Store builtin function visibility in scope metadata.
- Preserve function metadata in IR definitions if function definitions already
  lower there.
- Keep ordinary user functions unchanged.

### Phase 2: Model prelude loads

- Add `models.quew` to the prelude file list.
- Ensure `Model`, `ModelConfig`, `ModelBody`, `gemini`, `openai`, and `groq`
  are available in prelude-aware checking.
- Register `(model, body)` from prelude source.

### Phase 3: Provider builders type-check as Model

When the checker sees a call to a builtin model builder:

1. Validate arguments through the normal function-call path.
2. Infer the return type as the prelude-defined `Model` contract.
3. Allow that value in model declaration bodies and `with { model: ... }` fields
   where compatibility requires it.

During the transition, `Expr::Provider` may still infer to the current provider
semantic type. The checker can bridge that current type to `Model` in the
specific model-body validation path until the AST migration is complete.

### Phase 4: Parser compatibility

Provider keyword calls can remain as `Expr::Provider` during Plan 10 if needed.
But the migration should move toward one of these shapes:

- provider names stop lexing as provider keywords and become ordinary
  identifiers; or
- provider keyword tokens are accepted where function names/callees are expected
  during the compatibility period; or
- the parser desugars provider keyword calls into ordinary `Expr::Call` nodes.

Do not break existing Quew source.

### Phase 5: IR compatibility

IR lowering should continue to produce the same provider/model representation
that runtime code expects.

If provider builders become ordinary calls in the AST, IR lowering needs a
helper that recognizes calls to builtin model-builder functions and reconstructs
current provider metadata.

If that is too large for Plan 10, keep `Expr::Provider` as the parser/IR bridge
and focus the plan on builtin function declarations plus `(model, body)`
validation.

---

## Relationship To `with { model, fallback }`

Plan 9 deliberately kept `model`, `fallback`, and `tools` specialized in
`with { ... }` validation.

Plan 10 should not try to finish that migration. It should only make the new
`Model` contract available so future plans can migrate `with` more cleanly.

Acceptable Plan 10 behavior:

- `with { model: gemini("gemini-pro") }` remains valid;
- `with { fallback: groq("llama-3") }` remains valid;
- existing provider-specific diagnostics remain compatible;
- `WithBlock` may still intentionally exclude `model` and `fallback`.

A later plan can update `prelude/with.quew` to include `model?: Model` and
`fallback?: Model` once the model contract is stable.

---

## Tests

Testing should be layered and aggressive.

### Parser / AST Tests

- `@@function foo(): string` parses as a public builtin function.
- `!@@function foo(): string` parses as an internal builtin function.
- Ordinary user functions still parse as non-builtin functions.
- Existing function annotations such as `@tool` and `@desc` still parse.
- Provider keyword names are handled in builtin declarations during the
  transition, or provider keywords are intentionally removed from hardcoded
  tokenization with compatibility tests.

### Scope Tests

- Public builtin functions enter the symbol table.
- Internal builtin functions are tracked with internal visibility.
- Duplicate user/builtin function names produce diagnostics.
- `(model, body)` role bindings are registered.
- Duplicate `(model, body)` bindings produce diagnostics.
- Existing type role tests still pass.

### Prelude Tests

- Prelude loading includes `tools.quew`, `with.quew`, and `models.quew`.
- Model prelude parses without diagnostics.
- The symbol table contains `Model`, `ModelConfig`, `ModelBody`, `gemini`,
  `openai`, and `groq` from the prelude.
- Role registry contains `(model, body)`.

### Checker Tests

- `gemini("gemini-pro")` type-checks as `Model` in prelude-aware checking.
- `openai("gpt-4o")` type-checks as `Model`.
- `groq("llama-3")` type-checks as `Model`.
- `gemini(123)` reports an argument type mismatch when going through the
  builtin function path.
- `model Gemini = { model: gemini("gemini-pro") }` remains valid.
- `model Bad = { model: "gemini-pro" }` errors.
- `(model, body)` validation does not hardcode the name `ModelBody`.
- Prelude-free `check()` remains compatible with isolated tests.

### IR Tests

- Model declarations lower to the same provider metadata as before.
- Existing provider/model IR tests still pass.
- If builtin model-builder calls are lowered from ordinary calls, tests cover
  each builder.

### CLI / Workspace Tests

- CLI `check` sees model builders from the prelude.
- CLI `compile` sees model builders from the prelude.
- `cargo test --workspace` passes.

---

## Implementation Order

1. Add `BuiltinFunctionMeta` to the AST.
2. Add builtin metadata to `FunctionDecl`.
3. Parse `@@function` and `!@@function`.
4. Update existing AST/parser tests for the new field.
5. Extend symbol entries for builtin function visibility.
6. Extend role validation to accept `(model, body)`.
7. Add `prelude/models.quew`.
8. Load `models.quew` in the prelude loader.
9. Add role helper/tests for resolving `(model, body)`.
10. Validate model declaration bodies against the role-bound record where
    expressible.
11. Teach checker compatibility paths that provider builders produce or satisfy
    `Model`.
12. Keep or add compatibility handling for existing `Expr::Provider` paths.
13. Update IR lowering only as much as needed to preserve existing output.
14. Add parser, scope, prelude, checker, IR, and CLI tests.
15. Run `cargo test --workspace`.

---

## Definition Of Done

- [x] `BuiltinFunctionMeta` exists.
- [x] `FunctionDecl` stores builtin function metadata.
- [x] Public builtin functions parse.
- [x] Internal builtin functions parse.
- [x] Scope stores builtin function metadata.
- [x] `(model, body)` is an accepted role key.
- [x] `prelude/models.quew` exists.
- [x] Prelude loading includes `models.quew`.
- [x] `Model`, `ModelConfig`, and `ModelBody` are available from the prelude.
- [x] `gemini`, `openai`, and `groq` are declared as builtin functions in the
  prelude.
- [ ] Checker resolves role-bound `ModelBody` without hardcoding its name.
- [ ] Model declaration bodies are validated through `(model, body)` where
  expressible.
- [ ] Existing `model Gemini = { model: gemini("gemini-pro") }` behavior remains
  compatible.
- [ ] Model declarations lower to the same provider IR shape as before.
- [ ] `with { model, fallback }` specialized validation remains compatible.
- [ ] Prelude-free `check()` still supports isolated tests.
- [ ] Aggressive tests cover parser, scope, checker, prelude, and IR behavior.
- [ ] `cargo test --workspace` passes.

---

## Deferred After Plan 10

Plan 11 should introduce `#rust("id")` native builtin leaves so builtin
functions can point to compiler/runtime-native implementations explicitly.

Plan 12 should add `extend Type { ... }`, implicit `self`, and method lookup.

A later model/provider cleanup plan can remove the `Expr::Provider` compatibility
shim once provider builders are fully represented as ordinary builtin function
calls returning `Model`.
