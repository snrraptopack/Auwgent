// Auto-generated types for Main
// Do not edit manually
// Core Runtime Imports
import { createAuwgent } from "@snrraptopack/auwgent-sdk";
import type { ToolRegistry } from "@snrraptopack/auwgent-sdk";
import _importedIR from './main.agent.json' with { type: 'json' };
type MainIR = Omit<typeof _importedIR, "name" | "workflows" | "helpers"> & {
  name: "Main";
  workflows: ({ flowName: "delete"; flowParams: { id: number }; returns: string })[];
  helpers: undefined;
};
const agentIR = _importedIR as unknown as MainIR;
export type MainInput = {

}

export type MainOutput = {

}

export type MainContext = {
    is_vip: boolean;
}

export type MainTools = {
    deleteDatabase: (args: { id: number }) => Promise<string>;
}

/** Custom intents defined in the DSL (if any) */
export type MainCustomIntents =
    | never;

/**
 * API keys required for Main
 */
export type MainApiKeys = {
    geminiApiKey: string;
}

// Defined explicitly (not via ReturnType) so RouterMiddleware can derive from it without circularity
export type MainAgent = import("@snrraptopack/auwgent-sdk").TypedAuwgent<
    typeof agentIR,
    MainCustomIntents,
    MainOutput,
    MainTools
>;

/** Middleware object type — consistent with `MainAgent.onIntent` intent narrowing */
export type MainMiddleware<T extends import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent'] = import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent']> = import("@snrraptopack/auwgent-sdk").Middleware<
    typeof agentIR,
    MainCustomIntents,
    MainOutput,
    MainTools,
    T
>;

export type MainConfig = {
    tools: MainTools;
    middleware?: MainMiddleware[];
    context: MainContext;
    apiKeys: MainApiKeys;
}

export function createMain(config: MainConfig): MainAgent {
    return createAuwgent<
        typeof agentIR,
        MainCustomIntents,
        MainOutput,
        MainTools
    >(agentIR, {
        tools: config.tools,
        middleware: config.middleware as any,
        context: config.context,
        apiKeys: config.apiKeys
    });
}

export const auwgent = createMain;
export type AuwgentTools = MainTools;
export type AuwgentConfig = MainConfig;
export type AuwgentAgent = MainAgent;
export type AuwgentMiddleware = MainMiddleware;
export type AuwgentContext = MainContext;