# Auwgent v2 Graph IR

Status: design reference  
Source notes: `not_graph.txt`, `not.txt`, `not_standard.txt`

Auwgent v2 should compile the language into an immutable graph IR. Runtime progress must live in a separate execution state object.

This is the main architectural shift:

```text
v1:
  AgentIR describes capabilities.
  The engine loop interprets the agent at runtime.

v2:
  Graph IR describes executable steps.
  A graph executor schedules nodes and checkpoints state.
```

## 1. Core Principle

The compiled IR is static.

The execution state is dynamic.

The runtime never mutates the compiled IR.

```text
Compiled Graph IR:
  what exists
  what nodes run
  how values flow
  how control flows

Execution State:
  what has run
  what is running
  what each node produced
  what needs to resume
```

The IR is the blueprint. The state is the bookmark.

## 2. Top-Level IR Shape

```json
{
  "irVersion": "2",
  "kind": "auwgent.graph",
  "program": {
    "name": "SupportAgent",
    "entryAgent": "SupportAgent"
  },
  "definitions": {
    "types": {},
    "models": {},
    "tools": {},
    "functions": {},
    "agents": {},
    "middlewares": {},
    "intents": {}
  },
  "graphs": {
    "agent:SupportAgent": {
      "entryNode": "n0",
      "returnNode": "n9",
      "nodes": [],
      "edges": []
    }
  }
}
```

Definitions describe reusable declarations. Graphs describe executable plans.

## 3. Definitions

Definitions are static. They do not say what has run.

### 3.1 Models

```json
{
  "models": {
    "Gemini": {
      "provider": "gemini",
      "modelName": "gemini-pro",
      "config": {
        "temperature": 0.4
      }
    },
    "Groq": {
      "provider": "groq",
      "modelName": "llama-3.3-70b-versatile",
      "config": null
    }
  }
}
```

Dynamic model routing is represented in node config expressions, not by mutating this definition.

### 3.2 Tools

```json
{
  "tools": {
    "getWeather": {
      "kind": "host",
      "description": "Use this to get weather",
      "params": {
        "city": { "type": "string", "optional": false }
      },
      "returns": "string"
    },
    "userTools": {
      "kind": "group",
      "description": "User profile tools",
      "disclosure": "lazy",
      "members": ["getUsername", "getUserLocation"]
    }
  }
}
```

### 3.3 Functions

