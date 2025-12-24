// Auto-generated types for UIMaker
// Do not edit manually
// Core Runtime Imports
import { Agent } from "../javascript/loader/IrInterpreter";
import { DriverRegistry } from "../javascript/loader/DriverRegistry";
import type { AgentIR } from "../javascript/loader/types/ir";
export interface UIMakerInput {
    request: string;
}

export interface UIProgrammerOutput {
    code: string;
}

export interface UIMakerBaseOutput {
    result: string;
}

/** Union of possible output types (includes transfer destinations) */
export type UIMakerOutput = UIMakerBaseOutput | UIProgrammerOutput;

export interface UIMakerContext {

}


/**
 * Create a type-safe UIMaker agent instance
 */
export function createUIMaker(registry: DriverRegistry) {
    const agent = new Agent<UIMakerInput, UIMakerOutput, Record<string, never>, Record<string, never>>(registry);
    
    return {
        /**
         * Load the agent IR configuration
         */
        load: (ir: AgentIR) => agent.load(ir),
        
        /**
         * Run the agent with type-safe parameters
         */
        run: (input: UIMakerInput, configName?: never): Promise<UIMakerOutput> => 
            agent.run(input, undefined, {}, configName),
        
        /**
         * Fluent streaming API with callbacks
         * @example
         * const result = await agent
         *   .stream({ request: "..." })
         *   .onText(delta => console.log(delta))
         *   .run();
         */
        stream: (input: UIMakerInput, configName?: never) => 
            agent.stream(input, undefined, {}, configName)
    };
}
/** Type for the created agent instance */
export type UIMakerAgent = ReturnType<typeof createUIMaker>;
