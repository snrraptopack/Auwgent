// Auto-generated types for Manager
// Do not edit manually
// Core Runtime Imports
import { Agent } from "../javascript/loader/IrInterpreter";
import { DriverRegistry } from "../javascript/loader/DriverRegistry";
import type { AgentIR } from "../javascript/loader/types/ir";
export interface ManagerInput {
    request: string;
}

export interface ManagerOutput {
    result: string;
}

export interface ManagerContext {

}


/**
 * Create a type-safe Manager agent instance
 */
export function createManager(registry: DriverRegistry) {
    const agent = new Agent<ManagerInput, ManagerOutput, Record<string, never>, Record<string, never>>(registry);
    
    return {
        /**
         * Load the agent IR configuration
         */
        load: (ir: AgentIR) => agent.load(ir),
        
        /**
         * Run the agent with type-safe parameters
         */
        run: (input: ManagerInput, configName?: never): Promise<ManagerOutput> => 
            agent.run(input, undefined, {}, configName)
    };
}
/** Type for the created agent instance */
export type ManagerAgent = ReturnType<typeof createManager>;
