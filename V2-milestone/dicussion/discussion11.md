# Discussion 11: String Interpolation

*Proposed as Plan 18 — the most common operation in prompt authoring.*

---

## 1. Current State

String literals in quew are opaque blobs:

```quew
let greeting = "hello"
```

The lexer stores the raw content (`hello`), the parser wraps it in `Expr::Lit(Lit::String)`, the checker treats it as `Ty::String`, and the lowerer emits `IrLit::String`.

There is **no way** to embed an expression inside a string. Every prompt that needs dynamic content must use explicit concatenation:

```quew
let prompt = "user name: " + userName + "\nage: " + age
```

This is verbose and error-prone. Every example in `not.txt` uses interpolation:

```quew
return "cleaned now it is : {pr}"

function systemPrompt(userName: string) {
    return """
        You are .......
        user name : {userName}
    """
}
```

## 2. The Gap

No interpolation syntax exists. The lexer, parser, AST, checker, and IR all treat strings as atomic literals.

## 3. Design Goals

1. **Natural syntax** — `"hello {name}"` and `"""multi\nline {expr}"""`
2. **No lexer changes** — keep the logos grammar simple; handle interpolation in the parser
3. **Type safety** — interpolated expressions must be string-coercible
4. **Clean AST** — preserve interpolation structure for diagnostics and future formatting
5. **Zero runtime changes** — lower to `IrExpr::Binary` Add chain (runtime already supports string concat)

## 4. Proposed Design

### 4.1 Syntax

```quew
"hello {name}"
"""You are {role}.
User: {userName}"""
"result: {if ok then "yes" else "no"}"
"nested { "braces {inside}" }"
```

Rules:
- `{expr}` introduces an interpolated expression
- To emit a literal `{`, double it: `{{`
- Expressions are full quew expressions (identifiers, calls, postfix-if, etc.)
- Nested braces inside the expression are balanced: `{obj.field {nested: 1}}` is valid

### 4.2 Lexer — No Changes

String literals remain single `TokenKind::StringLiteral` / `TokenKind::TripleString` tokens. The logos regexes do not need to change.

### 4.3 Parser — Post-lex Segment Extraction

When the parser encounters a string literal token, it inspects the raw source slice:

1. If the slice contains no `{`, return `Expr::Lit(Lit::String(...))` as before
2. If the slice contains `{`, scan the content:
   - Text segments → `InterpolatedSegment::Text`
   - `{expr}` segments → recursively lex+parse `expr` into an `Expr`
   - `{{` → literal `{` in the text segment

The parser already has access to:
- The source text (via `parse()` parameter)
- The token span (byte offsets)
- The interner
- The lexer (`quew-lexer` is a dependency)

So it can create mini sub-sources for each `{expr}` segment and call `lex()` + `parse_expr()` on them.

### 4.4 AST — New Node

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

### 4.5 Checker

For each `InterpolatedSegment::Expr`:
- Type-check the expression recursively
- Ensure the resulting type is `String` (or coercible — for now, restrict to `String`)

The overall interpolated string has type `String`.

### 4.6 IR Lowering

Lower `Expr::Interpolated` to a left-associative chain of `IrExpr::Binary` with `BinaryOp::Add`:

```rust
"hello {name} world"
// becomes:
(("hello" + name) + " world")
```

Each `InterpolatedSegment::Text` becomes `IrExpr::Lit(IrLit::String(...))`.
Each `InterpolatedSegment::Expr` is lowered via `lower_expr()` recursively.

### 4.7 Runtime

No changes. The existing `BinaryOp::Add` on strings handles concatenation.

## 5. Open Questions

1. **Non-string interpolation** — Should `number`, `bool`, etc. be auto-converted to string in interpolations? For now: no, user must explicit-convert. This keeps type safety strict.
2. **Triple-quoted strings** — Same interpolation rules apply. The `{` `}` scanner is content-agnostic.
3. **Escape sequences inside `{expr}`** — The sub-source is lexed independently, so `"` and `\` inside the expression follow normal expression lexing rules.

## 6. Summary

String interpolation is a pure compiler-frontend feature. It requires parser, AST, checker, and IR changes, but zero lexer or runtime changes. The lowering strategy reuses the existing string-concatenation path, making it a self-contained, high-impact addition.
