// Auto-generated types for Basic
// Do not edit manually
// Core Runtime Imports
import { createAuwgent } from "@auwgent/runtime";
import type { ToolRegistry } from "@auwgent/runtime";
import _importedIR from './main.agent.json' with { type: 'json' };
type BasicIR = Omit<typeof _importedIR, "workflows" | "helpers"> & {
  workflows: undefined;
  helpers: ({ name: "BasicHelper1" } | { name: "BasicHelper2" } | { name: "BasicHelper3" } | { name: "BasicHelper4" } | { name: "BasicHelper5" })[];
};
const agentIR = _importedIR as unknown as BasicIR;
export type BasicInput = {

}

export type BasicHelper4Output = {

}

export type BasicHelper5Output = {

}

export type BasicBaseOutput = {

}

/** Union of possible output types (includes transfer destinations) */
export type BasicOutput = BasicBaseOutput | BasicHelper4Output | BasicHelper5Output;

export type BasicContext = {

}

export type BasicTools = {
    one: (args: {  }) => Promise<string>;
    two: (args: {  }) => Promise<string>;
}

/** Custom intents defined in the DSL (if any) */
export type BasicCustomIntents = never;

/**
 * API keys required for Basic
 */
export type BasicApiKeys = {
    geminiApiKey: string;
    openaiApiKey: string;
}


export type BasicConfig = {
    tools: BasicTools;
    middleware?: import("@auwgent/runtime").Middleware<
        typeof agentIR,
        BasicCustomIntents,
        never,
        BasicTools
    >[];
    apiKeys: BasicApiKeys;
}

export function createBasic(config: BasicConfig) {
    return createAuwgent<
        typeof agentIR,
        BasicCustomIntents,
        never,
        BasicTools
    >(agentIR, {
        tools: config.tools,
        middleware: config.middleware,
        
        apiKeys: config.apiKeys
    });
}

export type BasicAgent = ReturnType<typeof createBasic>;
export type BasicMiddleware = import("@auwgent/runtime").Middleware<
    typeof agentIR,
    BasicCustomIntents,
    never,
    BasicTools
>;
export const auwgent = createBasic;
export type AuwgentTools = BasicTools;
export type AuwgentConfig = BasicConfig;
export type AuwgentAgent = BasicAgent;
export type AuwgentMiddleware = BasicMiddleware;
export type AuwgentContext = BasicContext;
