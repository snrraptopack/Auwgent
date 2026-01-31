import { Synthesizer } from "./Synthesizer";
import type { AgentIR, HelperIR } from "./types/ir";
import type { AgentDriver, AgentMiddleware, ContentBlock, DriverResult, StreamChunk, SyntheticMessage, ToolArgs, ToolCall, ToolResult } from "./types/protocol";
import type { ToolMap } from "./types/tool";
import { WorkflowRunner } from "./WorkflowRunner";
import { StreamBuilder } from "./StreamBuilder";
import { DriverRegistry } from "./DriverRegistry";
import { logger } from "./Logger";
import { ConfigurationError } from "./types/errors";
import { createMiddlewareContext, runOnAfterModel, runOnAfterTool, runOnAgentEnd, runOnAgentStart, runOnBeforeModel, runOnBeforeTool, runOnThinking, runWithRetries, sortMiddlewares, wrapModelCall, wrapToolCall } from "./IrMiddleware";

/**
 * Configuration object for agent.run()
 */
export interface RunConfig<TTools = ToolMap, TContext = Record<string, unknown>, TOutput = unknown> {
    tools?: TTools;
    context?: TContext;
    configName?: string;
    modelOverride?: {
        providerType?: string;
        modelName?: string;
        temperature?: number;
    };
    middleware?: AgentMiddleware<any, any, any>[];
    middlewareState?: Record<string, any>;
    runId?: string;
}

export class Agent<
    TInput extends Record<string, any>,
    TOutput extends string | Record<string, any> = string,
    TContext extends Record<string, any> = Record<string, any>,
    TTools extends object = ToolMap
