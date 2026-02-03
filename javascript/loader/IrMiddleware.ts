import type { AgentMiddleware, ModelRequest, ModelResponse, MiddlewareContext, ToolArgs, ToolResult, ToolUseBlock, HelperSkipResult, StepSkipResult, WorkflowResumeResult } from "./types/protocol";

export const sortMiddlewares = (middlewares?: AgentMiddleware<any, any, any>[]): AgentMiddleware<any, any, any>[] => {
    if (!middlewares || middlewares.length === 0) {
        return [];
    }
    return [...middlewares].sort((a, b) => (a.priority ?? 0) - (b.priority ?? 0));
};

export const createMiddlewareContext = <TInput, TContext>(
    agentName: string,
    input: TInput,
    runContext: TContext | undefined,
    request: ModelRequest,
    middlewareState?: Record<string, any>,
    runId?: string
): MiddlewareContext<TInput, TContext, Record<string, any>> => {
    const randomId = typeof globalThis.crypto?.randomUUID === "function"
        ? globalThis.crypto.randomUUID()
        : `${Date.now()}_${Math.random().toString(16).slice(2)}`;
    return {
        agentName,
        runId: runId || randomId,
        attempt: 0,
        startedAt: Date.now(),
        state: middlewareState || {},
        input,
        userContext: runContext,
        request,
        response: undefined
    };
};

export const runOnAgentStart = async (
    middlewares: AgentMiddleware<any, any, any>[],
    ctx: MiddlewareContext<any, any, any>
): Promise<void> => {
    for (const mw of middlewares) {
        if (mw.onAgentStart) {
            await mw.onAgentStart(ctx);
        }
    }
};

export const runOnBeforeModel = async (
    middlewares: AgentMiddleware<any, any, any>[],
    ctx: MiddlewareContext<any, any, any>
): Promise<ModelRequest | undefined> => {
    let modified: ModelRequest | undefined;
    for (const mw of middlewares) {
        if (mw.onBeforeModel) {
            const next = await mw.onBeforeModel(ctx);
            if (next) {
                ctx.request = next;
                modified = next;
            }
        }
    }
    return modified;
};

export const wrapModelCall = (
    middlewares: AgentMiddleware<any, any, any>[],
    ctx: MiddlewareContext<any, any, any>,
    next: () => Promise<ModelResponse>
) => {
    return middlewares
        .filter(mw => mw.wrapModelCall)
        .reduceRight((current, mw) => () => mw.wrapModelCall!(ctx, current), next);
};

export const runOnThinking = async (
    middlewares: AgentMiddleware<any, any, any>[],
    ctx: MiddlewareContext<any, any, any>,
    res: ModelResponse
): Promise<ModelResponse> => {
    if (!res.thinking) {
        return res;
    }
    let thinking = res.thinking;
    for (const mw of middlewares) {
        if (mw.onThinking) {
            const next = await mw.onThinking(ctx, thinking);
            if (next) {
                thinking = next;
            }
        }
    }
    if (thinking === res.thinking) {
        return res;
    }
    return {
        ...res,
        thinking
    };
};

export const runOnAfterModel = async (
    middlewares: AgentMiddleware<any, any, any>[],
    ctx: MiddlewareContext<any, any, any>,
    res: ModelResponse
): Promise<ModelResponse> => {
    let current = res;
    for (const mw of middlewares) {
        if (mw.onAfterModel) {
            const next = await mw.onAfterModel(ctx, current);
            if (next) {
                current = next;
            }
        }
    }
    return current;
};

export const runOnBeforeTool = async (
    middlewares: AgentMiddleware<any, any, any>[],
    ctx: MiddlewareContext<any, any, any>,
    tool: ToolUseBlock
): Promise<boolean> => {
    for (const mw of middlewares) {
        if (mw.onBeforeTool) {
            const decision = await mw.onBeforeTool(ctx, tool);
            if (decision === false) {
                return false;
            }
        }
    }
    return true;
};

export const wrapToolCall = (
    middlewares: AgentMiddleware<any, any, any>[],
    ctx: MiddlewareContext<any, any, any>,
    tool: ToolUseBlock,
    next: (args: ToolArgs) => Promise<ToolResult>
) => {
    return middlewares
        .filter(mw => mw.wrapToolCall)
        .reduceRight((current, mw) => (args: ToolArgs) => mw.wrapToolCall!(ctx, tool, () => current(args)), next);
};

export const runOnAfterTool = async (
    middlewares: AgentMiddleware<any, any, any>[],
    ctx: MiddlewareContext<any, any, any>,
    tool: ToolUseBlock,
    result: ToolResult
): Promise<void> => {
    for (const mw of middlewares) {
        if (mw.onAfterTool) {
            await mw.onAfterTool(ctx, tool, result);
        }
    }
};

export const runOnAgentEnd = async (
    middlewares: AgentMiddleware<any, any, any>[],
    ctx: MiddlewareContext<any, any, any>,
    result?: ModelResponse,
    error?: Error
): Promise<void> => {
    for (const mw of middlewares) {
        if (mw.onAgentEnd) {
            await mw.onAgentEnd(ctx, result, error);
        }
    }
};

