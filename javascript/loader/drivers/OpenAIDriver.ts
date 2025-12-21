/**
 * OpenAI-compatible driver that works with:
 * - OpenAI
 * - Azure OpenAI
 * - Groq
 * - Together AI
 * - Any OpenAI API-compatible provider
 */
import OpenAI from "openai";
import type { AgentDriver, DriverResult, SyntheticRequest } from "../types/protocol";

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


        // Execute - DON'T mix tool_choice with response_format
        const completion = await this.client.chat.completions.create({
            model,
            messages,
            tools: hasTools ? tools : undefined,
            tool_choice: hasTools ? "auto" : undefined,
            response_format: !hasTools && request.responseSchema ? { type: "json_object" } : undefined,
            temperature: request.config.temperature ?? 0
        });

        const choice = completion.choices[0];
        if (!choice) {
            throw new Error("No response from OpenAI");
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
    }
}


