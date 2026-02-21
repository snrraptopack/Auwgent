# Auwgent API Reference

> **Quick Overview**: This guide covers all APIs available in the Auwgent framework, leveraging the new Rust-powered runtime and TypeScript bindings.

---

## Table of Contents

1. [Agent Creation & Configuration](#1-agent-creation--configuration)
2. [Execution Methods](#2-execution-methods)
3. [Streaming Events](#3-streaming-events)
4. [Middleware](#4-middleware)
5. [Type System](#5-type-system)
6. [Tools & Workflows](#6-tools--workflows)
7. [Helpers (Sub-Agents)](#7-helpers-sub-agents)
8. [Model Configuration](#8-model-configuration)
9. [Generated Types](#9-generated-types)

---

## 1. Agent Creation & Configuration

### `createAuwgent(ir, config)`

Creates a type-safe agent instance using the highly optimized Rust-backed runtime.

**Signature:**
```typescript
function createAuwgent<IR extends AgentIRShape>(
    ir: IR,
    config: AuwgentConfig<IR>
): TypedAuwgent<IR>
```

**Config Interface:**
```typescript
interface AuwgentConfig<IR> {
    tools: ToolRegistry<IR>;            // Required: Bind tools matching the IR
    middleware?: Middleware<IR>[];      // Optional: Middleware plugins
    context?: Record<string, unknown>;  // Optional: Bind initial context
    apiKeys?: ApiKeys;                  // Optional: API keys (e.g. { geminiApiKey })
    geminiApiKey?: string;              // Optional: Shortcut for Gemini API key
}
```

**Example:**
```typescript
import { createAuwgent, parseIR } from '@auwgent/runtime';
import irJson from './output/main.agent.json';

const agent = createAuwgent(parseIR(JSON.stringify(irJson)), {
    geminiApiKey: process.env.GEMINI_API_KEY,
    tools: {
        myTool: async (args) => { /* implementation */ }
    }
});
```

### `createAuwgentFromIRJson(irJson, config)`

An alternative helper that takes the raw IR JSON string directly.

---

## 2. Execution Methods

### `agent.run(input?)`

Executes the agent for a single turn and returns the updated `SessionState`.

**Signature:**
```typescript
async run(input?: string): Promise<SessionState>
```

**Example:**
```typescript
const session = await agent.run("Hello, what can you do?");
console.log(session.turns[session.turns.length - 1].model_response);
```

### Session Management

The runtime manages temporal conversational state internally in a lightweight structure.

- `exportSession(): SessionState` - Export the current session for persistence (e.g. database).
- `importSession(session: SessionState): void` - Restore a previously exported session.
- `clearSession(): void` - Reset the conversation trace to start fresh.

**Example:**
```typescript
const savedSession = agent.exportSession();
// ... later in another request ...
agent.importSession(savedSession);
await agent.run("Continue from where we left off");
```

---

## 3. Streaming Events

Auwgent's Rust runtime streams "intents" in real-time as the YAML output is parsed over the wire.

### `agent.onIntent(handler)` & `agent.onHandlers(handlers)`

Captures complete, fully-parsed intent blocks (e.g., when a tool finishes parsing). 

**Example:**
```typescript
agent.onHandlers({
    response_text: async (value) => {
        process.stdout.write(value.text);
    },
    tool_call: async (value) => {
        if (value.type === 'myTool') {
            console.log("Tool myTool executing with args:", value.args);
        }
    }
});
```

### `agent.onIntentPartial(handler)` & `agent.onHandlersPartial(handlers)`

Captures streaming delta updates while an intent is actively generating. Useful for building real-time UI components before the LLM finishes outputting the entire payload.

---

## 4. Middleware

Middleware intercept the execution lifecycle of the agent, allowing for context compaction, tracing, metrics, and advanced error handling.

**Interface:**
```typescript
export interface Middleware<IR> {
    name: string;
    onRunStart?: (session: SessionState, ctx: MiddlewareContext) => SessionState | Promise<SessionState>;
    onLLMStart?: (prompt: string, ctx: MiddlewareContext) => void | Promise<void>;
    onIntent?: MiddlewareIntentHandler<IR>;
    onLLMEnd?: (response: string, ctx: MiddlewareContext) => void | Promise<void>;
    onRunComplete?: (finalSession: SessionState, ctx: MiddlewareContext) => void | Promise<void>;
    onError?: (error: Error, session: SessionState, ctx: MiddlewareContext) => boolean | Promise<boolean> | void;
}
```

**Example:**
```typescript
const loggingMiddleware: Middleware<any> = {
    name: 'logger',
    onRunStart: (session) => {
        console.log("Starting run with", session.turns.length, "turns");
        return session; // Must return the session (can be mutated for pruning)
    }
};

const agent = createAuwgent(ir, {
    tools,
    middleware: [loggingMiddleware]
});
```

---

## 5. Type System

### Type Declarations

Define reusable types natively in the Auwgent DSL.

**Syntax:**
```typescript
type Point {
    x: number
    y: number
}

output type AnalysisResult {
    summary: string @desc "High-level summary"
    confidence: number @desc "Score between 0 and 1"
}
```

**Supported Features:**
- **Primitives**: `string`, `number`, `boolean`
- **Arrays**: `string[]`, `Point[]`
- **Type References**: `User`, `Address`, `Point`
- **Unions**: `"fast" | "thorough"`, `"red" | "green" | "blue"`
- **Inline Objects**: `{ title: string, url: string }`
- **Optional Fields**: `zipCode?: string`

---

## 6. Tools & Workflows

### Tools

External functions the agent can call, enforced by the compiler.

**DSL Syntax:**
```typescript
tool calculateDistance(p1: Point, p2: Point): number
@desc "Finds the distance"
```

**TypeScript Implementation:**
```typescript
const agent = createAuwgent(ir, {
    tools: {
        calculateDistance: async ({ p1, p2 }) => {
            return Math.sqrt((p2.x - p1.x) ** 2 + (p2.y - p1.y) ** 2);
        }
    }
});
```

### Workflows

Deterministic multi-step logic baked directly into the DSL. Workflows execute locally in the engine, eliminating unnecessary LLM round-trips while maintaining full access to tools and helpers.

**DSL Syntax:**
```typescript
workflow verifyUser(id: string): boolean {
    let result = checkDb(id)
    if result == "found" {
        return true
    } else {
        return false
    }
}
@desc "Workflow description"
```

---

## 7. Helpers (Sub-Agents)

Delegation to specialized sub-agents.

**DSL Syntax:**
```typescript
helper SearchHelper {
    input { query: string }
    output { results: string[] }
    tools [ searchWeb ]
}

agent MainAgent {
    helpers { SearchHelper }
}
```

Helpers run completely in the Rust engine! The engine dynamically routes state and tools between agents, preserving determinism and context without the latency of network-bound coordination.

---

## 8. Model Configuration

**Providers Configuration DSL:**
```typescript
agent MyAgent {
    default config {
        model: gemini("gemini-2.0-flash")
        prompt: "You are a helpful assistant."
    }
}
```

The underlying Rust engine actively uses this config internally to initialize native providers dynamically.

---

## 9. Generated Types

The CLI generates strong TypeScript typing based on your `.agent` files so the integration with your TS/Node host environment is strictly verified:
- **Input**: Strongly-typed payload to the agent.
- **Output**: Typed payload extraction from responses.
- **Tools**: Generates a signature-perfect interface block your bound TS tools must conform to.

*Full robust completion is ready out-of-the-box in your IDE.*
