# Resumable Graph IR Proposal

Status: conceptual proposal  
Source notes: `not.txt`, `not_graph.txt`, `runtime-tests/canonical.agent`, `runtime-tests/canonical.agent.json`

This document proposes a future IR shape for the new Auwgent DSL direction. The main change is that compiled output should become an immutable execution graph, while runtime progress should live in a separate serialized execution state object.

The goal is not only to make the DSL feel more like a small language. The larger goal is to make every meaningful unit of execution addressable, checkpointable, inspectable, and resumable.

## 1. Current Shape

The current canonical agent source is section-oriented:

```auwgent
agent RuntimeTest {
    default config { ... }
    input: Text
    context { ... }
    intent: Loud
    tool get_location(): string
    workflow marks_and_location(user_id: string): string { ... }
    helpers { Planner }
}
```

The compiled `runtime-tests/canonical.agent.json` is an agent definition. It describes the agent's capabilities:

```json
{
  "name": "RuntimeTest",
  "modelConfig": [],
  "input": null,
  "output": null,
  "context": {},
  "tools": [],
  "workflows": [],
  "helpers": [],
  "customIntents": []
}
```

This shape is useful for describing what the agent has, but it is not a precise execution plan. The runtime still has to interpret the agent as a loop around model output, intents, tools, workflows, helpers, and session state.

That makes partial resume difficult because the IR does not identify each execution step as a stable node.

## 2. Future DSL Direction

The proposed DSL in `not.txt` moves toward agents as typed executable units:

```auwgent
agent Hello(input: Text): Text {
    reply(input) with {
        prompt = "..."
        model = gemini("...")
    }
}
```

Important language concepts from the proposal:

- `agent` behaves like a typed callable.
- `reply(value) with { ... }` is the explicit LLM boundary.
- `function` represents deterministic or runtime-executed logic.
- `@tool function` exposes a function to the model as a callable tool.
- `tool` declarations represent host-provided external tools.
- `@context(...)` binds runtime context into the agent.
- Agents can call tools, functions, and other agents.
- `return OtherAgent(input)` exits the current agent with the child output.
- `return OtherAgent(input) with turns` preserves or exposes more of the child execution trace.
- `with { ... }` config can be dynamic: model selection, tools, fallback, retry, max turns, prompt, agents.

This language direction implies that the compiler should no longer only emit a capability object. It should emit an executable graph.

## 3. Core Principle

The future architecture should have two separate layers:

1. **Compiled IR:** static, immutable, shared by many runs.
2. **Execution State:** dynamic, mutable, serialized per run.

The compiled IR is the program. The execution state is the bookmark.

The runtime must never mutate the compiled IR. It only updates execution state.

## 4. Proposed Top-Level IR

```json
{
  "irVersion": "2",
  "kind": "auwgent.graph",
  "program": {
    "name": "RuntimeTest",
    "entryAgent": "RuntimeTest"
  },
  "definitions": {
    "types": {},
    "models": {},
    "tools": {},
    "functions": {},
    "agents": {},
    "intents": {}
  },
  "graphs": {
    "RuntimeTest": {
      "graphId": "agent:RuntimeTest",
      "entryNode": "n0",
      "returnNode": "n9",
      "nodes": [],
      "edges": []
    }
  }
}
```

The IR has two broad sections:

- `definitions`: reusable static declarations.
- `graphs`: executable control/data-flow plans.

Definitions answer "what exists?" Graphs answer "what runs?"

## 5. Definitions

Definitions preserve the useful parts of the current canonical JSON, but separate them from execution order.

### 5.1 Types

```json
{
  "types": {
    "PlannerOutput": {
      "type": "object",
      "properties": {
        "steps": {
          "type": { "type": "array", "items": "string" },
          "optional": false,
          "description": "Step-by-step plan"
        },
        "motivation": {
          "type": "string",
          "optional": false,
          "description": "Why this plan is the right approach"
        }
      }
    }
  }
}
```

