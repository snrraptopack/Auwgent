# Plan 7: Builtin Declarations and Role Bindings

**Status: Complete.**

Plan 7 follows Plan 6's generic type-system work. The goal is to give Quew a
compiler-recognized way to declare builtin types and bind those types to
language roles without hardcoding magic names such as `ToolResult`,
`WithBlock`, or `MiddlewareEvent`.

This plan is intentionally narrow. It does not implement extensions, native
Rust bindings, provider migration, or runtime execution.

---

## Codebase Starting Point

The current `quew-compiler` already has the right foundations:

- `quew-ast::TypeDecl` has `type_params`.
- `quew-ast::FunctionDecl` has `type_params`.
- `quew-types` supports `GenericParam` and `GenericInstance`.
- `quew-scope::Symbol` stores `kind`, `ty`, `def_span`, and `type_params`.
- `quew-checker` resolves generic types and checks instantiated field access.
- `quew-ir` preserves generic type and function definitions.

But the builtin-contract layer does not exist yet:

- `quew-lexer` has no `@@`, `!@@`, or role-binding tokens.
- `quew-ast::Item` has no builtin declaration metadata.
- `quew-scope::Symbol` has no builtin visibility.
- There is no role registry for `(keyword, place)` bindings.
- `reply(...) with { ... }` is still checked through hardcoded
  `WellKnownKeys`.
- `Text` is still treated as a primitive alias for `string` in `PrimKeys`.
- Providers are still hardcoded as `gemini`, `openai`, and `groq` parser forms.

Plan 7 should add the new contract infrastructure while preserving existing
behavior.

---

## Goal

By the end of this plan, Quew should parse, scope, check, and lower builtin
type declarations and compiler role bindings.

The first target syntax:

```quew
@@type Text = {
    value: string
}

@@(tool, value)
type ToolResult<T> = {
    data: T,
    error: string
}

@@(with, body)
type WithBlock = {
    model: Model,
    prompt: string
}
```

The compiler should understand:

- `Text` is a public builtin type;
- `ToolResult<T>` is public and bound to the `(tool, value)` role;
- `WithBlock` is public and bound to the `(with, body)` role;
- duplicate role bindings are errors;
- unknown role keywords or places are errors;
- generic builtin types reuse the existing Plan 6 generic machinery.

---

## Non-Goals

Do not implement these in Plan 7:

- `@@function`;
- `!@@function`;
- `#rust("id")`;
- `extend Type { ... }`;
- implicit `self`;
- method lookup;
- provider migration from hardcoded parser forms to builtin functions;
- replacing `Text` in `PrimKeys`;
- replacing all `with` validation with role-bound structural checking;
- graph executor or runtime journal work;
- TypeScript/Python/Dart SDK codegen.

The first version can keep the current hardcoded checker paths as a fallback.

---

## Syntax Scope

### Public Builtin Type

```quew
@@type Name<T> = {
    field: T
}
```

Equivalent to an ordinary `type`, but marked as globally available builtin
surface.

### Internal Builtin Type

```quew
!@@type InternalName = {
    value: string
}
```

Internal builtin types are visible to trusted prelude/compiler source but should
not be exposed as normal user-facing names once prelude loading exists.

For Plan 7, parsing and metadata are enough. Full user/prelude visibility
enforcement can be minimal until there is an actual prelude loader.

### Role-Bound Type

```quew
@@(tool, value)
type ToolResult<T> = {
    data: T,
    error: string
}
```

This is a type declaration plus a role binding.

Recommended supported role keys for Plan 7:

| Keyword | Place | Purpose |
|---|---|---|
| `tool` | `value` | Tool call result wrapper |
| `with` | `body` | `with { ... }` block contract |
| `middleware` | `args` | Middleware function argument contract |

The role registry should be generic enough to add more later, but tests should
cover only these initial keys.

---

## AST Design

Reuse `TypeDecl` instead of creating a parallel builtin type AST.

Add metadata to `TypeDecl`:

```rust
pub struct TypeDecl {
    pub name: InternedStr,
    pub type_params: Vec<InternedStr>,
    pub fields: Vec<FieldDef>,
    pub builtin: BuiltinTypeMeta,
    pub span: Span,
}
```

