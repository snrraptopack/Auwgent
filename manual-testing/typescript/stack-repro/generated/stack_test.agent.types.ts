// Auto-generated types for Router
// Do not edit manually
// Core Runtime Imports
import { createAuwgent } from "@snrraptopack/auwgent-sdk";
import type { ToolRegistry } from "@snrraptopack/auwgent-sdk";
import _importedIR from './stack_test.agent.json' with { type: 'json' };
type RouterIR = Omit<typeof _importedIR, "name" | "workflows" | "helpers"> & {
  name: "Router";
  workflows: undefined;
  helpers: ({ name: "StoryTeller" } | { name: "Analyzer" })[];
};
const agentIR = _importedIR as unknown as RouterIR;
export type RouterInput = {

}

export type StoryTellerOutput = {

}

export type RouterBaseOutput = {

}

/** Union of possible output types (includes transfer destinations) */
export type RouterOutput = RouterBaseOutput | StoryTellerOutput;

export type RouterContext = {
    user_name: string;
}

/** Custom intents defined in the DSL (if any) */
export type RouterCustomIntents =
    | { name: "thought"; value: { explain: string } }
    | { name: "questions"; value: { questions: string } };

/**
 * API keys required for Router
 */
export type RouterApiKeys = {
    geminiApiKey: string;
    my_groq_apiApiKey: string;  // API key for custom provider 'my-groq-api'
}

// Defined explicitly (not via ReturnType) so RouterMiddleware can derive from it without circularity
export type RouterAgent = import("@snrraptopack/auwgent-sdk").TypedAuwgent<
    typeof agentIR,
    RouterCustomIntents,
    RouterOutput,
    Record<string, never>
>;

/** Middleware object type — consistent with `RouterAgent.onIntent` intent narrowing */
export type RouterMiddleware<T extends import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent'] = import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent']> = import("@snrraptopack/auwgent-sdk").Middleware<
    typeof agentIR,
    RouterCustomIntents,
    RouterOutput,
    Record<string, never>,
    T
>;

export type RouterConfig = {
    middleware?: RouterMiddleware[];
    context: RouterContext;
    apiKeys: RouterApiKeys;
}

export function createRouter(config: RouterConfig): RouterAgent {
    return createAuwgent<
        typeof agentIR,
        RouterCustomIntents,
        RouterOutput,
        Record<string, never>
    >(agentIR, {
        tools: {} as Record<string, never>,
        middleware: config.middleware as any,
        context: config.context,
        apiKeys: config.apiKeys
    });
}

export const auwgent = createRouter;
export type AuwgentTools = Record<string, never>;
export type AuwgentConfig = RouterConfig;
export type AuwgentAgent = RouterAgent;
export type AuwgentMiddleware = RouterMiddleware;
export type AuwgentContext = RouterContext;