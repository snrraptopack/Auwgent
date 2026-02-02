/**
 * OpenAI-compatible driver that works with:
 * - OpenAI
 * - Azure OpenAI
 * - Groq
 * - Together AI
 * - Any OpenAI API-compatible provider
 */
import OpenAI from "openai";
import type { AgentDriver, ContentBlock, DriverResult, StreamChunk, SyntheticRequest, SyntheticMessage, ModelUsage } from "../types/protocol";
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

            const providerConfig = request.config.providerConfig ?? {};

            // Execute - Force tool use with 'required' when tools are available
            const completion = await this.client.chat.completions.create({
                ...providerConfig,
                model,
                messages,
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

            // Log streaming start (no provider tools involved)
            logger.debug(`[OpenAI] Streaming text-only response`);

            const providerConfig = request.config.providerConfig ?? {};

            // Use streaming API with usage tracking
            const stream = await (this.client.chat.completions.create({
                ...providerConfig,
                model,
                messages,
                temperature: request.config.temperature ?? 0,
                stream: true,
                stream_options: { include_usage: true }  // Get token usage in streaming
            }) as any);

            let fullText = '';
            let usage: ModelUsage | undefined;

            for await (const chunk of stream) {
                const delta = chunk.choices[0]?.delta;

                // Handle text content
                if (delta?.content) {
                    fullText += delta.content;
                    yield { type: 'text', delta: delta.content };
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


