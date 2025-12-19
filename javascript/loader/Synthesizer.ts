import type { AgentIR } from './types/ir';
import type { JsonSchema, SyntheticRequest, SyntheticMessage, SyntheticToolDef } from './types/protocol';

export class Synthesizer {
    constructor(private ir: AgentIR) { }

    /**
     * The main entry point.
     * Takes runtime inputs and converts them into a standardized request.
     */
    public synthesize(input: Record<string, any>): SyntheticRequest {
        // 1. Build Messages
        const messages = this.buildMessages(input);

        // 2. Build Output Schema
        const responseSchema = this.buildOutputSchema();

        // Build Tools
        const tools = this.buildTools()

        return {
            messages,
            responseSchema,
            tools,
            config: {
                model: this.getModelName(),
                temperature: 0
            }
        };
    }

    private getModelName(): string {
        return this.ir.modelConfig[0]?.defaultConfig.modelName ?? "gemini-2.0-flash-exp";
    }

    private buildMessages(input: Record<string, any>): SyntheticMessage[] {
        const promptTemplate = this.ir.modelConfig[0]?.defaultConfig.prompt;
        const userMessage = Object.entries(input)
            .map(([k, v]) => `${k}: ${v}`)
            .join("\n");

        const messages: SyntheticMessage[] = [];

        if (promptTemplate) {
            messages.push({ role: 'system', content: promptTemplate });
        }

        messages.push({ role: 'user', content: userMessage });

        return messages;
    }

    /**
     * Converts the IR 'output' definition into strict JSON Schema
     */
    private buildOutputSchema(): JsonSchema | undefined {
        if (!this.ir.output || Object.keys(this.ir.output).length === 0) {
            return undefined;
        }

        const properties: Record<string, JsonSchema> = {};

        for (const [key, val] of Object.entries(this.ir.output)) {
            const typeInfo = typeof val === 'string' ? { type: val } : val;

            properties[key] = this.convertTypeToSchema(typeInfo.type);
            // Add description if present
            if ((typeInfo as any).description) {
                properties[key].description = (typeInfo as any).description;
            }
        }

        return {
            type: "object",
            properties,
            required: Object.keys(properties)
        };
    }

    private buildTools(): SyntheticToolDef[] | undefined {
        if (!this.ir.tools || this.ir.tools.length === 0) {
            return undefined;
        }

        // 1. Convert regular Tools
        const toolDefs = (this.ir.tools || []).map(tool => ({
            name: tool.name,
            description: tool.description,
            parameters: this.paramsToSchema(tool.params)
        }));

        // 2. Convert Workflows (The Model sees them as tools too!)
        const workflowDefs = (this.ir.workflows || []).map(wf => ({
            name: wf.flowName,
            description: wf.description,
            parameters: this.paramsToSchema(wf.flowParams)
        }));

        const allTools = [...toolDefs, ...workflowDefs]

        if (allTools.length === 0) {
            return undefined;
        }
        return allTools
    }

    private paramsToSchema(params: Record<string, string>): JsonSchema {
        const properties: Record<string, JsonSchema> = {};

        for (const [key, type] of Object.entries(params)) {
            properties[key] = this.convertTypeToSchema(type);
        }
        return {
            type: "object",
            properties,
            required: Object.keys(properties)
        };
    }


    private convertTypeToSchema(irType: string): JsonSchema {
        if (irType.endsWith("[]")) {
            const innerType = irType.slice(0, -2);
            return {
                type: "array",
                items: {
                    type: this.normalizeType(innerType)
                }
            };
        }
        return {
            type: this.normalizeType(irType)
        };
    }

    private normalizeType(irType: string): string {
        switch (irType.toLowerCase()) {
            case 'int':
            case 'float':
            case 'number': return 'number';
            case 'bool':
            case 'boolean': return 'boolean';
            case 'string': return 'string';
            default: return 'string';
        }
    }
}