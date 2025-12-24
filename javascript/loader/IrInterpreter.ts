import { Synthesizer } from "./Synthesizer";
import type { AgentIR, HelperIR } from "./types/ir";
import type { AgentDriver, StreamChunk, SyntheticMessage } from "./types/protocol";
import type { ToolMap } from "./types/tool"; // Import new type
import { WorkflowRunner } from "./WorkflowRunner";
import { DriverRegistry } from "./DriverRegistry";
import { StreamBuilder } from "./StreamBuilder";

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
    // GUARD: Track completed calls to prevent infinite loops after thenContinue
    private completedCalls = new Set<string>();

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

                // GUARD: Check if this exact call was already completed via thenContinue
                const callSignature = `${name}::${JSON.stringify(args, Object.keys(args).sort())}`;
                if (this.completedCalls.has(callSignature)) {
                    console.log(`[Agent] BLOCKED: Duplicate call to "${name}" - already completed.`);
                    currentMessages.push({
                        role: 'user',
                        content: `[SYSTEM ERROR] You already completed "${name}" with these exact arguments. The result is in your conversation history. Do NOT repeat this call. Either proceed with a different task or finish by responding to the user.`
                    });
                    continue; // Skip execution, let model see error
                }

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
                                // thenContinue: result already sent to user, model can optionally add summary
                                console.log(`[Agent] Transfer with thenContinue - model can wrap up.`);

                                // GUARD: Track this call to prevent re-execution
                                this.completedCalls.add(callSignature);
                                console.log(`[Agent] Added to completed calls: ${callSignature}`);

                                currentMessages.push({
                                    role: 'user',
                                    content: `[System] The workflow "${name}" completed via helper "${toolResult.helperName}". The result was already delivered to the user. This specific task is done. If the user's request has additional parts requiring a DIFFERENT task, you may proceed. Otherwise, provide a brief acknowledgment and finish.`
                                });
                                // toolsStillAvailable = false so model can only respond with text
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
     * Fluent streaming API - returns a StreamBuilder for clean callback-based streaming.
     * 
     * @example
     * ```typescript
     * const result = await agent
     *   .stream({ request: "Create a login page" })
     *   .onText(delta => process.stdout.write(delta))
     *   .onHelperStart(name => console.log(`>>> ${name}`))
     *   .run();
     * ```
     */
    stream(
        input: TInput,
        tools?: TTools,
        context?: TContext,
        configName?: string
    ): StreamBuilder<TOutput> {
        return new StreamBuilder(() => this.runStream(input, tools, context, configName));
    }

    /**
     * Internal streaming execution - yields chunks as they arrive.
     * Use stream() for the public API.
     */
    private async *runStream(
        input: TInput,
        tools?: TTools,
        context?: TContext,
        configName?: string
    ): AsyncGenerator<StreamChunk, TOutput, unknown> {
        if (!this.synthesizer || !this.ir) throw new Error("Agent not loaded");

        const safeTools = tools as unknown as ToolMap;
        this.context = context;

        const request = await this.synthesizer.synthesize(input, context, configName);
        const modelName = request.config.model ?? "";
        const driver = this.getDriver(modelName);

        // Check if driver supports streaming
        if (!driver.executeStream) {
            // Fallback to non-streaming
            console.warn(`[Agent] Driver ${driver.name} does not support streaming, falling back to non-streaming`);
            const result = await this.run(input, tools, context, configName);
            return result;
        }

        let currentMessages: SyntheticMessage[] = [...request.messages];
        let turnCount = 0;
        let toolsStillAvailable = true;

        while (turnCount < this.maxTurns) {
            turnCount++;

            const currentRequest = {
                ...request,
                messages: currentMessages,
                tools: toolsStillAvailable ? request.tools : undefined
            };

            // Use streaming execution
            const stream = driver.executeStream(currentRequest);
            let result;

            // Yield chunks and capture final result
            while (true) {
                const { value, done } = await stream.next();
                if (done) {
                    result = value;
                    break;
                }
                yield value;  // Forward chunk to caller
            }

            // Handle tool calls (same logic as non-streaming)
            if (result.toolParams) {
                const { name, args } = result.toolParams;
                let toolResult: any;

                // GUARD: Check if this exact call was already completed via thenContinue
                const callSignature = `${name}::${JSON.stringify(args, Object.keys(args).sort())}`;
                if (this.completedCalls.has(callSignature)) {
                    console.log(`[Agent] BLOCKED: Duplicate call to "${name}" - already completed.`);
                    currentMessages.push({
                        role: 'user',
                        content: `[SYSTEM ERROR] You already completed "${name}" with these exact arguments. The result is in your conversation history. Do NOT repeat this call. Either proceed with a different task or finish by responding to the user.`
                    });
                    continue; // Skip execution, let model see error
                }

                // Check workflows first
                const workflow = this.ir.workflows?.find(w => w.flowName === name);
                if (workflow) {
                    console.log(`[Agent] >>> Dispatching to Workflow (streaming): ${name}`);

                    // Use async generator - yields chunks immediately!
                    const workflowStream = this.executeWorkflowStream(name, args, safeTools);
                    let workflowResult: any;

                    while (true) {
                        const { value, done } = await workflowStream.next();
                        if (done) {
                            workflowResult = value;
                            break;
                        }
                        yield value;  // Forward chunks immediately!
                    }

                    console.log(`[Agent] <<< Workflow ${name} completed.`);

                    // Handle TransferSignal from workflow
                    if (workflowResult && typeof workflowResult === 'object' && workflowResult.__type === 'TransferSignal') {
                        // Yield transfer event to client
                        yield {
                            type: 'transfer',
                            mode: workflowResult.mode,
                            helperName: workflowResult.helperName || name
                        };

                        if (workflowResult.mode === "direct") {
                            console.log(`[Agent] Transfer detected from workflow (mode: direct)`);
                            return workflowResult.value as TOutput;
                        } else if (workflowResult.mode === "thenContinue") {
                            // thenContinue: result already sent to user, model can optionally add summary
                            console.log(`[Agent] Transfer with thenContinue - model can wrap up.`);

                            // GUARD: Track this call to prevent re-execution
                            this.completedCalls.add(callSignature);
                            console.log(`[Agent] Added to completed calls: ${callSignature}`);

                            currentMessages.push({
                                role: 'user',
                                content: `[System] The workflow "${name}" completed via helper "${workflowResult.helperName}". The result was already delivered to the user. This specific task is done. If the user's request has additional parts requiring a DIFFERENT task, you may proceed. Otherwise, provide a brief acknowledgment and finish.`
                            });
                            // toolsStillAvailable = false so model can only respond with text
                            toolsStillAvailable = false;
                            continue;
                        }
                    }
                    toolResult = workflowResult;
                } else {
                    // Check helpers
                    const helper = this.ir.helpers?.find(h => h.name === name);
                    if (helper) {
                        console.log(`[Agent] >>> Calling Helper (streaming): ${name}`);
                        // Use streaming helper execution
                        const helperStream = this.executeHelperStream(helper, args);
                        while (true) {
                            const { value, done } = await helperStream.next();
                            if (done) {
                                toolResult = value;
                                break;
                            }
                            yield value;  // Forward helper chunks to caller
                        }
                        console.log(`[Agent] <<< Helper ${name} completed.`);
                    } else {
                        // User tool
                        if (!safeTools || !safeTools[name]) {
                            throw new Error(`Unknown tool: ${name}`);
                        }
                        console.log(`[Agent] >>> Calling Tool: ${name}`);
                        toolResult = await safeTools[name](args);
                        console.log(`[Agent] <<< Tool ${name} completed.`);
                    }
                }

                // Yield tool result to client
                yield { type: 'tool_result', name, result: toolResult };

                currentMessages.push({
                    role: 'user',
                    content: `Tool Result: ${JSON.stringify(toolResult)}`
                });
                // toolsStillAvailable = false; // Allow multi-turn tool usage
                continue;
            }

            // Final response
            if (request.responseSchema) {
                try {
                    return JSON.parse(result.text ?? "{}") as TOutput;
                } catch (e) {
                    if (toolsStillAvailable && request.responseSchema.properties) {
                        const firstProp = Object.keys(request.responseSchema.properties)[0];
                        if (firstProp) {
                            return { [firstProp]: result.text } as TOutput;
                        }
                    }
                    throw new Error("Model failed to return valid JSON");
                }
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
     * Executes a workflow with streaming - async generator that yields chunks immediately.
     */
    async *executeWorkflowStream(
        name: string,
        args: Record<string, any>,
        tools?: ToolMap
    ): AsyncGenerator<StreamChunk, any, unknown> {
        if (!this.ir) throw new Error("Agent not loaded");

        const safeTools = (tools || {}) as ToolMap;

        const runner = new WorkflowRunner(
            this.ir,
            safeTools,
            (helper, args) => this.executeHelper(helper, args),
            (helper, args) => this.executeHelperStream(helper, args)
        );

        // Yield* forwards all chunks and return value
        return yield* runner.runStream(name, args);
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

            // Resolve granted tools from parent
            const grantedTools = this.resolveGrantedTools(helper.name);

            // Convert HelperIR to AgentIR-like structure for loading
            const helperAsAgent: AgentIR = {
                name: helper.name,
                modelConfig: helper.modelConfig,
                input: helper.input,
                output: helper.output,
                context: helper.context,
                tools: [...helper.tools, ...grantedTools],  // Merge helper's tools with granted parent tools
                workflows: helper.workflows,
                helpers: [] // Helpers don't have nested helpers (for now)
            };

            subAgent.load(helperAsAgent);
            this.helperCache.set(helper.name, subAgent);
            console.log(`[Agent] Helper ${helper.name} cached for future calls.${grantedTools.length > 0 ? ` (with ${grantedTools.length} granted tools)` : ''}`);
        } else {
            console.log(`[Agent] Using cached helper: ${helper.name}`);
        }

        // Run the helper with provided args
        return await subAgent.run(args);
    }

    /**
     * Executes a helper as a sub-agent with streaming (yields chunks).
     * Returns final result after streaming completes.
     */
    private async *executeHelperStream(
        helper: HelperIR,
        args: Record<string, any>
    ): AsyncGenerator<StreamChunk, any, unknown> {
        // Get or create cached helper agent
        let subAgent = this.helperCache.get(helper.name);

        if (!subAgent) {
            subAgent = new Agent<Record<string, any>, any>(this.registry);

            const grantedTools = this.resolveGrantedTools(helper.name);
            const helperAsAgent: AgentIR = {
                name: helper.name,
                modelConfig: helper.modelConfig,
                input: helper.input,
                output: helper.output,
                context: helper.context,
                tools: [...helper.tools, ...grantedTools],
                workflows: helper.workflows,
                helpers: []
            };

            subAgent.load(helperAsAgent);
            this.helperCache.set(helper.name, subAgent);
            console.log(`[Agent] Helper ${helper.name} cached for streaming.`);
        }

        // Emit helper start
        yield { type: 'helper_start', name: helper.name };

        // Stream from helper
        const stream = subAgent.runStream(args);
        let result: any;

        while (true) {
            const { value, done } = await stream.next();
            if (done) {
                result = value;
                break;
            }
            // Wrap each chunk with helper context
            yield { type: 'helper_chunk', name: helper.name, chunk: value };
        }

        // Emit helper end with result
        yield { type: 'helper_end', name: helper.name, result };

        return result;
    }

    /**
     * Resolve granted tools from parent for a specific helper
     */
    private resolveGrantedTools(helperName: string): any[] {
        if (!this.ir) return [];

        const grants = this.ir.helperToolGrants;
        if (!grants || !grants[helperName]) {
            return [];
        }

        const grant = grants[helperName];

        if (grant === "all") {
            // Return all parent tools
            return this.ir.tools || [];
        }

        // Return only the specified tools
        return (this.ir.tools || []).filter(t => grant.includes(t.name));
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