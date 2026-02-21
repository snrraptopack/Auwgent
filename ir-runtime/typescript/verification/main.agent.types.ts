// Auto-generated types for Manger
// Do not edit manually
// Core Runtime Imports
import { createAuwgent } from "@auwgent/runtime";
import type { ToolRegistry } from "@auwgent/runtime";
import _importedIR from './main.agent.json' with { type: 'json' };
type MangerIR = Omit<typeof _importedIR, "workflows" | "helpers"> & {
  workflows: ({ flowName: "get_student_grade"; returns: string[] })[];
  helpers: undefined;
};
const agentIR = _importedIR as unknown as MangerIR;

export type Student = {

    name: string;

    id: string;

    location: string;

    grades: string[];
}


/** Output type */
export type One = {

    /** for direct response */
    simple_response?: string;

    /** for returnin structured resonse */
    structured_response?: { student: Student; descriptions: string };
}

export type MangerInput = {

}

export type MangerOutput = {
    simple_response?: string;
    structured_response?: { student: Student; descriptions: string };
}

export type MangerContext = {
    user_name: string;
    id: string;
}

export type MangerTools = {
    get_student_details: (args: { id: string }) => Promise<Student>;
    edit_student_details: (args: { id: string }) => Promise<Student>;
}

/** Custom intents defined in the DSL (if any) */
export type MangerCustomIntents = never;

/**
 * API keys required for Manger
 */
export type MangerApiKeys = {
    geminiApiKey: string;
}


export type MangerConfig = {
    tools: MangerTools;
    middleware?: import("@auwgent/runtime").Middleware<
        typeof agentIR,
        MangerCustomIntents,
        MangerOutput,
        MangerTools
    >[];
    context: MangerContext;
    apiKeys: MangerApiKeys;
}

export function createManger(config: MangerConfig) {
    return createAuwgent<
        typeof agentIR,
        MangerCustomIntents,
        MangerOutput,
        MangerTools
    >(agentIR, {
        tools: config.tools,
        middleware: config.middleware,
        context: config.context,
        apiKeys: config.apiKeys
    });
}

export type MangerAgent = ReturnType<typeof createManger>;
export type MangerMiddleware = import("@auwgent/runtime").Middleware<
    typeof agentIR,
    MangerCustomIntents,
    MangerOutput,
    MangerTools
>;
export const auwgent = createManger;
export type AuwgentTools = MangerTools;
export type AuwgentConfig = MangerConfig;
export type AuwgentAgent = MangerAgent;
export type AuwgentMiddleware = MangerMiddleware;
export type AuwgentContext = MangerContext;
