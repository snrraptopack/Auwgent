# Auwgent Middleware + Reasoning Proposal

## Goals
- Provide a provider-agnostic message format that supports tools and reasoning.
- Add a middleware layer for retries, auditing, safety, and policy enforcement.
- Keep reasoning/thinking separate from user-visible history by default.
- Avoid `any` in public-facing types and middleware arguments.

## Normalized Types

### JsonValue
Used for provider-neutral JSON-like data without `any`.

```ts
type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [k: string]: JsonValue };
```

### Content Blocks
All model content is normalized into blocks to enable tool and reasoning handling.

```ts
type ToolArgs = Record<string, JsonValue>;
type ToolResult = JsonValue | Record<string, JsonValue>;

type ContentBlock =
  | { type: 'text'; text: string }
  | { type: 'thinking'; text?: string; summary?: string; redacted?: boolean; tokenCount?: number }
  | { type: 'tool_use'; id: string; name: string; input: ToolArgs }
  | { type: 'tool_result'; toolUseId: string; content: ContentBlock[] | string; isError?: boolean };
```

### Message
Normalized for all providers and supports structured content.

```ts
type Message =
  | { role: 'system' | 'user'; content: ContentBlock[] | string }
  | { role: 'assistant'; content: ContentBlock[]; toolCalls?: { id: string; name: string; args: ToolArgs }[] }
  | { role: 'tool'; content: ContentBlock[] | string; toolCallId: string; name: string };
```

### ModelRequest

```ts
type ModelRequest = {
  model: string;
  messages: Message[];
  tools?: { name: string; description?: string; parameters: JsonValue }[];
  toolChoice?: 'auto' | 'required' | 'none' | { name: string };
  reasoning?: { enabled: boolean; budgetTokens?: number; effort?: 'low' | 'medium' | 'high'; visible?: boolean };
  responseFormat?: { type: 'text' | 'json_object' | 'json_schema'; schema?: JsonValue };
  temperature?: number;
};
```

**ModelRequest args**
- `model`: provider-specific model id.
- `messages`: normalized history (system/user/assistant/tool).
- `tools`: tool definitions for function calling.
- `toolChoice`: strict tool policy or named tool.
- `reasoning`: enables reasoning budgets and tuning for reasoning models.
- `responseFormat`: structured output requirements.
- `temperature`: generation variability.

### ModelResponse

```ts
type ModelResponse = {
  content: ContentBlock[];
  stopReason?: string;
  usage?: {
    input: number;
    response: number;
    thinking?: number;
    total: number;
  };
  raw?: unknown;
};
```

**ModelResponse args**
- `content`: normalized content blocks (text/thinking/tool_use).
- `stopReason`: provider stop reason.
- `usage`: per-request token usage with response, thinking, and total.
- `raw`: provider raw response for debugging.

**Token usage semantics**
- `usage.input`: tokens for the prompt/messages sent to the model.
- `usage.response`: tokens for the user-visible model reply.
- `usage.thinking`: tokens for hidden or summarized thinking, when provided.
- `usage.total`: `input + response + thinking` (treat missing thinking as 0).

## Thinking and Scratchpad

### Concept
- **Thinking** is modeled as `ContentBlock` with type `thinking`.
- **Scratchpad** is a separate internal trace store for tool use and reasoning.
- **User-visible history** excludes thinking blocks unless `reasoning.visible` is true.

### Why this is different from streaming
- Streaming emits transport-level deltas of text/tool args.
- Thinking is semantic content that may be hidden or summarized.
- Middleware uses `onThinking` to normalize, redact, or audit reasoning blocks.

## Middleware API

```ts
type MiddlewareContext<TInput, TContext, TState> = {
  agentName: string;
  runId: string;
  attempt: number;
  startedAt: number;
  state: TState;
  input: TInput;
  userContext?: TContext;
  request: ModelRequest;
  response?: ModelResponse;
};

type AgentMiddleware<TInput, TContext, TState> = {
  name: string;
  priority?: number;

  onAgentStart?: (ctx: MiddlewareContext<TInput, TContext, TState>) => void | Promise<void>;
  onBeforeModel?: (ctx: MiddlewareContext<TInput, TContext, TState>) => void | ModelRequest | Promise<void | ModelRequest>;
  wrapModelCall?: (
    ctx: MiddlewareContext<TInput, TContext, TState>,
    next: () => Promise<ModelResponse>
  ) => Promise<ModelResponse>;
  onThinking?: (
    ctx: MiddlewareContext<TInput, TContext, TState>,
    thinking: Extract<ContentBlock, { type: 'thinking' }>
  ) => void | Extract<ContentBlock, { type: 'thinking' }> | Promise<void | Extract<ContentBlock, { type: 'thinking' }>>;
  onAfterModel?: (ctx: MiddlewareContext<TInput, TContext, TState>, res: ModelResponse) => void | ModelResponse | Promise<void | ModelResponse>;
  onBeforeTool?: (ctx: MiddlewareContext<TInput, TContext, TState>, tool: Extract<ContentBlock, { type: 'tool_use' }>) => boolean | void | Promise<boolean | void>;
  wrapToolCall?: (
    ctx: MiddlewareContext<TInput, TContext, TState>,
    tool: Extract<ContentBlock, { type: 'tool_use' }>,
    next: (args: ToolArgs) => Promise<ToolResult>
  ) => Promise<ToolResult>;
  onAfterTool?: (ctx: MiddlewareContext<TInput, TContext, TState>, tool: Extract<ContentBlock, { type: 'tool_use' }>, result: ToolResult) => void | Promise<void>;
  onError?: (
    ctx: MiddlewareContext<TInput, TContext, TState>,
    error: Error,
    phase: 'model' | 'tool' | 'thinking'
  ) => { retry: boolean; delayMs?: number } | void | Promise<{ retry: boolean; delayMs?: number } | void>;
  onAgentEnd?: (ctx: MiddlewareContext<TInput, TContext, TState>, result?: ModelResponse, error?: Error) => void | Promise<void>;
};
```

### Middleware args usage
- `ctx.agentName`: routing and per-agent policies.
- `ctx.runId`: tracing and correlation across steps.
- `ctx.attempt`: retry logic and backoff.
- `ctx.startedAt`: latency tracking.
- `ctx.state`: mutable middleware state across hooks.
- `ctx.input`: original user input, typed.
- `ctx.userContext`: external context or memory input, typed.
- `ctx.request`: normalized request for safe modification.
- `ctx.response`: normalized response for auditing or mutation.

### Hook behaviors
- `onBeforeModel`: edit or replace the request.
- `wrapModelCall`: intercept execution, retry/fallback, or short-circuit.
- `onThinking`: sanitize or summarize thinking content.
- `onBeforeTool`: allow/deny tool execution.
- `wrapToolCall`: instrument or modify tool calls and outputs.
- `onError`: decide retries and delays by phase.

## Provider Normalization

### OpenAI
- Map `tool_use` to `tool_calls` with ids.
- Convert `tool_result` to tool role messages.
- If reasoning model supports effort, map from `reasoning.effort`.

### Gemini
- Map `tool_use` to `functionCall` parts.
- Convert `tool_result` to `functionResponse` parts.
- Use `systemInstruction` for system messages.

## Next Steps
- Update normalized types in runtime and drivers.
- Add scratchpad storage for tool calls + reasoning blocks.
- Implement middleware chain with typed generics.
