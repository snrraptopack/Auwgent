// Auto-generated types for TypeSystemTest
// Do not edit manually
// Core Runtime Imports
import { Agent, RunConfig } from "../javascript/loader/IrInterpreter";
import { GoogleDriver } from "../javascript/loader/drivers/GoogleDriver";
import type { AgentIR } from "../javascript/loader/types/ir";
import type { SyntheticMessage, ConversationState, LifecycleHooks } from "../javascript/loader/types/protocol";

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
    data: Point[];
    user: User;
    mode: "fast" | "thorough";
}

export interface TypeSystemTestOutput {
    analysis: AnalysisResult;
    searchResults: SearchResult;
}

export interface TypeSystemTestContext {
    sessionId: string;
}

export interface TypeSystemTestTools {
    [key: string]: (args: any) => Promise<any>;
    calculateDistance: (args: { p1: Point, p2: Point }) => Promise<number>;
    getUserInfo: (args: { userId: string }) => Promise<User>;
    searchWeb: (args: { query: string }) => Promise<SearchResult>;
}

/**
 * API keys required for TypeSystemTest
 */
export interface TypeSystemTestApiKeys {
    geminiApiKey: string;
}


/**
 * Create a type-safe TypeSystemTest agent instance
 * Auto-creates drivers based on required providers
 */
export function createTypeSystemTest(apiKeys: TypeSystemTestApiKeys) {
    const agent = new Agent<TypeSystemTestInput, TypeSystemTestOutput, TypeSystemTestContext, TypeSystemTestTools>({
        gemini: new GoogleDriver(apiKeys.geminiApiKey)
    });
    
    return {
        /**
         * Load the agent IR configuration
         */
        load: (ir: AgentIR) => agent.load(ir),
        
        /**
         * Run the agent with type-safe parameters
         */
        run: (input: TypeSystemTestInput, tools: TypeSystemTestTools, context: TypeSystemTestContext, configName?: never): Promise<TypeSystemTestOutput> => 
            agent.run(input, { tools, context, configName }),
        
        /**
         * Fluent streaming API with callbacks
         * @example
         * const result = await agent
         *   .stream({ request: "..." })
         *   .onText(delta => console.log(delta))
         *   .run();
         */
        stream: (input: TypeSystemTestInput, tools: TypeSystemTestTools, context: TypeSystemTestContext, configName?: never) => 
            agent.stream(input, { tools, context, configName })
    };
}
/** Type for the created agent instance */
export type TypeSystemTestAgent = ReturnType<typeof createTypeSystemTest>;
