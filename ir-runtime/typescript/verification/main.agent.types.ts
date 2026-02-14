// Auto-generated types for CHEF
// Do not edit manually
// Core Runtime Imports
import { createAuwgent } from "@auwgent/runtime";
import type { ToolRegistry } from "@auwgent/runtime";
import _importedIR from './main.agent.json' with { type: 'json' };
const agentIR = _importedIR as typeof _importedIR;
export type CHEFInput = {
    text: string;
}

export type CHEFOutput = {

}

export type CHEFContext = {

}

export type CHEFTools = {
    tool_get_user_name: (args: {  }) => Promise<string>;
}

/**
 * API keys required for CHEF
 */
export type CHEFApiKeys = {
    geminiApiKey: string;
}


export type CHEFConfig = {
    tools: CHEFTools;
    apiKeys: CHEFApiKeys;
}

export function createCHEF(config: CHEFConfig) {
    return createAuwgent(agentIR, {
        tools: config.tools as unknown as ToolRegistry<typeof agentIR>,
        
        apiKeys: config.apiKeys
    });
}

export type CHEFAgent = ReturnType<typeof createCHEF>;
export const auwgent = createCHEF;
export type AuwgentTools = CHEFTools;
export type AuwgentConfig = CHEFConfig;
export type AuwgentAgent = CHEFAgent;
export type AuwgentContext = CHEFContext;
