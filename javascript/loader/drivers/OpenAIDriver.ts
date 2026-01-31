/**
 * OpenAI-compatible driver that works with:
 * - OpenAI
 * - Azure OpenAI
 * - Groq
 * - Together AI
 * - Any OpenAI API-compatible provider
 */
import OpenAI from "openai";
import type { AgentDriver, ContentBlock, DriverResult, StreamChunk, SyntheticRequest, SyntheticMessage, ToolArgs, ModelUsage } from "../types/protocol";
import { DriverError, type ErrorType } from "../types/errors";
import { logger } from "../Logger";

export class OpenAIDriver implements AgentDriver {
    name = "openai";
    private client: OpenAI;

    constructor(apiKey: string, baseUrl?: string) {
        this.client = new OpenAI({
            apiKey,
            baseURL: baseUrl
        });
    }

    async execute(request: SyntheticRequest): Promise<DriverResult> {
        try {
            const model = request.config.modelName || request.config.model || "gpt-4o-mini";

            // Build messages
            const messages: OpenAI.Chat.ChatCompletionMessageParam[] = request.messages.map(m => this.toOpenAiMessage(m));

            // Build tools
            const tools: any[] | undefined = request.tools?.map(t => ({
                type: "function",
                function: {
                    name: t.name,
                    description: t.description,
                    parameters: t.parameters
                }
            }));

            const hasTools = tools && tools.length > 0;

            const responseSchema = request.responseFormat?.type === "json_schema"
                ? request.responseFormat.schema
                : request.responseSchema;
            const jsonObjectOnly = request.responseFormat?.type === "json_object";

            // Add schema instruction ONLY when no tools (final response turn)
            if (responseSchema && !hasTools) {
                const schemaInstruction = `\n\nYou must respond with valid JSON matching this schema: ${JSON.stringify(responseSchema)}`;
                const systemMsgIndex = messages.findIndex(m => m.role === 'system');
                if (systemMsgIndex >= 0) {
                    (messages[systemMsgIndex] as any).content += schemaInstruction;
                } else {
                    messages.unshift({
                        role: 'system',
                        content: `You are a helpful assistant.${schemaInstruction}`
                    });
                }
            } else if (jsonObjectOnly && !hasTools) {
                const schemaInstruction = `\n\nYou must respond with valid JSON.`;
                const systemMsgIndex = messages.findIndex(m => m.role === 'system');
                if (systemMsgIndex >= 0) {
                    (messages[systemMsgIndex] as any).content += schemaInstruction;
                } else {
                    messages.unshift({
                        role: 'system',
                        content: `You are a helpful assistant.${schemaInstruction}`
                    });
                }
            }


            const providerConfig = request.config.providerConfig ?? {};

            // Execute - Force tool use with 'required' when tools are available
            const completion = await this.client.chat.completions.create({
                ...providerConfig,
                model,
                messages,
                tools: hasTools ? tools : undefined,
                tool_choice: hasTools ? "required" : undefined,
                response_format: !hasTools && request.responseSchema ? { type: "json_object" } : undefined,
                temperature: request.config.temperature ?? 0
            });

            const choice = completion.choices[0];
            if (!choice) {
                throw new Error("No response from OpenAI");
            }

            const usage = this.extractUsage(completion.usage);
            if (usage) {
                logger.trackTokens({
                    promptTokens: usage.input,
                    completionTokens: usage.response,
                    totalTokens: usage.total
                });
                logger.debug(`[OpenAI] Tokens: ${usage.total} (${usage.input}+${usage.response})`);
            }

            // Check for tool calls
            if (choice.message?.tool_calls && choice.message.tool_calls.length > 0) {
                const toolCalls = choice.message.tool_calls.map((toolCall: any, index: number) => {
                    const name = toolCall.function?.name || toolCall.name;
                    const args = toolCall.function?.arguments || toolCall.arguments;
                    const parsedArgs = this.parseToolArgs(args);
                    return {
                        id: toolCall.id ?? `tool_${Date.now()}_${index}`,
                        name,
                        args: parsedArgs
                    };
                });

                return {
                    toolCalls,
                    usage
                };
            }

            return {
                content: this.textBlocks(choice.message?.content ?? ""),
                usage
            };
        } catch (error: any) {
            throw this.handleError(error);
        }
    }