### 5.2 Models

Current source:

```auwgent
model DefaultModel {
    provider: groq("llama-3.3-70b-versatile")
}
```

Proposed definition:

```json
{
  "models": {
    "DefaultModel": {
      "provider": {
        "type": "groq",
        "modelName": "llama-3.3-70b-versatile",
        "config": null
      },
      "embedding": null
    }
  }
}
```

Dynamic model expressions inside `with` blocks should be represented in graph nodes, not in the model definition itself.

### 5.3 Host Tools

Current source:

```auwgent
tool get_location(): string @desc "Return the current location for the active user"
tool get_marks(id: string @desc "the id of the user"): string @desc "Return the user's score"
```

Proposed definition:

```json
{
  "tools": {
    "get_location": {
      "kind": "host",
      "description": "Return the current location for the active user",
      "params": {},
      "returns": "string",
      "examples": []
    },
    "get_marks": {
      "kind": "host",
      "description": "Return the user's score",
      "params": {
        "id": {
          "type": "string",
          "optional": false,
          "description": "the id of the user"
        }
      },
      "returns": "string",
      "examples": []
    }
  }
}
```

The definition says the tool exists. A graph `tool_call` node says the tool is called by agent code before a model turn. A `reply` node's tool list says the tool is exposed to the model.

### 5.4 DSL Functions

The future DSL introduces ordinary functions:

```auwgent
function sanitizeInput(input: Text): Text {
    let pr = input.remove("hash word")
    return "cleaned now it is : {pr}"
}
```

Proposed definition:

```json
{
  "functions": {
    "sanitizeInput": {
      "visibility": "internal",
      "params": {
        "input": "text"
      },
      "returns": "text",
      "graph": "function:sanitizeInput"
    }
  }
}
```

Tool functions use the same structure with a different visibility:

```auwgent
@tool
@desc "use this to delete a user"
function delete_person(id: string, isAdmin: bool): string {
    ...
}
```

```json
{
  "functions": {
    "delete_person": {
      "visibility": "tool",
      "description": "use this to delete a user",
      "params": {
        "id": "string",
        "isAdmin": "boolean"
      },
      "returns": "string",
      "graph": "function:delete_person"
    }
  }
}
```

### 5.5 Agents

```json
{
  "agents": {
    "RuntimeTest": {
      "input": null,
      "output": null,
      "context": {
        "user_name": { "type": "string", "optional": false },
        "age": { "type": "number", "optional": false },
        "id": { "type": "string", "optional": false }
      },
      "graph": "agent:RuntimeTest"
    },
    "Planner": {
      "input": null,
      "output": { "typeRef": "PlannerOutput" },
      "context": null,
      "graph": "agent:Planner"
    }
  }
}
```

Helpers from the current DSL become agents with call policies. The old distinction can remain as metadata, but the execution model should treat them as graph-callable agents.

### 5.6 Intents

Current source:

```auwgent
intent Loud {
    description: "Use this to explain your thought process and actions out loud"
    fields {
        actions: string @desc "The action you are about to take"
        reason: string @desc "Why you are taking this action"
    }
}
```

Proposed definition:

```json
{
  "intents": {
    "Loud": {
      "description": "Use this to explain your thought process and actions out loud",
      "fields": {
        "actions": {
          "type": "string",
          "optional": false,
          "description": "The action you are about to take"
        },
        "reason": {
          "type": "string",
          "optional": false,
          "description": "Why you are taking this action"
        }
      },
      "examples": [
        {
          "actions": { "type": "literal", "value": "I will look up the user's location" },
          "reason": { "type": "literal", "value": "The user asked where they are" }
        }
      ]
    }
  }
}
```

## 6. Graph Shape

Each graph is a list of stable nodes and edges:

