import { Synthesizer } from "./Synthesizer";
import type { AgentIR, HelperIR } from "./types/ir";
import type { AgentDriver, AgentMiddleware, ContentBlock, DriverResult, StreamChunk, SyntheticMessage, ToolArgs, ToolCall, ToolResult, YamlIntent, YamlOutput } from "./types/protocol";
import type { ToolMap } from "./types/tool";
import { WorkflowRunner } from "./WorkflowRunner";
import { StreamBuilder } from "./StreamBuilder";
import { DriverRegistry } from "./DriverRegistry";
import { logger } from "./Logger";
import { ConfigurationError } from "./types/errors";
import { createMiddlewareContext, runOnAfterModel, runOnAfterTool, runOnAgentEnd, runOnAgentStart, runOnBeforeModel, runOnBeforeTool, runOnThinking, runWithRetries, sortMiddlewares, wrapModelCall, wrapToolCall } from "./IrMiddleware";
import { createStreamingParser, parseToJSON } from "auwgent-yaml-lite";
import { randomUUID } from "crypto";

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
    modelToolCalls?: "serial" | "parallel";
    modelToolCallFailure?: "fail" | "settle";
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
        const { tools, context, configName, modelOverride, modelToolCalls, modelToolCallFailure } = config ?? {};
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
            let streamingYaml: YamlOutput | null = null;
            const completedCalls = new Set<string>();

            const getCallSignature = (call: ToolCall) => {
                const args = call.args ?? {};
                const sortedKeys = Object.keys(args).sort();
                return `${call.name}::${JSON.stringify(args, sortedKeys)}`;
            };

            while (turnCount < this.maxTurns) {
                turnCount++;
                streamingYaml = null;

                const currentRequest = {
                    ...request,
                    messages: currentMessages
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

                const textOutput = this.extractText(finalResult);
                const yamlOutput = this.parseYamlOutput(textOutput);

                const assistantContent = this.buildAssistantContent(finalResult, yamlOutput, textOutput);
                if (assistantContent.length > 0) {
                    currentMessages.push({ role: 'assistant', content: assistantContent });
                }

                const toolCalls = this.buildToolCallsFromYaml(yamlOutput);
                if (toolCalls.length > 0) {
                    const resolvedModelToolCalls = yamlOutput?.parallel === true ? "parallel" : (modelToolCalls ?? "serial");
                    const failureMode = modelToolCallFailure ?? "settle";
                    if (toolCalls.length > 1) {
                        logger.debug(`[Agent] YAML intents requested ${toolCalls.length} tool calls: ${toolCalls.map(call => call.name).join(", ")}`);
                    }

                    currentMessages.push({
                        role: 'assistant',
                        content: toolCalls.map(call => ({ type: 'tool_use', id: call.id, name: call.name, input: call.args })),
                        toolCalls
                    });

                    const executeToolCall = async (call: ToolCall, signature: string) => {
                        const { id, name, args } = call;
                        const workflow = ir.workflows?.find(w => w.flowName === name);
                        const helper = ir.helpers?.find(h => h.name === name);
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
                            return wrappedTool(args);
                        });

                        await runOnAfterTool(middlewares, ctx, toolUseBlock, toolResult);
                        const transfer = workflow
                            ? this.getTransferSignal(toolResult)
                            : this.getHelperHandoffSignal(helper?.name, toolResult);
                        return { call, toolResult, transfer, workflow, helper };
                    };

                    const results: Array<{ call: ToolCall; signature: string; toolResult?: ToolResult; error?: Error; transfer?: any; workflow?: any; helper?: any }> = [];
                    const pendingCalls: Array<{ call: ToolCall; signature: string }> = [];
                    for (const call of toolCalls) {
                        const signature = getCallSignature(call);
                        if (completedCalls.has(signature)) {
                            logger.debug(`[Agent] BLOCKED: Duplicate call to "${call.name}" - already completed.`);
                            currentMessages.push({
                                role: 'user',
                                content: this.textBlocks(`[SYSTEM ERROR] You already completed "${call.name}" with these exact arguments. The result is in your conversation history. Do NOT repeat this call. Either proceed with a different task or finish by responding to the user.`)
                            });
                            results.push({ call, signature, error: new Error("Duplicate tool call") });
                            if (failureMode === "fail") {
                                throw new Error(`Duplicate tool call: ${call.name}`);
                            }
                            continue;
                        }
                        pendingCalls.push({ call, signature });
                    };

                    if (resolvedModelToolCalls === "parallel") {
                        if (failureMode === "fail") {
                            const parallelResults = await Promise.all(pendingCalls.map(async ({ call, signature }) => {
                                try {
                                    const output = await executeToolCall(call, signature);
                                    completedCalls.add(signature);
                                    return { ...output, signature };
                                } catch (error) {
                                    completedCalls.add(signature);
                                    throw error;
                                }
                            }));
                            results.push(...parallelResults);
                        } else {
                            const settled = await Promise.all(pendingCalls.map(async ({ call, signature }) => {
                                try {
                                    const output = await executeToolCall(call, signature);
                                    completedCalls.add(signature);
                                    return { ...output, signature };
                                } catch (error: any) {
                                    completedCalls.add(signature);
                                    return { call, signature, error };
                                }
                            }));
                            results.push(...settled);
                        }
                    } else {
                        for (const { call, signature } of pendingCalls) {
                            try {
                                const output = await executeToolCall(call, signature);
                                completedCalls.add(signature);
                                results.push({ ...output, signature });
                            } catch (error: any) {
                                completedCalls.add(signature);
                                if (failureMode === "fail") {
                                    throw error;
                                }
                                results.push({ call, signature, error });
                            }
                        }
                    }

                    if (results.some(result => result.transfer) && toolCalls.length > 1) {
                        throw new Error("Transfer is not supported when multiple tool calls are returned by the model");
                    }

                    for (const entry of results) {
                        const { call, toolResult, error, transfer, workflow, helper } = entry;
                        if (error) {
                            currentMessages.push({
                                role: 'user',
                                content: [{
                                    type: 'tool_result',
                                    toolUseId: call.id,
                                    content: `Tool Error: ${error.message}`,
                                    isError: true
                                }]
                            });
                            if (failureMode === "fail") {
                                throw error;
                            }
                            continue;
                        }

                        if (transfer) {
                            logger.debug(`[Agent] Transfer detected (mode: ${transfer.mode})`);
                            if (transfer.mode === "direct") {
                                return await complete(transfer.value as TOutput, finalResult);
                            }
                            if (transfer.mode === "thenContinue") {
                                const source = workflow ? `workflow "${call.name}"` : `helper "${call.name}"`;
                                currentMessages.push({
                                    role: 'user',
                                    content: this.textBlocks(`[System] The ${source} delivered its result to the user. This specific task is done. If the user's request has additional parts requiring a DIFFERENT task, you may proceed. Otherwise, provide a brief acknowledgment and finish.`)
                                });
                                continue;
                            }
                        }

                        const resultPrefix = helper
                            ? `Helper Result [${call.name}]`
                            : workflow
                                ? `Workflow Result [${call.name}]`
                                : `Tool Result [${call.name}]`;

                        currentMessages.push({
                            role: 'user',
                            content: [{
                                type: 'tool_result',
                                toolUseId: call.id,
                                content: `${resultPrefix}: ${JSON.stringify(toolResult)}`
                            }]
                        });
                    }
                    continue;
                }

                const finalOutput = this.resolveFinalOutput<TOutput>(yamlOutput, textOutput);
                return await complete(finalOutput, finalResult);
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
        const { tools, context, configName, modelOverride, modelToolCalls, modelToolCallFailure } = config ?? {};
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
            let streamingYaml: YamlOutput | null = null;
            const completedCalls = new Set<string>();

            const getCallSignature = (call: ToolCall) => {
                const args = call.args ?? {};
                const sortedKeys = Object.keys(args).sort();
                return `${call.name}::${JSON.stringify(args, sortedKeys)}`;
            };

            while (turnCount < this.maxTurns) {
                turnCount++;
                streamingYaml = null;

                const currentRequest = {
                    ...request,
                    messages: currentMessages
                };

                ctx.request = currentRequest;
                const modifiedRequest = await runOnBeforeModel(middlewares, ctx);
                if (modifiedRequest) {
                    ctx.request = modifiedRequest;
                }

                const queue = this.createStreamQueue<StreamChunk>();
                const resultPromise = runWithRetries(middlewares, ctx, "model", async () => {
                    const wrapped = wrapModelCall(middlewares, ctx, async () => {
                        const parser = createStreamingParser();
                        let inFlightYaml: YamlOutput | null = null;
                        let lastPreview: string | null = null;
                        const stream = driver.executeStream!(ctx.request);
                        let finalResult: DriverResult | undefined;
                        try {
                            while (true) {
                                const { value, done } = await stream.next();
                                if (done) {
                                    finalResult = value;
                                    break;
                                }
                                if (value.type === 'text') {
                                    try {
                                        parser.write(value.delta);
                                        inFlightYaml = parser.peek() as YamlOutput;
                                    } catch {
                                        // ignore partial parse errors during streaming
                                    }
                                    const preview = this.formatStreamingPreview(inFlightYaml);
                                    if (preview.display && preview.display !== lastPreview) {
                                        queue.push({ type: 'text', delta: preview.display, format: 'json', raw: preview.raw });
                                        lastPreview = preview.display;
                                    }
                                    continue;
                                }
                                queue.push(value);
                            }
                            try {
                                inFlightYaml = parser.end() as YamlOutput;
                            } catch {
                                // final parse failed - fall back to text parsing later
                            }
                            const finalPreview = this.formatStreamingPreview(inFlightYaml);
                            if (finalPreview.display && finalPreview.display !== lastPreview) {
                                queue.push({ type: 'text', delta: finalPreview.display, format: 'json', raw: finalPreview.raw });
                                lastPreview = finalPreview.display;
                            }
                            queue.close();
                            streamingYaml = inFlightYaml;
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

                const textOutput = this.extractText(finalResult);
                const yamlOutput = streamingYaml ?? this.parseYamlOutput(textOutput);

                const assistantContent = this.buildAssistantContent(finalResult, yamlOutput, textOutput);
                if (assistantContent.length > 0) {
                    currentMessages.push({ role: 'assistant', content: assistantContent });
                }

                const toolCalls = this.buildToolCallsFromYaml(yamlOutput, (_intent, index) => randomUUID());
                if (toolCalls.length > 0) {
                    const resolvedModelToolCalls = yamlOutput?.parallel === true ? "parallel" : (modelToolCalls ?? "serial");
                    const failureMode = modelToolCallFailure ?? "settle";
                    if (toolCalls.length > 1) {
                        logger.debug(`[Agent] YAML intents requested ${toolCalls.length} tool calls: ${toolCalls.map(call => call.name).join(", ")}`);
                    }

                    currentMessages.push({
                        role: 'assistant',
                        content: toolCalls.map(call => ({ type: 'tool_use', id: call.id, name: call.name, input: call.args })),
                        toolCalls
                    });

                    const executeToolCallStream = async (call: ToolCall, signature: string, emit: (chunk: StreamChunk) => void) => {
                        const { id, name, args } = call;
                        const workflow = ir.workflows?.find(w => w.flowName === name);
                        const helper = ir.helpers?.find(h => h.name === name);
                        const toolUseBlock = { type: 'tool_use', id, name, input: args } as const;
                        const shouldContinue = await runOnBeforeTool(middlewares, ctx, toolUseBlock);
                        if (shouldContinue === false) {
                            throw new Error("Tool execution blocked");
                        }

                        const emitToolStart = () => emit({ type: 'tool_start', name, id });
                        const emitToolArgs = () => emit({ type: 'tool_args', id, delta: this.formatToolArgsForStream(args) });
                        let toolEnded = false;
                        const emitToolEnd = () => {
                            if (toolEnded) {
                                return;
                            }
                            toolEnded = true;
                            emit({ type: 'tool_end', id });
                        };

                        emitToolStart();
                        emitToolArgs();

                        let toolResult: ToolResult;
                        try {
                            toolResult = await runWithRetries(middlewares, ctx, "tool", async () => {
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
                                        emit(value);
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
                                        emit(value);
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
                            return wrappedTool(args);
                        });
                        } catch (error) {
                            emitToolEnd();
                            throw error;
                        }

                        try {
                            await runOnAfterTool(middlewares, ctx, toolUseBlock, toolResult);
                        } catch (error) {
                            emitToolEnd();
                            throw error;
                        }
                        emitToolEnd();
                        const transfer = workflow
                            ? this.getTransferSignal(toolResult)
                            : this.getHelperHandoffSignal(helper?.name, toolResult);
                        return { call, toolResult, transfer, workflow, helper };
                    };

                    const results: Array<{ call: ToolCall; signature: string; toolResult?: ToolResult; error?: Error; transfer?: any; workflow?: any; helper?: any }> = [];
                    const pendingCalls: Array<{ call: ToolCall; signature: string }> = [];
                    for (const call of toolCalls) {
                        const callSignature = getCallSignature(call);
                        if (completedCalls.has(callSignature)) {
                            logger.debug(`[Agent] BLOCKED: Duplicate call to "${call.name}" - already completed.`);
                            currentMessages.push({
                                role: 'user',
                                content: this.textBlocks(`[SYSTEM ERROR] You already completed "${call.name}" with these exact arguments. The result is in your conversation history. Do NOT repeat this call. Either proceed with a different task or finish by responding to the user.`)
                            });
                            results.push({ call, signature: callSignature, error: new Error("Duplicate tool call") });
                            if (failureMode === "fail") {
                                throw new Error(`Duplicate tool call: ${call.name}`);
                            }
                            continue;
                        }
                        pendingCalls.push({ call, signature: callSignature });
                    }

                    if (resolvedModelToolCalls === "parallel") {
                        const toolQueue = this.createStreamQueue<StreamChunk>();
                        const emit = (chunk: StreamChunk) => toolQueue.push(chunk);
                        const completion = (async () => {
                            try {
                                if (failureMode === "fail") {
                                    const outputs = await Promise.all(pendingCalls.map(async ({ call, signature }) => {
                                        try {
                                            const output = await executeToolCallStream(call, signature, emit);
                                            completedCalls.add(signature);
                                            return { ...output, signature };
                                        } catch (error) {
                                            completedCalls.add(signature);
                                            throw error;
                                        }
                                    }));
                                    results.push(...outputs);
                                } else {
                                    const outputs = await Promise.all(pendingCalls.map(async ({ call, signature }) => {
                                        try {
                                            const output = await executeToolCallStream(call, signature, emit);
                                            completedCalls.add(signature);
                                            return { ...output, signature };
                                        } catch (error: any) {
                                            completedCalls.add(signature);
                                            return { call, signature, error };
                                        }
                                    }));
                                    results.push(...outputs);
                                }
                            } finally {
                                toolQueue.close();
                            }
                        })();
                        while (true) {
                            const { value, done } = await toolQueue.next();
                            if (done) {
                                break;
                            }
                            yield value;
                        }
                        await completion;
                    } else {
                        for (const { call, signature } of pendingCalls) {
                            const toolQueue = this.createStreamQueue<StreamChunk>();
                            const emit = (chunk: StreamChunk) => toolQueue.push(chunk);
                            const execPromise = (async () => {
                                try {
                                    return await executeToolCallStream(call, signature, emit);
                                } finally {
                                    toolQueue.close();
                                }
                            })();
                            while (true) {
                                const { value, done } = await toolQueue.next();
                                if (done) {
                                    break;
                                }
                                yield value;
                            }
                            try {
                                const output = await execPromise;
                                completedCalls.add(signature);
                                results.push({ ...output, signature });
                            } catch (error: any) {
                                completedCalls.add(signature);
                                if (failureMode === "fail") {
                                    throw error;
                                }
                                results.push({ call, signature, error });
                            }
                        }
                    }

                    if (results.some(result => result.transfer) && toolCalls.length > 1) {
                        throw new Error("Transfer is not supported when multiple tool calls are returned by the model");
                    }

                    for (const entry of results) {
                        const { call, toolResult, error, transfer, workflow, helper } = entry;
                        if (error) {
                            currentMessages.push({
                                role: 'user',
                                content: [{
                                    type: 'tool_result',
                                    toolUseId: call.id,
                                    content: `Tool Error: ${error.message}`,
                                    isError: true
                                }]
                            });
                            if (failureMode === "fail") {
                                throw error;
                            }
                            continue;
                        }

                        if (transfer) {
                            yield {
                                type: 'transfer',
                                mode: transfer.mode,
                                helperName: transfer.helperName || call.name
                            };

                            if (transfer.mode === "direct") {
                                return await complete(transfer.value as TOutput, finalResult);
                            }
                            if (transfer.mode === "thenContinue") {
                                const source = workflow ? `workflow "${call.name}"` : `helper "${call.name}"`;
                                currentMessages.push({
                                    role: 'user',
                                    content: this.textBlocks(`[System] The ${source} delivered its result to the user. This specific task is done. If the user's request has additional parts requiring a DIFFERENT task, you may proceed. Otherwise, provide a brief acknowledgment and finish.`)
                                });
                                continue;
                            }
                        }

                        const streamResult = toolResult ?? null;
                        yield { type: 'tool_result', name: call.name, result: streamResult };

                        const resultPrefix = helper
                            ? `Helper Result [${call.name}]`
                            : workflow
                                ? `Workflow Result [${call.name}]`
                                : `Tool Result [${call.name}]`;

                        currentMessages.push({
                            role: 'user',
                            content: [{
                                type: 'tool_result',
                                toolUseId: call.id,
                                    content: `${resultPrefix}: ${JSON.stringify(streamResult)}`
                            }]
                        });
                    }
                    continue;
                }

                const finalOutput = this.resolveFinalOutput<TOutput>(yamlOutput, textOutput);
                return await complete(finalOutput, finalResult);
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

    private buildAssistantContent(result: DriverResult, yaml: YamlOutput | null, rawText: string | undefined): ContentBlock[] {
        if (result.content && result.content.length > 0) {
            const hasNonText = result.content.some(block => block.type !== "text");
            if (hasNonText) {
                return result.content;
            }
            const combined = result.content
                .filter(block => block.type === "text")
                .map(block => (block.type === "text" ? block.text : ""))
                .join("");
            const formattedFromContent = this.formatAssistantDisplay(yaml, combined);
            if (formattedFromContent) {
                return this.textBlocks(formattedFromContent);
            }
            if (combined.trim().length > 0) {
                return this.textBlocks(combined);
            }
        }

        const formatted = this.formatAssistantDisplay(yaml, rawText);
        if (formatted) {
            return this.textBlocks(formatted);
        }

        return this.textBlocks(rawText ?? "");
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

    private resolveFinalOutput<TOutput>(yamlOutput: YamlOutput | null, textOutput: string | undefined): TOutput {
        const hasOutput = !!this.ir?.output && Object.keys(this.ir.output).length > 0;
        if (hasOutput) {
            if (!yamlOutput || typeof yamlOutput !== "object") {
                throw new Error("Model failed to return valid YAML");
            }
            const output = yamlOutput.output ?? yamlOutput;
            if (!output || typeof output !== "object") {
                throw new Error("Model returned malformed YAML output block");
            }
            return output as TOutput;
        }
        if (yamlOutput) {
            if (yamlOutput.output && typeof yamlOutput.output === "object") {
                return yamlOutput.output as unknown as TOutput;
            }
            if (yamlOutput.text !== undefined) {
                return String(yamlOutput.text ?? "") as TOutput;
            }
        }
        return (textOutput ?? "") as TOutput;
    }

    private parseYamlOutput(textOutput: string | undefined): YamlOutput | null {
        if (!textOutput) {
            return null;
        }
        const trimmed = textOutput.trim();
        if (!trimmed) {
            return { text: "" };
        }
        try {
            const parsed = parseToJSON(trimmed) as YamlOutput;
            if (parsed && typeof parsed === "object") {
                return parsed;
            }
        } catch {
            try {
                const streamingParser = createStreamingParser();
                streamingParser.write(trimmed);
                const parsed = streamingParser.end() as YamlOutput;
                if (parsed && typeof parsed === "object") {
                    return parsed;
                }
            } catch {
                // ignore and fall back to raw text
            }
        }
        return { text: textOutput };
    }

    private buildToolCallsFromYaml(yamlOutput: YamlOutput | null, idResolver?: (intent: YamlIntent, index: number) => string): ToolCall[] {
        if (!yamlOutput?.intents || yamlOutput.intents.length === 0) {
            return [];
        }
        const calls: ToolCall[] = [];
        yamlOutput.intents.forEach((intent, index) => {
            if (!intent || !intent.name) {
                return;
            }
            if (intent.type !== "tool_call" && intent.type !== "workflow" && intent.type !== "helper") {
                return;
            }
            if (intent.type === "helper") {
                const helperDeclared = this.ir?.helpers?.some(helper => helper.name === intent.name) ?? false;
                if (!helperDeclared) {
                    if (intent.name === "respond") {
                        logger.debug(`[Agent] Ignoring implicit respond helper intent.`);
                        return;
                    }
                    logger.warn(`[Agent] Model requested unknown helper "${intent.name}" - intent skipped.`);
                    return;
                }
            }
            const args = intent.args && typeof intent.args === "object" ? intent.args : {};
            const id = idResolver ? idResolver(intent, index) : randomUUID();
            calls.push({
                id,
                name: intent.name,
                args
            });
        });
        return calls;
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

    private formatToolArgsForStream(args: ToolArgs | undefined): string {
        if (!args || Object.keys(args).length === 0) {
            return "{}";
        }
        const sorted = Object.keys(args)
            .sort()
            .reduce<Record<string, any>>((acc, key) => {
                acc[key] = args[key];
                return acc;
            }, {});
        try {
            return JSON.stringify(sorted, null, 2);
        } catch (error) {
            return String(args);
        }
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

    private formatStreamingPreview(yaml: YamlOutput | null): { display: string | null; raw?: string } {
        const summary = this.buildAssistantSummary(yaml, undefined);
        const hasIntents = Array.isArray(summary.intents) && summary.intents.length > 0;
        const done = !hasIntents;

        const preview: Record<string, any> = {
            stage: done ? "final" : "plan",
            done
        };

        if (done) {
            if (summary.text) {
                preview.text = summary.text;
            }
            if (summary.output !== undefined) {
                preview.output = summary.output;
            }
            if (summary.question) {
                preview.question = summary.question;
            }
        } else {
            if (summary.intents && summary.intents.length > 0) {
                preview.intents = summary.intents;
            }
            if (summary.question) {
                preview.question = summary.question;
            }
        }

        const meaningfulKeys = Object.keys(preview).filter(key => !["stage", "done"].includes(key));
        if (meaningfulKeys.length === 0) {
            return { display: null };
        }

        return {
            display: JSON.stringify(preview, null, 2),
            raw: done && summary.text ? summary.text : undefined
        };
    }

    private formatAssistantDisplay(yaml: YamlOutput | null, rawText: string | undefined): string | null {
        const summary = this.buildAssistantSummary(yaml, rawText);
        const segments: string[] = [];
        if (summary.text) {
            segments.push(summary.text);
        }
        if (summary.output) {
            segments.push(`Output:\n${JSON.stringify(summary.output, null, 2)}`);
        }
        if (summary.question) {
            segments.push(`Question: ${summary.question}`);
        }
        if (summary.intents && summary.intents.length > 0) {
            segments.push(`Intents:\n${JSON.stringify(summary.intents, null, 2)}`);
        }
        if (segments.length === 0) {
            return null;
        }
        return segments.join("\n\n");
    }

    private extractTextFromRawYaml(raw: string | undefined): string | null {
        if (!raw) {
            return null;
        }
        const lines = raw.split(/\r?\n/);
        for (const line of lines) {
            const trimmed = line.trim();
            if (!trimmed.toLowerCase().startsWith("text:")) {
                continue;
            }
            let value = trimmed.slice(5).trim();
            if (value.length === 0) {
                continue;
            }
            if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
                value = value.slice(1, -1);
            }
            return value.length > 0 ? value : null;
        }
        return null;
    }

    private extractQuestionFromRawYaml(raw: string | undefined): string | null {
        if (!raw) {
            return null;
        }
        const lines = raw.split(/\r?\n/);
        for (const line of lines) {
            const trimmed = line.trim();
            if (!trimmed.toLowerCase().startsWith("question:")) {
                continue;
            }
            let value = trimmed.slice(9).trim();
            if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
                value = value.slice(1, -1);
            }
            if (value.toLowerCase() === "null") {
                return null;
            }
            return value.length > 0 ? value : null;
        }
        return null;
    }

    private buildAssistantSummary(yaml: YamlOutput | null, rawText: string | undefined): { text?: string; question?: string; output?: Record<string, any>; intents?: YamlIntent[] } {
        const summary: { text?: string; question?: string; output?: Record<string, any>; intents?: YamlIntent[] } = {};

        if (yaml && typeof yaml === "object") {
            if (typeof yaml.text === "string" && yaml.text.trim().length > 0) {
                summary.text = yaml.text.trim();
            }
            if (yaml.output !== undefined) {
                summary.output = yaml.output;
            }
            if (typeof yaml.question === "string" && yaml.question.trim().length > 0 && yaml.question.trim().toLowerCase() !== "null") {
                summary.question = yaml.question.trim();
            }
            const actionableIntents = yaml.intents?.filter(intent => {
                if (!intent || typeof intent !== "object") {
                    return false;
                }
                const normalizedType = typeof intent.type === "string" ? intent.type.trim().toLowerCase() : "";
                return normalizedType.length > 0 && normalizedType !== "respond";
            });
            if (actionableIntents && actionableIntents.length > 0) {
                summary.intents = actionableIntents;
            }
            if (summary.text || summary.question || summary.output || (summary.intents && summary.intents.length > 0)) {
                return summary;
            }
        }

        const textFromRaw = this.extractTextFromRawYaml(rawText);
        if (textFromRaw) {
            summary.text = textFromRaw;
        }
        const questionFromRaw = this.extractQuestionFromRawYaml(rawText);
        if (questionFromRaw) {
            summary.question = questionFromRaw;
        }
        return summary;
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