Suggested metadata:

```rust
pub enum BuiltinTypeMeta {
    User,
    Builtin {
        visibility: BuiltinVisibility,
        role: Option<RoleBindingSyntax>,
    },
}

pub enum BuiltinVisibility {
    Public,
    Internal,
}

pub struct RoleBindingSyntax {
    pub keyword: InternedStr,
    pub place: InternedStr,
    pub span: Span,
}
```

This keeps downstream handling simple:

- ordinary `type` remains `BuiltinTypeMeta::User`;
- `@@type` becomes public builtin with no role;
- `!@@type` becomes internal builtin with no role;
- `@@(tool, value) type` becomes public builtin with a role.

All existing tests constructing `TypeDecl` must be updated to set
`BuiltinTypeMeta::User`.

---

## Lexer Work

Add tokens for the new prefix forms.

Recommended minimal token set:

```rust
AtAt       // @@
BangAtAt   // !@@
```

Do not add a broad `#rust` token yet. Native binding is out of scope.

Important lexer ordering:

- `!@@` must beat `!` / error fallback;
- `@@` must beat the existing `Annotation(AnnotationKind)` regex;
- existing `@tool`, `@desc`, etc. must continue to lex exactly as they do now.

Add lexer tests for:

- `@@type`;
- `!@@type`;
- `@@(tool, value) type`;
- existing `@tool` still works;
- unknown single `@bad` still becomes `Annotation(Unknown)`.

---

## Parser Work

Update `type_decl` parsing in `quew-parser/src/parse_item.rs`.

The parser should accept:

```quew
type Box<T> = { value: T }
@@type Text = { value: string }
!@@type Internal = { value: string }
@@(tool, value) type ToolResult<T> = { data: T, error: string }
```

Recommended parser shape:

1. Parse an optional builtin prefix before `type`.
2. Reuse the existing type name, type params, `=`, and field parser.
3. Populate `TypeDecl::builtin`.

The role-binding prefix grammar:

```text
@@ ( ident_or_keyword , ident_or_keyword ) type ...
```

Use the existing `field_name()` style helper for role names so keywords such as
`tool`, `with`, and `middleware` can be accepted even if they are reserved
tokens.

Parser tests:

- public builtin type parses;
- internal builtin type parses;
- role-bound generic type parses;
- ordinary type still parses as user type;
- malformed role binding recovers without panicking.

---

## Scope Work

Extend `quew-scope`.

Suggested additions:

```rust
pub enum BuiltinVisibility {
    User,
    PublicBuiltin,
    InternalBuiltin,
}

pub struct Symbol {
    pub ty: Ty,
    pub kind: SymbolKind,
    pub def_span: Span,
    pub type_params: Vec<InternedStr>,
    pub visibility: BuiltinVisibility,
}
```

Add a role registry:

```rust
pub struct RoleRegistry {
    pub bindings: IndexMap<RoleKey, RoleBinding>,
}

pub struct RoleKey {
    pub keyword: InternedStr,
    pub place: InternedStr,
}

pub struct RoleBinding {
    pub type_name: InternedStr,
    pub span: Span,
}
```

Add it to `SymbolTable`:

```rust
pub struct SymbolTable {
    pub globals: IndexMap<InternedStr, Symbol>,
    pub roles: RoleRegistry,
    pub diagnostics: Vec<Diagnostic>,
}
```

Validation rules:

- duplicate global names remain errors;
- duplicate role binding for the same `(keyword, place)` is an error;
- unknown role keyword is an error;
- unknown role place is an error;
- role-bound type must still register as a normal type symbol;
- generic role-bound types preserve `type_params`.

Supported role keywords:

```text
tool
with
middleware
```

Supported role places:

```text
value
args
body
```

Plan 7 should validate the role key, but it should not yet enforce the full
semantic meaning of every role.

---

## Checker Work

The checker should read the role registry but keep existing behavior stable.

Minimum work:

1. Include `roles` in `CheckResult` through the existing `symbol_table`.
2. Add tests proving role-bound generic types resolve normally.
3. Add tests proving duplicate/unknown role bindings appear as checker
   diagnostics because `check()` already includes scope diagnostics.

