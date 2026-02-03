import type {
    AgentMiddleware,
    ContentBlock,
    MiddlewareContext,
    ModelResponse,
    ToolArgs,
    ToolResult
} from "../types/protocol";

/**
 * Common fields included in every audit event.
 */
export type AuditEventBase = {
    time: number;
    runId: string;
    agentName: string;
    attempt: number;
};

/**
 * Audit events emitted by the middleware for tracing agent execution.
 */
export type AuditEvent =
    | (AuditEventBase & { type: "agent_start" })
    | (AuditEventBase & {
        type: "before_model";
        messageCount: number;
        toolCount: number;
        model?: string;
        modelName?: string;
        temperature?: number;
        toolChoice?: string;
        responseFormat?: string;
        reasoning?: {
            enabled: boolean;
            budgetTokens?: number;
            effort?: "low" | "medium" | "high";
            visible?: boolean;
        };
    })
    | (AuditEventBase & {
        type: "after_model";
        stopReason?: string;
        toolCalls?: { id: string; name: string }[];
        usage?: {
            input: number;
            response: number;
            thinking?: number;
            total: number;
            cachedInput?: number;
        };
        responsePreview?: string;
    })
    | (AuditEventBase & {
        type: "thinking";
        thinking: { text?: string; summary?: string; redacted?: boolean; tokenCount?: number };
    })
    | (AuditEventBase & {
        type: "before_tool";
        tool: { id: string; name: string; args?: ToolArgs };
    })
    | (AuditEventBase & {
        type: "after_tool";
        tool: { id: string; name: string };
        result?: ToolResult;
    })
    | (AuditEventBase & {
        type: "error";
        phase: "model" | "tool" | "thinking" | "helper" | "workflow";
        name: string;
        message: string;
    })
    | (AuditEventBase & {
        type: "agent_end";
        durationMs: number;
        error?: { name: string; message: string };
    });

/**
 * Mutable middleware state container for audit events.
 */
export type AuditState = {
    events: AuditEvent[];
};

/**
 * Configuration options for audit middleware behavior.
 */
export type AuditMiddlewareOptions = {
    includeThinking?: boolean;
    includeToolArgs?: boolean;
    includeToolResults?: boolean;
    includeResponsePreview?: boolean;
    maxContentLength?: number;
};

const normalizeText = (content: ContentBlock[] | string | undefined): string => {
    if (!content) {
        return "";
    }
    if (typeof content === "string") {
        return content;
    }
    return content
        .filter(block => block.type === "text")
        .map(block => block.type === "text" ? block.text : "")
        .join("");
};

const truncate = (value: string, maxLength?: number): string => {
    if (!maxLength || value.length <= maxLength) {
        return value;
    }
    return value.slice(0, maxLength);
};

const createSafeReplacer = () => {
    const seen = new WeakSet<object>();
    return (_key: string, value: unknown) => {
        if (typeof value === "bigint") {
            return value.toString();
        }
        if (typeof value === "function") {
            return "[Function]";
        }
        if (typeof value === "symbol") {
            return value.toString();
        }
        if (value && typeof value === "object") {
            if (seen.has(value as object)) {
                return "[Circular]";
            }
            seen.add(value as object);
        }
        return value;
    };
};

const toAuditValue = (value: unknown, maxLength?: number): ToolResult | undefined => {
    if (value === undefined) {
        return undefined;
    }
    if (value === null || typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
        return value;
    }
    const raw = JSON.stringify(value, createSafeReplacer());
    if (raw === undefined) {
        return undefined;
    }
    if (maxLength && raw.length > maxLength) {
        return truncate(raw, maxLength);
    }
    try {
        return JSON.parse(raw) as ToolResult;
    } catch {
        return raw;
    }
};

const ensureState = (ctx: MiddlewareContext<any, any, AuditState>): AuditState => {
    if (!ctx.state.events) {
        ctx.state.events = [];
    }
    return ctx.state;
};

/**
 * Creates an audit middleware that records lifecycle, model, tool, and error events.
 */
