# Discussion 10: Graph IR Runtime Executor

*Proposed as Plan 14 — the logical next step after the compiler pipeline is complete.*

---

## 1. Current State

After Plans 10–13, the `quew-compiler` produces a complete `QuewGraphIR` in memory:

- **Definitions** — types, models, tools, functions, agents, and extension methods.
- **Graphs** — one `AgentGraph` per agent body, function body, and extension method body.
- **Node kinds** — `Input`, `Context`, `Output`, `LetBind`, `Branch`, `FuncCall`, `HostToolCall`, `Reply`, `AgentCall`.

The CLI can compile `.quew` files and print a summary:

```text
compile ok
entry agent: Main
definitions: 5 type(s), 0 model(s), 0 tool(s), 6 function(s), 1 agent(s)
graphs: 10 graph(s), 25 node(s), 11 edge(s)
```

## 2. The Gap

There is **no runtime**. The `QuewGraphIR` is an in-memory Rust struct tree that nobody consumes.

The `quew-ir` README states:

> "The runtime holds it in memory and shares it (behind an `Arc`) across all concurrent runs of the same program."

But no `quew-runtime` crate exists. Without one:

- Compiled graphs are **dead code**.
- `FuncCall` nodes (including extension methods) have no executor.
- `HostToolCall` nodes have no dispatch mechanism.
- `Reply` nodes have no LLM integration.
- `AgentCall` nodes have no child-agent orchestration.
- **Checkpoint/resumption** — a stated v2 design goal — is impossible.

## 3. Design Goals

1. **Execute `QuewGraphIR` directly in Rust** — no JSON serialization step.
2. **Two-layer separation** — immutable IR + mutable execution state (journal), as described in `not_graph.txt`.
3. **Checkpoint after every effectful node** — deterministic nodes replay instantly.
4. **Resume from journal** — without re-executing completed nodes.
5. **Support nested graph calls** — `FuncCall`, `AgentCall` (black-box and with-turns).

## 4. Proposed Architecture

### 4.1 Core Types

```rust
/// Owns the compiled IR, native registry, and checkpoint store.
/// Shared across all concurrent executions via `Arc`.
pub struct Runtime {
    pub ir: Arc<QuewGraphIR>,
    pub natives: NativeRegistry,
    pub checkpoint: Box<dyn CheckpointStore>,
}

/// One run of one graph.
pub struct Execution<'a> {
    pub runtime: &'a Runtime,
    pub journal: Journal,
    pub graph_id: String,
}

/// Serialized execution state — the "bookmark".
pub struct Journal {
    pub node_status: HashMap<NodeKey, NodeStatus>,
    pub node_outputs: HashMap<NodeKey, Value>,
    pub active_nodes: HashMap<NodeKey, ActiveNodeState>,
}

/// Globally unique node identifier: `"agent:Main:n3"`.
pub struct NodeKey(String);
```

### 4.2 Value System

Runtime values need a `Value` enum that can represent every Quew type:

```rust
pub enum Value {
    String(String),
    Number(i64),
    Float(f64),
    Bool(bool),
    Null,
    Object(IndexMap<String, Value>),
    Array(Vec<Value>),
}
```

`IrExpr` evaluation produces `Value`:

| `IrExpr` variant | Evaluation rule |
|------------------|-----------------|
| `Lit` | Convert `IrLit` → `Value` |
| `Ref` | Look up `node_outputs[DataRef.node]`, optionally select `slot` |
| `Binary` | Evaluate `left` and `right`, apply `BinaryOp` |
| `Unary` | Evaluate `expr`, apply `UnaryOp` |
| `Member` | Evaluate `base`, select `field` |
| `Call` | Look up native function in `NativeRegistry`, dispatch with evaluated args |
| `Array` | Evaluate each element |
| `Ternary` | Evaluate `cond`, pick `then` or `else_` |

### 4.3 Execution Loop

```rust
impl Execution<'_> {
    pub async fn run(&mut self, input: Value) -> Result<Value, RuntimeError> {
        // 1. Seed the Input node.
        let input_key = self.key(self.graph.entry_node);
        self.journal.node_status.insert(input_key.clone(), NodeStatus::Done);
        self.journal.node_outputs.insert(input_key, input);

        // 2. Walk the graph.
        loop {
            let ready = self.find_ready_nodes();
            if ready.is_empty() {
                break;
            }

            for node in ready {
                if self.journal.node_status.get(&self.key(node.id)) == Some(&NodeStatus::Done) {
                    continue; // already computed — skip
                }
                self.execute_node(node).await?;
            }
        }

        // 3. Return the Output node's value.
        let output_key = self.key(self.graph.return_node);
        self.journal.node_outputs
            .get(&output_key)
            .cloned()
            .ok_or(RuntimeError::MissingOutput)
    }
}
```

A node is **ready** when all upstream nodes (those with edges pointing to it) are `Done`.

### 4.4 Node Execution

