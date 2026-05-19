# Plan 12: Extension Methods And Implicit Self

**Status: Complete.**

Plan 11 preserved native Rust binding metadata with `@@rust("id")`. Plan 12
turns that foundation into the first user-facing method layer:

```ts
extend string {
    function isEmpty(): bool {
        return string_is_empty(self)
    }
}
```

The goal is compiler support only. Runtime dispatch of native ids remains a
later runtime plan.

---

## Goals

1. Parse `extend Type { function ... }`.
2. Store extension declarations in the AST with receiver type and method
   functions.
3. Register extension methods in scope metadata keyed by receiver type and
   method name.
4. Inject implicit `self` while checking extension method bodies.
5. Resolve `value.method(args)` through extension method metadata.
6. Preserve extension method metadata in IR definitions.
7. Keep existing global function calls and record field access behavior
   compatible.

---

## Non-Goals

- Runtime execution or native dispatch.
- Import-aware extension visibility.
- Trait/interface constraints.
- Overload sets.
- Generic receiver specialization beyond the existing straightforward type
  substitution rules.
- Rewriting call expressions during lowering.

---

## First Slice

The first implementation should support:

- primitive receivers such as `string`;
- methods declared as ordinary Quew functions inside `extend`;
- implicit `self` inside the method body;
- zero or more explicit method parameters;
- method calls type-checking against declared method params and return type;
- IR preserving enough metadata to know that `isEmpty` is an extension method
  on `string`.

Example:

```ts
@@rust("std.string.is_empty")
!@@function string_is_empty(value: string): bool

extend string {
    function isEmpty(): bool {
        return string_is_empty(self)
    }
}

function check(value: string): bool {
    return value.isEmpty()
}
```

---

## Definition Of Done

- [x] `extend` lexes as a keyword.
- [x] `ExtendDecl` exists in `quew-ast`.
- [x] Parser accepts `extend Type { function ... }`.
- [x] Scope records extension methods by receiver type and method name.
- [x] Duplicate extension methods produce diagnostics.
- [x] Checker injects `self` in extension method bodies.
- [x] Checker resolves method calls through the extension method table.
- [x] Existing hardcoded `string.isEmpty()` behavior is removed or routed
      through the method table.
- [x] IR preserves extension method metadata.
- [x] Tests cover lexer, parser, scope, checker, and IR behavior.
- [x] `cargo test --workspace` passes.

---

## Deferred After Plan 12

After extension methods are stable, the next useful work is either:

1. runtime dispatch for native builtin leaves, or
2. the v2 graph executor and journal.

Imports should still wait until the single-file compiler/runtime contract is
more complete.