export const runOnError = async (
    middlewares: AgentMiddleware<any, any, any>[],
    ctx: MiddlewareContext<any, any, any>,
    error: Error,
    phase: "model" | "tool" | "thinking" | "helper" | "workflow"
): Promise<{ retry: boolean; delayMs?: number } | undefined> => {
    for (const mw of middlewares) {
        if (mw.onError) {
            const decision = await mw.onError(ctx, error, phase);
            if (decision) {
                return decision;
            }
        }
    }
    return undefined;
};

export const runWithRetries = async <T>(
    middlewares: AgentMiddleware<any, any, any>[],
    ctx: MiddlewareContext<any, any, any>,
    phase: "model" | "tool" | "thinking" | "helper" | "workflow",
    executor: () => Promise<T>
): Promise<T> => {
    let attempt = 0;
    while (true) {
        ctx.attempt = attempt;
        try {
            return await executor();
        } catch (error: any) {
            const decision = await runOnError(middlewares, ctx, error, phase);
            if (!decision || !decision.retry) {
                throw error;
            }
            const delay = decision.delayMs ?? 0;
            if (delay > 0) {
                await new Promise(resolve => setTimeout(resolve, delay));
            }
            attempt += 1;
        }
    }
};

// ═══════════════════════════════════════════════════════════════════════════════
// HELPER PHASE EXECUTORS
// ═══════════════════════════════════════════════════════════════════════════════

/**
 * Run onBeforeHelper hooks. If any returns { skip: true, result }, skip execution.
 */
export const runOnBeforeHelper = async (
    middlewares: AgentMiddleware<any, any, any>[],
    ctx: MiddlewareContext<any, any, any>,
    helperName: string,
    args: Record<string, any>
): Promise<HelperSkipResult | undefined> => {
    for (const mw of middlewares) {
        if (mw.onBeforeHelper) {
            const result = await mw.onBeforeHelper(ctx, helperName, args);
            if (result && (result as HelperSkipResult).skip) {
                return result as HelperSkipResult;
            }
        }
    }
    return undefined;
};

/**
 * Run onAfterHelper hooks after successful helper execution.
 */
export const runOnAfterHelper = async (
    middlewares: AgentMiddleware<any, any, any>[],
    ctx: MiddlewareContext<any, any, any>,
    helperName: string,
    args: Record<string, any>,
    result: any
): Promise<void> => {
    for (const mw of middlewares) {
        if (mw.onAfterHelper) {
            await mw.onAfterHelper(ctx, helperName, args, result);
        }
    }
};

// ═══════════════════════════════════════════════════════════════════════════════
// WORKFLOW PHASE EXECUTORS
// ═══════════════════════════════════════════════════════════════════════════════

/**
 * Run onWorkflowStart hooks. Returns resume info if any middleware provides it.
 */
export const runOnWorkflowStart = async (
    middlewares: AgentMiddleware<any, any, any>[],
    ctx: MiddlewareContext<any, any, any>,
    workflowName: string,
    args: Record<string, any>
): Promise<WorkflowResumeResult | undefined> => {
    for (const mw of middlewares) {
        if (mw.onWorkflowStart) {
            const result = await mw.onWorkflowStart(ctx, workflowName, args);
            if (result && typeof (result as WorkflowResumeResult).resumeFromStep === 'number') {
                return result as WorkflowResumeResult;
            }
        }
    }
    return undefined;
};

/**
 * Run onBeforeStep hooks. If any returns { skip: true, result }, skip execution.
 */
export const runOnBeforeStep = async (
    middlewares: AgentMiddleware<any, any, any>[],
    ctx: MiddlewareContext<any, any, any>,
    stepIndex: number,
    stepType: string
): Promise<StepSkipResult | undefined> => {
    ctx.stepIndex = stepIndex;
    for (const mw of middlewares) {
        if (mw.onBeforeStep) {
            const result = await mw.onBeforeStep(ctx, stepIndex, stepType);
            if (result && (result as StepSkipResult).skip) {
                return result as StepSkipResult;
            }
        }
    }
    return undefined;
};

/**
 * Run onAfterStep hooks after successful step execution.
 */
export const runOnAfterStep = async (
    middlewares: AgentMiddleware<any, any, any>[],
    ctx: MiddlewareContext<any, any, any>,
    stepIndex: number,
    stepType: string,
    result: any
): Promise<void> => {
    for (const mw of middlewares) {
        if (mw.onAfterStep) {
            await mw.onAfterStep(ctx, stepIndex, stepType, result);
        }
    }
};

/**
 * Run onWorkflowEnd hooks when workflow completes or fails.
 */
export const runOnWorkflowEnd = async (
    middlewares: AgentMiddleware<any, any, any>[],
    ctx: MiddlewareContext<any, any, any>,
    workflowName: string,
    result: any,
    error?: Error
): Promise<void> => {
    for (const mw of middlewares) {
        if (mw.onWorkflowEnd) {
            await mw.onWorkflowEnd(ctx, workflowName, result, error);
        }
    }
};
