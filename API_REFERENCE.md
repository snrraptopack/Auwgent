# Auwgent API Reference

> **Quick Overview**: This guide covers all APIs available in the Auwgent framework. Each section provides a brief introduction - refer to detailed documentation for in-depth explanations.

---

## Table of Contents

1. [Agent Creation & Configuration](#1-agent-creation--configuration)
2. [Execution Methods](#2-execution-methods)
3. [Streaming APIs](#3-streaming-apis)
4. [Context Management](#4-context-management)
5. [Type System](#5-type-system)
6. [Tools & Workflows](#6-tools--workflows)
7. [Helpers (Sub-Agents)](#7-helpers-sub-agents)
8. [Lifecycle Hooks](#8-lifecycle-hooks)
9. [Model Configuration](#9-model-configuration)
10. [Generated Types](#10-generated-types)

---

## 1. Agent Creation & Configuration

### `createAgent(config)`

Creates a type-safe agent instance with unified configuration.

**Signature:**
```typescript
function createAgent(config: AgentConfig): Agent
```

**Config Interface:**
```typescript
interface AgentConfig {
    apiKeys: ApiKeys;           // Required: API keys for LLM providers
    ir: AgentIR;                // Required: Agent IR (from .agent.json)
    context?: Context;          // Optional: Bind context at creation
    tools?: Tools;              // Optional: Bind tools at creation
    lifecycle?: Lifecycle;      // Optional: Memory management hooks
}
```

**Example:**
```typescript
const agent = createTypeSystemTest({
    apiKeys: { geminiApiKey: '...' },
    ir: agentIR,
    context: { sessionId: "123" },
    tools: { myTool: async (args) => {...} },
    lifecycle: { prune, load, save }
});
```

**Key Features:**
- ✅ Validates IR structure immediately
- ✅ Validates tools match IR requirements
- ✅ Validates lifecycle hooks if required
- ✅ Fails fast with clear error messages

**See Also:** [DX_IMPROVEMENTS.md](./DX_IMPROVEMENTS.md)

---

## 2. Execution Methods

### `agent.run(input, overrides?)`

Executes the agent and returns the final result.

**Signature:**
```typescript
async run(input: Input, overrides?: Overrides): Promise<Output>
```

**Example:**
```typescript
const result = await agent.run({ message: "Hello" });
console.log(result.analysis);
```

**With Overrides:**
```typescript
const result = await agent.run(
    { message: "Hello" },
    { context: { sessionId: "override" } }
);
```

**Behavior:**
- Executes agent with bound configuration
- Handles tool calls automatically
- Returns structured output matching schema
- Throws on validation errors

---

## 3. Streaming APIs

### `agent.stream(input, overrides?)`

Fluent streaming API with callback-based event handling.

**Signature:**
```typescript
stream(input: Input, overrides?: Overrides): StreamBuilder
```

**Example:**
```typescript
const result = await agent.stream({ message: "Hello" })
    .onChunk(delta => console.log(delta))
    .onToolStart(name => console.log(`Tool: ${name}`))
    .onToolResult((name, result) => console.log(name, result))
    .onToolEnd(name => console.log(`Done: ${name}`))
    .onHelperStart(name => console.log(`Helper: ${name}`))
    .onHelperEnd((name, result) => console.log(name, result))
    .run();
```

**Available Callbacks:**
- `onChunk(delta: string)` - Text chunks as they arrive
- `onToolStart(name: string)` - Tool execution starts
- `onToolArgs(name: string, args: any)` - Tool arguments
- `onToolResult(name: string, result: any)` - Tool result
- `onToolEnd(name: string)` - Tool execution ends
- `onHelperStart(name: string)` - Helper (sub-agent) starts
- `onHelperEnd(name: string, result: any)` - Helper completes

**Returns:** Final structured output after streaming completes

---

### `agent.streamIterable(input, overrides?)`

Native async iteration over stream chunks.

**Signature:**
```typescript
streamIterable(input: Input, overrides?: Overrides): AsyncGenerator<StreamChunk>
```

**Example:**
```typescript
for await (const chunk of agent.streamIterable({ message: "Hello" })) {
    if (chunk.type === 'text') {
        process.stdout.write(chunk.delta);
    }
    if (chunk.type === 'tool_result') {
        console.log(`Tool: ${chunk.name}`, chunk.result);
    }
    if (chunk.type === 'helper_start') {
        console.log(`Helper started: ${chunk.name}`);
    }
}
```

**Chunk Types:**
```typescript
type StreamChunk =
    | { type: 'text'; delta: string }
    | { type: 'tool_start'; name: string }
    | { type: 'tool_args'; name: string; args: any }
    | { type: 'tool_result'; name: string; result: any }
    | { type: 'tool_end'; name: string }
    | { type: 'helper_start'; name: string }
    | { type: 'helper_chunk'; name: string; chunk: StreamChunk }
    | { type: 'helper_end'; name: string; result: any }
    | { type: 'transfer'; mode: 'direct' | 'thenContinue'; helperName: string }
```

**Use Case:** When you need fine-grained control over stream processing

---

## 4. Context Management

### `agent.forContext(context)`

Creates a new agent instance with bound context for multi-turn conversations.

**Signature:**
```typescript
forContext(context: Context): BoundAgent
```

**Example:**
```typescript
// Bind session once
const sessionAgent = agent.forContext({ sessionId: "user-123" });

// Multiple calls with same context
const result1 = await sessionAgent.run({ message: "My name is John" });
const result2 = await sessionAgent.run({ message: "What's my name?" });
const result3 = await sessionAgent.run({ message: "Tell me a joke" });
```

**Use Case:** Multi-turn conversations where context should persist

**Benefits:**
- No need to pass context every call
- Cleaner code for conversational flows
- Each bound agent maintains separate context

**Multiple Sessions:**
```typescript
const user1 = agent.forContext({ sessionId: "user-1" });
const user2 = agent.forContext({ sessionId: "user-2" });

await user1.run({ message: "I like pizza" });
await user2.run({ message: "I like burgers" });
// Each maintains separate context
```

---

## 5. Type System

### Type Declarations

Define reusable types in your DSL.

**Syntax:**
```
type TypeName {
    property: type
    optionalProp?: type
}

output type OutputType {
    field: type @desc "Description"
}
```

**Example:**
```
type Point {
    x: number
    y: number
}

type User {
    id: string
    name: string
    address: Address  // Type reference
}

output type AnalysisResult {
    summary: string @desc "High-level summary"
    confidence: number @desc "Score between 0 and 1"
    keyFindings: string[] @desc "List of findings"
}
```

**Supported Types:**
- **Primitives**: `string`, `number`, `boolean`
- **Arrays**: `string[]`, `Point[]`, `{ x: number, y: number }[]`
- **Type References**: `User`, `Address`, `Point`
- **Unions**: `"fast" | "thorough"`, `"red" | "green" | "blue"`
- **Inline Objects**: `{ title: string, url: string }`
- **Optional**: `zipCode?: string`

**Type Resolution:**
- Type references are recursively resolved to full JSON schemas
- Descriptions preserved at all levels
- Optional fields handled correctly in `required` arrays

**See Also:** [ENHANCED_TYPE_SYSTEM_SUMMARY.md](./ENHANCED_TYPE_SYSTEM_SUMMARY.md)

---

## 6. Tools & Workflows

### Tools

External functions the agent can call.

**DSL Syntax:**
```
tool functionName(param1: type, param2: type): returnType
@desc "Tool description"
```

**TypeScript Implementation:**
```typescript
const tools = {
    calculateDistance: async ({ p1, p2 }: { p1: Point; p2: Point }) => {
        return Math.sqrt((p2.x - p1.x) ** 2 + (p2.y - p1.y) ** 2);
    }
};

const agent = createAgent({
    apiKeys,
    ir,
    tools  // Bind at creation
});
```

**Validation:**
- Tools are validated against IR at creation time
- Missing tools throw clear error messages
- Type-safe tool signatures

---

### Workflows

Multi-step logic defined in DSL, executed by the agent.

**DSL Syntax:**
```
workflow workflowName(param: type): returnType {
    let result = toolCall(param)
    if result == "success" {
        return "Done"
    } else {
        return "Failed"
    }
}
@desc "Workflow description"
```

**Features:**
- Variables, conditionals, tool calls
- Helper calls (sub-agents)
- Transfer statements (delegate to helper)
- Return values

**Execution:**
- Workflows appear as tools to the LLM
- Agent decides when to call them
- Executed by `WorkflowRunner`

---

## 7. Helpers (Sub-Agents)

Helpers are sub-agents that can be called by the main agent or workflows.

**DSL Syntax:**
```
helper HelperName {
    input { ... }
    output { ... }
    tools { ... }
    default config { ... }
}
@desc "Helper description"

agent MainAgent {
    helpers {
        HelperName with tools [tool1, tool2]
        // or
        HelperName with all tools
    }
}
```

**Transfer Statements:**
```
workflow delegateTask(request: string): string {
    transfer HelperName(request)  // Direct transfer
    // or
    transfer HelperName(request) then continue  // Helper delivers, agent can add summary
}
```

**Behavior:**
- Helpers are cached for performance
- Can be granted parent tools
- Support streaming
- Can transfer control (direct or thenContinue)

---

## 8. Lifecycle Hooks

Memory management for conversational agents.

**Interface:**
```typescript
interface Lifecycle<Context, Output> {
    prune: (args: {
        context: Context;
        agent: Agent;
        usage: {
            currentTokens: number;
            maxTokens: number;
            currentMessages: number;
            maxMessages: number;
        };
    }) => Promise<ConversationState>;
    
    load: (args: {
        context: Context;
    }) => Promise<ConversationState>;
    
    save: (args: {
        newMessages: SyntheticMessage[];
        context: Context;
        output: Output;
    }) => Promise<void>;
}
```

**Example:**
```typescript
const lifecycle = {
    prune: async ({ context, usage }) => {
        // Decide what to keep in memory
        if (usage.currentTokens > usage.maxTokens * 0.8) {
            // Summarize or remove old messages
        }
        return { messages: [] };
    },
    
    load: async ({ context }) => {
        // Load conversation history from database
        const messages = await db.getMessages(context.sessionId);
        return { messages };
    },
    
    save: async ({ newMessages, context, output }) => {
        // Save new messages to database
        await db.saveMessages(context.sessionId, newMessages);
    }
};
```

**DSL Configuration:**
```
agent MyAgent {
    use lifecycle {
        maxTokens: 100000
        maxMessages: 100
    }
}
```

**Validation:**
- If lifecycle enabled in DSL, hooks must be provided
- Throws clear error if missing

---

## 9. Model Configuration

### Providers

Supported LLM providers:
- **Gemini**: `gemini("model-name")`
- **OpenAI**: `openai("model-name")`
- **Custom**: `custom("url", "model-name")`

**DSL Syntax:**
```
agent MyAgent {
    default config {
        model: gemini("gemini-2.0-flash")
        prompt: "You are a helpful assistant."
    }
    
    config "advanced" {
        model: openai("gpt-4")
        prompt: "You are an expert analyst."
    }
}
```

**TypeScript Usage:**
```typescript
// Use default config
await agent.run(input);

// Use named config
await agent.run(input, { configName: "advanced" });
```

**Driver Auto-Creation:**
- Factory automatically creates required drivers
- Based on models used in IR
- API keys validated at creation

---

## 10. Generated Types

### Type-Safe Interfaces

The CLI generates TypeScript interfaces for:
- **Input**: `AgentNameInput`
- **Output**: `AgentNameOutput`
- **Context**: `AgentNameContext`
- **Tools**: `AgentNameTools`
- **Lifecycle**: `AgentNameLifecycle`
- **Config**: `AgentNameConfig`
- **Custom Types**: All DSL type declarations

**Example:**
```typescript
// Generated from DSL
export interface Point {
    x: number;
    y: number;
}

export interface AnalysisResult {
    /** High-level summary of findings */
    summary: string;
    /** Confidence score between 0 and 1 */
    confidence: number;
    /** List of key findings */
    keyFindings: string[];
}

export interface TypeSystemTestInput {
    message: string;
}

export interface TypeSystemTestOutput {
    analysis: AnalysisResult;
    searchResults: SearchResult;
}
```

**Benefits:**
- Full TypeScript type safety
- IntelliSense support
- Compile-time validation
- JSDoc comments from `@desc`

---

## Quick Reference

### Common Patterns

**1. Simple Agent:**
```typescript
const agent = createAgent({ apiKeys, ir });
const result = await agent.run(input);
```

**2. Agent with Tools:**
```typescript
const agent = createAgent({ apiKeys, ir, tools });
const result = await agent.run(input);
```

**3. Streaming:**
```typescript
await agent.stream(input)
    .onChunk(c => console.log(c))
    .run();
```

**4. Multi-turn:**
```typescript
const session = agent.forContext({ sessionId: "123" });
await session.run({ message: "First" });
await session.run({ message: "Second" });
```

**5. Async Iteration:**
```typescript
for await (const chunk of agent.streamIterable(input)) {
    if (chunk.type === 'text') console.log(chunk.delta);
}
```

---

## Next Steps

- **Getting Started**: See `javascript/example-usage.ts`
- **Type System**: See `ENHANCED_TYPE_SYSTEM_SUMMARY.md`
- **DX Improvements**: See `DX_IMPROVEMENTS.md`
- **Testing**: See `javascript/TESTING_SCHEMA.md`
- **Changelog**: See `changelog/2026-01-27.md`

---

## Support

For detailed documentation on specific topics:
1. Read the relevant section above
2. Check the linked documentation files
3. Review example code in `javascript/example-usage.ts`
4. Inspect generated types in `.agent.types.ts` files