| NodeKind | Action | Checkpoint? |
|----------|--------|-------------|
| `Input` | Already seeded — no-op | Never |
| `Context` | Injected by runtime setup — no-op | Never |
| `Output` | Return `DataRef` value — no-op | Never |
| `LetBind` | Evaluate `IrExpr`, store result | Optional |
| `Branch` | Evaluate condition, mark taken branch | Optional |
| `FuncCall` | Look up graph, spawn child `Execution`, await result | Optional (Required if function calls host tools) |
| `HostToolCall` | Dispatch to host callback via tool name | Required |
| `Reply` | Run the full LLM conversation loop | Required |
| `AgentCall` | Spawn child agent `Execution` (black-box or inlined) | Required |

### 4.5 Native Function Registry

For `@@rust` builtin functions, the runtime maintains a registry:

```rust
pub struct NativeRegistry {
    entries: HashMap<String, NativeEntry>,
}

pub enum NativeEntry {
    Sync(fn(&[Value]) -> Result<Value, NativeError>),
    Async(
        fn(&[Value]) -> Pin<Box<dyn Future<Output = Result<Value, NativeError>> + Send>>,
    ),
}
```

The runtime populates this at startup. `IrExpr::Call` with a function name that starts with `function:` or `extension:` is **not** a native call — it is a graph call that recurses into `QuewGraphIR.graphs`. `IrExpr::Call` with a bare name (e.g. `"string_is_empty"`) is a native call looked up in `NativeRegistry`.

### 4.6 Checkpoint / Resume

After every effectful node execution:

```rust
self.journal.node_status.insert(node_key.clone(), NodeStatus::Done);
self.journal.node_outputs.insert(node_key, output_value.clone());

if node.checkpoint == CheckpointPolicy::Required {
    self.runtime.checkpoint.save(&self.journal).await?;
}
```

On resume:

1. Load `QuewGraphIR` (immutable).
2. Load `Journal` from checkpoint store.
3. Rebuild scheduler from `graphs`.
4. For every node:
   - `Done` → skip, use cached `node_outputs`.
   - `Running` → restore `active_nodes[node_key]`, continue execution.
   - `Pending` → wait for dependencies.
   - `Failed` → apply retry/fallback/error policy if present.
5. Continue from `Running` nodes first, then schedule newly ready nodes.

### 4.7 Reply Node Execution

The `Reply` node is the most complex. It encapsulates the entire v1 runtime loop:

1. Generate system prompt from `config.prompt`.
2. Build the message list from `DataRef::message` + history.
3. Dispatch to the model driver (`ModelDef.provider`).
4. Stream response:
   - Block mode: feed chunks to `BlockOrchestrator`.
   - Native mode: handle `NativeToolCall` / `NativeStructuredOutput` events.
5. Parse intents, dispatch tool calls.
6. Build tool result messages, continue the conversation loop.
7. Respect `max_turn`, `retry`, `fallback`.
8. Return the final assistant message as the node's output.

The journal `active_nodes` entry for a `Reply` stores:

```rust
pub struct ActiveReplyState {
    pub protocol: ProtocolMode,      // Block or Native
    pub turn_count: u32,
    pub transcript: Vec<Message>,
    pub pending_tool_calls: Vec<ToolCall>,
    pub partial_response_text: String,
}
```

### 4.8 AgentCall Modes

**Black-box** (`AgentCallMode::BlackBox`):

- Spawn a new `Execution` with its own `Journal`.
- Parent journal stores a nested `child_journal` in `active_nodes`.
- Parent only sees the child's final output.

**With-turns** (`AgentCallMode::WithTurns`):

- Inline the child's graph into the parent's execution scope.
- Child nodes share the parent's `Journal`.
- Child's turn trace is inspectable from the parent.

## 5. Implementation Phases

| Phase | Scope | Deliverable |
|-------|-------|-------------|
| **1** | Value system + expression evaluator | `Value` enum, `eval_expr()` |
| **2** | Deterministic graph executor | Execute `Input`, `Context`, `LetBind`, `Branch`, `Output` |
| **3** | FuncCall + NativeRegistry | Execute `FuncCall` (regular + extension), dispatch `@@rust` builtins |
| **4** | HostToolCall | Dispatch host tools, checkpoint before/after |
| **5** | AgentCall | Black-box and with-turns child agent execution |
| **6** | Reply node | LLM boundary: model driver, streaming, tool dispatch, retry |
| **7** | Checkpoint store | Serialize/deserialize `Journal`, resume from disk |

Each phase is independently testable. Phase 2 alone lets us execute pure computation graphs. Phase 3 lets us run the stdlib. Phase 6 is the full agent runtime.

## 6. Relationship to Existing Code

| Existing Component | Runtime Role |
|--------------------|--------------|
| `QuewGraphIR` | Immutable program definition, held in `Arc<QuewGraphIR>` |
| `AgentGraph` | Executed by `Execution::run_graph()` |
| `Definitions` | Lookup tables for `FuncCall`, `HostToolCall`, `AgentCall`, `Reply` config |
| `NodeKind::FuncCall` | Look up `definitions.functions` (or `definitions.extensions`), find graph in `graphs`, recurse |
| `NodeKind::HostToolCall` | Look up `definitions.tools`, dispatch to host callback |
| `NodeKind::Reply` | The v1 runtime loop becomes the implementation of this node |
| `NodeKind::AgentCall` | Spawn child `Execution` or inline child graph |
| `CheckpointPolicy` | Determines whether `Execution` saves the journal after the node |

