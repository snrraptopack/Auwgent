import { Synthesizer } from "./Synthesizer";
import type { AgentIR, HelperIR } from "./types/ir";
import type { AgentDriver, SyntheticMessage } from "./types/protocol";
import type { ToolMap } from "./types/tool"; // Import new type
import { WorkflowRunner } from "./WorkflowRunner";
import { DriverRegistry } from "./DriverRegistry";

export class Agent<
    TInput extends Record<string, any>,
    TOutput extends string | Record<string, any> = string,
    TContext extends Record<string, any> = Record<string, any>,
    TTools extends object = ToolMap
> {
    private synthesizer: Synthesizer | null = null;
    private ir: AgentIR | null = null;
    private maxTurns = 10; // Prevent infinite loops
    private context?: Record<string, any>

    // OPTIMIZATION: Cache helper agents to avoid recreation
    private helperCache = new Map<string, Agent<any, any>>();
    // OPTIMIZATION: Cache resolved drivers
    private driverCache = new Map<string, AgentDriver>();

    constructor(private registry: DriverRegistry) { }

    load(ir: AgentIR) {
        this.ir = ir;
        this.synthesizer = new Synthesizer(ir);

        // STRICT VALIDATION: Ensure we have drivers for ALL used models
        const requiredModels = this.synthesizer.getRequiredModels();
        const missingModels: string[] = [];

        for (const model of requiredModels) {
            if (!this.registry.resolve(model)) {
                missingModels.push(model);
            }
        }

        if (missingModels.length > 0) {
            throw new Error(`Missing drivers for the following models: ${missingModels.join(", ")}. Please register them with the DriverRegistry.`);
        }
    }

    async run(
        input: TInput,
        tools?: TTools,
        context?: TContext,
        configName?: string
    ): Promise<TOutput> {
        if (!this.synthesizer || !this.ir) throw new Error("Agent not loaded");

        const safeTools = tools as unknown as ToolMap

        this.context = context

        // 1. Initial Synthesis
        const request = await this.synthesizer.synthesize(input, context, configName);

        // RESOLVE DRIVER (cached)
        const modelName = request.config.model ?? "";
        const driver = this.getDriver(modelName);

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
            const result = await driver.execute(currentRequest);

            // CASE A: Tool Call
            if (result.toolParams) {
                const { name, args } = result.toolParams;

                // Add model's tool call to history
                currentMessages.push({
                    role: 'assistant',
                    content: `Call ${name} with ${JSON.stringify(args)}`
                });

                const workflow = this.ir.workflows?.find(w => w.flowName === name);
                const helper = this.ir.helpers?.find(h => h.name === name);

                try {
                    let toolResult: any;

                    // 1. Check if it's a Helper (sub-agent) - called directly by model
                    if (helper) {
                        console.log(`[Agent] >>> Delegating to Helper Agent: ${name}`);
                        toolResult = await this.executeHelper(helper, args);
                        console.log(`[Agent] <<< Helper ${name} completed.`);
                        // Helper called directly by model always returns to model
                    }
                    // 2. Check if it's a Workflow
                    else if (workflow) {
                        console.log(`[Agent] >>> Dispatching to Workflow: ${name}`);
                        const runner = new WorkflowRunner(
                            this.ir,
                            safeTools,
                            (helper, args) => this.executeHelper(helper, args)
                        );
                        toolResult = await runner.run(name, args, this.context);
                        console.log(`[Agent] <<< Workflow ${name} completed.`);

                        // Handle TransferSignal from workflow
                        if (toolResult && typeof toolResult === 'object' && toolResult.__type === 'TransferSignal') {
                            console.log(`[Agent] Transfer detected from workflow (mode: ${toolResult.mode})`);

                            if (toolResult.mode === "direct") {
                                // Direct transfer: return helper result to user immediately
                                console.log(`[Agent] Returning transferred result directly to user.`);
                                return toolResult.value as TOutput;
                            } else if (toolResult.mode === "thenContinue") {
                                // Then continue: mark that we should append after user gets the result
                                // For now, we'll send the transferred value and continue
                                // TODO: Implement streaming/async continuation
                                console.log(`[Agent] Transfer with continue - sending result, workflow continues.`);
                                // Add the transfer result to messages and continue
                                currentMessages.push({
                                    role: 'user',
                                    content: `Helper Result (sent to user): ${JSON.stringify(toolResult.value)}`
                                });
                                toolsStillAvailable = false;
                                continue;
                            }
                        }
                    } else {
                        // It must be a user tool
                        if (!tools || !safeTools[name]) {
                            throw new Error(`Model tried to call unknown tool: ${name}`);
                        }
                        console.log(`[Agent] >>> Calling Tool: ${name}`);
                        toolResult = await safeTools[name](args);
                        console.log(`[Agent] <<< Tool ${name} completed.`);
                    }

                    // 3. Add result to history
                    currentMessages.push({
                        role: 'user',
                        content: `Tool Result: ${JSON.stringify(toolResult)}`
                    });

                    // 4. IMPORTANT: Remove tools for next turn so model can return structured output
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

        const runner = new WorkflowRunner(
            this.ir,
            safeTools,
            (helper, args) => this.executeHelper(helper, args)
        );
        return runner.run(name, args);
    }

    /**
     * Executes a helper as a sub-agent (with caching).
     */
    private async executeHelper(helper: HelperIR, args: Record<string, any>): Promise<any> {
        // Check cache first
        let subAgent = this.helperCache.get(helper.name);

        if (!subAgent) {
            // Create and cache the helper agent
            subAgent = new Agent<Record<string, any>, any>(this.registry);

            // Convert HelperIR to AgentIR-like structure for loading
            const helperAsAgent: AgentIR = {
                name: helper.name,
                modelConfig: helper.modelConfig,
                input: helper.input,
                output: helper.output,
                context: helper.context,
                tools: helper.tools,
                workflows: helper.workflows,
                helpers: [] // Helpers don't have nested helpers (for now)
            };

            subAgent.load(helperAsAgent);
            this.helperCache.set(helper.name, subAgent);
            console.log(`[Agent] Helper ${helper.name} cached for future calls.`);
        } else {
            console.log(`[Agent] Using cached helper: ${helper.name}`);
        }

        // Run the helper with provided args
        return await subAgent.run(args);
    }

    /**
     * Get or cache a driver for a model name.
     */
    private getDriver(modelName: string): AgentDriver {
        let driver = this.driverCache.get(modelName);

        if (!driver) {
            driver = this.registry.resolve(modelName);
            if (!driver) {
                throw new Error(`No driver found for model: ${modelName}`);
            }
            this.driverCache.set(modelName, driver);
        }

        return driver;
    }
}