// Auto-generated types for School
// Do not edit manually
// Core Runtime Imports
import { Agent } from "./javascript/loader/IrInterpreter";
import { GoogleDriver } from "./javascript/loader/drivers/GoogleDriver";
import type { AgentIR } from "./javascript/loader/types/ir";
import type { SyntheticMessage, ConversationState, LifecycleHooks } from "./javascript/loader/types/protocol";

export interface Student {

    name: string;

    class: string;

    age: string;
}


/** Output type */
export interface Response {

    student: Student;
}

export interface SchoolInput {
    text: string;
}

export interface SchoolOutput {
    response: Response;
}

export interface SchoolContext {

}

export interface SchoolTools {
    [key: string]: (args: any) => Promise<any>;
    get_student_with_lower_grade: (args: {  }) => Promise<Student>;
    get_student_with_higher_grade: (args: {  }) => Promise<Student>;
}

/**
 * API keys required for School
 */
export interface SchoolApiKeys {
    geminiApiKey: string;
}


/**
 * Configuration for School agent
 */
export interface SchoolConfig {
    apiKeys: SchoolApiKeys;
    ir: AgentIR;
    tools?: SchoolTools;
}

/**
 * Create a type-safe School agent instance
 * 
 * @example
 * ```typescript
 * const agent = createSchool({
 *     apiKeys: { geminiApiKey: '...' },
 *     ir: agentIR,
 *     tools: { ... },
 * });
 * 
 * // Clean execution - config bound at creation
 * const result = await agent.run({ ... });
 * const stream = await agent.stream({ ... });
 * ```
 */
export function createSchool(config: SchoolConfig) {
    // Create agent with drivers
    const agent = new Agent<SchoolInput, SchoolOutput, Record<string, never>, SchoolTools>({
        gemini: new GoogleDriver(config.apiKeys.geminiApiKey)
    });
    
    // Load and validate IR immediately
    agent.load(config.ir);

    // Validate tools match IR requirements
    if (config.ir.tools && config.ir.tools.length > 0) {
        for (const toolDef of config.ir.tools) {
            if (!config.tools?.[toolDef.name]) {
                throw new Error(
                    `Missing required tool: ${toolDef.name}\n` +
                    `Expected in tools configuration`
                );
            }
        }
    }
    
    return {
        /**
         * Run the agent with type-safe parameters
         * @param input - Agent input
         * @param overrides - Optional overrides for context, tools, lifecycle, or configName
         */
        run: (input: SchoolInput, overrides?: { tools?: SchoolTools; modelOverride?: { providerType?: string; modelName?: string; temperature?: number }; configName?: never }): Promise<SchoolOutput> => 
            agent.run(input, { tools: overrides?.tools ?? config.tools, modelOverride: overrides?.modelOverride, configName: overrides?.configName }),
        
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
        stream: (input: SchoolInput, overrides?: { tools?: SchoolTools; modelOverride?: { providerType?: string; modelName?: string; temperature?: number }; configName?: never }) => 
            agent.stream(input, { tools: overrides?.tools ?? config.tools, modelOverride: overrides?.modelOverride, configName: overrides?.configName }),
        
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
        streamIterable: (input: SchoolInput, overrides?: { tools?: SchoolTools; modelOverride?: { providerType?: string; modelName?: string; temperature?: number }; configName?: never }) => 
            agent.runStream(input, { tools: overrides?.tools ?? config.tools, modelOverride: overrides?.modelOverride, configName: overrides?.configName }),
    };
}

/** Type for the created agent instance */
export type SchoolAgent = ReturnType<typeof createSchool>;
