// Auto-generated types for Chef
// Do not edit manually
// Core Runtime Imports
import { Agent } from "../javascript/loader/IrInterpreter";
import { DriverRegistry } from "../javascript/loader/DriverRegistry";
import type { AgentIR } from "../javascript/loader/types/ir";
export interface ChefInput {
    request: string;
}

export interface ChefOutput {
    result: string;
}

export interface ChefContext {

}


/**
 * Create a type-safe Chef agent instance
 */
export function createChef(registry: DriverRegistry) {
    const agent = new Agent<ChefInput, ChefOutput, Record<string, never>, Record<string, never>>(registry);
    
    return {
        /**
         * Load the agent IR configuration
         */
        load: (ir: AgentIR) => agent.load(ir),
        
        /**
         * Run the agent with type-safe parameters
         */
        run: (input: ChefInput, configName?: never): Promise<ChefOutput> => 
            agent.run(input, undefined, {}, configName)
    };
}
/** Type for the created agent instance */
export type ChefAgent = ReturnType<typeof createChef>;
