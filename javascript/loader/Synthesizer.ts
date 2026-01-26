import { ExpressionEvaluator } from './ExpressionEvaluator';
import type { AgentIR } from './types/ir';
import type { JsonSchema, SyntheticRequest, SyntheticMessage, SyntheticToolDef } from './types/protocol';

export class Synthesizer {
    constructor(private ir: AgentIR) { }

    /**
     * The main entry point.
     * Takes runtime inputs and converts them into a standardized request.
     */
    public async synthesize(input: Record<string, any>, context?: Record<string, any>, configName?: string): Promise<SyntheticRequest> {
        // 1. Build Messages
        const messages = await this.buildMessages(input, context, configName);

        // 2. Build Output Schema
        const responseSchema = this.buildOutputSchema();

        // Build Tools
        const tools = this.buildTools()

        const modelProvider = this.getModelProvider(configName);
        return {
            messages,
            responseSchema,
            tools,
            config: {
                model: modelProvider.type,  // Provider type: "gemini", "openai", "custom"
                modelName: modelProvider.modelName,
                temperature: 0
            }
        };
    }
    /**
     * Extract all unique provider types used in this agent configuration
     */
    public getRequiredModels(): string[] {
        const providers = new Set<string>();

        if (this.ir.modelConfig[0]?.defaultConfig?.model) {
            providers.add(this.ir.modelConfig[0].defaultConfig.model.type);
        }

        if (this.ir.modelConfig[0]?.namedConfig) {
            for (const config of this.ir.modelConfig[0].namedConfig) {
                if (config.model) {
                    providers.add(config.model.type);
                }
            }
        }

        return Array.from(providers);
    }

    private getModelProvider(configName?: string) {
        const config = this.getConfig(configName);
        return config?.model ?? { type: "gemini", modelName: "gemini-2.0-flash-exp" };
    }

    private getConfig(configName?: string) {
        if (configName) {
            return this.ir.modelConfig[0]?.namedConfig?.find(c => c.configName === configName) || this.ir.modelConfig[0]?.defaultConfig;
        }
        return this.ir.modelConfig[0]?.defaultConfig;
    }

    private async buildMessages(input: Record<string, any>, context?: Record<string, any>, configName?: string): Promise<SyntheticMessage[]> {
        const config = this.getConfig(configName);
        const promptConfig = config?.prompt;

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
        // 1. Convert regular Tools
        const toolDefs = (this.ir.tools || []).map(tool => ({
            name: tool.name,
            description: tool.description,
            parameters: this.paramsToSchema(tool.params)
        }));

        // 2. Convert Workflows (The Model sees them as tools too!)
        const workflowDefs = (this.ir.workflows || []).map(wf => {
            const usesThenContinue = this.workflowUsesThenContinue(wf);
            const baseDesc = wf.description || '';

            // Add awareness about thenContinue pattern
            const continueMeta = usesThenContinue
                ? ' [CONTINUES AFTER DELIVERY: This workflow automatically delivers its result to the user. After completion, do NOT call this workflow again unless there is a genuinely different task. If there is nothing else to do, simply notify the user that the task is complete.]'
                : '';

            return {
                name: wf.flowName,
                description: baseDesc + continueMeta,
                parameters: this.paramsToSchema(wf.flowParams)
            };
        });

        // 3. Convert Helpers (Exposed as special agent-tools)
        // Note: Transfer semantics are now controlled at call-site in workflows,
        // not as a static property of the helper
        const helperDefs = (this.ir.helpers || []).map(helper => ({
            name: helper.name,
            description: `[HELPER AGENT] ${helper.description}`,
            parameters: this.paramsToSchema(helper.input || {}),
            _meta: { isHelper: true }
        }));

        const allTools = [...toolDefs, ...workflowDefs, ...helperDefs];

        if (allTools.length === 0) {
            return undefined;
        }
        return allTools;
    }

    /**
     * Check if a workflow contains a transfer statement with thenContinue mode
     */
    private workflowUsesThenContinue(wf: any): boolean {
        return this.scanForThenContinue(wf.body || []);
    }

    private scanForThenContinue(statements: any[]): boolean {
        for (const stmt of statements) {
            if (stmt.type === 'transfer' && stmt.mode === 'thenContinue') {
                return true;
            }
            // Recurse into if statements
            if (stmt.type === 'if') {
                if (stmt.then && this.scanForThenContinue(stmt.then)) return true;
                if (stmt.else && this.scanForThenContinue(stmt.else)) return true;
            }
        }
        return false;
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