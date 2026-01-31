import { ExpressionEvaluator } from './ExpressionEvaluator';
import type { AgentIR } from './types/ir';
import type { ContentBlock, JsonSchema, SyntheticRequest, SyntheticMessage, SyntheticToolDef } from './types/protocol';

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
                messages.push({ role: 'system', content: this.textBlocks(await systemContent) });
            }
        }

        messages.push({ role: 'user', content: this.textBlocks(userMessage) });
        return messages;
    }

    private textBlocks(text: string): ContentBlock[] {
        return [{ type: 'text', text }];
    }

    private async resolvePrompt(prompt: any, input: Record<string, any>, context?: Record<string, any>): Promise<string> {
        // Case 1: Simple string
        if (typeof prompt === 'string') {
            return prompt;
        }

        // Case 2: Parts with expressions/if statements
        if (prompt.type === 'parts' && Array.isArray(prompt.value)) {
            const evaluator = new ExpressionEvaluator();
            const ctx = context ?? {};
            const scope = new Map(Object.entries({ ...input, ...ctx, ctx, input }));
            return await evaluator.evaluateStatements(prompt.value, scope, true);
        }

        // Case 3: Simple wrapper
        if (prompt.type === 'simple') {
            return prompt.value;
        }

        // Case 4: Literal (string or number from grammar)
        if (prompt.type === 'literal') {
            return String(prompt.value);
        }

        if (prompt.type === 'template') {
            const evaluator = new ExpressionEvaluator();
            const ctx = context ?? {};
            const scope = new Map(Object.entries({ ...input, ...ctx, ctx, input }));
            return String(await evaluator.evaluate(prompt, scope));
        }

        // Case 5: Concatenation with + operator
        if (prompt.type === 'concat') {
            const left = await this.resolvePrompt(prompt.left, input, context);
            const right = await this.resolvePrompt(prompt.right, input, context);
            return left + right;
        }

        // Case 6: Prompt reference (named prompt)
        if (prompt.type === 'promptRef' && Array.isArray(prompt.value)) {
            const evaluator = new ExpressionEvaluator();
            const ctx = context ?? {};
            const scope = new Map(Object.entries({ ...input, ...ctx, ctx, input }));
            if (Array.isArray(prompt.args) && Array.isArray(prompt.params) && prompt.params.length > 0) {
                const argValues = [] as any;
                for (const arg of prompt.args) {
                    argValues.push(await evaluator.evaluate(arg, scope));
                }
                const promptScope = new Map(scope);
                prompt.params.forEach((param: string, index: number) => {
                    promptScope.set(param, argValues[index]);
                });
                return await evaluator.evaluateStatements(prompt.value, promptScope, true);
            }
            return await evaluator.evaluateStatements(prompt.value, scope, true);
        }

        // Case 7: Inline prompt block { ... }
        if (prompt.type === 'inlinePrompt' && Array.isArray(prompt.parts)) {
            const evaluator = new ExpressionEvaluator();
            const ctx = context ?? {};
            const scope = new Map(Object.entries({ ...input, ...ctx, ctx, input }));
            return await evaluator.evaluateStatements(prompt.parts, scope, true);
        }

        // Legacy case: ref type (for backwards compatibility)
        if (prompt.type === 'ref' && Array.isArray(prompt.value)) {
            const evaluator = new ExpressionEvaluator();
            const ctx = context ?? {};
            const scope = new Map(Object.entries({ ...input, ...ctx, ctx, input }));
            return await evaluator.evaluateStatements(prompt.value, scope, true);
        }

        if (prompt?.type) {
            const evaluator = new ExpressionEvaluator();
            const ctx = context ?? {};
            const scope = new Map(Object.entries({ ...input, ...ctx, ctx, input }));
            return String(await evaluator.evaluate(prompt, scope));
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

        for (const [key, typeInfo] of Object.entries(this.ir.output)) {
            // Convert IRType to JSON Schema
            properties[key] = this.convertTypeToSchema(typeInfo.type);

            // Add description if present
            if (typeInfo.description) {
                properties[key].description = typeInfo.description;
            }

            // Only add to required if NOT optional
            if (!typeInfo.optional) {
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

        for (const [key, typeInfo] of Object.entries(params)) {
            // Convert IRType to JSON Schema
            properties[key] = this.convertTypeToSchema(typeInfo.type);

            if (!typeInfo.optional) {
                requiredFields.push(key);
            }
        }
        return {
            type: "object",
            properties,
            required: requiredFields
        };
    }

    // Helper to unwrap nested type structures - DEPRECATED, kept for compatibility
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


    /**
     * Converts an IRType to JSON Schema
     * Handles: primitives, arrays, type references, unions, and inline objects
     */
    private convertTypeToSchema(irType: any): JsonSchema {
        // Case 1: Primitive string types
        if (typeof irType === 'string') {
            // Handle legacy array syntax "string[]"
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

        // Case 2: Array type object { type: "array", items: ... }
        if (typeof irType === 'object' && irType.type === 'array') {
            return {
                type: "array",
                items: this.convertTypeToSchema(irType.items)
            };
        }

        // Case 3: Type reference { type: "typeRef", name: "TypeName" }
        if (typeof irType === 'object' && irType.type === 'typeRef') {
            return this.typeDefToSchema(irType.name);
        }

        // Case 4: Union type { type: "union", options: [...] }
        if (typeof irType === 'object' && irType.type === 'union' && Array.isArray(irType.options)) {
            return {
                type: 'string',
                enum: irType.options.map((o: string) => o.replace(/^["']|["']$/g, ''))
            };
        }

        // Case 5: Inline object type { type: "object", properties: {...} }
        if (typeof irType === 'object' && irType.type === 'object' && irType.properties) {
            return this.objectTypeToSchema(irType.properties);
        }

        // Fallback: treat as string
        return { type: 'string' };
    }

    /**
     * Resolves a type reference to its JSON Schema definition
     * Recursively resolves nested type references
     */
    private typeDefToSchema(typeName: string): JsonSchema {
        if (!this.ir.types || !this.ir.types[typeName]) {
            // Type not found, return generic object
            return { type: 'object' };
        }

        const typeDef = this.ir.types[typeName];
        const properties: Record<string, JsonSchema> = {};
        const required: string[] = [];

        for (const [propName, propInfo] of Object.entries(typeDef.properties)) {
            // Recursively convert property type
            properties[propName] = this.convertTypeToSchema(propInfo.type);

            // Add description if present
            if (propInfo.description) {
                properties[propName].description = propInfo.description;
            }

            // Track required fields
            if (!propInfo.optional) {
                required.push(propName);
            }
        }

        return {
            type: "object",
            properties,
            required
        };
    }


    /**
     * Converts an inline object type definition to JSON Schema
     * Input: { title: "string", url: "string", snippet: "string" }
     * Output: JSON Schema with nested properties
     */
    private objectTypeToSchema(properties: Record<string, any>): JsonSchema {
        const schemaProps: Record<string, JsonSchema> = {};
        const required: string[] = [];

        for (const [key, irType] of Object.entries(properties)) {
            // Recursively convert each property type
            schemaProps[key] = this.convertTypeToSchema(irType);
            
            // For inline objects, all properties are required by default
            // (optional handling is done at the PropertyInfo level in type definitions)
            required.push(key);
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
