import type { AgentMiddleware, ContentBlock, MiddlewareContext, ModelResponse, SyntheticMessage } from "../types/protocol";

export type ShortMemoryState = {
    recent: SyntheticMessage[];
    maxMessages: number;
};

export type ShortMemoryOptions = {
    maxMessages?: number;
    includeThinking?: boolean;
    includeToolCall?: boolean;
};

export const createShortMemoryMiddleware = <TInput, TContext>(
    options: number | ShortMemoryOptions = 20
): AgentMiddleware<TInput, TContext, ShortMemoryState> => ({
    name: "short_memory",
    priority: 20,
    onAgentStart: (ctx: MiddlewareContext<TInput, TContext, ShortMemoryState>) => {
        const resolved = typeof options === "number" ? { maxMessages: options } : options;
        const maxMessages = resolved?.maxMessages ?? 20;
        ctx.state.recent = [];
        ctx.state.maxMessages = maxMessages;
    },
    onBeforeModel: (ctx: MiddlewareContext<TInput, TContext, ShortMemoryState>) => {
        const resolved = typeof options === "number" ? { maxMessages: options } : options;
        const maxMessages = resolved?.maxMessages ?? 20;
        const limit = ctx.state.maxMessages ?? maxMessages;
        const pruned = ctx.request.messages.slice(-limit);
        ctx.state.recent = pruned;
        return {
            ...ctx.request,
            messages: pruned
        };
    },
    onAfterModel: (ctx: MiddlewareContext<TInput, TContext, ShortMemoryState>, res: ModelResponse) => {
        const resolved = typeof options === "number" ? { maxMessages: options } : options;
        const maxMessages = resolved?.maxMessages ?? 20;
        const includeThinking = resolved?.includeThinking ?? false;
        const includeToolCall = resolved?.includeToolCall ?? true;
        const limit = ctx.state.maxMessages ?? maxMessages;
        const contentBlocks: ContentBlock[] = [];
        if (includeThinking && res.thinking) {
            contentBlocks.push({ type: "thinking", ...res.thinking });
        }
        if (res.content) {
            contentBlocks.push(...res.content);
        }
        if (includeToolCall && res.toolCalls) {
            for (const call of res.toolCalls) {
                const hasToolUse = contentBlocks.some(block => block.type === "tool_use" && block.id === call.id);
                if (!hasToolUse) {
                    contentBlocks.push({
                        type: "tool_use",
                        id: call.id,
                        name: call.name,
                        input: call.args
                    });
                }
            }
        }
        const assistantMessage: SyntheticMessage | undefined = contentBlocks.length > 0
            ? { role: "assistant", content: contentBlocks }
            : undefined;
        const next = assistantMessage
            ? [...ctx.state.recent, assistantMessage].slice(-limit)
            : ctx.state.recent;
        ctx.state.recent = next;
        return res;
    }
});
