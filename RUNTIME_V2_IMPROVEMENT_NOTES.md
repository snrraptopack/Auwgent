# Runtime v2 Improvement Notes

Status: conceptual notes  
Source review: `runtime/crates/auwgent-engine`, `runtime/crates/auwgent-evaluator`, `runtime/crates/auwgent-session`, `runtime/crates/auwgent-middleware`, `runtime/crates/auwgent-bridge`

This document captures what the current runtime does well, where it is stretched by the current IR model, and what should improve for the graph-based v2 runtime.

## 1. Current Runtime Summary

The current runtime evaluates the old IR as an agent loop:

```text
AgentIR
  -> AuwgentEngine
  -> evaluate model config
  -> generate prompt
  -> start session turn
  -> stream model output
  -> parse intents
  -> execute tools/workflows/helpers
  -> append results
  -> continue or complete
```

The important crates are:

- `auwgent-engine`: owns the runtime loop, tool/workflow/helper execution, prompt generation, and middleware coordination.
- `auwgent-evaluator`: evaluates prompt expressions, model config expressions, workflow bodies, templates, variable declarations, and function calls.
- `auwgent-session`: stores session turns and reconstructs provider messages.
- `auwgent-middleware`: defines runtime event payloads and parses middleware responses.
- `auwgent-bridge`: exposes the runtime to target SDKs and host language callbacks.
- `auwgent-protocol`: parses block protocol output into intents.
- `auwgent-native`: builds provider-native callable schemas and routes native calls.

The current design is extensible, but the main execution unit is still the agent loop, not a resumable graph node.

## 2. Things Worth Keeping

### 2.1 JSON Callback Boundary

The middleware callback receives JSON and optionally returns JSON:

```text
event JSON -> host callback -> optional response JSON
```

This is a good cross-language abstraction. It works across TypeScript, Python, Dart, Rust, Node, and WASM without requiring every target to implement native Rust traits.

Keep this boundary for v2.

### 2.2 Middleware Event Model

Current middleware events map well to the future graph model:

- `run_start`
- `llm_start`
- `llm_end`
- `intent`
- `run_complete`
- `error`

These should remain, but v2 should attach graph/node metadata to every event.

### 2.3 Intent Control

Current intent middleware can:

- skip an intent,
- override the result,
- allow normal execution.

This is a strong extension point. It lets host code change behavior without changing the compiled IR.

Keep this, but make control effects explicit in execution state.

### 2.4 Session Export/Import

Current `SessionState` export/import is useful and should remain as a compatibility layer.

In v2, session export should become a view over execution state rather than the primary resume mechanism.

### 2.5 Helper Sub-Engine Model

Helpers currently run by creating a sub-engine with:

- copied drivers,
- authorized parent tools,
- inherited context,
- optional session preload/save hooks,
- stack-aware execution.

This maps naturally to black-box child graph execution in v2.

### 2.6 Protocol Split

The current runtime already branches on:

```rust
resolve_tool_protocol() == "native"
```

That split should survive inside the v2 `reply` node implementation:

- block mode uses bracket protocol and orchestrator,
- native mode uses provider tool calls and provider output schemas.

## 3. Current Pain Points

### 3.1 Agent Loop Is Too Coarse

The current runtime knows it is "inside a run", but not precisely which logical DSL step is active.

For example, it can resume a session, but it cannot directly say:

```text
workflow marks_and_location is done with get_marks but not get_location
reply node n20 is waiting on tool get_location
agent call Analyze completed and branch n3 is next
```

The v2 graph must make these addressable.

### 3.2 Session Is Doing Too Much

Current `SessionState` acts as:

- conversation memory,
- provider message reconstruction source,
- helper stack carrier,
- partial resume surface,
- native tool-call persistence surface,
- binding cursor source.

For v2, split this into clearer layers:

- `ExecutionState`: node status, outputs, active nodes, checkpoints.
- `TranscriptState`: model/user/tool messages for reply nodes.
- `SessionView`: user-facing conversation/session export.
- `TraceLog`: optional event timeline.

### 3.3 Workflows Are Not Resumable Internally

`execute_workflow()` evaluates workflow body statements in a loop. If a process dies halfway through, the only durable unit is the surrounding session, not each workflow statement.

In v2, workflow/function bodies should lower into graph nodes:

```text
params -> tool_call get_marks -> tool_call get_location -> template -> return
```

Each external call inside the workflow should checkpoint.

### 3.4 Helper Resume Uses Stack Fast-Forward

The current runtime uses `session.stack` and `fast_forward_stack` to re-enter helper execution.

This works, but it is indirect. It resumes by reconstructing a path through helper names rather than by restoring a running node.

In v2, helper/agent calls should have active state:

```json
{
  "node": "agent:Main:n8",
  "kind": "agent_call",
  "status": "running",
  "childState": {}
}
```

### 3.5 Middleware Effects Are Not Persisted As First-Class State

Middleware can mutate:

- session,
- prompt,
- stack,
- provider,
- model,
- config,
- headers,
- API key,
- error retry behavior,
- intent execution.