```json
{
  "graphId": "agent:RuntimeTest",
  "entryNode": "n0",
  "returnNode": "n9",
  "nodes": [
    {
      "id": "n0",
      "type": "input"
    },
    {
      "id": "n1",
      "type": "context"
    },
    {
      "id": "n2",
      "type": "reply"
    },
    {
      "id": "n9",
      "type": "output"
    }
  ],
  "edges": [
    { "from": "n0", "to": "n2", "slot": "message" },
    { "from": "n1", "to": "n2", "slot": "context" },
    { "from": "n2", "to": "n9", "slot": "value" }
  ]
}
```

Node ids must be stable for a given compiled IR. The execution state uses these ids as checkpoint keys.

## 7. Node Categories

### 7.1 Boundary Nodes

Boundary nodes connect the graph to the outside run call.

```json
{ "id": "n0", "type": "input", "inputType": null }
{ "id": "n1", "type": "context", "contextType": "RuntimeTestContext" }
{ "id": "n9", "type": "output" }
```

### 7.2 Value Nodes

Value nodes represent deterministic expressions.

```json
{
  "id": "n3",
  "type": "literal",
  "value": "hello"
}
```

```json
{
  "id": "n4",
  "type": "object",
  "fields": {
    "input": { "from": "n0" },
    "fetchedData": { "from": "n8" }
  }
}
```

```json
{
  "id": "n5",
  "type": "expression",
  "op": "contains",
  "args": [
    { "from": "n0" },
    { "literal": "weather" }
  ]
}
```

These nodes are normally replayable. They do not require a checkpoint unless the runtime wants complete inspection data.

### 7.3 Control Nodes

Control nodes model branching and early returns.

```json
{
  "id": "n6",
  "type": "branch",
  "condition": { "from": "n5" },
  "then": "n7",
  "else": "n10"
}
```

```json
{
  "id": "n11",
  "type": "return",
  "value": { "from": "n10" }
}
```

The graph does not have to be purely dataflow. It can represent control flow explicitly where needed.

### 7.4 Function Call Nodes

Internal DSL function call:

```json
{
  "id": "n7",
  "type": "function_call",
  "function": "sanitizeInput",
  "args": {
    "input": { "from": "n0" }
  }
}
```

If the function is deterministic and internal, it can be inlined or executed as a child graph. If it touches external runtime capabilities, it should checkpoint like any other effectful node.

### 7.5 Host Tool Call Nodes

Agent code can call a host tool before the model sees anything:

```auwgent
let fetchedData = getweather(input)
reply({ input, fetchedData }) with { ... }
```

Graph node:

```json
{
  "id": "n8",
  "type": "tool_call",
  "tool": "getweather",
  "args": {
    "query": { "from": "n0" }
  },
  "checkpoint": "required"
}
```

This is different from exposing a tool to the model. This node means the program itself is calling the tool.

### 7.6 Reply Nodes

`reply(...) with { ... }` is the most important new node. It represents an LLM turn loop that may stream, emit tool calls, receive tool results, call helpers/agents, and eventually produce output.

```json
{
  "id": "n20",
  "type": "reply",
  "message": { "from": "n0" },
  "config": {
    "prompt": {
      "type": "template",
      "parts": [
        { "type": "literal", "value": "You are a helpful test assistant." }
      ]
    },
    "model": { "ref": "DefaultModel" },
    "toolProtocol": "block",
    "tools": [
      { "ref": "get_location" },
      { "ref": "get_marks" }
    ],
    "agents": [
      {
        "ref": "Planner",
        "exposure": "model_tool",
        "handoff": "return"
      },
      {
        "ref": "Joker",
        "exposure": "model_tool",
        "handoff": "user"
      }
    ],
    "intents": [
      { "ref": "Loud" }
    ],
    "retry": null,
    "fallback": null,
    "maxTurns": 12
  },
  "outputType": null,
  "checkpoint": "required"
}
```

Dynamic config expressions should be represented as expression references:

```json
{
  "model": {
    "type": "if",
    "condition": { "path": ["context", "isVip"] },
    "then": { "ref": "Gemini" },
    "else": { "ref": "Groq" }
  },
  "tools": {
    "type": "if",
    "condition": { "path": ["context", "isAdmin"] },
    "then": [{ "ref": "getweather" }, { "ref": "delete_user" }],
    "else": [{ "ref": "getweather" }]
  }
}
```

The reply node is resumable because its active state can store transcript, partial output, pending calls, and turn count.

### 7.7 Agent Call Nodes

Black-box child agent call:

```json
{
  "id": "n30",
  "type": "agent_call",
  "agent": "One",
  "args": {
    "input": { "from": "n0" }
  },
  "mode": "black_box",
  "checkpoint": "required"
}
```

Transparent child agent call:

```json
{
  "id": "n31",
  "type": "agent_call",
  "agent": "One",
  "args": {
    "input": { "from": "n0" }
  },
  "mode": "with_turns",
  "checkpoint": "required"
}
```

Recommended semantics:

- `black_box`: parent stores only child final output plus nested child state while running.
- `with_turns`: parent preserves the child's execution trace. The runtime can implement this by inlining the child graph or by attaching a trace projection to the parent state.

### 7.8 Tool Group Nodes and Progressive Disclosure

The proposal includes grouped tools:

```auwgent
tools usertools {
    getusername(): string
    getuserlocation(): string
} @desc "This contains two tools getusername and location"
```

This should not just be a namespace. It has runtime behavior: the model first sees the group, then asks to reveal the inner tools.

Definition:

```json
{
  "tools": {
    "usertools": {
      "kind": "group",
      "description": "This contains two tools getusername and location",
      "disclosure": "lazy",
      "members": ["getusername", "getuserlocation"]
    }
  }
}
```

Reply exposure:

```json
{
  "tools": [
    { "ref": "getweather" },
    { "ref": "usertools", "exposure": "lazy_group" }
  ]
}
```

The runtime can expose internal discovery tools for lazy groups without changing the compiled graph.

## 8. Edges

Edges connect node outputs to node input slots.

```json
{
  "from": "n1",
  "to": "n20",
  "slot": "context"
}
```

For object fields and named arguments, either edges can carry a path:

```json
{
  "from": "n1",
  "fromPath": ["id"],
  "to": "n8",
  "slot": "args.id"
}
```

or nodes can reference sources directly:

```json
{
  "id": "n8",
  "type": "tool_call",
  "args": {
    "id": { "from": "n1", "path": ["id"] }
  }
}
```

The second style is more compact for expressions. The first style is better for graph analysis. The IR can support both, but it should prefer one canonical form.

Recommendation: use explicit source references inside node payloads for values, and use `edges` for scheduling/dependency analysis.

## 9. Execution State

Execution state is serialized separately from the IR.

```json
{
  "stateVersion": "1",
  "runId": "run_abc123",
  "irVersion": "2",
  "irHash": "sha256:...",
  "entryGraph": "agent:RuntimeTest",
  "status": "running",
  "createdAt": "2026-05-14T00:00:00Z",
  "updatedAt": "2026-05-14T00:00:10Z",
  "nodeStatus": {
    "agent:RuntimeTest:n0": "done",
    "agent:RuntimeTest:n1": "done",
    "agent:RuntimeTest:n20": "running"
  },
  "nodeOutputs": {
    "agent:RuntimeTest:n0": {
      "data": "hello user",
      "error": null
    },
    "agent:RuntimeTest:n1": {
      "data": {
        "user_name": "Kofi",
        "age": 30,
        "id": "42"
      },
      "error": null
    }
  },
  "activeNodes": {
    "agent:RuntimeTest:n20": {
      "kind": "reply",
      "turnCount": 2,
      "transcript": [],
      "pendingActions": [],
      "partialResponse": ""
    }
  },
  "eventLog": []
}
```

Node keys include graph id and node id so nested graphs cannot collide:

```text
agent:RuntimeTest:n20
function:delete_person:n4
agent:Planner:n8
```

## 10. Node Status

Recommended statuses:

