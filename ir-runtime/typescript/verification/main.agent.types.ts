// Auto-generated types for Manager
// Do not edit manually
// Core Runtime Imports
import { createAuwgent } from "@auwgent/runtime";
import type { ToolRegistry } from "@auwgent/runtime";
import _importedIR from './main.agent.json' with { type: 'json' };
type ManagerIR = Omit<typeof _importedIR, "workflows" | "helpers"> & {
  workflows: undefined;
  helpers: ({ name: "Joker" })[];
};
const agentIR = _importedIR as unknown as ManagerIR;

export type Student = {

    user_name: string;

    age: number;

    id: string;

    grades: string[];
}

export type ManagerInput = {

}

export type JokerOutput = {

}

export type ManagerBaseOutput = {

}

/** Union of possible output types (includes transfer destinations) */
export type ManagerOutput = ManagerBaseOutput | JokerOutput;

export type ManagerContext = {
    user_name: string;
}

export type ManagerTools = {
    getStudentDetails: (args: { id: string }) => Promise<Student>;
}

/** Custom intents defined in the DSL (if any) */
export type ManagerCustomIntents = never;

/**
 * API keys required for Manager
 */
export type ManagerApiKeys = {
    geminiApiKey: string;
}


// Defined explicitly (not via ReturnType) so RouterMiddleware can derive from it without circularity
export type ManagerAgent = import("@auwgent/runtime").TypedAuwgent<
    typeof agentIR,
    ManagerCustomIntents,
    never,
    ManagerTools
>;

/** Middleware object type — consistent with `ManagerAgent.onIntent` intent narrowing */
export type ManagerMiddleware = import("@auwgent/runtime").Middleware<
    typeof agentIR,
    ManagerCustomIntents,
    never,
    ManagerTools
>;

export type ManagerConfig = {
    tools: ManagerTools;
    middleware?: ManagerMiddleware[];
    context: ManagerContext;
    apiKeys: ManagerApiKeys;
}

export function createManager(config: ManagerConfig): ManagerAgent {
    return createAuwgent<
        typeof agentIR,
        ManagerCustomIntents,
        never,
        ManagerTools
    >(agentIR, {
        tools: config.tools,
        middleware: config.middleware as any,
        context: config.context,
        apiKeys: config.apiKeys
    });
}

export const auwgent = createManager;
export type AuwgentTools = ManagerTools;
export type AuwgentConfig = ManagerConfig;
export type AuwgentAgent = ManagerAgent;
export type AuwgentMiddleware = ManagerMiddleware;
export type AuwgentContext = ManagerContext;
