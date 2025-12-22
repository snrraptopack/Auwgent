import { ExpressionEvaluator } from './ExpressionEvaluator';
import type { AgentIR } from './types/ir';
import type { JsonSchema, SyntheticRequest, SyntheticMessage, SyntheticToolDef } from './types/protocol';

export class Synthesizer {
    constructor(private ir: AgentIR) { }

    /**
     * The main entry point.
     * Takes runtime inputs and converts them into a standardized request.
     */
    public async synthesize(input: Record<string, any>, context?: Record<string, any>): Promise<SyntheticRequest> {
        // 1. Build Messages
        const messages = await this.buildMessages(input, context);

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

    private async buildMessages(input: Record<string, any>, context?: Record<string, any>): Promise<SyntheticMessage[]> {
        const promptConfig = this.ir.modelConfig[0]?.defaultConfig.prompt;
        const userMessage = Object.entries(input)
            .map(([k, v]) => `${k}: ${v}`)
            .join("\n");

        const messages: SyntheticMessage[] = [];

        if (promptConfig) {
            const systemContent = this.resolvePrompt(promptConfig, input, context);
            if (systemContent) {
                messages.push({ role: 'system', content: await systemContent });
            }
        }

        messages.push({ role: 'user', content: userMessage });
        return messages;
    }

    private async resolvePrompt(prompt: any, input: Record<string, any>, context?: Record<string, any>): Promise<string> {
        // Case 1: Simple string
        if (typeof prompt === 'string') {
            return prompt;
        }

        // Case 2: Parts with expressions/if statements
        if (prompt.type === 'parts' && Array.isArray(prompt.value)) {
            const evaluator = new ExpressionEvaluator();
            const scope = new Map(Object.entries({ ...input, ...context }));
            return await evaluator.evaluateStatements(prompt.value, scope, true);
        }

        // Case 3: Simple wrapper
        if (prompt.type === 'simple') {
            return prompt.value;
        }

        if (prompt.type === 'ref' && Array.isArray(prompt.value)) {
            const evaluator = new ExpressionEvaluator();
            const scope = new Map(Object.entries(input));
            return await evaluator.evaluateStatements(prompt.value, scope, true);
        }

        return '';
    }

    private evaluatePromptParts(parts: any[]): string {
        // Similar to template literal evaluation
        // For now, just concatenate literals
        // Later you can add expression evaluation if needed
        return parts.map(part => {
            if (part.type === 'literal') return part.value;
            // Handle expressions if needed
            return '';
        }).join('');
    }


    /**
     * Converts the IR 'output' definition into strict JSON Schema
     */
    private buildOutputSchema(): JsonSchema | undefined {
        if (!this.ir.output || Object.keys(this.ir.output).length === 0) {
            return undefined;
        }

        const properties: Record<string, JsonSchema> = {};
        const requiredFields: string[] = [];

        for (const [key, val] of Object.entries(this.ir.output)) {
            const typeInfo = typeof val === 'string' ? { type: val } : val;
            const actualType = this.unwrapType(typeInfo.type);
            const isOptional = (typeInfo as any).optional === true;

            // Handle union type objects
            if (typeof actualType === 'object' && actualType.type === 'union' && Array.isArray(actualType.options)) {
                properties[key] = {
                    type: 'string',
                    enum: actualType.options.map((o: string) => o.replace(/^["']|["']$/g, ''))
                };
            } else if (typeof actualType === 'object' && actualType.type === 'object' && actualType.properties) {
                properties[key] = this.objectTypeToSchema(actualType.properties);
            } else {
                properties[key] = this.convertTypeToSchema(typeof actualType === 'string' ? actualType : 'string');
            }

            // Add description if present
            if ((typeInfo as any).description) {
                properties[key].description = (typeInfo as any).description;
            }

            // Only add to required if NOT optional
            if (!isOptional) {
                requiredFields.push(key);
            }
        }

        return {
            type: "object",
            properties,
            required: requiredFields
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

    private paramsToSchema(params: Record<string, any>): JsonSchema {
        const properties: Record<string, JsonSchema> = {};
        const requiredFields: string[] = [];

        for (const [key, typeVal] of Object.entries(params)) {
            const actualType = this.unwrapType(typeVal);
            const isOptional = typeof typeVal === 'object' && typeVal?.optional === true;

            // Handle union type objects
            if (typeof actualType === 'object' && actualType.type === 'union') {
                properties[key] = {
                    type: 'string',
                    enum: actualType.options.map((o: string) => o.replace(/^["']|["']$/g, ''))
                };
            } else if (typeof actualType === 'object' && actualType.type === 'object' && actualType.properties) {
                properties[key] = this.objectTypeToSchema(actualType.properties);
            } else {
                properties[key] = this.convertTypeToSchema(typeof actualType === 'string' ? actualType : 'string');
            }

            if (!isOptional) {
                requiredFields.push(key);
            }
        }
        return {
            type: "object",
            properties,
            required: requiredFields
        };
    }

    // Helper to unwrap nested type structures
    private unwrapType(typeVal: any): any {
        if (typeof typeVal === 'string') {
            return typeVal;
        }
        if (typeVal && typeof typeVal.type === 'object') {
            return this.unwrapType(typeVal.type);
        }
        if (typeVal && typeof typeVal.type === 'string') {
            return typeVal.type;
        }
        return typeVal;
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


    /**
 * Converts an object type definition to JSON Schema
 * Input: { name: "string", age: "number", address: { type: "object", properties: {...} } }
 * Output: JSON Schema with nested properties
 */
    private objectTypeToSchema(properties: Record<string, any>): JsonSchema {
        const schemaProps: Record<string, JsonSchema> = {};
        const required: string[] = [];

        for (const [key, typeVal] of Object.entries(properties)) {
            const actualType = this.unwrapType(typeVal);
            const isOptional = typeof typeVal === 'object' && typeVal?.optional === true;

            // Recursive: Handle nested objects
            if (typeof actualType === 'object' && actualType.type === 'object' && actualType.properties) {
                schemaProps[key] = this.objectTypeToSchema(actualType.properties);
            }
            // Handle unions
            else if (typeof actualType === 'object' && actualType.type === 'union') {
                schemaProps[key] = {
                    type: 'string',
                    enum: actualType.options.map((o: string) => o.replace(/^["']|["']$/g, ''))
                };
            }
            // Handle primitives and arrays
            else {
                schemaProps[key] = this.convertTypeToSchema(typeof actualType === 'string' ? actualType : 'string');
            }
            if (!isOptional) {
                required.push(key);
            }
        }

        return {
            type: "object",
            properties: schemaProps,
            required
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