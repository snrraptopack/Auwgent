# Middleware Layer Guide

**Date**: 2026-01-30  
**Status**: In Progress

---

## Overview

The middleware layer lets you observe and control agent execution without modifying drivers or core runtime logic. Middleware runs in a defined order and can:

- inspect or mutate model requests and responses
- intercept model and tool execution
- collect telemetry or audit trails
- implement retries or policy gates

---

## Execution Flow

Middleware hooks run in this order during a typical agent run:

1. `onAgentStart`
2. `onBeforeModel`
3. `wrapModelCall`
4. `onThinking` (only if the provider emits a reasoning block)
5. `onAfterModel`
6. `onBeforeTool` (per tool call)
7. `wrapToolCall` (per tool call)
8. `onAfterTool` (per tool call)
9. `onError` (when a phase throws)
10. `onAgentEnd`

`wrapModelCall` and `wrapToolCall` are composed right-to-left, so higher priority numbers wrap earlier middleware.

---

## Context and State

Every hook receives a `MiddlewareContext` that includes:

- `agentName`, `runId`, `attempt`, `startedAt`
- `input` and optional `userContext`
- `request` and `response`
- `state`, a mutable object shared across all hooks for a single run

You can seed state by passing `middlewareState` when calling `agent.run()`. This is per-run state and not persisted by the runtime.

---

## Hook Behavior

- `onBeforeModel` can return a new `ModelRequest` to replace the outgoing request.
- `wrapModelCall` can retry, short-circuit, or decorate the model call.
- `onThinking` can transform or redact the reasoning block before it reaches the user.
- `onBeforeTool` can block a tool call by returning `false`.
- `wrapToolCall` can modify tool input and output or add tracing.
- `onError` can request retries by returning `{ retry: true, delayMs }`.
- `onAgentEnd` runs once and is ideal for cleanup or final persistence.

---

## Using Middleware

```ts
import { createAuditMiddleware } from "./loader/middleware/AuditMiddleware";

const middleware = [
  createAuditMiddleware({
    includeThinking: false,
    includeToolArgs: true,
    includeToolResults: false
  })
];

const result = await agent.run(input, { middleware });
```

---

## Storage and Persistence

The runtime does not provide a built-in persistence layer for middleware state. If you need storage management, you should implement it inside middleware, typically by:

- writing to an external store in `onAfterTool`, `onAfterModel`, or `onAgentEnd`
- hydrating per-run state via `middlewareState` passed to `agent.run()`
- using `runId` to correlate stored data across calls

---

## Short-Term Memory Example

```ts
import type { AgentMiddleware, MiddlewareContext, SyntheticMessage } from "./loader/types/protocol";

type ShortTermMemoryState = {
  recent: SyntheticMessage[];
  maxMessages: number;
};

export const createShortTermMemoryMiddleware = (
  maxMessages = 20
): AgentMiddleware<any, any, ShortTermMemoryState> => ({
  name: "short_term_memory",
  priority: 20,
  onAgentStart: (ctx: MiddlewareContext<any, any, ShortTermMemoryState>) => {
    ctx.state.recent = [];
    ctx.state.maxMessages = maxMessages;
  },
  onBeforeModel: (ctx: MiddlewareContext<any, any, ShortTermMemoryState>) => {
    const limit = ctx.state.maxMessages ?? maxMessages;
    const pruned = ctx.request.messages.slice(-limit);
    ctx.state.recent = pruned;
    return {
      ...ctx.request,
      messages: pruned
    };
  },
  onAfterModel: (ctx: MiddlewareContext<any, any, ShortTermMemoryState>, res) => {
    const limit = ctx.state.maxMessages ?? maxMessages;
    const assistantMessage: SyntheticMessage | undefined = res.content
      ? { role: "assistant", content: res.content }
      : undefined;
    const next = assistantMessage
      ? [...ctx.state.recent, assistantMessage].slice(-limit)
      : ctx.state.recent;
    ctx.state.recent = next;
    return res;
  }
});
```

---

## Related Files

- `javascript/loader/types/protocol.ts` for core middleware types
- `javascript/loader/IrMiddleware.ts` for hook execution order
- `javascript/loader/IrInterpreter.ts` for middleware wiring in the runtime
- `javascript/loader/middleware/AuditMiddleware.ts` for a production-ready middleware example
