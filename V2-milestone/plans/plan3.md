# Plan 3: AST Definitions and Parser

## Where We Are

`quew-lexer` is complete and locked. **73 tests, 0 failures.**

**What exists:**
- `quew-errors`, `quew-interner`, `quew-source` — tested foundations
- `quew-lexer` — `TokenKind`, `LexResult`, `lex()`, 4 valid fixtures, 2 invalid fixtures

**Next two crates:**
- `quew-ast` — pure data structures for every syntactic construct
- `quew-parser` — consumes `LexResult`, produces `ParseResult<Module>`

These are separate crates for a reason: `quew-ast` can be depended on by the checker, IR
lowerer, and codegen without pulling in parser logic. The parser is only needed by the
pipeline entry point.

---

## Goals of This Plan

By the end:

1. `quew-ast` defines every AST node with a `Span` on every struct/variant
2. `quew-parser` parses a flat token stream into a `Module` (a list of top-level `Item`s)
3. The parser recovers from errors rather than aborting on the first failure
4. `cargo test -p quew-ast` and `cargo test -p quew-parser` both pass with 0 failures

---

## crate: `quew-ast`

**Single responsibility:** own the data types. No parsing logic. No type info.

**Dependencies:** `quew-errors` (Span), `quew-interner` (InternedStr).

### Design rules

- **Every node carries a `Span`** — required for error messages and IDE tooling.
- **No semantic content** — no resolved types, no symbol IDs, no inferred kinds.
  That belongs in the checker. The AST is purely structural.
- **Named structs over tuples** — fields are named for readability and forward
  compatibility. `Expr::Call { callee, args, span }` not `Expr::Call(Box<Expr>, Vec<Expr>, Span)`.
- **`Box<T>` for recursive nodes** — `Expr` and `Stmt` are recursive; use `Box` to
  keep the enum size bounded.
- **`InternedStr` for all names** — zero-allocation handles from the shared interner.

### AST node catalog

#### Top-level: `Module` and `Item`

```rust
pub struct Module {
    pub items: Vec<Item>,
    pub span: Span,
}

pub enum Item {
    Agent(AgentDecl),
    Function(FunctionDecl),
    Tool(ToolDecl),
    Tools(ToolsDecl),       // shorthand group or progressive disclosure group
    Type(TypeDecl),
    Model(ModelDecl),
    Let(LetStmt),           // top-level let binding
}
```

#### Agent declaration

```rust
pub struct AgentDecl {
    pub annotations: Vec<Annotation>,
    pub name: InternedStr,
    pub param: Param,              // always exactly one — the `input` param
    pub return_ty: Option<TypeExpr>,
    pub body: Vec<Stmt>,
    pub span: Span,
}
```

#### Function declaration

```rust
pub struct FunctionDecl {
    pub annotations: Vec<Annotation>,
    pub name: InternedStr,
    pub params: Vec<Param>,
    pub return_ty: Option<TypeExpr>,
    pub body: Vec<Stmt>,
    pub span: Span,
}
```

#### Tool declarations

```rust
// Single host-backed tool: `tool name(params): ReturnType @desc "..."`
pub struct ToolDecl {
    pub name: InternedStr,
    pub params: Vec<Param>,
    pub return_ty: TypeExpr,
    pub desc: Option<StringLit>,
    pub span: Span,
}

// Tool group: `tools { ... }` or `tools name { ... } @desc "..."`
pub struct ToolsDecl {
    pub name: Option<InternedStr>,   // None = shorthand, Some = named progressive group
    pub entries: Vec<ToolEntry>,
    pub desc: Option<StringLit>,
    pub span: Span,
}

pub struct ToolEntry {
    pub name: InternedStr,
    pub params: Vec<Param>,
    pub return_ty: TypeExpr,
    pub desc: Option<StringLit>,
    pub span: Span,
}
```

#### Type and model declarations

```rust
pub struct TypeDecl {
    pub name: InternedStr,
    pub fields: Vec<FieldDef>,
    pub span: Span,
}

pub struct FieldDef {
    pub name: InternedStr,
    pub ty: TypeExpr,
    pub optional: bool,     // `name?: Type`
    pub span: Span,
}

pub struct ModelDecl {
    pub name: InternedStr,
    pub provider: ProviderCall,
    pub config: Vec<ConfigField>,
    pub span: Span,
}
```

#### Annotations

```rust
// Represents any @annotation on a declaration.
pub struct Annotation {
    pub kind: AnnotationKind,     // from quew-lexer
    pub args: AnnotationArgs,
    pub span: Span,
}

pub enum AnnotationArgs {
    None,
    Params(Vec<Param>),           // @tool(id: string)
    Type(TypeExpr),               // @context(Context)
    String(StringLit),            // @desc "..."
}
```