Optional, if small:

4. In `check_with_block`, look up `(with, body)` and keep the result available
   for future structural validation.

Do not remove the current hardcoded `with` field validation in this plan.

Do not wrap tool call types in `(tool, value)` yet. That will change type
behavior and should be a separate plan once the role registry is proven.

---

## IR Work

Extend `quew-ir::Definitions` to preserve builtin metadata and role bindings.

Suggested changes:

```rust
pub struct Definitions {
    pub types: IndexMap<InternedStr, TypeDef>,
    pub models: IndexMap<InternedStr, ModelDef>,
    pub tools: IndexMap<InternedStr, ToolDef>,
    pub functions: IndexMap<InternedStr, FunctionDef>,
    pub agents: IndexMap<InternedStr, AgentDef>,
    pub roles: IndexMap<IrRoleKey, IrRoleBinding>,
}
```

Add metadata to `TypeDef`:

```rust
pub struct TypeDef {
    pub type_params: Vec<InternedStr>,
    pub fields: IndexMap<InternedStr, IrField>,
    pub visibility: IrTypeVisibility,
}
```

IR tests:

- `@@type Text` lowers as public builtin type;
- `!@@type Internal` lowers as internal builtin type;
- `@@(tool, value) type ToolResult<T>` lowers the role binding;
- generic params and generic fields are still preserved.

---

## Prelude Loading

Prelude loading is allowed only if it stays small.

Preferred Plan 7 endpoint:

- support syntax and role registry in ordinary source first;
- then add a tiny embedded prelude only if the parser/checker path is stable.

Tiny prelude candidate:

```quew
@@(tool, value)
type ToolResult<T> = {
    data: T,
    error: string
}
```

Do not move `Text` into prelude yet unless the `PrimKeys` behavior is addressed
with focused tests.

---

## Testing Mandate

Plan 7 needs broad but focused tests.

### `quew-lexer`

- `@@type` tokenizes correctly.
- `!@@type` tokenizes correctly.
- `@@(tool, value) type` tokenizes correctly.
- Existing `@tool` annotations still tokenize correctly.

### `quew-ast`

- `TypeDecl` can represent user, public builtin, internal builtin, and
  role-bound builtin types.

### `quew-parser`

- ordinary type declaration still parses;
- public builtin type parses;
- internal builtin type parses;
- role-bound generic type parses;
- malformed role binding recovers.

### `quew-scope`

- builtin type registers as `SymbolKind::Type`;
- builtin visibility is preserved;
- role binding registry records `(tool, value)`;
- duplicate role binding errors;
- unknown role keyword errors;
- unknown role place errors;
- role-bound generic type preserves `type_params`.

### `quew-checker`

- role-bound generic type resolves;
- duplicate/unknown role diagnostics flow through `check()`;
- existing checker tests still pass unchanged in behavior.

### `quew-ir`

- builtin visibility lowers into `TypeDef`;
- role bindings lower into `Definitions`;
- generic builtin type definitions preserve params and fields.

---

## Definition of Done

- [x] Lexer supports `@@` and `!@@`.
- [x] Parser supports `@@type`, `!@@type`, and `@@(keyword, place) type`.
- [x] AST records builtin type metadata.
- [x] Scope records builtin visibility.
- [x] Scope builds a role registry.
- [x] Duplicate role bindings produce diagnostics.
- [x] Unknown role keywords/places produce diagnostics.
- [x] Checker receives role diagnostics through `check()`.
- [x] IR preserves builtin type metadata.
- [x] IR preserves role bindings.
- [x] `cargo test --workspace` passes.

---

## Deferred Follow-Up Plans

After Plan 7, separate plans can tackle:

- Plan 8: use `(tool, value)` to wrap host and DSL tool call return types;
- Plan 9: move `with { ... }` structural checking to `(with, body)`;
- Plan 10: builtin functions and provider builders;
- Plan 11: `#rust("id")` native builtin leaves;
- Plan 12: `extend Type { ... }`, implicit `self`, and method lookup.
