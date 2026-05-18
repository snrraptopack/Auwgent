# Plan 11: Native Rust Builtin Leaves

**Status: Complete.**

Plan 11 introduces explicit native implementation bindings for trusted builtin
functions:

```quew
@@rust("std.string.is_empty")
!@@function string_is_empty(value: string): bool
```

This is the next layer after Plan 10. Quew can now declare builtin function
signatures in prelude source; Plan 11 gives selected trusted signatures a stable
compiler/runtime-native implementation id without turning native execution into
a general user escape hatch.

---

## Starting Point

Plan 10 completed:

- builtin function metadata in the AST;
- `@@function` and `!@@function` parsing;
- builtin function visibility in scope metadata;
- model/provider prelude declarations;
- `(model, body)` model declaration validation through role lookup;
- provider compatibility with the existing parser and IR shape.

Current builtin functions are signatures only. There is no syntax or metadata
for binding a function declaration to a native Rust implementation id.

Discussion 5 defined the original concept as a native implementation marker.
Discussion 7 records the concrete syntax change from `#rust("id")` to
`@@rust("id")` so native leaves stay in the builtin annotation family.

```quew
@@rust("std.string.is_empty")
function string_is_empty(value: string): bool
```

The milestone-compatible form should preserve the current builtin visibility
syntax:

```quew
@@rust("std.string.is_empty")
!@@function string_is_empty(value: string): bool
```

---

## Goals

By the end of Plan 11:

1. The lexer recognizes `@@rust`.
2. The parser accepts `@@rust("id")` immediately before builtin function
   declarations.
3. Native binding metadata exists in the AST.
4. `FunctionDecl` can store an optional native binding id.
5. Scope records native binding metadata for builtin functions.
6. IR preserves native binding ids for function definitions if function metadata
   is already lowered there.
7. Trusted prelude source can declare internal native builtin leaves.
8. User functions remain ordinary Quew functions and cannot accidentally become
   native leaves.
9. Tests cover lexer, parser, scope, prelude, checker compatibility, and IR
   preservation.

---

## Non-Goals

Do not implement these in Plan 11:

- runtime execution or dispatch of native ids;
- general user-defined native plugins;
- `extend Type { ... }`;
- implicit `self`;
- method lookup;
- replacing the existing temporary `isEmpty` checker path;
- provider runtime changes;
- SDK changes;
- imports or module privacy rules.

This plan records and validates native binding metadata only.

---

## Syntax

Native binding syntax:

```quew
@@rust("native.id")
@@function publicName(arg: Type): ReturnType

@@rust("native.internal.id")
!@@function internalName(arg: Type): ReturnType
```

Initial parser rule:

- `@@rust("id")` is accepted only before `@@function` or `!@@function`;
- it is not accepted before ordinary `function`;
- it is not accepted before `type`, `agent`, `model`, `tool`, or `tools`;
- the id must be a string literal;
- duplicate `@@rust` prefixes on one declaration are parse errors.

Recommended AST shape:

```rust
pub struct NativeBinding {
    pub id: StringLit,
    pub span: Span,
}
```

Then:

```rust
pub struct FunctionDecl {
    pub native: Option<NativeBinding>,
    ...
}
```

If keeping native metadata inside `BuiltinFunctionMeta` is cleaner after code
inspection, that is acceptable, but it should not make ordinary user functions
look builtin.

---

## Trusted Source Rules

Plan 11 should be conservative:

- `@@rust` is intended for compiler-owned prelude source.
- The parser may accept it syntactically in user source for now, but the checker
  should reject `@@rust` on non-builtin declarations.
- If a user writes `@@rust("x") function foo(): string { ... }`, emit a clear
  diagnostic.
- If a user writes `@@rust("x") @@function foo(): string`, it may parse and enter
  scope, but future trusted-source restrictions can decide whether user source
  may declare public builtin signatures at all.

The immediate invariant is:

```text
native binding metadata belongs to builtin function declarations only
```

---

## Prelude Use

Add a small prelude file only if it helps keep the feature visible:

```text
quew-compiler/prelude/native.quew
```

Initial content can be minimal and internal:

```quew
@@rust("std.string.is_empty")
!@@function string_is_empty(value: string): bool

@@rust("std.string.contains")
!@@function string_contains(value: string, substring: string): bool
```

Do not wire these into method lookup yet. Plan 12 owns `extend` and methods.

If adding a new prelude file causes unnecessary churn, these declarations can
wait until the AST/scope/IR pipeline is proven by tests.

---

## IR Preservation

If `quew-ir` already lowers function definitions, add optional native metadata
there. The IR does not need to execute it yet.

The key requirement is that downstream runtime/compiler stages can later answer:

```text
Which native id backs this builtin function?
```

without reparsing source.

---

## Tests

Testing should be broad.

### Lexer Tests

- `@@rust` lexes as a dedicated token.
- `@@rust("std.string.is_empty")` produces the expected token sequence.
- `@@rusty` remains an identifier/error according to existing lexer rules and
  does not accidentally become `@@rust`.

### Parser / AST Tests

- `@@rust("id") @@function foo(): string` parses.
- `@@rust("id") !@@function foo(): string` parses.
- Native id is preserved on `FunctionDecl`.
- Ordinary builtin functions without `@@rust` still parse with `native = None`.
- Ordinary user functions still parse with `native = None`.
- `@@rust("id") function foo(): string { ... }` produces a useful parser or
  checker diagnostic.

### Scope Tests

- Native metadata is stored for public builtin functions.
- Native metadata is stored for internal builtin functions.
- User functions cannot carry native metadata silently.
- Duplicate function-name diagnostics still work.

### Prelude Tests

- Native prelude declarations parse without diagnostics.
- Internal native functions appear in the symbol table with internal builtin
  visibility.
- Native ids are available through scope or lowered IR metadata.

### IR Tests

- Builtin function native ids lower into IR metadata.
- Existing function/type/model/provider IR tests remain compatible.
- Functions without native ids lower unchanged.

### Workspace Tests

- `cargo test --workspace` passes.

---

## Implementation Order

1. Add lexer token support for `@@rust`.
2. Add `NativeBinding` AST metadata.
3. Add `native: Option<NativeBinding>` to `FunctionDecl`.
4. Update existing AST construction/tests for the new field.
5. Parse optional `@@rust("id")` before builtin function declarations.
6. Reject or diagnose `@@rust` on ordinary user functions.
7. Extend scope symbols to preserve native binding metadata.
8. Preserve native metadata in IR where function definitions lower.
9. Optionally add `prelude/native.quew`.
10. Add lexer, parser, scope, prelude, checker, and IR tests.
11. Run `cargo test --workspace`.

---

## Definition Of Done

- [x] `@@rust` lexes as a dedicated token.
- [x] Native binding metadata exists in the AST.
- [x] `FunctionDecl` stores optional native binding metadata.
- [x] `@@rust("id") @@function ...` parses.
- [x] `@@rust("id") !@@function ...` parses.
- [x] Ordinary functions remain non-native.
- [x] Native bindings are accepted only for builtin function declarations.
- [x] Scope preserves native binding metadata.
- [x] IR preserves native binding metadata where function definitions lower.
- [x] Existing builtin function tests remain compatible.
- [x] Prelude/native tests exist if a native prelude file is added.
- [x] `cargo test --workspace` passes.

---

## Deferred After Plan 11

Plan 12 should add `extend Type { ... }`, implicit `self`, and method lookup.
That plan can use the native leaves introduced here to expose user-facing
methods such as:

```quew
extend string {
    function isEmpty(): bool {
        string_is_empty(self)
    }
}
```
