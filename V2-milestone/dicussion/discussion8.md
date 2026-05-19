# Discussion 8: The Checker to IR Type Information Gap

*Identified during a codebase review following Plan 12.*

---

## The Issue

Plan 12 introduced extension methods (`extend Type { function ... }`) and implicit `self`. The compiler frontend successfully handles this:
- `quew-ast` parses the syntax.
- `quew-scope` collects the extension methods into `table.extension_methods`.
- `quew-checker` correctly type-checks method calls like `value.isEmpty()` by inferring the receiver type and resolving it against the registered extension methods.

However, the compiler panics during the IR lowering phase (`quew-ir`) when it encounters an extension method call. Specifically, `quew-ir/src/lower/expr.rs` expects the callee of an `Expr::Call` to be an identifier. When it receives an `Expr::Member` (which is what `value.isEmpty()` is represented as), it panics with:
`"lowering bug: non-identifier call callee in pure expression"`.

### Reproduction

To verify this, a valid test file has been created at `quew-compiler/test.quew` which uses an extension method call. 
*(Note: Please delete `quew-compiler/test.quew` once this bug is resolved).*

Running `cargo run --bin quew -- compile test.quew` produces the following panic trace:

```text
thread 'main' panicked at crates\quew-ir\src\lower\expr.rs:37:22:
lowering bug: non-identifier call callee in pure expression
error: process didn't exit successfully: `target\debug\quew.exe compile test.quew` (exit code: 101)
```

## The Architectural Gap

The root cause of this panic is a data loss between the Checker and the IR Lowerer. 

In `quew-checker/src/lib.rs`, type inference is performed using a `UnifyTable`. When the checker finishes, it returns a `CheckResult`:
```rust
pub struct CheckResult {
    pub symbol_table: SymbolTable,
    pub diagnostics: Vec<Diagnostic>,
}
```
**All type information calculated for expressions is discarded.** 

When `quew-ir` traverses the AST to build the `QuewGraphIR`, it operates blindly without type context. Because it does not know the type of `value`, it cannot determine if `value.isEmpty()` is invoking `string.isEmpty`, `array.isEmpty`, or any other extension method. Without this resolution, the lowerer cannot construct the correct `NodeKind::FuncCall` in the graph.

## Additional Findings: Missing Graph Lowering

During investigation, two deeper gaps were discovered that compound the extension method problem.

### Finding 1: Function bodies are not lowered into graphs

`lower_function()` in `quew-ir/src/lower/defs.rs` accepts a `_graphs` parameter but **ignores it**:

```rust
fn lower_function(
    decl: &FunctionDecl,
    interner: &Arc<Interner>,
    defs: &mut Definitions,
    _graphs: &mut IndexMap<String, AgentGraph>,  // ← unused
) {
    let graph_ref = format!("function:{}", interner.resolve(decl.name));
    // ... inserts into defs.functions or defs.tools
    // Never emits a graph.
}
```

The `graph_ref` string (e.g. `"function:custom_string_is_empty"`) is stored in `FunctionDef`, but no corresponding graph exists in `QuewGraphIR.graphs`.

### Finding 2: Extension method bodies are not lowered into graphs

`lower_extend()` in the same file does not even accept a `graphs` parameter:

```rust
fn lower_extend(
    decl: &quew_ast::ExtendDecl,
    interner: &Arc<Interner>,
    defs: &mut Definitions,  // ← no graphs output
) {
    // ... pushes ExtensionDef entries
    // Never emits a graph.
}
```

Each `ExtensionDef` carries a `graph_ref` like `"extension:string:isSuperEmpty"`, but again no graph is produced.

### Finding 3: Only `agent` bodies get graphs

`lower.rs` only creates graphs for `Item::Agent`:

```rust
for item in &module.items {
    if let quew_ast::Item::Agent(agent) = item {
        let graph = graph_lower::lower_agent(agent, check, interner, &mut definitions);
        graphs.insert(graph_key, graph);
    }
}
```

There is no loop for `Item::Function` or `Item::Extend`.

### Implication

Even if the `lower_expr` panic is fixed (e.g. by teaching it to resolve extension methods), an extension method call like `input.isSuperEmpty()` would have **no graph to call into**. The `FuncCall` node would reference `"extension:string:isSuperEmpty"`, but `QuewGraphIR.graphs` contains no entry with that key.

This means the fix requires **two** changes:
1. Bridge checker resolution to the lowerer (so `lower_expr` knows what to emit)
2. Lower function and extension method bodies into graphs (so there is something to call)

## Proposed Solutions

To fix this drift and allow `quew-ir` to lower extension method calls, we must bridge the gap between `quew-checker` and `quew-ir`. 

Here are three potential approaches:

### 1. Type Map Export (Sidecar Map)
Modify `CheckResult` to export a mapping of expressions to their resolved types (or resolved method identities).
- **How it works:** The checker populates a `HashMap<Span, Ty>` (or a map of AST node IDs) during inference. `CheckResult` carries this map to `quew-ir`.
- **Pros:** Keeps the AST immutable. Standard pattern in many compilers.
- **Cons:** Requires `quew-ir` to re-do the method lookup logic (matching the receiver type to the extension method) or requires the map to explicitly store "Resolved Method Name" for member expressions.

### 2. AST Rewriting (Desugaring in the Checker)
Have the checker rewrite the AST before passing it to the lowerer.
- **How it works:** When `quew-checker` successfully resolves `value.isEmpty()` as the `string_is_empty` extension method, it rewrites the `Expr::Member` call into a standard `Expr::Ident` function call: `string_is_empty(value)`.
- **Pros:** `quew-ir` remains completely unaware of extension methods. It just sees standard function calls, which it already knows how to lower perfectly.
- **Cons:** Mutating the AST after parsing violates the current immutable AST design. We would need to introduce a mutable pass or a lowered AST representation.

### 3. Annotated AST (Typed AST)
Introduce type annotations directly into the AST nodes.
- **How it works:** Every `Expr` node gets a `Cell<Option<Ty>>` or a similar interior mutability wrapper that the checker fills in.
- **Pros:** `quew-ir` has direct access to the type of any node it visits.
- **Cons:** Pollutes the clean AST structs with runtime resolution data and interior mutability.

---

## Decision Required
Before we can consider Plan 12 truly complete and move on to the V2 Graph Executor, we must decide on how to preserve the checker's resolution data for the IR lowerer. 
