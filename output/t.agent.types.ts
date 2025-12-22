// Auto-generated types for OrderProcessor
// Do not edit manually
// To use the factory function, import Agent and AgentDriver from your loader:
import { Agent } from "../javascript/loader/IrInterpreter";
import type { AgentDriver } from "../javascript/loader/types/protocol";
import type { AgentIR } from "../javascript/loader/types/ir";
export interface OrderProcessorInput {
    request: string;
}

export interface OrderProcessorOutput {
    result: string;
}

export interface OrderProcessorContext {
    id: unknown;
    isAdmin: boolean;
}

export interface OrderProcessorTools {
    [key: string]: (args: any) => Promise<any>;
    getstudentgrade: (args: { id: number }) => Promise<string>;
    getstudentlocation: (args: { id: number }) => Promise<string>;
    totalstudent: (args: {}) => Promise<number>;
    getstudentname: (args: { id: number }) => Promise<string>;
}


/**
 * Create a type-safe OrderProcessor agent instance
 */
export function createOrderProcessor(driver: AgentDriver) {
    const agent = new Agent<OrderProcessorInput, OrderProcessorOutput, OrderProcessorContext, OrderProcessorTools>(driver);

    return {
        /**
         * Load the agent IR configuration
         */
        load: (ir: AgentIR) => agent.load(ir),

        /**
         * Run the agent with type-safe parameters
         */
        run: (input: OrderProcessorInput, tools: OrderProcessorTools, context: OrderProcessorContext): Promise<OrderProcessorOutput> =>
            agent.run(input, tools, context)
    };
}

/** Type for the created agent instance */
export type OrderProcessorAgent = ReturnType<typeof createOrderProcessor>;
