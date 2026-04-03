// Auto-generated types for Assistants
// Do not edit manually
// Core Runtime Imports
import { createAuwgent } from "@snrraptopack/auwgent-sdk";
import type { ToolRegistry } from "@snrraptopack/auwgent-sdk";
import _importedIR from './main.agent.json' with { type: 'json' };
type AssistantsIR = Omit<typeof _importedIR, "name" | "workflows" | "helpers"> & {
  name: "Assistants";
  workflows: undefined;
  helpers: undefined;
};
const agentIR = _importedIR as unknown as AssistantsIR;
export type AssistantsInput = {

}

export type AssistantsOutput = {

}

export type AssistantsContext = {
    user_name: string;
}

/** Custom intents defined in the DSL (if any) */
export type AssistantsCustomIntents =
    | never;

/**
 * API keys required for Assistants
 */
export type AssistantsApiKeys = {
    geminiApiKey: string;
}

// Defined explicitly (not via ReturnType) so RouterMiddleware can derive from it without circularity
export type AssistantsAgent = import("@snrraptopack/auwgent-sdk").TypedAuwgent<
    typeof agentIR,
    AssistantsCustomIntents,
    AssistantsOutput,
    Record<string, never>
>;

/** Middleware object type — consistent with `AssistantsAgent.onIntent` intent narrowing */
export type AssistantsMiddleware<T extends import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent'] = import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent']> = import("@snrraptopack/auwgent-sdk").Middleware<
    typeof agentIR,
    AssistantsCustomIntents,
    AssistantsOutput,
    Record<string, never>,
    T
>;

export type AssistantsConfig = {
    middleware?: AssistantsMiddleware[];
    context: AssistantsContext;
    apiKeys: AssistantsApiKeys;
}

export function createAssistants(config: AssistantsConfig): AssistantsAgent {
    return createAuwgent<
        typeof agentIR,
        AssistantsCustomIntents,
        AssistantsOutput,
        Record<string, never>
    >(agentIR, {
        tools: {} as Record<string, never>,
        middleware: config.middleware as any,
        context: config.context,
        apiKeys: config.apiKeys
    });
}

export const auwgent = createAssistants;
export type AuwgentTools = Record<string, never>;
export type AuwgentConfig = AssistantsConfig;
export type AuwgentAgent = AssistantsAgent;
export type AuwgentMiddleware = AssistantsMiddleware;
export type AuwgentContext = AssistantsContext;