import { GoogleGenAI } from "@google/genai";
import type { AgentDriver, DriverResult, StreamChunk, SyntheticRequest } from "../types/protocol";

export class GoogleDriver implements AgentDriver {
    name = "google";
    private client: GoogleGenAI;

    constructor(apiKey: string) {
        this.client = new GoogleGenAI({ apiKey });
    }

    async execute(request: SyntheticRequest): Promise<DriverResult> {
        // 1. Map Configuration
        const model = request.config.modelName || "gemini-2.0-flash";

        // 2. Map Messages to Google Content Format
        const contents = request.messages
            .filter(m => m.role !== 'system') // System prompts go in config
            .map(m => ({
                role: m.role === 'assistant' ? 'model' : 'user',
                parts: [{ text: m.content }]
            }));

        let systemInstruction = request.messages
            .find(m => m.role === 'system')?.content ?? "";

        // 3. Map Tools
        let toolsConfig: any[] = [];
        if (request.tools && request.tools.length > 0) {
            toolsConfig = [{
                functionDeclarations: request.tools.map(t => ({
                    name: t.name,
                    description: t.description,
                    parameters: t.parameters
                }))
            }];
        }

        // 4. Map Schema (IMPORTANT: Only use structured output if NO tools are present)
        let generationConfig: any = {};
        const hasTools = toolsConfig.length > 0;
        // Google doesn't support function calling + JSON schema at the same time
        // Only use structured output when there are no tools
        if (request.responseSchema && !hasTools) {
            generationConfig.responseMimeType = "application/json";
            generationConfig.responseJsonSchema = request.responseSchema;
        }

        // 5. Execute
        const result = await this.client.models.generateContent({
            model,
            contents,
            config: {
                systemInstruction,
                ...generationConfig,
                tools: toolsConfig.length > 0 ? toolsConfig : undefined
            }
        });

        const candidates = result.candidates;
        const firstPart = candidates && candidates[0]?.content?.parts ? candidates[0].content.parts[0] : null;

        // Check for function call
        if (firstPart?.functionCall) {
            return {
                toolParams: {
                    name: firstPart.functionCall.name ?? "unknown_tool",
                    args: firstPart.functionCall.args
                }
            };
        }

        // Return text response
        return {
            text: result.text ?? ""
        };
    }

    /**
     * Streaming execution using async generator
     */
    async *executeStream(request: SyntheticRequest): AsyncGenerator<StreamChunk, DriverResult, unknown> {
        const model = request.config.modelName || "gemini-2.0-flash";

        const contents = request.messages
            .filter(m => m.role !== 'system')
            .map(m => ({
                role: m.role === 'assistant' ? 'model' : 'user',
                parts: [{ text: m.content }]
            }));

        let systemInstruction = request.messages
            .find(m => m.role === 'system')?.content ?? "";

        let toolsConfig: any[] = [];
        if (request.tools && request.tools.length > 0) {
            toolsConfig = [{
                functionDeclarations: request.tools.map(t => ({
                    name: t.name,
                    description: t.description,
                    parameters: t.parameters
                }))
            }];
        }

        let generationConfig: any = {};
        const hasTools = toolsConfig.length > 0;
        if (request.responseSchema && !hasTools) {
            generationConfig.responseMimeType = "application/json";
            generationConfig.responseJsonSchema = request.responseSchema;
        }

        // Use streaming API
        const stream = await this.client.models.generateContentStream({
            model,
            contents,
            config: {
                systemInstruction,
                ...generationConfig,
                tools: toolsConfig.length > 0 ? toolsConfig : undefined
            }
        });

        let fullText = '';
        let toolParams: { name: string; args: any } | undefined;
        let toolCallId = 0;

        for await (const chunk of stream) {
            // Check for function call in chunk
            const candidates = chunk.candidates;
            const firstPart = candidates && candidates[0]?.content?.parts ? candidates[0].content.parts[0] : null;

            if (firstPart?.functionCall) {
                const id = `google_tool_${toolCallId++}`;
                const name = firstPart.functionCall.name ?? "unknown_tool";
                const args = firstPart.functionCall.args;

                // Emit full tool call lifecycle (Google doesn't stream args incrementally)
                yield { type: 'tool_start', name, id };
                yield { type: 'tool_args', id, delta: JSON.stringify(args) };
                yield { type: 'tool_end', id };

                toolParams = { name, args };
                continue;
            }

            // Stream text delta
            const delta = chunk.text ?? '';
            if (delta) {
                fullText += delta;
                yield { type: 'text', delta };
            }
        }

        // Return final result
        if (toolParams) {
            return { toolParams };
        }
        return { text: fullText };
    }
}