    /**
     * Streaming execution using async generator
     */
    async *executeStream(request: SyntheticRequest): AsyncGenerator<StreamChunk, DriverResult, unknown> {
        try {
            const model = request.config.modelName || request.config.model || "gpt-4o-mini";

            const messages: OpenAI.Chat.ChatCompletionMessageParam[] = request.messages.map(m => this.toOpenAiMessage(m));

            const tools: any[] | undefined = request.tools?.map(t => ({
                type: "function",
                function: {
                    name: t.name,
                    description: t.description,
                    parameters: t.parameters
                }
            }));

            const hasTools = tools && tools.length > 0;

            const responseSchema = request.responseFormat?.type === "json_schema"
                ? request.responseFormat.schema
                : request.responseSchema;
            const jsonObjectOnly = request.responseFormat?.type === "json_object";

            if (responseSchema && !hasTools) {
                const schemaInstruction = `\n\nYou must respond with valid JSON matching this schema: ${JSON.stringify(responseSchema)}`;
                const systemMsgIndex = messages.findIndex(m => m.role === 'system');
                if (systemMsgIndex >= 0) {
                    (messages[systemMsgIndex] as any).content += schemaInstruction;
                } else {
                    messages.unshift({
                        role: 'system',
                        content: `You are a helpful assistant.${schemaInstruction}`
                    });
                }
            } else if (jsonObjectOnly && !hasTools) {
                const schemaInstruction = `\n\nYou must respond with valid JSON.`;
                const systemMsgIndex = messages.findIndex(m => m.role === 'system');
                if (systemMsgIndex >= 0) {
                    (messages[systemMsgIndex] as any).content += schemaInstruction;
                } else {
                    messages.unshift({
                        role: 'system',
                        content: `You are a helpful assistant.${schemaInstruction}`
                    });
                }
            }

            // Log tools being sent
            logger.debug(`[OpenAI] Streaming with ${hasTools ? tools?.length : 0} tools`);

            const providerConfig = request.config.providerConfig ?? {};

            // Use streaming API with usage tracking
            const stream = await (this.client.chat.completions.create({
                ...providerConfig,
                model,
                messages,
                tools: hasTools ? tools : undefined,
                tool_choice: hasTools ? "required" : undefined,
                response_format: !hasTools && request.responseSchema ? { type: "json_object" } : undefined,
                temperature: request.config.temperature ?? 0,
                stream: true,
                stream_options: { include_usage: true }  // Get token usage in streaming
            }) as any);

            let fullText = '';
            let toolCalls: { id: string; name: string; args: any }[] | undefined;
            let usage: ModelUsage | undefined;
            const toolArgsBuffer: Record<string, string> = {};
            const toolNameBuffer: Record<string, string> = {};  // Track tool names
            const activeToolIds: Record<number, string> = {}; // Track index -> id mapping
            const toolOrder: string[] = [];

            for await (const chunk of stream) {
                const delta = chunk.choices[0]?.delta;

                // Handle text content
                if (delta?.content) {
                    fullText += delta.content;
                    yield { type: 'text', delta: delta.content };
                }

                // Handle tool calls (support multiple parallel tools)
                if (delta?.tool_calls && delta.tool_calls.length > 0) {
                    for (const toolDelta of delta.tool_calls) {
                        const index = toolDelta?.index ?? 0;

                        let id = toolDelta?.id;

                        // If new tool call with ID, map index to ID
                        if (id) {
                            activeToolIds[index] = id;
                        } else {
                            // Otherwise use existing ID for this index
                            id = activeToolIds[index] || `tool_${index}`;
                        }

                        if (toolDelta) {
                            // Tool call start (has function name)
                            if (toolDelta.function?.name) {
                                toolNameBuffer[id] = toolDelta.function.name;
                                yield { type: 'tool_start', name: toolDelta.function.name, id };
                                toolArgsBuffer[id] = '';
                                if (!toolOrder.includes(id)) {
                                    toolOrder.push(id);
                                }
                            }

                            // Tool arguments streaming
                            if (toolDelta.function?.arguments) {
                                toolArgsBuffer[id] = (toolArgsBuffer[id] || '') + toolDelta.function.arguments;
                                yield { type: 'tool_args', id, delta: toolDelta.function.arguments };
                            }
                        }
                    }
                }

                // Check for finish reason to emit tool_end and capture final tool calls
                if (chunk.choices[0]?.finish_reason === 'tool_calls') {
                    // Emit tool_end for all buffered tools
                    for (const id of Object.keys(toolNameBuffer)) {
                        yield { type: 'tool_end', id };
                    }

                    toolCalls = toolOrder.map(id => ({
                        id,
                        name: toolNameBuffer[id] || '',
                        args: this.parseToolArgs(toolArgsBuffer[id])
                    })).filter(call => call.name);
                }

                // Track token usage from final chunk (OpenAI sends it with stream_options)
                if ((chunk as any).usage) {
                    const chunkUsage = this.extractUsage((chunk as any).usage);
                    if (chunkUsage) {
                        usage = chunkUsage;
                        logger.trackTokens({
                            promptTokens: chunkUsage.input,
                            completionTokens: chunkUsage.response,
                            totalTokens: chunkUsage.total
                        });
                        logger.debug(`[OpenAI] Tokens: ${chunkUsage.total} (${chunkUsage.input}+${chunkUsage.response})`);
                    }
                }
            }

            // Return final result
            if (toolCalls && toolCalls.length > 0) {
                return { toolCalls, usage };
            }

            // Fallback: If we have a buffered tool call that wasn't captured (e.g. missed finish_reason)
            const bufferedIds = Object.keys(toolNameBuffer);
            if (bufferedIds.length > 0) {
                try {
                    return {
                        toolCalls: bufferedIds.map(id => ({
                            id,
                            name: toolNameBuffer[id] || '',
                            args: this.parseToolArgs(toolArgsBuffer[id])
                        })).filter(call => call.name),
                        usage
                    };
                } catch (e) {
                    logger.warn("[OpenAI] Failed to parse buffered tool args", e);
                    // Return text if parsing failed, or partial tool? Better to fall through to text or throw.
                    // Assuming if we have a name, it was intended as a tool call.
                }
            }

            return { content: this.textBlocks(fullText), usage };
        } catch (error: any) {
            throw this.handleError(error);
        }
    }

