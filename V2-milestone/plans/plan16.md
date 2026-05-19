# Plan 16: FuncCall + NativeRegistry

**Status:** Completed. ✅

**Scope:** Execute `FuncCall` nodes by looking up function/extension graphs and recursing into them. Add `NativeRegistry` for dispatching `@@rust` builtin functions.

---

## Where We Are

Plan 15 built the deterministic executor:
- `Value` enum with all operations
- `eval_expr` evaluates pure expressions
- `Execution::run` walks graphs and handles `Input`, `Context`, `Output`, `LetBind`, `Branch`

**What works:** Pure computation graphs execute end-to-end.

**What is missing:** `FuncCall` nodes currently trigger `ExecutionError::UnsupportedNode`.

## Goals

1. Add `NativeRegistry` — maps stable `@@rust("id")` strings to Rust function implementations.
2. Extend `Execution` with a registry reference.
3. Execute `FuncCall` nodes:
   - Look up the function name in `definitions.functions` or `definitions.extensions`
   - Find the corresponding graph in `ir.graphs`
   - Spawn a child execution and run the graph
   - Store the child result as the current node's output
4. Handle `IrExpr::Call` inside `eval_expr` by dispatching to the native registry.
5. Support both sync and async native functions (async deferred to Plan 19).

## Non-Goals

- Host tool dispatch (`HostToolCall`) — Plan 17
- Agent delegation (`AgentCall`) — Plan 18
- LLM reply loop (`Reply`) — Plan 19
- Checkpoint / resume — Plan 20
- Async execution — Plan 19

## Design

### NativeRegistry

```rust
pub struct NativeRegistry {
    entries: HashMap<String, NativeEntry>,
}

pub enum NativeEntry {
    Sync(fn(&[Value]) -> Result<Value, NativeError>),
}

pub struct NativeError {
    pub message: String,
}
```

The registry is an empty container at runtime startup. Actual builtin implementations live in a separate `quew-stdlib` crate (or host-provided crates) and register themselves at link time. The runtime never hardcodes a list of builtins — it only holds what was injected.

> **Design note:** The long-term approach is a `#[quew_builtin]` proc-macro (see `one.txt`) that generates Quew prelude declarations, runtime dispatch entries, and optional extension-method wrappers from a single Rust function annotation. The macro uses link-time collection (e.g. `inventory` crate) so that `NativeRegistry` is populated automatically when the executable links against `quew-stdlib`.

### FuncCall Execution

When `Execution::run` encounters a `FuncCall` node:

1. Resolve the function name:
   - If it starts with `function:` or `extension:`, look up the graph in `ir.graphs`
   - Otherwise, look up the name in `definitions.functions` and use its `graph_ref`

2. Build arguments by resolving each `DataRef` in the `args` map

3. Spawn a child `Execution` with the same `ir` and `interner`

4. Call `child.run(graph_id, Value::Object(args_map))`

5. Store the returned value as this node's output

### Native Dispatch in eval_expr

When `eval_expr` encounters `IrExpr::Call`:

1. Check if the function name is a bare name (not `function:` or `extension:` prefix)
2. Look it up in `NativeRegistry`
3. If found, evaluate all argument expressions recursively, then dispatch
4. If not found, return `Value::Null` (placeholder for now)

## Implementation Steps

### Step 1: Add `native.rs` to `quew-runtime`

- `NativeRegistry` struct with `register` and `get` methods
- `NativeEntry` enum (sync only for now)
- `NativeError` struct
- No built-in registrations in the runtime crate (single responsibility); tests register their own natives inline

### Step 2: Extend `Execution` with registry

- Add `natives: &'a NativeRegistry` field
- Update `Execution::new` signature
- Update all test call sites

### Step 3: Handle `FuncCall` in `Execution::run`

- Match `NodeKind::FuncCall` in the node dispatch loop
- Resolve graph ID from function name
- Build argument values
- Recurse into child graph
- Store result

### Step 4: Handle `IrExpr::Call` in `eval_expr`

- Accept `&NativeRegistry` parameter
- Dispatch bare-name calls to registry
- Evaluate arguments recursively before dispatch

### Step 5: Integration tests

- Compile a `.quew` with a function call → execute → assert result
- Compile a `.quew` with an extension method call → execute → assert result
- Test native dispatch directly

## Test Plan

1. **Unit tests:**
   - `NativeRegistry::register` and `NativeRegistry::get`
   - Sync native function dispatch with correct args
   - Sync native function dispatch with wrong args (error)

2. **Integration tests:**
   - Compile and execute a function that calls another function
   - Compile and execute an extension method call
   - Native builtin dispatch (e.g. string length)

3. **Existing tests:** `cargo test` in full workspace must still pass.

## Acceptance Criteria

- [x] `NativeRegistry` exists and can register/lookup sync functions
- [x] `Execution` handles `FuncCall` nodes by recursing into child graphs
- [x] `eval_expr` dispatches bare-name calls through `NativeRegistry` (with fallback to graph recursion)
- [x] Integration test: compiled function calling another function executes correctly
- [x] Integration test: compiled extension method call executes correctly
- [x] All existing tests pass (357 tests across full workspace)
