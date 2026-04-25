// Auto-generated types for Hello
// Do not edit manually
// Core Runtime Imports
import { createAuwgent } from "@snrraptopack/auwgent-sdk";
import type { ToolRegistry } from "@snrraptopack/auwgent-sdk";
import _importedIR from './main.agent.json' with { type: 'json' };
type HelloIR = Omit<typeof _importedIR, "name" | "workflows" | "helpers"> & {
  name: "Hello";
  workflows: undefined;
  helpers: ({ name: "Joker"; input: { joker_prompt: string }; output: null })[];
};
const agentIR = _importedIR as unknown as HelloIR;
export type Person = {
    age: number;
    name: string;

    /** location of the user */
    location: string;
}
export type HelloInput = {

}

export type HelloOutput = {
    name: string;
    age: number;
    location: string;
}

export type HelloContext = {

}

export type HelloTools = {
    get_user_name_age: (args: {  }) => Promise<Person>;
    get_location: (args: {  }) => Promise<string>;
}

/** Custom intents defined in the DSL (if any) */
export type HelloCustomIntents =
    | never;

/**
 * API keys required for Hello
 */
export type HelloApiKeys = {
    groqApiKey: string;
}

// Defined explicitly (not via ReturnType) so RouterMiddleware can derive from it without circularity
export type HelloAgent = import("@snrraptopack/auwgent-sdk").TypedAuwgent<
    typeof agentIR,
    HelloCustomIntents,
    HelloOutput,
    HelloTools
>;

/** Middleware object type — consistent with `HelloAgent.onIntent` intent narrowing */
export type HelloMiddleware<T extends import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent'] = import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent']> = import("@snrraptopack/auwgent-sdk").Middleware<
    typeof agentIR,
    HelloCustomIntents,
    HelloOutput,
    HelloTools,
    T
>;

export type HelloConfig = {
    tools: HelloTools;
    middleware?: HelloMiddleware[];
    apiKeys: HelloApiKeys;
}

export function createHello(config: HelloConfig): HelloAgent {
    return createAuwgent<
        typeof agentIR,
        HelloCustomIntents,
        HelloOutput,
        HelloTools
    >(agentIR, {
        tools: config.tools,
        middleware: config.middleware as any,
        apiKeys: config.apiKeys
    });
}

export const auwgent = createHello;
export type AuwgentTools = HelloTools;
export type AuwgentConfig = HelloConfig;
export type AuwgentAgent = HelloAgent;
export type AuwgentMiddleware = HelloMiddleware;
export type AuwgentContext = HelloContext;