import { GoogleGenAI } from "@google/genai";
import type { AgentDriver, ContentBlock, DriverResult, StreamChunk, SyntheticRequest, ModelUsage } from "../types/protocol";
import { DriverError, type ErrorType } from "../types/errors";

export class GoogleDriver implements AgentDriver {
    name = "gemini";
    private client: GoogleGenAI;
    
    constructor(apiKey: string) {
        this.client = new GoogleGenAI({ apiKey });
    }

    async execute(request: SyntheticRequest): Promise<DriverResult> {
        try {
            // 1. Map Configuration
            const model = request.config.modelName || request.config.model || "gemini-2.0-flash";

            // 2. Map Messages to Google Content Format
            const contents = request.messages
                .filter(m => m.role !== 'system')
                .map(m => ({
                    role: m.role === 'assistant' ? 'model' : 'user',
                    parts: [{ text: this.contentToText(m.content) }]
                }));

            let systemInstruction = this.contentToText(request.messages.find(m => m.role === 'system')?.content ?? "");

            // 3. Map Tools
            let generationConfig: any = {};

            const providerConfig = request.config.providerConfig ?? {};

            // 5. Execute
            const result = await this.client.models.generateContent({
                model,
                contents,
                config: {
                    ...providerConfig,
                    systemInstruction,
                    ...generationConfig
                }
            });

            const usage = this.extractUsage((result as any).usageMetadata);

            return {
                content: this.textBlocks(result.text ?? ""),
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
            const model = request.config.modelName || "gemini-2.0-flash";

            const contents = request.messages
                .filter(m => m.role !== 'system')
                .map(m => ({
                    role: m.role === 'assistant' ? 'model' : 'user',
                    parts: [{ text: this.contentToText(m.content) }]
                }));

            let systemInstruction = this.contentToText(request.messages.find(m => m.role === 'system')?.content ?? "");

            let generationConfig: any = {};

            const providerConfig = request.config.providerConfig ?? {};

            // Use streaming API
            const stream = await this.client.models.generateContentStream({
                model,
                contents,
                config: {
                    ...providerConfig,
                    systemInstruction,
                    ...generationConfig
                }
            });

            let fullText = '';
            let usage: ModelUsage | undefined;

            for await (const chunk of stream) {
                const chunkUsage = this.extractUsage((chunk as any).usageMetadata);
                if (chunkUsage) {
                    usage = chunkUsage;
                }

                // Stream text delta
                const delta = chunk.text ?? '';
                if (delta) {
                    fullText += delta;
                    yield { type: 'text', delta };
                }
            }

            // Return final result
            return { content: this.textBlocks(fullText), usage };
        } catch (error: any) {
            throw this.handleError(error);
        }
    }

    private extractUsage(usageMetadata: any): ModelUsage | undefined {
        if (!usageMetadata) {
            return undefined;
        }
        const input = usageMetadata.promptTokenCount ?? usageMetadata.prompt_token_count;
        const response = usageMetadata.candidatesTokenCount ?? usageMetadata.candidates_token_count;
        const total = usageMetadata.totalTokenCount ?? usageMetadata.total_token_count;
        const thinking = usageMetadata.thoughtsTokenCount ?? usageMetadata.thoughts_token_count;
        const cachedInput = usageMetadata.cachedContentTokenCount ?? usageMetadata.cached_content_token_count;
        if (typeof input !== "number" || typeof response !== "number" || typeof total !== "number") {
            return undefined;
        }
        return {
            input,
            response,
            thinking: typeof thinking === "number" ? thinking : undefined,
            total,
            cachedInput: typeof cachedInput === "number" ? cachedInput : undefined
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

    /**
     * Classify and wrap errors from Google AI SDK
     */
    private handleError(error: any): DriverError {
        // Extract error details
        const message = error.message || 'Unknown error';
        const statusCode = error.status || error.statusCode;

        // Classify error type
        let type: ErrorType = 'UNKNOWN_ERROR';
        let retryable = false;

        if (statusCode === 401 || statusCode === 403 || message.includes('API key')) {
            type = 'AUTH_ERROR';
            retryable = false;
        } else if (statusCode === 429 || message.includes('rate limit')) {
            type = 'RATE_LIMIT';
            retryable = true;
        } else if (statusCode === 400 || message.includes('invalid')) {
            type = 'INVALID_REQUEST';
            retryable = false;
        } else if (message.includes('content') && message.includes('policy')) {
            type = 'CONTENT_POLICY';
            retryable = false;
        } else if (message.includes('token') || message.includes('context')) {
            type = 'TOKEN_LIMIT';
            retryable = false;
        } else if (statusCode === 404 || message.includes('model not found')) {
            type = 'MODEL_NOT_FOUND';
            retryable = false;
        } else if (error.code === 'ECONNREFUSED' || error.code === 'ETIMEDOUT' || error.code === 'ENOTFOUND') {
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
