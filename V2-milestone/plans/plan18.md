# Plan 18: String Interpolation

**Status:** ✅ Completed. All acceptance criteria met.

**Scope:** Add `"hello {name}"` syntax to the quew language. No lexer or runtime changes — handled entirely in parser, AST, checker, and IR lowerer.

---

## Where We Are

Plan 17 completed the `#[quew_builtin]` proc-macro and `quew-stdlib`. The compiler can now parse, check, lower, and execute pure computation graphs including function calls and extension methods.

String literals are opaque — there is no way to embed expressions inside them.

## Goals

1. Parse `"hello {name}"` into `Expr::Interpolated` with text and expression segments.
2. Type-check each interpolated expression segment.
3. Lower interpolated strings to `IrExpr::Binary` Add chains.
4. Support both regular `"..."` and triple-quoted `"""..."""` strings.
5. Support escaped braces: `{{` → literal `{`.

## Non-Goals

- Auto-conversion of non-string types (number, bool) inside interpolations — user must cast
- Format specifiers (`{value:.2}`) — deferred
- Multiline expression segments spanning lines inside `"""` — expressions must be single-line for now

## Architecture

### AST Changes (`quew-ast`)

```rust
pub enum Expr {
    // ... existing variants
    Interpolated(InterpolatedString),
}

pub struct InterpolatedString {
    pub segments: Vec<InterpolatedSegment>,
    pub span: Span,
}

pub enum InterpolatedSegment {
    Text(String),
    Expr(Box<Expr>),
}
```

### Parser Changes (`quew-parser`)

When parsing `Lit::String`, inspect the raw source slice:
- No `{` → return `Expr::Lit(Lit::String(...))` as before
- Contains `{` → scan segments, recursively lex+parse each `{expr}`

The parser uses `lex()` + `parse_expr()` on sub-slices for each expression segment.

### Checker Changes (`quew-checker`)

- Recursively check each `InterpolatedSegment::Expr`
- Assert type is `String` (strict, no auto-conversion)
- Overall expression type = `String`

### IR Changes (`quew-ir`)

Lower `Expr::Interpolated` to left-associative `BinaryOp::Add` chain:
```rust
"a {x} b"  →  (("a" + x) + "b")
```

## Implementation Steps

### Step 1: AST (`quew-ast`)

- Add `Expr::Interpolated` variant
- Add `InterpolatedString` and `InterpolatedSegment` structs
- Update `Expr::span()`

### Step 2: Parser (`quew-parser`)

- Add `parse_interpolated_string()` helper
- Scan raw string content for `{` / `}` pairs
- Handle `{{` escape → literal `{`
- For each `{expr}`, call `lex()` + `parse_expr()` on the sub-slice
- Return `Expr::Interpolated` or `Expr::Lit` depending on content

### Step 3: Checker (`quew-checker`)

- Add `check_interpolated()` helper
- Recursively check each segment
- Emit error if segment type ≠ `String`

### Step 4: IR Lowerer (`quew-ir`)

- Add `lower_interpolated()` helper
- Build left-associative `IrExpr::Binary` chain
- Text segments → `IrExpr::Lit(IrLit::String)`
- Expr segments → `lower_expr()` recursively

### Step 5: Tests

- Parser: `"hello {name}"`, `"""{a} and {b}"""`, `"{{literal brace}}"`
- Checker: valid string interpolation, invalid non-string segment
- IR: lowering produces correct Add chain
- Runtime: execute interpolated expression through graph

## Acceptance Criteria

- [x] `"hello {name}"` parses into `Expr::Interpolated`
- [x] Triple-quoted strings support interpolation
- [x] `{{` escape produces literal `{`
- [x] Checker rejects non-string expressions in interpolation
- [x] IR lowers to `BinaryOp::Add` chain
- [x] Runtime executes interpolated strings correctly
- [x] All existing tests pass
