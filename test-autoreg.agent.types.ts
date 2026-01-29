// Auto-generated types for Support
// Do not edit manually
// Core Runtime Imports
import { Agent } from "./javascript/loader/IrInterpreter";
import type { AgentIR } from "./javascript/loader/types/ir";
import type { SyntheticMessage, ConversationState, LifecycleHooks } from "./javascript/loader/types/protocol";
import _importedIR from './test-autoreg.agent.json' with { type: 'json' };
const agentIR = _importedIR as unknown as AgentIR;

export interface Product {

    name: string;

    id: string;

    price: number;
}

export interface SupportInput {
    text: string;
}

export interface SupportOutput {

}

export interface SupportContext {
    isVerified: boolean;
    user_id: string;
}

export interface SupportTools {
    [key: string]: (args: any) => Promise<any>;
    search_product_by_name: (args: { name: string }) => Promise<Product>;
    search_product_by_id: (args: { id: string }) => Promise<Product>;
    purchase_product: (args: { product_id: string, user_id: string }) => Promise<boolean>;
}


/**
 * Configuration for Support agent
 */
export interface SupportConfig {
    context?: SupportContext;
    tools?: SupportTools;
}

/**
 * Create a type-safe Support agent instance
 * 
 * @example
 * ```typescript
 * const agent = createSupport({
 *     apiKeys: { geminiApiKey: '...' },
 *     context: { sessionId: "123" },
 *     tools: { ... },
 * });
 * 
 * // Clean execution - config bound at creation
 * const result = await agent.run({ ... });
 * const stream = await agent.stream({ ... });
 * ```
 */
export function createSupport(config: SupportConfig) {
    // Create agent with drivers
    const agent = new Agent<SupportInput, SupportOutput, SupportContext, SupportTools>({});
    
    // Load and validate IR from imported file
    agent.load(agentIR);

    // Validate tools against IR
    const toolMap = new Map<string, any>();
    if (agentIR.tools && agentIR.tools.length > 0) {
        for (const toolDef of agentIR.tools) {
            toolMap.set(toolDef.name, toolDef);
        }
    }
    if (agentIR.workflows && agentIR.workflows.length > 0) {
        for (const workflow of agentIR.workflows) {
            if (workflow.tools && workflow.tools.length > 0) {
                for (const toolDef of workflow.tools) {
                    toolMap.set(toolDef.name, toolDef);
                }
            }
        }
    }
    for (const toolDef of toolMap.values()) {
        if (!config.tools?.[toolDef.name]) {
            throw new Error(
                `Missing required tool: ${toolDef.name}\n` +
                `Expected in tools configuration`
            );
        }
    }
    
    return {
        /**
         * Run the agent with type-safe parameters
         * @param input - Agent input
         * @param overrides - Optional overrides for context, tools, lifecycle, or configName
         */
        run: (input: SupportInput, overrides?: { context?: SupportContext; tools?: SupportTools; modelOverride?: { providerType?: string; modelName?: string; temperature?: number }; configName?: never }): Promise<SupportOutput> => 
            agent.run(input, { tools: overrides?.tools ?? config.tools, context: overrides?.context ?? config.context, modelOverride: overrides?.modelOverride, configName: overrides?.configName }),
        
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
        stream: (input: SupportInput, overrides?: { context?: SupportContext; tools?: SupportTools; modelOverride?: { providerType?: string; modelName?: string; temperature?: number }; configName?: never }) => 
            agent.stream(input, { tools: overrides?.tools ?? config.tools, context: overrides?.context ?? config.context, modelOverride: overrides?.modelOverride, configName: overrides?.configName }),
        
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
        streamIterable: (input: SupportInput, overrides?: { context?: SupportContext; tools?: SupportTools; modelOverride?: { providerType?: string; modelName?: string; temperature?: number }; configName?: never }) => 
            agent.runStream(input, { tools: overrides?.tools ?? config.tools, context: overrides?.context ?? config.context, modelOverride: overrides?.modelOverride, configName: overrides?.configName }),
        
        /**
         * Create a new agent instance with bound context
         * Useful for multi-turn conversations with the same session
         * 
         * @example
         * ```typescript
         * const sessionAgent = agent.forContext({ sessionId: '123' });
         * await sessionAgent.run({ message: "First" });
         * await sessionAgent.run({ message: "Second" });
         * ```
         */
        forContext: (context: SupportContext) => {
            const boundContext = context;
            return {
                run: (input: SupportInput, overrides?: { configName?: never; modelOverride?: { providerType?: string; modelName?: string; temperature?: number } }) => 
                    agent.run(input, { context: boundContext, configName: overrides?.configName, modelOverride: overrides?.modelOverride }),
                stream: (input: SupportInput, overrides?: { configName?: never; modelOverride?: { providerType?: string; modelName?: string; temperature?: number } }) => 
                    agent.stream(input, { context: boundContext, configName: overrides?.configName, modelOverride: overrides?.modelOverride }),
                streamIterable: (input: SupportInput, overrides?: { configName?: never; modelOverride?: { providerType?: string; modelName?: string; temperature?: number } }) => 
                    agent.runStream(input, { context: boundContext, configName: overrides?.configName, modelOverride: overrides?.modelOverride })
            };
        }
    };
}

/** Type for the created agent instance */
export type SupportAgent = ReturnType<typeof createSupport>;
