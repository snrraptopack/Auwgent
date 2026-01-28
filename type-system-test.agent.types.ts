// Auto-generated types for TypeSystemTest
// Do not edit manually
// Core Runtime Imports
import { Agent} from "./javascript/loader/IrInterpreter";
import { GoogleDriver } from "./javascript/loader/drivers/GoogleDriver";
import type { AgentIR } from "./javascript/loader/types/ir";
import type { SyntheticMessage, ConversationState, LifecycleHooks } from "./javascript/loader/types/protocol";

export interface Point {

    x: number;

    y: number;
}


export interface Address {

    street: string;

    city: string;

    zipCode?: string;
}


export interface User {

    id: string;

    name: string;

    address: Address;
}


/** Output type */
export interface AnalysisResult {

    /** High-level summary of findings */
    summary: string;

    /** Confidence score between 0 and 1 */
    confidence: number;

    /** List of key findings */
    keyFindings: string[];
}


/** Output type */
export interface SearchResult {

    /** The original search query */
    query: string;

    /** Array of search results */
    results: { title: string; url: string; snippet: string }[];

    /** Total number of results found */
    totalCount: number;
}

export interface TypeSystemTestInput {
    message: string;
}

export interface TypeSystemTestOutput {
    analysis: AnalysisResult;
    searchResults: SearchResult;
}

export interface TypeSystemTestContext {
    sessionId: string;
}

/**
 * API keys required for TypeSystemTest
 */
export interface TypeSystemTestApiKeys {
    geminiApiKey: string;
}


/**
 * Configuration for TypeSystemTest agent
 */
export interface TypeSystemTestConfig {
    apiKeys: TypeSystemTestApiKeys;
    ir: AgentIR;
    context?: TypeSystemTestContext;
}

/**
 * Create a type-safe TypeSystemTest agent instance
 * 
 * @example
 * ```typescript
 * const agent = createTypeSystemTest({
 *     apiKeys: { geminiApiKey: '...' },
 *     ir: agentIR,
 *     context: { sessionId: "123" },
 * });
 * 
 * // Clean execution - config bound at creation
 * const result = await agent.run({ ... });
 * const stream = await agent.stream({ ... });
 * ```
 */
export function createTypeSystemTest(config: TypeSystemTestConfig) {
    // Create agent with drivers
    const agent = new Agent<TypeSystemTestInput, TypeSystemTestOutput, TypeSystemTestContext, Record<string, never>>({
        gemini: new GoogleDriver(config.apiKeys.geminiApiKey)
    });
    
    // Load and validate IR immediately
    agent.load(config.ir);

    
    return {
        /**
         * Run the agent with type-safe parameters
         * @param input - Agent input
         * @param overrides - Optional overrides for context, tools, lifecycle, or configName
         */
        run: (input: TypeSystemTestInput, overrides?: { context?: TypeSystemTestContext; modelOverride?: { providerType?: string; modelName?: string; temperature?: number }; configName?: never }): Promise<TypeSystemTestOutput> => 
            agent.run(input, { context: overrides?.context ?? config.context, modelOverride: overrides?.modelOverride, configName: overrides?.configName }),
        
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
        stream: (input: TypeSystemTestInput, overrides?: { context?: TypeSystemTestContext; modelOverride?: { providerType?: string; modelName?: string; temperature?: number }; configName?: never }) => 
            agent.stream(input, { context: overrides?.context ?? config.context, modelOverride: overrides?.modelOverride, configName: overrides?.configName }),
        
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
        streamIterable: (input: TypeSystemTestInput, overrides?: { context?: TypeSystemTestContext; modelOverride?: { providerType?: string; modelName?: string; temperature?: number }; configName?: never }) => 
            agent.runStream(input, { context: overrides?.context ?? config.context, modelOverride: overrides?.modelOverride, configName: overrides?.configName }),
        
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
        forContext: (context: TypeSystemTestContext) => {
            const boundContext = context;
            return {
                run: (input: TypeSystemTestInput, overrides?: { configName?: never; modelOverride?: { providerType?: string; modelName?: string; temperature?: number } }) => 
                    agent.run(input, { context: boundContext, configName: overrides?.configName, modelOverride: overrides?.modelOverride }),
                stream: (input: TypeSystemTestInput, overrides?: { configName?: never; modelOverride?: { providerType?: string; modelName?: string; temperature?: number } }) => 
                    agent.stream(input, { context: boundContext, configName: overrides?.configName, modelOverride: overrides?.modelOverride }),
                streamIterable: (input: TypeSystemTestInput, overrides?: { configName?: never; modelOverride?: { providerType?: string; modelName?: string; temperature?: number } }) => 
                    agent.runStream(input, { context: boundContext, configName: overrides?.configName, modelOverride: overrides?.modelOverride })
            };
        }
    };
}

/** Type for the created agent instance */
export type TypeSystemTestAgent = ReturnType<typeof createTypeSystemTest>;