```text
pending
ready
running
done
failed
skipped
cancelled
```

Meanings:

- `pending`: dependencies are not complete.
- `ready`: dependencies are complete and node can run.
- `running`: node started and may need resume data.
- `done`: node has a successful output.
- `failed`: node produced an unrecovered error.
- `skipped`: branch or control flow made this node unreachable.
- `cancelled`: run was stopped before completion.

## 11. Output Envelope

Every node output should use one envelope shape:

```json
{
  "data": {},
  "error": null,
  "meta": {}
}
```

Failure:

```json
{
  "data": null,
  "error": {
    "code": "TOOL_ERROR",
    "message": "tool failed",
    "recoverable": true
  },
  "meta": {}
}
```

This matches the language proposal's repeated idea that tool/function results behave like `{ data, error }`.

## 12. Active Reply State

A running reply node needs enough data to resume the model loop.

```json
{
  "kind": "reply",
  "protocol": "block",
  "turnCount": 2,
  "resolvedConfig": {
    "model": {
      "type": "groq",
      "modelName": "llama-3.3-70b-versatile"
    },
    "prompt": "You are a helpful test assistant.",
    "tools": ["get_location", "get_marks"],
    "agents": ["Planner", "Joker"],
    "intents": ["Loud"],
    "maxTurns": 12
  },
  "transcript": [
    {
      "role": "system",
      "content": "You are a helpful test assistant."
    },
    {
      "role": "user",
      "content": "hello user"
    },
    {
      "role": "model",
      "content": "[tool_call: get_location]\n[/tool_call]"
    },
    {
      "role": "tool",
      "name": "get_location",
      "content": "Accra, Ghana"
    }
  ],
  "pendingActions": [],
  "partialResponse": "",
  "lastCheckpoint": {
    "reason": "tool_result_received",
    "at": "2026-05-14T00:00:10Z"
  }
}
```

Native mode would use the same high-level active state but store provider call ids:

```json
{
  "kind": "reply",
  "protocol": "native",
  "turnCount": 1,
  "transcript": [
    {
      "role": "user",
      "content": "hello user"
    },
    {
      "role": "model",
      "content": null,
      "toolCalls": [
        {
          "id": "call_abc",
          "providerName": "tool_get_location",
          "canonicalName": "get_location",
          "args": {}
        }
      ]
    },
    {
      "role": "tool",
      "toolCallId": "call_abc",
      "name": "tool_get_location",
      "content": "Accra, Ghana"
    }
  ],
  "pendingActions": []
}
```

## 13. Active Agent Call State

Black-box child agent:

```json
{
  "kind": "agent_call",
  "agent": "Planner",
  "mode": "black_box",
  "childState": {
    "stateVersion": "1",
    "runId": "run_child_123",
    "entryGraph": "agent:Planner",
    "status": "running",
    "nodeStatus": {},
    "nodeOutputs": {},
    "activeNodes": {}
  }
}
```

Transparent child agent:

```json
{
  "kind": "agent_call",
  "agent": "Planner",
  "mode": "with_turns",
  "traceRef": "trace:agent:Planner:run_child_123"
}
```

The transparent mode should make the child path inspectable from the parent without forcing the parent graph itself to mutate.

## 14. Event Log

`nodeStatus` and `nodeOutputs` are the fast resume surface. `eventLog` is optional but useful for debugging and audit.

```json
{
  "eventId": "evt_001",
  "at": "2026-05-14T00:00:04Z",
  "node": "agent:RuntimeTest:n20",
  "type": "model_stream_started",
  "data": {
    "provider": "groq",
    "model": "llama-3.3-70b-versatile"
  }
}
```

Useful event types:

```text
run_started
node_started
node_completed
node_failed
model_stream_started
model_chunk
model_tool_call
tool_dispatched
tool_result
intent_emitted
middleware_event
checkpoint_saved
run_completed
```

For large streaming responses, the runtime may choose not to persist every chunk. It should persist enough to resume safely.