> {
    private synthesizer: Synthesizer | null = null;
    private ir: AgentIR | null = null;
    private maxTurns = 10; // Prevent infinite loops

    // OPTIMIZATION: Cache helper agents to avoid recreation
    private helperCache = new Map<string, Agent<any, any>>();
    // OPTIMIZATION: Cache resolved drivers
    private driverCache = new Map<string, AgentDriver>();
    private driverRegistry: DriverRegistry;

    constructor(private drivers: Record<string, AgentDriver>, driverRegistry?: DriverRegistry) {
        this.driverRegistry = driverRegistry ?? new DriverRegistry();
        this.registerDefaultDrivers();
    }

    load(ir: AgentIR) {
        this.ir = ir;
        this.synthesizer = new Synthesizer(ir);
        this.helperCache.clear();
        this.driverCache.clear();

        // STRICT VALIDATION: Ensure we have drivers for ALL required provider types
        const requiredProviders = this.synthesizer.getRequiredModels();
        const missingProviders: string[] = [];

        for (const providerType of requiredProviders) {
            if (!this.drivers[providerType]) {
                missingProviders.push(providerType);
            }
        }

        if (missingProviders.length > 0) {
            throw new ConfigurationError(`Missing drivers for providers: ${missingProviders.join(", ")}. Required drivers: { ${missingProviders.map(p => `${p}: Driver`).join(", ")} }`);
        }
    }

    async run(
        input: TInput,
        config?: RunConfig<TTools, TContext, TOutput>
    ): Promise<TOutput> {
        if (!this.synthesizer || !this.ir) throw new Error("Agent not loaded");
        const synthesizer = this.synthesizer;
        const ir = this.ir;

        // Destructure config
        const { tools, context, configName, modelOverride } = config ?? {};
        const safeTools = tools as unknown as ToolMap;
        const runContext = context as TContext | undefined;

        // 1. Initial Synthesis
        const request = await synthesizer.synthesize(input, context, configName);
        if (modelOverride?.providerType) {
            request.config.model = modelOverride.providerType;
        }
        if (modelOverride?.modelName) {
            request.config.modelName = modelOverride.modelName;
        }
        if (modelOverride?.temperature !== undefined) {
            request.config.temperature = modelOverride.temperature;
        }

        const providerType = request.config.model ?? "";
        const driver = this.getDriver(providerType, request.config.modelName);
        const middlewares = sortMiddlewares(config?.middleware);
        const ctx = createMiddlewareContext(this.ir?.name || "unknown", input, runContext, request, config?.middlewareState, config?.runId);
        let ended = false;
        const complete = async (output: TOutput, result?: DriverResult) => {
            if (!ended) {
                ended = true;
                await runOnAgentEnd(middlewares, ctx, result);
            }
            return output;
        };

        try {
            await runOnAgentStart(middlewares, ctx);

            let currentMessages: SyntheticMessage[] = [...request.messages];
            let turnCount = 0;
            let toolsStillAvailable = true;
            const completedCalls = new Set<string>();

            while (turnCount < this.maxTurns) {
                turnCount++;

                const currentRequest = {
                    ...request,
                    messages: currentMessages,
                    tools: toolsStillAvailable ? request.tools : undefined
                };

                ctx.request = currentRequest;
                const modifiedRequest = await runOnBeforeModel(middlewares, ctx);
                if (modifiedRequest) {
                    ctx.request = modifiedRequest;
                }

                const result = await runWithRetries(middlewares, ctx, "model", async () => {
                    const wrapped = wrapModelCall(middlewares, ctx, () => driver.execute(ctx.request));
                    return wrapped();
                });

                ctx.response = result;
                const afterThinking = await runOnThinking(middlewares, ctx, result);
                const afterModel = await runOnAfterModel(middlewares, ctx, afterThinking);
                const finalResult = afterModel ?? afterThinking;
                ctx.response = finalResult;

                if (finalResult.toolCall) {
                    const { id, name, args } = finalResult.toolCall;
                    const callSignature = `${name}::${JSON.stringify(args, Object.keys(args).sort())}`;
                    if (completedCalls.has(callSignature)) {
                        logger.debug(`[Agent] BLOCKED: Duplicate call to "${name}" - already completed.`);
                        currentMessages.push({
                            role: 'user',
                            content: this.textBlocks(`[SYSTEM ERROR] You already completed "${name}" with these exact arguments. The result is in your conversation history. Do NOT repeat this call. Either proceed with a different task or finish by responding to the user.`)
                        });
                        continue;
                    }

                    currentMessages.push({
                        role: 'assistant',
                        content: [{ type: 'tool_use', id, name, input: args }],
                        toolCalls: [finalResult.toolCall]
                    });

                    const workflow = ir.workflows?.find(w => w.flowName === name);
                    const helper = ir.helpers?.find(h => h.name === name);

                    try {
                        const toolCall: ToolCall = { id, name, args };
                        const toolUseBlock = { type: 'tool_use', id, name, input: args } as const;
                        const shouldContinue = await runOnBeforeTool(middlewares, ctx, toolUseBlock);
                        if (shouldContinue === false) {
                            throw new Error("Tool execution blocked");
                        }

                        const toolResult = await runWithRetries(middlewares, ctx, "tool", async () => {
                            const wrappedTool = wrapToolCall(middlewares, ctx, toolUseBlock, async (toolArgs: ToolArgs) => {
                                if (helper) {
                                    logger.debug(`[Agent] >>> Delegating to Helper Agent: ${name}`);
                                    logger.trackHelperCall(name);
                                    const result = await this.executeHelper(helper, toolArgs);
                                    logger.debug(`[Agent] <<< Helper ${name} completed.`);
                                    return result;
                                }
                                if (workflow) {
                                    logger.debug(`[Agent] >>> Dispatching to Workflow: ${name}`);
                                    logger.trackWorkflowCall(name);
                                    const runner = new WorkflowRunner(
                                        ir,
                                        safeTools,
                                        (helperTool, helperArgs) => this.executeHelper(helperTool, helperArgs)
                                    );
                                    const workflowResult = await runner.run(name, toolArgs, runContext);
                                    logger.debug(`[Agent] <<< Workflow ${name} completed.`);
                                    return workflowResult;
                                }
                                const isDeclaredTool = ir.tools?.some(t => t.name === name);
                                if (!isDeclaredTool) {
                                    throw new Error(`Model tried to call unknown tool: ${name}`);
                                }
                                if (!tools || !safeTools[name]) {
                                    throw new Error(`Tool implementation missing for: ${name}`);
                                }
                                logger.debug(`[Agent] >>> Calling Tool: ${name}`);
                                logger.trackToolCall(name);
                                const result = await safeTools[name](toolArgs);
                                logger.debug(`[Agent] <<< Tool ${name} completed.`);
                                return result;
                            });
                            return wrappedTool(toolCall.args);
                        });

                        await runOnAfterTool(middlewares, ctx, toolUseBlock, toolResult);

                        const transfer = workflow
                            ? this.getTransferSignal(toolResult)
                            : this.getHelperHandoffSignal(helper?.name, toolResult);
                        if (transfer) {
                            logger.debug(`[Agent] Transfer detected (mode: ${transfer.mode})`);

                            if (transfer.mode === "direct") {
                                return await complete(transfer.value as TOutput, finalResult);
                            }
                            if (transfer.mode === "thenContinue") {
                                completedCalls.add(callSignature);
                                logger.debug(`[Agent] Added to completed calls: ${callSignature}`);
                                const source = workflow ? `workflow "${name}"` : `helper "${name}"`;
                                currentMessages.push({
                                    role: 'user',
                                    content: this.textBlocks(`[System] The ${source} delivered its result to the user. This specific task is done. If the user's request has additional parts requiring a DIFFERENT task, you may proceed. Otherwise, provide a brief acknowledgment and finish.`)
                                });
                                toolsStillAvailable = false;
                                continue;
                            }
                        }

                        const resultPrefix = helper
                            ? `Helper Result [${name}]`
                            : workflow
                                ? `Workflow Result [${name}]`
                                : `Tool Result [${name}]`;

                        currentMessages.push({
                            role: 'user',
                            content: [{
                                type: 'tool_result',
                                toolUseId: id,
                                content: `${resultPrefix}: ${JSON.stringify(toolResult)}`
                            }]
                        });

                        toolsStillAvailable = false;
                    } catch (e: any) {
                        console.error(`[Agent] Execution Error:`, e);
                        currentMessages.push({
                            role: 'user',
                            content: [{
                                type: 'tool_result',
                                toolUseId: id,
                                content: `Tool Error: ${e.message}`,
                                isError: true
                            }]
                        });
                        toolsStillAvailable = false;
                    }

                    continue;
                }

                const textOutput = this.extractText(finalResult);
                if (request.responseSchema) {
                    try {
                        const parsed = JSON.parse(textOutput ?? "{}") as TOutput;
                        return await complete(parsed, finalResult);
                    } catch (e) {
                        console.error("Failed to parse JSON response:", textOutput);
                        throw new Error("Model failed to return valid JSON");
                    }
                }

                return await complete((textOutput ?? "") as TOutput, finalResult);
            }

            throw new Error("Agent exceeded maximum turns");
        } catch (error: any) {
            if (!ended) {
                ended = true;
                await runOnAgentEnd(middlewares, ctx, ctx.response, error);
            }
            throw error;
        }
    }

    /**
     * Fluent streaming API - returns a StreamBuilder for clean callback-based streaming.
     * 
     * @example
     * ```typescript
     * const result = await agent
     *   .stream({ request: "Create a login page" })
     *   .onText(delta => process.stdout.write(delta))
     *   .onHelperStart(name => logger.debug(`>>> ${name}`))
     *   .run();
     * ```
     */
    stream(
        input: TInput,
        config?: RunConfig<TTools, TContext, TOutput>
    ): StreamBuilder<TOutput> {
        return new StreamBuilder(() => this.runStream(input, config));
    }

    /**
     * Internal streaming execution - yields chunks as they arrive.
     * Exposed as public for direct async iteration support.
     * For fluent API, use stream() instead.
     */
    async *runStream(
        input: TInput,
        config?: RunConfig<TTools, TContext, TOutput>
    ): AsyncGenerator<StreamChunk, TOutput, unknown> {
        if (!this.synthesizer || !this.ir) throw new Error("Agent not loaded");
        const synthesizer = this.synthesizer;
        const ir = this.ir;

        // Destructure config
        const { tools, context, configName, modelOverride } = config ?? {};
        const safeTools = tools as unknown as ToolMap;
        const runContext = context as TContext | undefined;

        const request = await synthesizer.synthesize(input, context, configName);
        if (modelOverride?.providerType) {
            request.config.model = modelOverride.providerType;
        }
        if (modelOverride?.modelName) {
            request.config.modelName = modelOverride.modelName;
        }
        if (modelOverride?.temperature !== undefined) {
            request.config.temperature = modelOverride.temperature;
        }
        const providerType = request.config.model ?? "";
        const driver = this.getDriver(providerType, request.config.modelName);
        const middlewares = sortMiddlewares(config?.middleware);
        const ctx = createMiddlewareContext(this.ir?.name || "unknown", input, runContext, request, config?.middlewareState, config?.runId);
        let ended = false;
        const complete = async (output: TOutput, result?: DriverResult) => {
            if (!ended) {
                ended = true;
                await runOnAgentEnd(middlewares, ctx, result);
            }
            return output;
        };

        if (!driver.executeStream) {
            console.warn(`[Agent] Driver ${driver.name} does not support streaming, falling back to non-streaming`);
            const result = await this.run(input, config);
            return result;
        }

        try {
            await runOnAgentStart(middlewares, ctx);

            let currentMessages: SyntheticMessage[] = [...request.messages];
            let turnCount = 0;
            let toolsStillAvailable = true;
            const completedCalls = new Set<string>();

            while (turnCount < this.maxTurns) {
                turnCount++;

                const currentRequest = {
                    ...request,
                    messages: currentMessages,
                    tools: toolsStillAvailable ? request.tools : undefined
                };

                ctx.request = currentRequest;
                const modifiedRequest = await runOnBeforeModel(middlewares, ctx);
                if (modifiedRequest) {
                    ctx.request = modifiedRequest;
                }

                const queue = this.createStreamQueue<StreamChunk>();
                const resultPromise = runWithRetries(middlewares, ctx, "model", async () => {
                    const wrapped = wrapModelCall(middlewares, ctx, async () => {
                        const stream = driver.executeStream!(ctx.request);
                        let finalResult: DriverResult | undefined;
                        try {
                            while (true) {
                                const { value, done } = await stream.next();
                                if (done) {
                                    finalResult = value;
                                    break;
                                }
                                queue.push(value);
                            }
                            queue.close();
                            if (!finalResult) {
                                throw new Error("Stream ended without a final result");
                            }
                            return finalResult;
                        } catch (error: any) {
                            queue.fail(error);
                            throw error;
                        }
                    });
                    return wrapped();
                });
                let result: DriverResult;
                try {
                    while (true) {
                        const { value, done } = await queue.next();
                        if (done) {
                            break;
                        }
                        yield value;
                    }
                    result = await resultPromise;
                } catch (error: any) {
                    queue.fail(error);
                    throw error;
                }

                ctx.response = result;
                const afterThinking = await runOnThinking(middlewares, ctx, result);
                const afterModel = await runOnAfterModel(middlewares, ctx, afterThinking);
                const finalResult = afterModel ?? afterThinking;
                ctx.response = finalResult;

                if (finalResult.toolCall) {
                    const { id, name, args } = finalResult.toolCall;
                    const callSignature = `${name}::${JSON.stringify(args, Object.keys(args).sort())}`;
                    if (completedCalls.has(callSignature)) {
                        logger.debug(`[Agent] BLOCKED: Duplicate call to "${name}" - already completed.`);
                        currentMessages.push({
                            role: 'user',
                            content: this.textBlocks(`[SYSTEM ERROR] You already completed "${name}" with these exact arguments. The result is in your conversation history. Do NOT repeat this call. Either proceed with a different task or finish by responding to the user.`)
                        });
                        continue;
                    }

                    currentMessages.push({
                        role: 'assistant',
                        content: [{ type: 'tool_use', id, name, input: args }],
                        toolCalls: [finalResult.toolCall]
                    });

                    const workflow = ir.workflows?.find(w => w.flowName === name);
                    const helper = ir.helpers?.find(h => h.name === name);

                    try {
                        const toolCall: ToolCall = { id, name, args };
                        const toolUseBlock = { type: 'tool_use', id, name, input: args } as const;
                        const shouldContinue = await runOnBeforeTool(middlewares, ctx, toolUseBlock);
                        if (shouldContinue === false) {
                            throw new Error("Tool execution blocked");
                        }

                        const toolQueue = this.createStreamQueue<StreamChunk>();
                        const toolResultPromise = runWithRetries(middlewares, ctx, "tool", async () => {
                            try {
                                const wrappedTool = wrapToolCall(middlewares, ctx, toolUseBlock, async (toolArgs: ToolArgs) => {
                                    if (workflow) {
                                        logger.debug(`[Agent] >>> Dispatching to Workflow (streaming): ${name}`);
                                        logger.trackWorkflowCall(name);
                                        const workflowStream = this.executeWorkflowStream(name, toolArgs, safeTools, runContext);
                                        let workflowResult: any;
                                        while (true) {
                                            const { value, done } = await workflowStream.next();
                                            if (done) {
                                                workflowResult = value;
                                                break;
                                            }
                                            toolQueue.push(value);
                                        }
                                        logger.debug(`[Agent] <<< Workflow ${name} completed.`);
                                        return workflowResult;
                                    }
                                    if (helper) {
                                        logger.debug(`[Agent] >>> Calling Helper (streaming): ${name}`);
                                        logger.trackHelperCall(name);
                                        const helperStream = this.executeHelperStream(helper, toolArgs);
                                        let helperResult: any;
                                        while (true) {
                                            const { value, done } = await helperStream.next();
                                            if (done) {
                                                helperResult = value;
                                                break;
                                            }
                                            toolQueue.push(value);
                                        }
                                        logger.debug(`[Agent] <<< Helper ${name} completed.`);
                                        return helperResult;
                                    }
                                    const isDeclaredTool = ir.tools?.some(t => t.name === name);
                                    if (!isDeclaredTool) {
                                        throw new Error(`Model tried to call unknown tool: ${name}`);
                                    }
                                    if (!tools || !safeTools[name]) {
                                        throw new Error(`Tool implementation missing for: ${name}`);
                                    }
                                    logger.debug(`[Agent] >>> Calling Tool: ${name}`);
                                    logger.trackToolCall(name);
                                    const result = await safeTools[name](toolArgs);
                                    logger.debug(`[Agent] <<< Tool ${name} completed.`);
                                    return result;
                                });
                                const result = await wrappedTool(toolCall.args);
                                toolQueue.close();
                                return result;
                            } catch (error: any) {
                                toolQueue.fail(error);
                                throw error;
                            }
                        });
                        let toolResult: ToolResult;
                        try {
                            while (true) {
                                const { value, done } = await toolQueue.next();
                                if (done) {
                                    break;
                                }
                                yield value;
                            }
                            toolResult = await toolResultPromise;
                        } catch (error: any) {
                            toolQueue.fail(error);
                            throw error;
                        }

                        await runOnAfterTool(middlewares, ctx, toolUseBlock, toolResult);

                        const transfer = workflow
                            ? this.getTransferSignal(toolResult)
                            : this.getHelperHandoffSignal(helper?.name, toolResult);
                        if (transfer) {
                            yield {
                                type: 'transfer',
                                mode: transfer.mode,
                                helperName: transfer.helperName || name
                            };

                            if (transfer.mode === "direct") {
                                return await complete(transfer.value as TOutput, finalResult);
                            }
                            if (transfer.mode === "thenContinue") {
                                completedCalls.add(callSignature);
                                logger.debug(`[Agent] Added to completed calls: ${callSignature}`);
                                const source = workflow ? `workflow "${name}"` : `helper "${name}"`;
                                currentMessages.push({
                                    role: 'user',
                                    content: this.textBlocks(`[System] The ${source} delivered its result to the user. This specific task is done. If the user's request has additional parts requiring a DIFFERENT task, you may proceed. Otherwise, provide a brief acknowledgment and finish.`)
                                });
                                toolsStillAvailable = false;
                                continue;
                            }
                        }

                        yield { type: 'tool_result', name, result: toolResult };

                        const resultPrefix = helper
                            ? `Helper Result [${name}]`
                            : workflow
                                ? `Workflow Result [${name}]`
                                : `Tool Result [${name}]`;

                        currentMessages.push({
                            role: 'user',
                            content: [{
                                type: 'tool_result',
                                toolUseId: id,
                                content: `${resultPrefix}: ${JSON.stringify(toolResult)}`
                            }]
                        });
                        toolsStillAvailable = false;
                        continue;
                    } catch (e: any) {
                        currentMessages.push({
                            role: 'user',
                            content: [{
                                type: 'tool_result',
                                toolUseId: id,
                                content: `Tool Error: ${e.message}`,
                                isError: true
                            }]
                        });
                        toolsStillAvailable = false;
                        continue;
                    }
                }

                const textOutput = this.extractText(finalResult);
                if (request.responseSchema) {
                    try {
                        const parsed = JSON.parse(textOutput ?? "{}") as TOutput;
                        return await complete(parsed, finalResult);
                    } catch (e) {
                        throw new Error("Model failed to return valid JSON");
                    }
                }
                return await complete((textOutput ?? "") as TOutput, finalResult);
            }

            throw new Error("Agent exceeded maximum turns");
        } catch (error: any) {
            if (!ended) {
                ended = true;
                await runOnAgentEnd(middlewares, ctx, ctx.response, error);
            }
            throw error;
        }
    }

    private textBlocks(text: string): ContentBlock[] {
        if (!text) {
            return [];
        }
        return [{ type: "text", text }];
    }

    private extractText(result: DriverResult): string | undefined {
        if (!result.content || result.content.length === 0) {
            return undefined;
        }
        return result.content
            .filter(block => block.type === "text")
            .map(block => block.type === "text" ? block.text : "")
            .join("");
    }

    private getTransferSignal(value: unknown): { __type: "TransferSignal"; value: unknown; mode: "direct" | "thenContinue"; helperName?: string } | null {
        if (!value || typeof value !== "object") {
            return null;
        }
        const record = value as Record<string, unknown>;
        if (record.__type !== "TransferSignal") {
            return null;
        }
        const mode = record.mode;
        if (mode !== "direct" && mode !== "thenContinue") {
            return null;
        }
        const helperName = typeof record.helperName === "string" ? record.helperName : undefined;
        return {
            __type: "TransferSignal",
            value: record.value,
            mode,
            helperName
        };
    }

    private getHelperHandoffSignal(helperName: string | undefined, value: unknown): { __type: "TransferSignal"; value: unknown; mode: "direct" | "thenContinue"; helperName?: string } | null {
        if (!helperName || !this.ir?.helperHandoff) {
            return null;
        }
        const mode = this.ir.helperHandoff[helperName];
        if (!mode) {
            return null;
        }
        return {
            __type: "TransferSignal",
            value,
            mode: mode === "thenContinue" ? "thenContinue" : "direct",
            helperName
        };
    }

    /**
     * Executes a workflow defined in the agent's IR.
     */
    async executeWorkflow(name: string, args: Record<string, any>, tools?: ToolMap, context?: Record<string, any>): Promise<any> {
        if (!this.ir) throw new Error("Agent not loaded");

        // Use provided tools or empty map if none
        const safeTools = (tools || {}) as ToolMap;

        const runner = new WorkflowRunner(
            this.ir,
            safeTools,
            (helper, args) => this.executeHelper(helper, args)
        );
        return runner.run(name, args, context);
    }

    /**
     * Executes a workflow with streaming - async generator that yields chunks immediately.
     */
    async *executeWorkflowStream(
        name: string,
        args: Record<string, any>,
        tools?: ToolMap,
        context?: Record<string, any>
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
        return yield* runner.runStream(name, args, context);
    }

    /**
     * Executes a helper as a sub-agent (with caching).
     */
    private async executeHelper(helper: HelperIR, args: Record<string, any>): Promise<any> {
        // Check cache first
        let subAgent = this.helperCache.get(helper.name);

        if (!subAgent) {
            // Create and cache the helper agent
            subAgent = new Agent<Record<string, any>, any>(this.drivers);

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
            logger.debug(`[Agent] Helper ${helper.name} cached for future calls.${grantedTools.length > 0 ? ` (with ${grantedTools.length} granted tools)` : ''}`);
        } else {
            logger.debug(`[Agent] Using cached helper: ${helper.name}`);
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
            subAgent = new Agent<Record<string, any>, any>(this.drivers);

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
            logger.debug(`[Agent] Helper ${helper.name} cached for streaming.`);
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

    private createStreamQueue<T>() {
        let closed = false;
        let error: any;
        const buffer: T[] = [];
        let pending: { resolve: (value: IteratorResult<T>) => void; reject: (error: any) => void } | null = null;

        const push = (item: T) => {
            if (closed) return;
            if (pending) {
                const { resolve } = pending;
                pending = null;
                resolve({ value: item, done: false });
                return;
            }
            buffer.push(item);
        };

        const close = () => {
            if (closed) return;
            closed = true;
            if (pending) {
                const { resolve } = pending;
                pending = null;
                resolve({ value: undefined as any, done: true });
            }
        };

        const fail = (err: any) => {
            if (closed) return;
            closed = true;
            error = err;
            if (pending) {
                const { reject } = pending;
                pending = null;
                reject(err);
            }
        };

        const next = async (): Promise<IteratorResult<T>> => {
            if (error) {
                throw error;
            }
            if (buffer.length > 0) {
                return { value: buffer.shift() as T, done: false };
            }
            if (closed) {
                return { value: undefined as any, done: true };
            }
            return new Promise<IteratorResult<T>>((resolve, reject) => {
                pending = { resolve, reject };
            });
        };

        return { push, close, fail, next };
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
     * Get or cache a driver for a provider type.
     */
    private getDriver(providerType?: string, modelName?: string): AgentDriver {
        const cacheKey = `${providerType ?? ""}::${modelName ?? ""}`;
        let driver = this.driverCache.get(cacheKey);

        if (!driver) {
            driver = providerType ? this.drivers[providerType] : undefined;
            if (!driver && modelName) {
                driver = this.driverRegistry.resolve(modelName);
            }
            if (!driver && providerType) {
                driver = this.driverRegistry.resolve(providerType);
            }
            if (!driver) {
                throw new Error(`No driver found for provider: ${providerType ?? "unknown"}`);
            }
            this.driverCache.set(cacheKey, driver);
        }

        return driver;
    }

    private registerDefaultDrivers(): void {
        const geminiDriver = this.drivers["gemini"] ?? this.drivers["google"];
        if (geminiDriver) {
            this.driverRegistry.registerProvider("google", geminiDriver);
        }
        const openAiDriver = this.drivers["openai"];
        if (openAiDriver) {
            this.driverRegistry.registerProvider("openai", openAiDriver);
            this.driverRegistry.registerProvider("together", openAiDriver);
            this.driverRegistry.registerProvider("groq", openAiDriver);
            this.driverRegistry.registerProvider("kimi", openAiDriver);
        }
    }
}
