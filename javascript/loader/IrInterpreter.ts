import { Synthesizer } from "./Synthesizer";
import type { AgentIR } from "./types/ir";
import type { AgentDriver, SyntheticMessage } from "./types/protocol";
import type { ToolMap } from "./types/tool"; // Import new type
import { WorkflowRunner } from "./WorkflowRunner";

export class Agent<
    TInput extends Record<string, any>,
    TOutput extends string | Record<string, any> = string,
    TTools extends object = ToolMap
> {
    private synthesizer: Synthesizer | null = null;
    private ir: AgentIR | null = null;
    private maxTurns = 10; // Prevent infinite loops

    constructor(private driver: AgentDriver) { }

    load(ir: AgentIR) {
        this.ir = ir;
        this.synthesizer = new Synthesizer(ir);
    }

    // Updated Signature: Accept tools!
   async run(
    input: TInput, tools?: TTools,
): Promise<TOutput> {
    if (!this.synthesizer || !this.ir) throw new Error("Agent not loaded");

    const safeTools = tools as unknown as ToolMap

    // 1. Initial Synthesis
    const request = await this.synthesizer.synthesize(input);
    let currentMessages: SyntheticMessage[] = [...request.messages];
    let turnCount = 0;
    let toolsStillAvailable = true; // Track if tools should be sent to model

    // 2. The Loop
    while (turnCount < this.maxTurns) {
        turnCount++;

        // Build the current request
        const currentRequest = {
            ...request,
            messages: currentMessages,
            // Remove tools after first tool call to enable structured output
            tools: toolsStillAvailable ? request.tools : undefined
        };

        // Execute Driver
        const result = await this.driver.execute(currentRequest);

        // CASE A: Tool Call
        if (result.toolParams) {
            const { name, args } = result.toolParams;

            // Add model's tool call to history
            currentMessages.push({
                role: 'assistant',
                content: `Call ${name} with ${JSON.stringify(args)}`
            });

            const workflow = this.ir.workflows?.find(w => w.flowName === name);

            try {
                let toolResult: any;

                // 1. Dispatch: Check if it's a Workflow or a Tool
                if (workflow) {
                    console.log(`[Agent] >>> Dispatching to Workflow: ${name}`);
                    const runner = new WorkflowRunner(this.ir, safeTools);
                    toolResult = await runner.run(name, args);
                    console.log(`[Agent] <<< Workflow ${name} completed.`);
                } else {
                    // It must be a user tool
                    if (!tools || !safeTools[name]) {
                        throw new Error(`Model tried to call unknown tool: ${name}`);
                    }
                    console.log(`[Agent] >>> Calling Tool: ${name}`);
                    toolResult = await safeTools[name](args);
                    console.log(`[Agent] <<< Tool ${name} completed.`);
                }

                // 2. Add result to history
                currentMessages.push({
                    role: 'user',
                    content: `Tool Result: ${JSON.stringify(toolResult)}`
                });

                // 3. IMPORTANT: Remove tools for next turn so model can return structured output
                toolsStillAvailable = false;

            } catch (e: any) {
                console.error(`[Agent] Execution Error:`, e);
                currentMessages.push({
                    role: 'user',
                    content: `Tool Error: ${e.message}`
                });
                // Still remove tools even on error
                toolsStillAvailable = false;
            }

            // Continue loop -> Model sees result -> Decides next step
            continue;
        }

        // CASE B: Final Text Result
       
        if (result.toolParams === undefined) {
            // Model didn't call a tool - it's giving a final response
            
            // If we expect JSON but got plain text, and tools were still available,
            // the model chose not to use tools. We should still try to get JSON.
            if (request.responseSchema) {
                // Try to parse as JSON
                try {
                    return JSON.parse(result.text ?? "{}") as TOutput;
                } catch (e) {
                    // If tools were available, model might have just responded with text
                    // Wrap the text in the expected schema format
                    if (toolsStillAvailable && request.responseSchema.properties) {
                        // Get the first property name from schema (e.g., "result")
                        const firstProp = Object.keys(request.responseSchema.properties)[0];
                        if (firstProp) {
                            return { [firstProp]: result.text } as TOutput;
                        }
                    }
                    console.error("Failed to parse JSON response:", result.text);
                    throw new Error("Model failed to return valid JSON");
                }
            }

            return (result.text ?? "") as TOutput;
        }


        return (result.text ?? "") as TOutput;
    }

    throw new Error("Agent exceeded maximum turns");
}


    /**
     * Executes a workflow defined in the agent's IR.
     */
    async executeWorkflow(name: string, args: Record<string, any>, tools?: ToolMap): Promise<any> {
        if (!this.ir) throw new Error("Agent not loaded");

        // Use provided tools or empty map if none
        const safeTools = (tools || {}) as ToolMap;

        const runner = new WorkflowRunner(this.ir, safeTools);
        return runner.run(name, args);
    }
}