## 15. Checkpoint Rules

The runtime should save state after every external or non-deterministic transition:

- run starts
- reply node starts
- model stream starts
- model emits a tool call
- tool call is dispatched
- tool result is received
- helper or agent call starts
- child agent state changes
- model returns final output
- middleware mutates prompt, session, stack, or control flow
- node fails
- run completes

Pure deterministic nodes can be replayed. However, storing their outputs is still useful for debugging and graph inspection.

## 16. Resume Algorithm

Resume should not inspect or rewrite the IR. It should only combine IR plus state.

1. Load compiled IR.
2. Load execution state.
3. Verify `irHash` or compatible version.
4. Rebuild scheduler from `graphs`.
5. For every node:
   - `done`: use cached `nodeOutputs`.
   - `running`: restore `activeNodes[nodeKey]`.
   - `pending`: wait for dependencies.
   - `failed`: apply retry/fallback/error policy if present.
6. Continue from running nodes first.
7. Schedule newly ready nodes.
8. Persist state after each checkpoint boundary.

Pseudocode:

```typescript
while (!stateIsTerminal(state)) {
  const running = findRunningNodes(state)

  for (const node of running) {
    await resumeNode(ir, state, node)
    checkpoint(state)
  }

  const ready = findReadyNodes(ir, state)

  for (const node of ready) {
    await executeNode(ir, state, node)
    checkpointIfNeeded(state, node)
  }
}
```

## 17. Lowering Examples

### 17.1 Basic Agent

Source:

```auwgent
agent Hello(input: Text): Text {
   reply(input) with {
        prompt = "..."
        model = gemini("...")
   }
}
```

Graph:

```json
{
  "graphId": "agent:Hello",
  "entryNode": "n0",
  "returnNode": "n2",
  "nodes": [
    { "id": "n0", "type": "input", "inputType": null },
    {
      "id": "n1",
      "type": "reply",
      "message": { "from": "n0" },
      "config": {
        "prompt": { "type": "literal", "value": "..." },
        "model": {
          "provider": "gemini",
          "modelName": "..."
        },
        "tools": [],
        "maxTurns": 12
      },
      "outputType": "text",
      "checkpoint": "required"
    },
    {
      "id": "n2",
      "type": "output",
      "value": { "from": "n1" }
    }
  ],
  "edges": [
    { "from": "n0", "to": "n1", "slot": "message" },
    { "from": "n1", "to": "n2", "slot": "value" }
  ]
}
```

### 17.2 Input Sanitization

Source:

```auwgent
agent Hello(input: Text): Text {
    let user_input = if input.contains("hash word") then sanitizeInput(input) else input

    reply(user_input) with {
        prompt = "..."
        model = gemini("...")
    }
}
```

Graph:

```json
{
  "nodes": [
    { "id": "n0", "type": "input" },
    {
      "id": "n1",
      "type": "expression",
      "op": "contains",
      "args": [{ "from": "n0" }, { "literal": "hash word" }]
    },
    {
      "id": "n2",
      "type": "branch_value",
      "condition": { "from": "n1" },
      "then": {
        "type": "function_call",
        "function": "sanitizeInput",
        "args": { "input": { "from": "n0" } }
      },
      "else": { "from": "n0" }
    },
    {
      "id": "n3",
      "type": "reply",
      "message": { "from": "n2" },
      "config": {}
    }
  ]
}
```

### 17.3 Pre-Fetch Before Reply

Source:

```auwgent
agent Hello(input: Text): Text {
    if input.contains("weather") {
        let fetchedData = getweather(input)
        reply({ input, fetchedData }) with { ... }
    }

    reply(input) with { ... }
}
```

Graph should preserve the branch:

