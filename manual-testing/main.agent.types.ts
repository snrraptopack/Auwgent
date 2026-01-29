// Auto-generated types for MainAgent
// Do not edit manually
// Core Runtime Imports
import { Agent, RunConfig } from "../javascript/loader/IrInterpreter";
import { OpenAIDriver } from "../javascript/loader/drivers/OpenAIDriver";
import type { AgentIR } from "../javascript/loader/types/ir";
import type { SyntheticMessage, ConversationState, LifecycleHooks } from "../javascript/loader/types/protocol";
import _importedIR from './main.agent.json' with { type: 'json' };
const agentIR = _importedIR as unknown as AgentIR;

export interface User {

    name: string;

    email: string;

    age?: number;
}

export interface MainAgentInput {
    user: User;
}

export interface MainAgentOutput {
    result: string;
}

export interface MainAgentContext {

}

/**
 * API keys required for MainAgent
 */
export interface MainAgentApiKeys {
    openaiApiKey: string;
}


/**
 * Configuration for MainAgent agent
 */
export interface MainAgentConfig {
    apiKeys: MainAgentApiKeys;
}

/**
 * Create a type-safe MainAgent agent instance
 * 
 * @example
 * ```typescript
 * const agent = createMainAgent({
 *     apiKeys: { geminiApiKey: '...' },
 * });
 * 
 * // Clean execution - config bound at creation
 * const result = await agent.run({ ... });
 * const stream = await agent.stream({ ... });
 * ```
 */
export function createMainAgent(config: MainAgentConfig) {
    // Create agent with drivers
    const agent = new Agent<MainAgentInput, MainAgentOutput, Record<string, never>, Record<string, never>>({
        openai: new OpenAIDriver(config.apiKeys.openaiApiKey)
    });
    
    // Load and validate IR from imported file
    agent.load(agentIR);

    
    return {
        /**
         * Run the agent with type-safe parameters
         * @param input - Agent input
         * @param overrides - Optional overrides for context, tools, lifecycle, or configName
         */
        run: (input: MainAgentInput, overrides?: { modelOverride?: { providerType?: string; modelName?: string; temperature?: number }; configName?: never }): Promise<MainAgentOutput> => 
            agent.run(input, { modelOverride: overrides?.modelOverride, configName: overrides?.configName }),
        
        /**
         * Fluent streaming API with callbacks
         * @param input - Agent input
         * @param overrides - Optional overrides for context, tools, lifecycle, or configName
         * 
         * @example
         * ```typescript
         * const result = await agent
         *   .stream({ request: "..." })
         *   .onChunk(delta => console.log(delta))
         *   .onToolResult((name, result) => console.log(name, result))
         *   .run();
         * ```
         */
        stream: (input: MainAgentInput, overrides?: { modelOverride?: { providerType?: string; modelName?: string; temperature?: number }; configName?: never }) => 
            agent.stream(input, { modelOverride: overrides?.modelOverride, configName: overrides?.configName }),
        
        /**
         * Native async iteration over stream chunks
         * @param input - Agent input
         * @param overrides - Optional overrides for context, tools, lifecycle, or configName
         * 
         * @example
         * ```typescript
         * for await (const chunk of agent.streamIterable({ request: "..." })) {
         *     if (chunk.type === 'text') console.log(chunk.delta);
         * }
         * ```
         */
        streamIterable: (input: MainAgentInput, overrides?: { modelOverride?: { providerType?: string; modelName?: string; temperature?: number }; configName?: never }) => 
            agent.runStream(input, { modelOverride: overrides?.modelOverride, configName: overrides?.configName }),
    };
}

/** Type for the created agent instance */
export type MainAgentAgent = ReturnType<typeof createMainAgent>;
