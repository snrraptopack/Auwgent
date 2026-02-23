// Auto-generated types for Router
// Do not edit manually
// Core Runtime Imports
import { createAuwgent } from "@auwgent/runtime";
import type { ToolRegistry } from "@auwgent/runtime";
import _importedIR from './main.agent.json' with { type: 'json' };
type RouterIR = Omit<typeof _importedIR, "workflows" | "helpers"> & {
  workflows: undefined;
  helpers: ({ name: "FoodWizard" } | { name: "Story" })[];
};
const agentIR = _importedIR as unknown as RouterIR;
export type RouterInput = {

}

export type FoodWizardOutput = {

}

export type StoryOutput = {

}

export type RouterBaseOutput = {

}

/** Union of possible output types (includes transfer destinations) */
export type RouterOutput = RouterBaseOutput | FoodWizardOutput | StoryOutput;

export type RouterContext = {

}

/** Custom intents defined in the DSL (if any) */
export type RouterCustomIntents = never;

/**
 * API keys required for Router
 */
export type RouterApiKeys = {
    geminiApiKey: string;
}


// Defined explicitly (not via ReturnType) so RouterMiddleware can derive from it without circularity
export type RouterAgent = import("@auwgent/runtime").TypedAuwgent<
    typeof agentIR,
    RouterCustomIntents,
    never,
    Record<string, never>
>;

/** Middleware object type — consistent with `RouterAgent.onIntent` intent narrowing */
export type RouterMiddleware = import("@auwgent/runtime").Middleware<
    typeof agentIR,
    RouterCustomIntents,
    never,
    Record<string, never>
>;

export type RouterConfig = {
    middleware?: RouterMiddleware[];
    apiKeys: RouterApiKeys;
}

export function createRouter(config: RouterConfig): RouterAgent {
    return createAuwgent<
        typeof agentIR,
        RouterCustomIntents,
        never,
        Record<string, never>
    >(agentIR, {
        tools: {} as Record<string, never>,
        middleware: config.middleware as any,
        
        apiKeys: config.apiKeys
    });
}

export const auwgent = createRouter;
export type AuwgentTools = Record<string, never>;
export type AuwgentConfig = RouterConfig;
export type AuwgentAgent = RouterAgent;
export type AuwgentMiddleware = RouterMiddleware;
export type AuwgentContext = RouterContext;