```json
{
  "nodes": [
    { "id": "n0", "type": "input" },
    {
      "id": "n1",
      "type": "expression",
      "op": "contains",
      "args": [{ "from": "n0" }, { "literal": "weather" }]
    },
    {
      "id": "n2",
      "type": "branch",
      "condition": { "from": "n1" },
      "then": "n3",
      "else": "n6"
    },
    {
      "id": "n3",
      "type": "tool_call",
      "tool": "getweather",
      "args": { "query": { "from": "n0" } },
      "checkpoint": "required"
    },
    {
      "id": "n4",
      "type": "object",
      "fields": {
        "input": { "from": "n0" },
        "fetchedData": { "from": "n3" }
      }
    },
    {
      "id": "n5",
      "type": "reply",
      "message": { "from": "n4" },
      "config": {}
    },
    {
      "id": "n6",
      "type": "reply",
      "message": { "from": "n0" },
      "config": {}
    }
  ]
}
```

### 17.4 Agent Routing

Source:

```auwgent
agent Main(input: Text): Text {
   let inputType = Analyze(input)

   if inputType.data.includes("high") {
        return One(input)
   }

   return Two(input)
}
```

Graph:

```json
{
  "nodes": [
    { "id": "n0", "type": "input" },
    {
      "id": "n1",
      "type": "agent_call",
      "agent": "Analyze",
      "args": { "input": { "from": "n0" } },
      "mode": "black_box",
      "checkpoint": "required"
    },
    {
      "id": "n2",
      "type": "expression",
      "op": "includes",
      "args": [
        { "from": "n1", "path": ["data"] },
        { "literal": "high" }
      ]
    },
    {
      "id": "n3",
      "type": "branch",
      "condition": { "from": "n2" },
      "then": "n4",
      "else": "n5"
    },
    {
      "id": "n4",
      "type": "agent_call",
      "agent": "One",
      "args": { "input": { "from": "n0" } },
      "mode": "black_box",
      "checkpoint": "required"
    },
    {
      "id": "n5",
      "type": "agent_call",
      "agent": "Two",
      "args": { "input": { "from": "n0" } },
      "mode": "black_box",
      "checkpoint": "required"
    },
    {
      "id": "n6",
      "type": "output",
      "value": {
        "type": "branch_result",
        "branch": "n3"
      }
    }
  ]
}
```

## 18. Mapping Current Canonical Agent

The current canonical agent:

- has one text input,
- has context fields,
- exposes `get_location` and `get_marks`,
- exposes workflow `marks_and_location`,
- exposes helpers `Planner` and `Joker`,
- exposes custom intent `Loud`,
- uses default model config.

In the graph IR, those become:

- definitions for model, tools, workflow/function, agents, and intent,
- one `RuntimeTest` graph,
- a `reply` node that exposes tools/functions/agents/intents to the model,
- separate graphs for `Planner`, `Joker`, and `marks_and_location`.

Approximate compiled graph:

```json
{
  "graphId": "agent:RuntimeTest",
  "entryNode": "n0",
  "returnNode": "n3",
  "nodes": [
    { "id": "n0", "type": "input", "inputType": null },
    { "id": "n1", "type": "context" },
    {
      "id": "n2",
      "type": "reply",
      "message": { "from": "n0" },
      "context": { "from": "n1" },
      "config": {
        "model": { "ref": "DefaultModel" },
        "prompt": {
          "type": "template",
          "parts": [
            {
              "type": "literal",
              "value": "You are a helpful test assistant. You have access to tools and helpers..."
            }
          ]
        },
        "toolProtocol": "block",
        "tools": [
          { "ref": "get_location" },
          { "ref": "get_marks" },
          { "ref": "marks_and_location", "kind": "function_tool" }
        ],
        "agents": [
          { "ref": "Planner", "handoff": "return" },
          { "ref": "Joker", "handoff": "user" }
        ],
        "intents": [
          { "ref": "Loud" }
        ],
        "maxTurns": 12
      },
      "checkpoint": "required"
    },
    {
      "id": "n3",
      "type": "output",
      "value": { "from": "n2" }
    }
  ],
  "edges": [
    { "from": "n0", "to": "n2", "slot": "message" },
    { "from": "n1", "to": "n2", "slot": "context" },
    { "from": "n2", "to": "n3", "slot": "value" }
  ]
}
```