#### Parameters

```rust
pub struct Param {
    pub binding: ParamBinding,
    pub name: InternedStr,
    pub ty: TypeExpr,
    pub optional: bool,     // `name?: Type`
    pub span: Span,
}

pub enum ParamBinding {
    Normal,           // `name: Type`
    BoundRef,         // `@name: Type` — binds to @tool arg with the same name
}
```

#### Statements

```rust
pub enum Stmt {
    Let(LetStmt),
    If(IfStmt),
    Return(ReturnStmt),
    Reply(ReplyStmt),
    For(ForStmt),
    Expr(ExprStmt),     // expression used as statement (e.g. a bare call)
}

pub struct LetStmt {
    pub name: InternedStr,
    pub ty: Option<TypeExpr>,   // optional explicit type annotation
    pub init: Expr,
    pub span: Span,
}

pub struct IfStmt {
    pub condition: Expr,
    pub then_body: Vec<Stmt>,
    pub else_body: ElseClause,
    pub span: Span,
}

pub enum ElseClause {
    None,
    Else(Vec<Stmt>),
    ElseIf(Box<IfStmt>),
}

pub struct ReturnStmt {
    pub value: Option<Expr>,
    pub span: Span,
}

pub struct ReplyStmt {
    pub input: Expr,
    pub with_block: WithBlock,
    pub span: Span,
}

pub struct WithBlock {
    pub fields: Vec<WithField>,
    pub span: Span,
}

pub struct WithField {
    pub key: InternedStr,
    pub value: Expr,
    pub span: Span,
}

pub struct ForStmt {
    pub index: Option<InternedStr>,   // `idx` in `for idx, value in`
    pub value: InternedStr,
    pub iterable: Expr,
    pub body: Vec<Stmt>,
    pub span: Span,
}

pub struct ExprStmt {
    pub expr: Expr,
    pub span: Span,
}
```

#### Expressions

```rust
pub enum Expr {
    Lit(Lit),
    Ident(IdentExpr),
    Binary(BinaryExpr),
    Unary(UnaryExpr),
    Call(CallExpr),
    Provider(ProviderCall),      // gemini("model"), openai(...), groq(...)
    Member(MemberExpr),          // a.b
    Index(IndexExpr),            // a[i]
    Array(ArrayExpr),            // [a, b, c]
    PostfixIf(PostfixIfExpr),    // expr if cond else expr
    Is(IsExpr),                  // expr is Type
}

pub struct BinaryExpr {
    pub left: Box<Expr>,
    pub op: BinaryOp,
    pub right: Box<Expr>,
    pub span: Span,
}

pub enum BinaryOp {
    Add, Sub, Mul, Div, Mod,
    Eq, NotEq,
    And, Or,
    Assign,   // `=`
}

pub struct UnaryExpr {
    pub op: UnaryOp,
    pub operand: Box<Expr>,
    pub span: Span,
}

pub enum UnaryOp { Not }

pub struct CallExpr {
    pub callee: Box<Expr>,
    pub args: Vec<Expr>,
    pub span: Span,
}

pub struct ProviderCall {
    pub provider: Provider,
    pub model_name: StringLit,
    pub config: Vec<ConfigField>,   // optional second arg `{ ... }`
    pub span: Span,
}

pub enum Provider { Gemini, OpenAi, Groq }

pub struct MemberExpr {
    pub object: Box<Expr>,
    pub field: InternedStr,
    pub span: Span,
}

pub struct PostfixIfExpr {
    pub value: Box<Expr>,
    pub condition: Box<Expr>,
    pub else_value: Box<Expr>,
    pub span: Span,
}

pub struct IsExpr {
    pub value: Box<Expr>,
    pub ty: TypeExpr,
    pub span: Span,
}
```

#### Literals

```rust
pub enum Lit {
    Int(i64, Span),
    Float(f64, Span),
    String(StringLit),
    Bool(bool, Span),
    Null(Span),
}

pub struct StringLit {
    pub value: InternedStr,   // interned content (without quotes)
    pub kind: StringKind,
    pub span: Span,
}

pub enum StringKind { Regular, Triple }
```

#### Types

```rust
pub enum TypeExpr {
    Named(InternedStr, Span),         // `string`, `bool`, user-defined `MyType`
    Union(Vec<TypeExpr>, Span),       // `A | B | C`
    Optional(Box<TypeExpr>, Span),    // `Type?`
    Generic(InternedStr, Vec<TypeExpr>, Span), // `Type<A, B>` — future use
}
```

---

## crate: `quew-parser`

**Single responsibility:** consume `LexResult`, produce `ParseResult<Module>`.

**Dependencies:** `quew-lexer`, `quew-ast`, `quew-errors`, `quew-interner`, `quew-source`.

### Parser library: chumsky 0.13 with `pratt` feature