## 7. Crate Layout

A new `quew-runtime` crate (or workspace) with modules:

```
quew-runtime/
  src/
    lib.rs
    value.rs          # Value, eval_expr
    execution.rs      # Execution, run loop, node dispatch
    journal.rs        # Journal, NodeStatus, checkpoint trait
    native.rs         # NativeRegistry, NativeEntry
    reply/            # Reply node implementation
      mod.rs
      block.rs        # BlockOrchestrator integration
      native.rs       # Provider-native tool calling
    checkpoint/       # Disk / Redis / Memory stores
      mod.rs
      memory.rs
      disk.rs
```

## 8. Open Questions

1. **Async runtime dependency** — Should `quew-runtime` depend on `tokio`, or be executor-agnostic?
2. **Model driver reuse** — Should the runtime import `auwgent-drivers` from the Auwgent runtime workspace, or define its own `ModelDriver` trait?
3. **Error model** — Are node failures always fatal, or should the runtime support per-node retry policies?
4. **Journal serialization format** — JSON (human-debuggable), MessagePack (compact), or custom?
5. **Host tool dispatch boundary** — Should `HostToolCall` invoke async Rust closures, FFI, or WASM modules?

## 9. Summary

The compiler pipeline is complete but produces dead code. A runtime executor is the natural next step to make `.quew` files executable. The design follows the two-layer separation (immutable IR + mutable journal) already specified in `not_graph.txt` and `RESUMABLE_GRAPH_IR_PROPOSAL.md`.

This is a large undertaking, but the phased approach lets us deliver incremental value:
- **Phase 1–2** → deterministic computation works
- **Phase 3** → stdlib builtins work
- **Phase 4–5** → host tools and sub-agents work
- **Phase 6–7** → full LLM agent runtime with checkpoint/resume

**Recommendation:** Start with Phase 1 (Value + eval_expr) and Phase 2 (deterministic executor). These are small, self-contained, and immediately testable against the graphs the compiler already emits.

## 10. Antigravity's Findings & Recommendations

### 10.1 The `lower_expr` Argument Dropping Bug (Blocker for Phase 1 & 2)
In Section 4.2, the evaluation of `IrExpr::Call` is defined. However, there is a critical compiler bug in `quew-ir/src/lower/expr.rs` (lines 90–93) where the lowerer silently drops arguments for regular function calls:
```rust
IrExpr::Call {
    function,
    args: Default::default(), // All arguments are silently discarded!
}
```
- **Impact:** Any regular function call (e.g., `max(a, b)`) evaluates with empty arguments, resulting in runtime crashes or incorrect values during Phase 1 & 2 execution.
- **Resolution:** Before starting Phase 1, we must update the compiler to lookup parameter names from `definitions.functions` and map the arguments correctly, falling back to positional names (`arg0`, `arg1`) if not defined.

### 10.2 Symbol Table API Leak Cleanup
To keep the public interface of `quew-checker` clean, we should implement `Default` for `CheckResult` inside `quew-checker`. This allows `quew-ir` unit tests to construct mock check results using `CheckResult::default()` and lets us remove the leaked `pub use quew_scope::SymbolTable;` from `quew-checker/src/lib.rs`.

### 10.3 WASM & Single-Threaded Future Constraints
Since Auwgent targets edge environments and WASM (Cloudflare Workers, browsers), the runtime executor needs to be designed with `wasm32` constraints in mind:
- **The Registry:** `NativeEntry::Async` uses `Pin<Box<dyn Future<Output = ...> + Send>>`. On `wasm32`, futures do not require the `Send` trait. We should design the async dispatch to support `#[cfg(target_arch = "wasm32")]` with `dyn Future` (no `Send`) to prevent compilation failures when targeting WASM.
- **Time/Instant compatibility:** To prevent accidental usage of standard library `std::time` types that panic or fail to compile on WASM, we should add a `clippy.toml` configuration to disallow `std::time::SystemTime` and `std::time::Instant` in favor of `web_time::SystemTime` and `web_time::Instant`:
  ```toml
  # Used by mise task clippy-wasm via CLIPPY_CONF_DIR.
  # On wasm32, web_time types are distinct from std::time so clippy correctly
  # flags only direct std::time usage without false-positiving on web_time.
  disallowed-types = [
      { path = "std::time::SystemTime", reason = "use web_time::SystemTime for WASM compatibility", replacement = "web_time::SystemTime" },
      { path = "std::time::Instant", reason = "use web_time::Instant for WASM compatibility", replacement = "web_time::Instant" },
  ]
  ```
- **Checkpoint Stores:** The `CheckpointStore` trait must be async and non-blocking so it doesn't block the WASM single thread.

