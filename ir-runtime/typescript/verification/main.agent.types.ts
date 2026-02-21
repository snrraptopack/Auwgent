// Auto-generated types for Manger
// Do not edit manually
// Core Runtime Imports
import { createAuwgent } from "@auwgent/runtime";
import type { ToolRegistry } from "@auwgent/runtime";
import _importedIR from './main.agent.json' with { type: 'json' };
const agentIR = _importedIR as typeof _importedIR;

export type Student = {

    name: string;

    id: string;

    location: string;

    grades: string[];
}

export type MangerInput = {

}

export type MangerOutput = {

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
    context: MangerContext;
    apiKeys: MangerApiKeys;
}

export function createManger(config: MangerConfig) {
    return createAuwgent<
        typeof agentIR,
        MangerCustomIntents,
        never,
        MangerTools
    >(agentIR, {
        tools: config.tools,
        context: config.context,
        apiKeys: config.apiKeys
    });
}

export type MangerAgent = ReturnType<typeof createManger>;
export const auwgent = createManger;
export type AuwgentTools = MangerTools;
export type AuwgentConfig = MangerConfig;
export type AuwgentAgent = MangerAgent;
export type AuwgentContext = MangerContext;