```json
{
  "functions": {
    "sanitizeInput": {
      "visibility": "internal",
      "params": { "input": "text" },
      "returns": "text",
      "graph": "function:sanitizeInput"
    },
    "delete_person": {
      "visibility": "tool",
      "description": "Use this to delete a user",
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

### 3.4 Agents

```json
{
  "agents": {
    "SupportAgent": {
      "input": "text",
      "output": "text",
      "context": "SupportContext",
      "graph": "agent:SupportAgent"
    },
    "Planner": {
      "input": "text",
      "output": "text",
      "context": null,
      "graph": "agent:Planner"
    }
  }
}
```

## 4. Graph Nodes

Every important execution unit should have a stable node id.

```json
{
  "graphId": "agent:SupportAgent",
  "entryNode": "n0",
  "returnNode": "n5",
  "nodes": [
    { "id": "n0", "type": "input" },
    { "id": "n1", "type": "context" },
    { "id": "n2", "type": "tool_call", "tool": "getAccount" },
    { "id": "n3", "type": "reply" },
    { "id": "n5", "type": "output" }
  ],
  "edges": [
    { "from": "n0", "to": "n3", "slot": "message" },
    { "from": "n1", "to": "n2", "slot": "args.userId" },
    { "from": "n2", "to": "n3", "slot": "account" },
    { "from": "n3", "to": "n5", "slot": "value" }
  ]
}
```

## 5. Node Types

### 5.1 Boundary Nodes

```json
{ "id": "n0", "type": "input", "inputType": "text" }
{ "id": "n1", "type": "context", "contextType": "SupportContext" }
{ "id": "n9", "type": "output", "value": { "from": "n8" } }
```

### 5.2 Expression Nodes

```json
{
  "id": "n2",
  "type": "expression",
  "op": "contains",
  "args": [
    { "from": "n0" },
    { "literal": "weather" }
  ]
}
```

Expression nodes are replayable unless they use non-deterministic standard library calls.

### 5.3 Branch Nodes

```json
{
  "id": "n3",
  "type": "branch",
  "condition": { "from": "n2" },
  "then": "n4",
  "else": "n7"
}
```

Branches let the executor know which path is active and which path should be skipped.

### 5.4 Tool Call Nodes

```json
{
  "id": "n4",
  "type": "tool_call",
  "tool": "getWeather",
  "args": {
    "city": { "from": "n0" }
  },
  "checkpoint": "required"
}
```

This means agent code is calling a tool directly before a model turn.

### 5.5 Function Call Nodes

```json
{
  "id": "n5",
  "type": "function_call",
  "function": "sanitizeInput",
  "args": {
    "input": { "from": "n0" }
  }
}
```

The compiler may inline pure functions or keep them as child graphs.

### 5.6 Reply Nodes

`reply(...) with { ... }` lowers to a `reply` node.

```json
{
  "id": "n6",
  "type": "reply",
  "message": { "from": "n0" },
  "config": {
    "prompt": {
      "type": "template",
      "parts": [
        { "literal": "Help the user." }
      ]
    },
    "model": { "ref": "Gemini" },
    "fallback": { "ref": "Groq" },
    "retry": 3,
    "maxTurn": 3,
    "tools": [
      { "ref": "getWeather" }
    ],
    "builtin": [
      { "ref": "web_search" }
    ],
    "agents": [
      { "ref": "Planner", "handoff": "return" }
    ]
  },
  "checkpoint": "required"
}
```

The current v1 runtime loop should eventually become the implementation of this node type.

### 5.7 Agent Call Nodes

```json
{
  "id": "n7",
  "type": "agent_call",
  "agent": "Planner",
  "args": {
    "input": { "from": "n0" }
  },
  "mode": "black_box",
  "checkpoint": "required"
}
```

Transparent call:

```json
{
  "id": "n8",
  "type": "agent_call",
  "agent": "Planner",
  "args": {
    "input": { "from": "n0" }
  },
  "mode": "with_turns",
  "checkpoint": "required"
}
```

## 6. Execution State

The execution state is separate from the IR.

```json
{
  "stateVersion": "1",
  "runId": "run_abc123",
  "irHash": "sha256:...",
  "status": "running",
  "entryGraph": "agent:SupportAgent",
  "nodeStatus": {
    "agent:SupportAgent:n0": "done",
    "agent:SupportAgent:n1": "done",
    "agent:SupportAgent:n6": "running"
  },
  "nodeOutputs": {
    "agent:SupportAgent:n0": {
      "data": "hello",
      "error": null
    }
  },
  "activeNodes": {
    "agent:SupportAgent:n6": {
      "kind": "reply",
      "turnCount": 1,
      "transcriptId": "transcript:n6",
      "pendingActions": [],
      "partialResponse": ""
    }
  },
  "transcripts": {},
  "eventLog": []
}
```

## 7. Node Statuses

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

The scheduler uses these statuses to resume after a process restart.

## 8. Reply Active State

Reply nodes need richer active state because they can loop over model calls and tool results.

```json
{
  "kind": "reply",
  "protocol": "block",
  "turnCount": 2,
  "resolvedConfig": {
    "model": { "provider": "gemini", "modelName": "gemini-pro" },
    "prompt": "Help the user.",
    "tools": ["getWeather"],
    "maxTurn": 3
  },
  "transcript": [
    { "role": "system", "content": "Help the user." },
    { "role": "user", "content": "weather in Accra" },
    { "role": "model", "content": "[tool_call: getWeather]\ncity: \"Accra\"\n[/tool_call]" },
    { "role": "tool", "name": "getWeather", "content": "sunny" }
  ],
  "pendingActions": [],
  "partialResponse": ""
}
```

## 9. Resume Algorithm

```text
1. Load immutable graph IR.
2. Load execution state.
3. Verify IR hash or compatible version.
4. Restore running nodes from activeNodes.
5. Reuse done node outputs.
6. Find ready pending nodes.
7. Execute or resume nodes.
8. Save state after every external transition.
9. Complete when return node is done.
```

Pseudocode:

```ts
while (!state.isTerminal()) {
  for (const node of state.runningNodes()) {
    await executor.resumeNode(node)
    await checkpoint.save(state)
  }

  for (const node of executor.readyNodes()) {
    await executor.executeNode(node)
    await checkpoint.saveIfNeeded(state, node)
  }
}
```

## 10. Checkpoint Triggers

Checkpoint after:

- run start,
- node start for external nodes,
- model stream start,
- model emits tool call,
- tool is dispatched,
- tool result is received,
- child agent starts,
- child agent checkpoints,
- middleware changes execution,
- model returns final output,
- node fails,
- run completes.

Pure deterministic expressions can replay, but their outputs may still be stored for debugging.

## 11. Sub-Agent Resumption

Black-box child:

```json
{
  "nodeStatus": {
    "agent:Main:n5": "running"
  },
  "activeNodes": {
    "agent:Main:n5": {
      "kind": "agent_call",
      "agent": "WeatherAgent",
      "mode": "black_box",
      "childState": {
        "entryGraph": "agent:WeatherAgent",
        "nodeStatus": {},
        "nodeOutputs": {},
        "activeNodes": {}
      }
    }
  }
}
```

Transparent child:

```json
{
  "kind": "agent_call",
  "agent": "WeatherAgent",
  "mode": "with_turns",
  "traceRef": "trace:agent:WeatherAgent:run_child_123"
}
```

## 12. Decided Direction

- IR is immutable.
- Execution state is separate and mutable.
- Every external or long-running step must have a stable node id.
- `reply` is a resumable node.
- Tool calls and agent calls are resumable nodes.
- Old workflows/functions become graph bodies.
- Session export becomes a view over execution state, not the source of truth.

## 13. Still Deciding

- Exact node schema naming.
- Whether transparent child calls inline nodes or attach trace references.
- How much pure expression output to persist by default.
- Whether middleware lives in graph IR or a separate AST/bytecode section.
- Exact checkpoint storage interface.
- How v1 `AgentIR` compatibility is represented in the v2 compiler.