chumsky 0.13 is already pinned in the workspace. Key things to know about the 0.13 API
(completely different from 0.9):

- Parsers are generic over `Input` — our input is `&[(TokenKind, Span)]`
- `just(token)` matches a single token; `select!` maps token variants to values
- `.pratt(ops)` (from the `pratt` feature) builds precedence-climbing expression parsers
- Error recovery uses `.recover_with(skip_then_any_output(...))` and `nested_delimiters`
- `choice((a, b, c))` tries alternatives left-to-right
- `parse_field_name` — after `.`, use `select!` that matches `Ident` OR any keyword token
  and interns the raw slice from the source — keywords are valid field names in member
  position (`config.model`, `response.is`, etc.)


### `ParseResult` shape

```rust
pub struct ParseResult {
    pub module: Module,
    pub errors: Vec<Diagnostic>,
}
```

The parser never aborts. When it fails to parse a construct, it emits a `Diagnostic`,
skips to a known recovery point (next top-level keyword or `}`), and continues.

### Parser structure (modular files)

```
quew-parser/src/
  lib.rs          ← public ParseResult, parse() — drives chumsky on &[(TokenKind, Span)]
  common.rs       ← shared combinators: ident(), field_name(), string_lit(), skip_newlines()
  parse_type.rs   ← type expression combinators
  parse_annot.rs  ← annotation combinators (@tool, @desc, @context, etc.)
  parse_param.rs  ← parameter list combinators (including @binding params)
  parse_expr.rs   ← expression parser using .pratt() for operator precedence
  parse_stmt.rs   ← statement combinators (let, if, return, reply, for, expr)
  parse_item.rs   ← top-level item combinators (agent, function, tool, tools, type, model)
```

### `parse_field_name()` — keywords as field names

After a `.` in a member-access expression, the parser must accept **any keyword token**
as a valid field name, not just `Ident`. Examples that must parse without error:

```text
config.model      ← model is KwModel
response.is       ← is    is KwIs
result.not        ← not   is KwNot
obj.for           ← for   is KwFor
```

`parse_field_name()` in `cursor.rs` accepts `TokenKind::Ident` OR any keyword/provider
token and interns the raw source slice. The AST always stores `InternedStr` — the
distinction is invisible to any downstream crate.

### `parse()` entry point

```rust
pub fn parse(
    result: &LexResult,
    source: &str,
    source_id: SourceId,
    interner: &Arc<Interner>,
) -> ParseResult
```

Takes the full `LexResult` (never re-lexes). Returns a `ParseResult` with the complete
module even if there were errors.

### Error recovery strategy

| Situation | Recovery |
|-----------|----------|
| Unexpected token at top level | Skip until next `agent`/`function`/`tool`/`type`/`model` |
| Unexpected token in statement | Skip until `}` or newline, emit error |
| Malformed expression | Return an `Expr::Error` sentinel, continue |
| Unclosed `{` or `(` | Emit error, treat as closed, resume |

---

## Testing Mandate

### `quew-ast`
- Every public struct and enum variant must have at least one construction test
- `Span` must be present and non-zero-length on constructed nodes
- No parse logic — tests construct nodes manually and verify field access

### `quew-parser`
- Every top-level `Item` variant must be produced by at least one test
- Every `Stmt` variant must be produced
- Every `Expr` variant must be produced  
- Error recovery: parser must continue after a bad token and return remaining items
- Fixture-based integration tests reusing `.quew` files from `quew-lexer`
- Edge cases: empty file, file with only comments, duplicate `let` name (parses fine,
  checker rejects), annotation with no following declaration

---

## What We Are NOT Doing in This Plan

- Name resolution (plan 4 — `quew-scope` and `quew-resolve`)
- Type inference (plan 5 — `quew-checker` + `quew-unify`)
- IR lowering (plan 6 — `quew-ir`)
- Codegen (plan 7 — `quew-codegen`)

---

## Definition of Done

- [x] All AST nodes defined in `quew-ast/src/` with `Span` on every struct
- [x] `quew-ast` has unit tests for every public node type
- [x] `cargo test -p quew-ast` → 0 failures — **46 tests, 0 failures**
- [x] Shared chumsky 0.13 combinators in `common.rs` (`ident()`, `field_name()`, `type_name()`, `string_lit()`)
- [x] All top-level `Item` variants parsed (agent, function, tool, tools, type, model, let)
- [x] All `Stmt` variants parsed (let, if, return, reply, for, expr)
- [x] All `Expr` variants parsed (including postfix-if and `is`) using `.pratt()`
- [x] Error recovery tested — parser does not abort on bad input
- [x] `cargo test -p quew-parser` → 0 failures — **24 tests, 0 failures**

---

## ✅ PLAN 3 COMPLETE
