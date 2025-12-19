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
        const model = request.config.model || "gemini-2.0-flash-exp";

        // 2. Map Messages to Google Content Format
        const contents = request.messages
            .filter(m => m.role !== 'system') // System prompts go in config
            .map(m => ({
                role: m.role === 'assistant' ? 'model' : 'user',
                parts: [{ text: m.content }]
            }));

        const systemInstruction = request.messages
            .find(m => m.role === 'system')?.content;

        // 3. Map Schema (The crucial part)
        let generationConfig: any = {};

        if (request.responseSchema) {
            generationConfig.responseMimeType = "application/json";
            generationConfig.responseSchema = request.responseSchema;
        }

        //Map Tools
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


        // 4. Execute
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
        if (firstPart?.functionCall) {
            return {
                toolParams: {
                    name: firstPart.functionCall.name ?? "unknow_tool",
                    args: firstPart.functionCall.args
                }
            };
        }
        return {
            text: result.text ?? ""
        };
    }
}