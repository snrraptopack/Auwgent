import { GoogleGenAI } from "@google/genai";
import type { AgentDriver, DriverResult, SyntheticRequest } from "../types/protocol";

export class GoogleDriver implements AgentDriver {
    name = "google";
    private client: GoogleGenAI;

    constructor(apiKey: string) {
        this.client = new GoogleGenAI({ apiKey });
    }

    async execute(request: SyntheticRequest): Promise<DriverResult> {
        // 1. Map Configuration
        const model = request.config.model || "gemini-2.0-flash";

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
            generationConfig.responseSchema = request.responseSchema;
            systemInstruction += `\n\nYou must respond with valid JSON matching this schema: ${JSON.stringify(request.responseSchema)}`;
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

}