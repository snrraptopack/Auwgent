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

export type MangerToolArgs = {
    [K in keyof MangerTools]: Parameters<MangerTools[K]>[0];
}
export type MangerToolResults = {
    [K in keyof MangerTools]: Awaited<ReturnType<MangerTools[K]>>;
}
export type MangerText = string;
export type MangerHooks = {
    onToolCall?: <K extends keyof MangerTools>(name: K, args: MangerToolArgs[K]) => void;
    onToolEnd?: <K extends keyof MangerTools>(name: K, args: MangerToolArgs[K], result: MangerToolResults[K]) => void;
    onWorkflowCall?: (name: string, args: unknown) => void;
    onWorkflowEnd?: (name: string, args: unknown, result: unknown) => void;
    onText?: (output: MangerText) => void;
}
export type AuwgentHooks = MangerHooks;
/**
 * API keys required for Manger
 */
export type MangerApiKeys = {
    geminiApiKey: string;
}


export type MangerConfig = {
    tools: MangerTools;
    context: MangerContext;
    hooks?: MangerHooks;
    apiKeys: MangerApiKeys;
}

export function createManger(config: MangerConfig) {
    const hooks = config.hooks;
    const onText = hooks?.onText
        ? (data: unknown) => hooks.onText((data as any)?.text ?? (data as string))
        : undefined;
    return createAuwgent(agentIR, {
        tools: config.tools as unknown as ToolRegistry<typeof agentIR>,
        hooks: hooks ? { ...hooks, onText } : undefined,
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
export type AuwgentHooks = MangerHooks;
export type AuwgentText = MangerText;