Today those effects happen during the loop, but they are not recorded as structured execution decisions.

In v2, middleware decisions should be checkpointed:

```json
{
  "event": "llm_start",
  "node": "agent:Main:n20",
  "decision": {
    "model": "llama-3.3-70b-versatile",
    "provider": "groq",
    "configPatch": {},
    "promptChanged": true
  }
}
```

This matters for replay, audit, and deterministic resume.

### 3.6 Pending Intents Are Ephemeral

Current parsed intents are stored in an in-memory `pending_intents` queue. If the runtime dies after a model emits a tool call but before the tool result is recorded, the exact pending action may be lost unless it was already captured in session/native turn state.

In v2, pending actions should live in `activeNodes[nodeId]`:

```json
{
  "pendingActions": [
    {
      "kind": "tool_call",
      "name": "get_location",
      "args": {},
      "status": "dispatched"
    }
  ]
}
```

### 3.7 Result Continuation Is Protocol-Specific In The Loop

Current block mode builds `[result]` blocks and starts a new turn. Native mode stores native tool results and starts an empty continuation turn.

That logic belongs inside a reusable `ReplyNodeRuntime` rather than the global engine loop.

In v2:

```text
GraphExecutor
  -> ReplyNodeRuntime
      -> block continuation
      -> native continuation
```

### 3.8 Error Handling Is Global Instead Of Node-Scoped

Current error middleware can force restart at `llm_start` or `run_start`.

In v2, error policies should support node-scoped restart:

```text
retry this tool node
retry this reply turn
fallback this reply node to another model
skip this optional node
fail the whole run
```

The old force-start idea is still useful, but graph execution needs finer targets.

## 4. Recommended v2 Runtime Shape

### 4.1 Main Components

```text
GraphExecutor
  - owns graph scheduling
  - reads immutable graph IR
  - updates execution state
  - checkpoints state

NodeRuntime
  - executes one node type
  - returns node output or active state

ReplyNodeRuntime
  - owns current model loop behavior
  - uses block/native protocols
  - emits intents and tool calls

MiddlewareHost
  - sends JSON events to host callbacks
  - parses control decisions
  - records decisions in state

CheckpointStore
  - saves/loads execution state
  - local file, memory, custom adapter, or hosted storage

SessionProjector
  - derives old-style session export from execution state
```

### 4.2 Execution Loop

```text
load immutable graph IR
load or create execution state
seed input/context nodes
while run is not terminal:
  resume running nodes
  find ready nodes
  execute ready nodes
  checkpoint after external transitions
return output node value
```

The scheduler should own run progress. The reply node should no longer own the entire runtime.

### 4.3 Node Runtime Trait Concept

Conceptually:

```rust
trait NodeRuntime {
    async fn start(&self, ctx: NodeContext) -> NodeStep;
    async fn resume(&self, ctx: NodeContext, active: Value) -> NodeStep;
}
```

Possible results:

```text
Done(output)
Running(activeState)
Failed(error)
Skipped
```

This gives every node a consistent lifecycle.

### 4.4 Middleware Events Should Include Node Context

Every middleware event should include:

```json
{
  "runId": "run_123",
  "graphId": "agent:Main",
  "nodeId": "n20",
  "nodeType": "reply",
  "activeAgent": "Main",
  "stack": ["Main"],
  "rootAgent": "Main"
}
```

This keeps old middleware concepts but makes them graph-aware.

## 5. Better Middleware Surface For v2

### 5.1 Keep Existing Events

Keep:

- `run_start`
- `llm_start`
- `llm_end`
- `intent`
- `run_complete`
- `error`

### 5.2 Add Graph Events

Add:

- `node_start`
- `node_complete`
- `node_error`
- `checkpoint`
- `agent_call_start`
- `agent_call_complete`
- `tool_dispatch`
- `tool_result`

### 5.3 Add Explicit Control Responses

Current middleware response shapes are flexible but implicit.

For v2, define explicit response envelopes:

```json
{
  "control": "continue"
}
```

```json
{
  "control": "skip"
}
```

```json
{
  "control": "override",
  "result": {}
}
```

```json
{
  "control": "retry",
  "target": "current_node"
}
```

```json
{
  "control": "fail",
  "error": {}
}
```

Backward compatibility can still support old shapes like:

```json
{ "skip": true }
{ "result": {} }
{ "forceStart": "llm_start" }
```

## 6. Better Persistence Model

### 6.1 Current Persistence

Current persistence surfaces:

- `export_session()`
- `import_session()`
- `on_sub_engine_start`
- `on_sub_engine_complete`

These are enough for conversation continuation and helper session persistence, but not enough for exact graph resume.

### 6.2 v2 Persistence

Add a first-class checkpoint API:

```text
load_state(run_id) -> ExecutionState?
save_state(run_id, state) -> void
append_event(run_id, event) -> void
```

Storage should be pluggable:

- memory store for tests,
- local file store for dev,
- user-provided SDK adapter,
- hosted store for paid managed resumability.

### 6.3 Paid Tier Boundary

The graph IR and local execution should be core.

The paid tier should be managed persistence:

- durable cloud checkpoints,
- cross-device resume,
- run dashboard,
- replay timeline,
- team/project run history,
- hosted queues and retries,
- audit logs,
- managed secrets.

## 7. Better State Shape

Recommended split:

```json
{
  "runId": "run_123",
  "status": "running",
  "nodeStatus": {},
  "nodeOutputs": {},
  "activeNodes": {},
  "transcripts": {},
  "eventLog": []
}
```

Where:

- `nodeStatus` tells the scheduler what to do.
- `nodeOutputs` stores completed node values.
- `activeNodes` stores resumable in-progress state.
- `transcripts` stores reply-node conversations.
- `eventLog` stores optional timeline/debug data.

## 8. Better Reply Node State

Current reply state is spread across:

- current raw response,
- pending intents,
- pending tool results,
- session turn,
- native assistant turn,
- native tool results,
- orchestrator state.

In v2, a reply node should own:

```json
{
  "kind": "reply",
  "protocol": "block",
  "turnCount": 2,
  "resolvedConfig": {},
  "transcriptId": "transcript:n20",
  "pendingActions": [],
  "partialResponse": "",
  "orchestratorState": {},
  "nativeCallState": {}
}
```

This makes the reply loop resumable without relying on global engine fields.

## 9. Better Workflow/Function Execution

Current workflows execute as evaluator statements.

V2 should lower functions/workflows into graph nodes.

Example:

```auwgent
function marks_and_location(user_id: string): string {
  let marks = get_marks(user_id)
  let location = get_location()
  return "Location: {location}\nMarks: {marks}"
}
```

Should become:

```text
params
  -> tool_call get_marks
  -> tool_call get_location
  -> template
  -> return
```

This gives tool calls inside functions the same checkpoint and middleware behavior as top-level tool calls.

## 10. Better Host Tool Semantics

Current tools are registered as callbacks:

```text
tool name -> async callback(args) -> result
```

Keep this.

Improve with:

- stable tool invocation ids,
- persisted dispatch state,
- idempotency keys,
- timeout metadata,
- retry policy,
- result envelope standardization.

Recommended tool call active state:

```json
{
  "kind": "tool_call",
  "tool": "get_marks",
  "invocationId": "toolinv_123",
  "args": { "id": "42" },
  "status": "dispatched",
  "attempt": 1,
  "idempotencyKey": "run_123:n8:attempt1"
}
```

## 11. Better Error Model

Standardize errors:

```json
{
  "code": "TOOL_ERROR",
  "message": "Tool failed",
  "recoverable": true,
  "source": {
    "graphId": "agent:Main",
    "nodeId": "n8",
    "nodeType": "tool_call"
  }
}
```

This gives middleware and hosted dashboards enough structure to reason about failures.

## 12. Better Observability

The current runtime emits structured JSONL stream events and intent callbacks, but graph v2 should make observability native.

Useful views:

- graph node timeline,
- model turn timeline,
- tool call timeline,
- middleware decision timeline,
- checkpoint timeline,
- replay from checkpoint,
- diff between original config and middleware-mutated config.

This is especially important if managed resumability becomes a paid feature.

## 13. Migration Strategy

### 13.1 Compile Old IR Into A Simple Graph

Old agents can become:

```text
input -> context -> reply -> output
```

Old workflows become function graphs.

Old helpers become agent graphs.

This preserves old behavior while moving the runtime to the new scheduler.

### 13.2 Keep Current Runtime Loop As ReplyNodeRuntime

The current `run()` loop should not be thrown away. Most of it becomes the implementation of a v2 `reply` node:

- prompt generation,
- block/native protocol handling,
- streaming,
- intent processing,
- tool result continuation,
- llm middleware.

The graph executor should call into that node runtime.

### 13.3 Compatibility Session Export

Existing SDKs expect session export/import. Keep it by projecting v2 execution state into old session format.

Eventually:

```text
ExecutionState -> SessionView
```

not:

```text
SessionState -> Resume
```

## 14. Most Important Improvements

Priority order:

1. Introduce `ExecutionState` separate from `SessionState`.
2. Add graph/node metadata to middleware events.
3. Move current model loop into a `ReplyNodeRuntime`.
4. Make pending intents/actions persistent inside active node state.
5. Lower workflows/functions into resumable graph nodes.
6. Add checkpoint store abstraction.
7. Add stable invocation ids for tools and agent calls.
8. Make middleware decisions explicit and persisted.
9. Project old session export from graph state for compatibility.
10. Keep JSON callback boundaries for all target SDKs.

## 15. Summary

The current runtime already has strong extensibility:

- host tool callbacks,
- intent callbacks,
- partial intent callbacks,
- middleware events,
- session import/export,
- helper session preload/save,
- block/native protocol split.

The main limitation is that execution progress is attached to the agent loop and session turns, not to stable graph nodes.

V2 should keep the callback and middleware architecture, but move execution ownership to a graph scheduler:

```text
old:
  session + run loop drive execution

new:
  graph state + scheduler drive execution
  session becomes a projected view
```

That is the clean path to real resumability without losing the extensibility already present in the runtime.
