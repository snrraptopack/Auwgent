// Auto-generated types for MultiAgent
// Do not edit manually
// Core Runtime Imports
import { Agent } from "../javascript/loader/IrInterpreter";
import { DriverRegistry } from "../javascript/loader/DriverRegistry";
import type { AgentIR } from "../javascript/loader/types/ir";
export interface MultiAgentInput {
    q: string;
}

export interface MultiAgentOutput {
    a: string;
}

export interface MultiAgentContext {

}


/**
 * Create a type-safe MultiAgent agent instance
 */
export function createMultiAgent(registry: DriverRegistry) {
    const agent = new Agent<MultiAgentInput, MultiAgentOutput, Record<string, never>, Record<string, never>>(registry);
    
    return {
        /**
         * Load the agent IR configuration
         */
        load: (ir: AgentIR) => agent.load(ir),
        
        /**
         * Run the agent with type-safe parameters
         */
        run: (input: MultiAgentInput, configName?: "specialized"): Promise<MultiAgentOutput> => 
            agent.run(input, undefined, {}, configName)
    };
}
/** Type for the created agent instance */
export type MultiAgentAgent = ReturnType<typeof createMultiAgent>;
