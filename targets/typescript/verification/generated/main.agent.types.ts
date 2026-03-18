// Auto-generated types for Hello
// Do not edit manually
// Core Runtime Imports
import { createAuwgent } from "@snrraptopack/auwgent-sdk";
import type { ToolRegistry } from "@snrraptopack/auwgent-sdk";
import _importedIR from './main.agent.json' with { type: 'json' };
type HelloIR = Omit<typeof _importedIR, "name" | "workflows" | "helpers"> & {
  name: "Hello";
  workflows: undefined;
  helpers: undefined;
};
const agentIR = _importedIR as unknown as HelloIR;
export type HelloInput = {

}

export type HelloOutput = {

}

export type HelloContext = {

}

/** Custom intents defined in the DSL (if any) */
export type HelloCustomIntents =
    | { name: "question"; value: { text: string; options: string[] } };

/**
 * API keys required for Hello
 */
export type HelloApiKeys = {
    geminiApiKey: string;
    my_kimi_apiApiKey: string;  // API key for custom provider 'my-kimi-api'
}

// Defined explicitly (not via ReturnType) so RouterMiddleware can derive from it without circularity
export type HelloAgent = import("@snrraptopack/auwgent-sdk").TypedAuwgent<
    typeof agentIR,
    HelloCustomIntents,
    never,
    Record<string, never>
>;

/** Middleware object type — consistent with `HelloAgent.onIntent` intent narrowing */
export type HelloMiddleware<T extends import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent'] = import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent']> = import("@snrraptopack/auwgent-sdk").Middleware<
    typeof agentIR,
    HelloCustomIntents,
    never,
    Record<string, never>,
    T
>;

export type HelloConfig = {
    middleware?: HelloMiddleware[];
    apiKeys: HelloApiKeys;
}

export function createHello(config: HelloConfig): HelloAgent {
    return createAuwgent<
        typeof agentIR,
        HelloCustomIntents,
        never,
        Record<string, never>
    >(agentIR, {
        tools: {} as Record<string, never>,
        middleware: config.middleware as any,
        apiKeys: config.apiKeys
    });
}

export const auwgent = createHello;
export type AuwgentTools = Record<string, never>;
export type AuwgentConfig = HelloConfig;
export type AuwgentAgent = HelloAgent;
export type AuwgentMiddleware = HelloMiddleware;
export type AuwgentContext = HelloContext;