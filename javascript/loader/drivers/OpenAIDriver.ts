/**
 * OpenAI-compatible driver that works with:
 * - OpenAI
 * - Azure OpenAI
 * - Groq
 * - Together AI
 * - Any OpenAI API-compatible provider
 */
import OpenAI from "openai";
import type { AgentDriver, DriverResult, StreamChunk, SyntheticRequest } from "../types/protocol";
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
            const model = request.config.model || "gpt-4o-mini";

            // Build messages
            const messages: OpenAI.Chat.ChatCompletionMessageParam[] = request.messages.map(m => ({
                role: m.role as "user" | "assistant" | "system",
                content: m.content
            }));

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

            // Add schema instruction ONLY when no tools (final response turn)
            if (request.responseSchema && !hasTools) {
                const schemaInstruction = `\n\nYou must respond with valid JSON matching this schema: ${JSON.stringify(request.responseSchema)}`;
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


            // Execute - Force tool use with 'required' when tools are available
            const completion = await this.client.chat.completions.create({
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

            // Track token usage
            if (completion.usage) {
                logger.trackTokens({
                    promptTokens: completion.usage.prompt_tokens,
                    completionTokens: completion.usage.completion_tokens,
                    totalTokens: completion.usage.total_tokens
                });
                logger.debug(`[OpenAI] Tokens: ${completion.usage.total_tokens} (${completion.usage.prompt_tokens}+${completion.usage.completion_tokens})`);
            }

            // Check for tool calls
            if (choice.message?.tool_calls && choice.message.tool_calls.length > 0) {
                const toolCall = choice.message.tool_calls[0] as any;
                const name = toolCall.function?.name || toolCall.name;
                const args = toolCall.function?.arguments || toolCall.arguments;

                return {
                    toolParams: {
                        name,
                        args: typeof args === 'string' ? JSON.parse(args) : args
                    }
                };
            }

            // Return text response
            return {
                text: choice.message?.content ?? ""
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
            const model = request.config.model || "gpt-4o-mini";

            const messages: OpenAI.Chat.ChatCompletionMessageParam[] = request.messages.map(m => ({
                role: m.role as "user" | "assistant" | "system",
                content: m.content
            }));

            const tools: any[] | undefined = request.tools?.map(t => ({
                type: "function",
                function: {
                    name: t.name,
                    description: t.description,
                    parameters: t.parameters
                }
            }));

            const hasTools = tools && tools.length > 0;

            if (request.responseSchema && !hasTools) {
                const schemaInstruction = `\n\nYou must respond with valid JSON matching this schema: ${JSON.stringify(request.responseSchema)}`;
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

            // Use streaming API with usage tracking
            const stream = await (this.client.chat.completions.create({
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
            let toolParams: { name: string; args: any } | undefined;
            const toolArgsBuffer: Record<string, string> = {};
            const toolNameBuffer: Record<string, string> = {};  // Track tool names
            const activeToolIds: Record<number, string> = {}; // Track index -> id mapping

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

                    // Capture first tool for return (TODO: support returning multiple)
                    const firstToolId = Object.keys(toolArgsBuffer)[0];
                    if (firstToolId && toolNameBuffer[firstToolId]) {
                        toolParams = {
                            name: toolNameBuffer[firstToolId],
                            args: JSON.parse(toolArgsBuffer[firstToolId] || '{}')
                        };
                    }
                }

                // Track token usage from final chunk (OpenAI sends it with stream_options)
                if ((chunk as any).usage) {
                    const usage = (chunk as any).usage;
                    logger.trackTokens({
                        promptTokens: usage.prompt_tokens,
                        completionTokens: usage.completion_tokens,
                        totalTokens: usage.total_tokens
                    });
                    logger.debug(`[OpenAI] Tokens: ${usage.total_tokens} (${usage.prompt_tokens}+${usage.completion_tokens})`);
                }
            }

            // Return final result
            if (toolParams) {
                return { toolParams };
            }

            // Fallback: If we have a buffered tool call that wasn't captured (e.g. missed finish_reason)
            const firstToolId = Object.keys(toolNameBuffer)[0];
            if (firstToolId) {
                try {
                    return {
                        toolParams: {
                            name: toolNameBuffer[firstToolId] || '',
                            args: JSON.parse(toolArgsBuffer[firstToolId] || '{}')
                        }
                    };
                } catch (e) {
                    logger.warn("[OpenAI] Failed to parse buffered tool args", e);
                    // Return text if parsing failed, or partial tool? Better to fall through to text or throw.
                    // Assuming if we have a name, it was intended as a tool call.
                }
            }

            return { text: fullText };
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
}


