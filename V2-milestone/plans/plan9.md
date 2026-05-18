# Plan 9: Prelude-Backed `with` Body Contract

**Status: In Progress.**

Plan 9 moves `reply(...) with { ... }` validation toward the role-binding model
introduced in Plan 7 and proven by Plan 8.

The goal is not to remove every hardcoded `with` rule yet. Provider builders,
tool exposure, and richer collection types are still transitional. The goal is
to let the prelude define the parts of the `with` body shape that Quew can
express today, then have the checker consume the `(with, body)` role.

---

## Goals

1. Add `quew-compiler/prelude/with.quew`.
2. Bind a public builtin `WithBlock` type to `(with, body)`.
3. Teach the prelude loader to load more than one trusted Quew file.
4. Add checker role helpers for resolving role-bound types.
5. Validate expressible `with` fields through the role-bound record type.
6. Preserve current specialized validation for `model`, `fallback`, and `tools`.
7. Keep prelude-free `check()` compatible with isolated tests.

---

## Initial Prelude Contract

```quew
@@(with, body)
type WithBlock = {
    prompt?: string
    retry?: number
    maxTurn?: number
    builtin?: string
}
```

This contract intentionally excludes `model`, `fallback`, and `tools` for now.
Those require builtin provider functions and collection/tool-list types before
they can be represented cleanly in Quew source.

---

## Definition Of Done

- [ ] `prelude/with.quew` exists.
- [ ] Prelude loading includes `tools.quew` and `with.quew`.
- [ ] `(with, body)` role is registered from prelude source.
- [ ] Checker resolves role-bound `WithBlock` without hardcoding its name.
- [ ] `prompt`, `retry`, `maxTurn`, and `builtin` can be validated through the
  role-bound record.
- [ ] `model`, `fallback`, and `tools` keep existing behavior.
- [ ] Prelude-free `check()` still supports existing tests.
- [ ] Aggressive checker tests cover valid and invalid role-backed fields.
- [ ] `cargo test --workspace` passes.

---

## Deferred After Plan 9

Plan 10 should add builtin functions and begin provider migration.

Plan 11 should introduce `#rust("id")` native builtin leaves.

Plan 12 should add `extend Type { ... }`, implicit `self`, and method lookup.