The workflow `marks_and_location` can become a function graph:

```json
{
  "graphId": "function:marks_and_location",
  "entryNode": "n0",
  "returnNode": "n4",
  "nodes": [
    { "id": "n0", "type": "params" },
    {
      "id": "n1",
      "type": "tool_call",
      "tool": "get_marks",
      "args": {
        "id": { "from": "n0", "path": ["user_id"] }
      },
      "checkpoint": "required"
    },
    {
      "id": "n2",
      "type": "tool_call",
      "tool": "get_location",
      "args": {},
      "checkpoint": "required"
    },
    {
      "id": "n3",
      "type": "template",
      "parts": [
        { "literal": "Location: " },
        { "from": "n2" },
        { "literal": "\nMarks: " },
        { "from": "n1" }
      ]
    },
    {
      "id": "n4",
      "type": "return",
      "value": { "from": "n3" }
    }
  ]
}
```

## 19. Open Design Decisions

### 19.1 Context Access

The proposal currently allows context fields directly in scope:

```auwgent
if isAdmin { ... }
```

This creates collision pressure with local variables.

Recommended long-term shape:

```auwgent
ctx.isAdmin
ctx.user_id
```

If direct context access remains, the checker must reject any local binding that collides with context at any nested scope.

### 19.2 Output Defaults

The notes suggest that `Text` may be a default output even when an agent declares `Image`.

This should be clarified before implementation. The simpler and safer rule is:

- omitted output means `Text`,
- `: Image` means only `Image`,
- `: Text | Image` means either text or image.

Hidden unions will make type checking and generated SDK signatures harder.

### 19.3 Graph Granularity

The compiler can choose coarse or fine nodes.

Coarse:

- one node for an entire function body.

Fine:

- one node per let/call/branch/reply.

Recommendation: external boundaries must always be nodes. Pure expressions can be grouped until debugging, optimization, or replay needs require finer nodes.

### 19.4 Transparent Agent Calls

`with turns` needs exact semantics.

Possible meanings:

- expose child turns in parent final session,
- inline child graph into parent state,
- attach child trace while preserving parent graph boundaries.

Recommendation: start with trace attachment. It preserves immutable graph boundaries and still gives observability.

### 19.5 Standard Library Execution

The notes mention a future standard library:

```auwgent
let response = fetch<string>(".../location")
```

This raises a major runtime question:

- Does `fetch` execute in the Rust runtime?
- Does it compile to target-language helper code?
- Does it use a portable interpreter?

For resumability, standard library functions that touch the outside world should be effectful graph nodes with checkpoints.

## 20. Migration Path

Recommended migration path:

1. Keep current canonical JSON support.
2. Introduce graph IR as `irVersion: "2"` or `kind: "auwgent.graph"`.
3. Compile old section-oriented agents into a simple graph with one main `reply` node.
4. Compile old workflows into function graphs.
5. Compile helpers into agent graphs.
6. Add execution state serialization beside the existing session export/import.
7. Make runtime execution graph-aware while preserving current block/native intent behavior inside `reply` nodes.
8. Gradually lower new language constructs into graph nodes.

This allows the runtime to become resumable before the whole new DSL surface is finished.

## 21. Summary

The old IR describes an agent's capabilities. The proposed IR describes an executable graph.

The important change is not just syntax. It is addressability:

- every reply has a node id,
- every external tool call has a node id,
- every child agent call has a node id,
- every branch can be resumed,
- every long-running model loop has active state,
- every run can be reconstructed from immutable IR plus serialized state.

This gives Auwgent a stronger runtime model:

```text
source DSL
  -> typed AST
  -> executable graph IR
  -> immutable compiled artifact
  -> per-run execution state
  -> resumable scheduler
```

The compiled IR is the blueprint. The execution state is the bookmark.