export const createAuditMiddleware = <TInput, TContext>(
    options: AuditMiddlewareOptions = {}
): AgentMiddleware<TInput, TContext, AuditState> => {
    const {
        includeThinking = false,
        includeToolArgs = false,
        includeToolResults = false,
        includeResponsePreview = false,
        maxContentLength = 2000
    } = options;

    const push = (ctx: MiddlewareContext<TInput, TContext, AuditState>, event: AuditEvent) => {
        ensureState(ctx).events.push(event);
    };

    return {
        name: "audit",
        priority: 10,
        onAgentStart: (ctx) => {
            push(ctx, {
                type: "agent_start",
                time: Date.now(),
                runId: ctx.runId,
                agentName: ctx.agentName,
                attempt: ctx.attempt
            });
        },
        onBeforeModel: (ctx) => {
            const toolChoice = typeof ctx.request.toolChoice === "string"
                ? ctx.request.toolChoice
                : ctx.request.toolChoice?.name;
            const responseFormat = ctx.request.responseFormat?.type;

            push(ctx, {
                type: "before_model",
                time: Date.now(),
                runId: ctx.runId,
                agentName: ctx.agentName,
                attempt: ctx.attempt,
                messageCount: ctx.request.messages.length,
                toolCount: ctx.request.tools?.length ?? 0,
                model: ctx.request.config.model,
                modelName: ctx.request.config.modelName,
                temperature: ctx.request.config.temperature,
                toolChoice,
                responseFormat,
                reasoning: ctx.request.reasoning
            });
        },
        onThinking: (ctx, thinking) => {
            if (!includeThinking) {
                return;
            }
            push(ctx, {
                type: "thinking",
                time: Date.now(),
                runId: ctx.runId,
                agentName: ctx.agentName,
                attempt: ctx.attempt,
                thinking
            });
        },
        onAfterModel: (ctx, res) => {
            const responsePreview = includeResponsePreview
                ? truncate(normalizeText(res.content), maxContentLength)
                : undefined;
            push(ctx, {
                type: "after_model",
                time: Date.now(),
                runId: ctx.runId,
                agentName: ctx.agentName,
                attempt: ctx.attempt,
                stopReason: res.stopReason,
                toolCalls: res.toolCalls?.map(call => ({ id: call.id, name: call.name })),
                usage: res.usage,
                responsePreview
            });
            return res;
        },
        onBeforeTool: (ctx, tool) => {
            push(ctx, {
                type: "before_tool",
                time: Date.now(),
                runId: ctx.runId,
                agentName: ctx.agentName,
                attempt: ctx.attempt,
                tool: {
                    id: tool.id,
                    name: tool.name,
                    args: includeToolArgs ? tool.input : undefined
                }
            });
            return true;
        },
        onAfterTool: (ctx, tool, result) => {
            push(ctx, {
                type: "after_tool",
                time: Date.now(),
                runId: ctx.runId,
                agentName: ctx.agentName,
                attempt: ctx.attempt,
                tool: { id: tool.id, name: tool.name },
                result: includeToolResults ? toAuditValue(result, maxContentLength) : undefined
            });
        },
        onError: (ctx, error, phase) => {
            push(ctx, {
                type: "error",
                time: Date.now(),
                runId: ctx.runId,
                agentName: ctx.agentName,
                attempt: ctx.attempt,
                phase,
                name: error.name,
                message: error.message
            });
        },
        onAgentEnd: (ctx, result, error) => {
            const durationMs = Date.now() - ctx.startedAt;
            const response = result as ModelResponse | undefined;
            if (response && includeResponsePreview) {
                const preview = truncate(normalizeText(response.content), maxContentLength);
                if (preview) {
                    push(ctx, {
                        type: "after_model",
                        time: Date.now(),
                        runId: ctx.runId,
                        agentName: ctx.agentName,
                        attempt: ctx.attempt,
                        stopReason: response.stopReason,
                        toolCalls: response.toolCalls?.map(call => ({ id: call.id, name: call.name })),
                        usage: response.usage,
                        responsePreview: preview
                    });
                }
            }
            push(ctx, {
                type: "agent_end",
                time: Date.now(),
                runId: ctx.runId,
                agentName: ctx.agentName,
                attempt: ctx.attempt,
                durationMs,
                error: error ? { name: error.name, message: error.message } : undefined
            });
        }
    };
};