    /**
     * Classify and wrap errors from OpenAI SDK
     */
    private handleError(error: any): DriverError {
        // Extract error details
        const message = error.message || 'Unknown error';
        const statusCode = error.status || error.statusCode;

        // Classify error type
        let type: ErrorType = 'UNKNOWN_ERROR';
        let retryable = false;

        if (statusCode === 401 || statusCode === 403 || message.includes('API key') || message.includes('Incorrect API key')) {
            type = 'AUTH_ERROR';
            retryable = false;
        } else if (statusCode === 429 || message.includes('rate_limit') || message.includes('Rate limit')) {
            type = 'RATE_LIMIT';
            retryable = true;
        } else if (statusCode === 400 || message.includes('invalid')) {
            type = 'INVALID_REQUEST';
            retryable = false;
        } else if (message.includes('content_policy') || message.includes('content filter')) {
            type = 'CONTENT_POLICY';
            retryable = false;
        } else if (message.includes('context_length') || message.includes('maximum context length')) {
            type = 'TOKEN_LIMIT';
            retryable = false;
        } else if (statusCode === 404 || message.includes('model') && message.includes('does not exist')) {
            type = 'MODEL_NOT_FOUND';
            retryable = false;
        } else if (error.code === 'ECONNREFUSED' || error.code === 'ETIMEDOUT' || error.code === 'ENOTFOUND' || message.includes('fetch failed')) {
            type = 'NETWORK_ERROR';
            retryable = true;
        }

        return new DriverError(
            type,
            message,
            error,
            retryable,
            statusCode
        );
    }

    private extractUsage(usage: any): ModelUsage | undefined {
        if (!usage) {
            return undefined;
        }
        const input = usage.prompt_tokens;
        const response = usage.completion_tokens;
        const total = usage.total_tokens;
        const thinking = usage.completion_tokens_details?.reasoning_tokens ?? usage.reasoning_tokens;
        if (typeof input !== "number" || typeof response !== "number" || typeof total !== "number") {
            return undefined;
        }
        return {
            input,
            response,
            thinking: typeof thinking === "number" ? thinking : undefined,
            total
        };
    }

    private parseToolArgs(args: unknown): ToolArgs {
        if (typeof args === "string") {
            try {
                const parsed = JSON.parse(args);
                if (parsed && typeof parsed === "object") {
                    return parsed as ToolArgs;
                }
            } catch {
                return {};
            }
            return {};
        }
        if (args && typeof args === "object") {
            return args as ToolArgs;
        }
        return {};
    }

    private toOpenAiMessage(message: SyntheticMessage): OpenAI.Chat.ChatCompletionMessageParam {
        if (message.role === "tool") {
            return {
                role: "tool",
                content: this.contentToText(message.content),
                tool_call_id: message.toolCallId
            };
        }
        return {
            role: message.role,
            content: this.contentToText(message.content)
        };
    }

    private contentToText(content: ContentBlock[] | string): string {
        if (typeof content === "string") return content;
        const parts = content.map(block => {
            if (block.type === "text") return block.text;
            if (block.type === "thinking") return block.summary ?? block.text ?? "";
            if (block.type === "tool_use") return `Call ${block.name} with ${JSON.stringify(block.input)}`;
            if (block.type === "tool_result") {
                if (typeof block.content === "string") return block.content;
                return this.contentToText(block.content);
            }
            return "";
        });
        return parts.join("");
    }

    private textBlocks(text: string): ContentBlock[] {
        return [{ type: "text", text }];
    }
}


