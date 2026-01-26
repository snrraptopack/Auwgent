// Auto-generated types for TestAutoReg
// Do not edit manually
// Core Runtime Imports
import { Agent, RunConfig } from "./javascript/loader/IrInterpreter";
import { GoogleDriver } from "./javascript/loader/drivers/GoogleDriver";
import type { AgentIR } from "./javascript/loader/types/ir";
import type { SyntheticMessage, ConversationState, LifecycleHooks } from "./javascript/loader/types/protocol";
export interface TestAutoRegInput {
    message: string;
}

export interface TestAutoRegOutput {
    reply: string;
}

export interface TestAutoRegContext {
    chatId: string;
}

/**
 * Lifecycle hooks for TestAutoReg
 * Implement these to manage conversation history and memory
 */
export interface TestAutoRegLifecycle {
    prune: (args: {
        context: TestAutoRegContext;
        agent: any;
        usage: {
            currentTokens: number;
            maxTokens: number;
            currentMessages: number;
            maxMessages: number;
        };
    }) => Promise<ConversationState>;

    load: (args: {
        context: TestAutoRegContext;
    }) => Promise<ConversationState>;

    save: (args: {
        newMessages: SyntheticMessage[];
        context: TestAutoRegContext;
        output: TestAutoRegOutput;
    }) => Promise<void>;
}

/**
 * API keys required for TestAutoReg
 */
export interface TestAutoRegApiKeys {
    geminiApiKey: string;
}


/**
 * Create a type-safe TestAutoReg agent instance
 * Auto-creates drivers based on required providers
 */
export function createTestAutoReg(apiKeys: TestAutoRegApiKeys) {
    const agent = new Agent<TestAutoRegInput, TestAutoRegOutput, TestAutoRegContext, Record<string, never>>({
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
        run: (input: TestAutoRegInput, context: TestAutoRegContext, lifecycle: TestAutoRegLifecycle, configName?: never): Promise<TestAutoRegOutput> =>
            agent.run(input, { context, lifecycle, configName }),

        /**
         * Fluent streaming API with callbacks
         * @example
         * const result = await agent
         *   .stream({ request: "..." })
         *   .onText(delta => console.log(delta))
         *   .run();
         */
        stream: (input: TestAutoRegInput, context: TestAutoRegContext, lifecycle: TestAutoRegLifecycle, configName?: never) =>
            agent.stream(input, { context, lifecycle, configName })
    };
}
/** Type for the created agent instance */
export type TestAutoRegAgent = ReturnType<typeof createTestAutoReg>;
