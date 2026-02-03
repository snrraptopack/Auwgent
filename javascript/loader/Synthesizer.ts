import { ExpressionEvaluator } from './ExpressionEvaluator';
import type { AgentIR, ModelConfig } from './types/ir';
import type { ContentBlock, JsonSchema, SyntheticRequest, SyntheticMessage } from './types/protocol';

export class Synthesizer {
    constructor(private ir: AgentIR) { }

    /**
     * The main entry point.
     * Takes runtime inputs and converts them into a standardized request.
     */
    public async synthesize(input: Record<string, any>, context?: Record<string, any>, configName?: string): Promise<SyntheticRequest> {
        // Build messages with embedded tool/output schema in system prompt
        const messages = await this.buildMessages(input, context, configName);

        const modelProvider = this.getModelProvider(configName);
        const providerConfig = await this.resolveProviderConfig(modelProvider?.config, input, context);

        // No tools field - everything is in the prompt, model outputs YAML
        return {
            messages,
            config: {
                model: modelProvider.type,  // Provider type: "gemini", "openai", "custom"
                modelName: modelProvider.modelName,
                temperature: 0,
                providerConfig
            }
        };
    }
    /**
     * Extract all unique provider types used in this agent configuration
     */
    public getRequiredModels(): string[] {
        const providers = new Set<string>();

        const collectFromConfigBlock = (configs?: ModelConfig[]) => {
            for (const block of configs ?? []) {
                this.addProvider(providers, block?.defaultConfig?.model);
                for (const named of block?.namedConfig ?? []) {
                    this.addProvider(providers, named?.model);
                }
            }
        };

        collectFromConfigBlock(this.ir.modelConfig);
        for (const helper of this.ir.helpers ?? []) {
            collectFromConfigBlock(helper.modelConfig);
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

    private async resolveProviderConfig(config: any, input: Record<string, any>, context?: Record<string, any>): Promise<Record<string, any> | undefined> {
        if (!config) {
            return undefined;
        }
        const evaluator = new ExpressionEvaluator();
        const ctx = context ?? {};
        const scope = new Map(Object.entries({ ...input, ...ctx, ctx, input }));
        const resolved = await evaluator.evaluate(config, scope);
        if (resolved && typeof resolved === "object") {
            return resolved;
        }
        return undefined;
    }

    private async buildMessages(input: Record<string, any>, context?: Record<string, any>, configName?: string): Promise<SyntheticMessage[]> {
        const config = this.getConfig(configName);
        const promptConfig = config?.prompt;

        const userMessage = Object.entries(input)
            .map(([key, value]) => this.formatInputLine(key, value))
            .join("\n");

        const messages: SyntheticMessage[] = [];

        const systemContent = this.buildSystemPrompt(promptConfig, input, context);
        if (systemContent) {
            messages.push({ role: 'system', content: this.textBlocks(await systemContent) });
        }

        messages.push({ role: 'user', content: this.textBlocks(userMessage) });
        return messages;
    }

    private textBlocks(text: string): ContentBlock[] {
        return [{ type: 'text', text }];
    }

    private addProvider(set: Set<string>, provider?: { type?: string | undefined } | null): void {
        if (!provider || !provider.type) {
            return;
        }
        set.add(provider.type);
    }

    private formatInputLine(key: string, value: unknown): string {
        const serialized = this.serializeValue(value);
        if (!serialized.includes('\n')) {
            return `${key}: ${serialized}`;
        }
        const indented = serialized
            .split('\n')
            .map((line, index) => (index === 0 ? line : `  ${line}`))
            .join('\n');
        return `${key}: ${indented}`;
    }

    private serializeValue(value: unknown): string {
        if (typeof value === 'string') {
            return value;
        }
        if (value === undefined) {
            return 'undefined';
        }
        if (typeof value === 'number' || typeof value === 'boolean' || value === null) {
            return String(value);
        }
        try {
            return JSON.stringify(value, null, 2);
        } catch (error) {
            return String(value);
        }
    }

    private async resolvePrompt(prompt: any, input: Record<string, any>, context?: Record<string, any>): Promise<string> {
        // Case 1: Simple string
        if (typeof prompt === 'string') {
            return prompt;
        }

        // Create evaluator with schema context for {{@schema()}} directives
        const evaluator = this.createEvaluatorWithSchemaContext();
        const ctx = context ?? {};
        const scope = new Map(Object.entries({ ...input, ...ctx, ctx, input }));

        // Case 2: Parts with expressions/if statements
        if (prompt.type === 'parts' && Array.isArray(prompt.value)) {
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

        // Case 5: Concatenation with + operator
        if (prompt.type === 'concat') {
            const left = await this.resolvePrompt(prompt.left, input, context);
            const right = await this.resolvePrompt(prompt.right, input, context);
            return left + right;
        }

        // Case 6: Prompt reference (named prompt)
        if (prompt.type === 'promptRef' && Array.isArray(prompt.value)) {
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
            return await evaluator.evaluateStatements(prompt.parts, scope, true);
        }

        // Case 8: Template with {{#if}}, {{@schema}}, etc.
        if (prompt.type === 'template' && Array.isArray(prompt.value)) {
            return await evaluator.evaluate(prompt, scope);
        }

        // Legacy case: ref type (for backwards compatibility)
        if (prompt.type === 'ref' && Array.isArray(prompt.value)) {
            return await evaluator.evaluateStatements(prompt.value, scope, true);
        }

        if (prompt?.type) {
            return String(await evaluator.evaluate(prompt, scope));
        }

        return '';
    }

    /**
     * Create an ExpressionEvaluator with schema context for {{@schema()}} directives.
     * The context is scoped to the current agent's definitions.
     */
    private createEvaluatorWithSchemaContext(): ExpressionEvaluator {
        const evaluator = new ExpressionEvaluator();
        evaluator.setSchemaContext({
            output: this.ir.output,
            input: this.ir.input,
            context: this.ir.context,
            types: this.ir.types
        });
        return evaluator;
    }

    private async buildSystemPrompt(promptConfig: any, input: Record<string, any>, context?: Record<string, any>): Promise<string> {
        const userPrompt = promptConfig ? await this.resolvePrompt(promptConfig, input, context) : "";
        const defaultPrompt = this.buildDefaultPrompt();
        return [userPrompt?.trim(), defaultPrompt?.trim()].filter(Boolean).join("\n\n");
    }

    private buildDefaultPrompt(): string {
        const hasOutput = this.hasOutput();
        const hasTooling = this.hasTooling();

        if (!hasOutput && !hasTooling) {
            return "Respond in plain text. No code fences. Be concise.";
        }

        const lines: string[] = [];

        lines.push(hasTooling ? "You are an AI agent. Output YAML only. No code fences." : "Output YAML only. No code fences.");

        const toolLines = this.renderToolSignatures();
        if (toolLines.length > 0) {
            lines.push("", "Tools:");
            lines.push(...toolLines);
        }

        const helperLines = this.renderHelperSignatures();
        if (helperLines.length > 0) {
            lines.push("", "Helpers (sub-agents that handle tasks autonomously):");
            lines.push(...helperLines);
        }

        const workflowLines = this.renderWorkflowSignatures();
        if (workflowLines.length > 0) {
            lines.push("", "Workflows:");
            lines.push(...workflowLines);
        }

        // Add constraint once at the end
        if (toolLines.length > 0 || helperLines.length > 0 || workflowLines.length > 0) {
            lines.push("", "Only use names listed above.");
        }

        lines.push("", "Schema:");

        if (hasTooling) {
            const intentTypes = this.buildAvailableIntentTypes();
            lines.push("text: string");
            lines.push("question: string");
            lines.push("intents:");
            lines.push(`  - type: ${intentTypes.join("|")}`);
            lines.push("    name: string");
            lines.push("    args: {}");
            lines.push("parallel: boolean");
        }

        if (hasOutput) {
            lines.push("output:");
            lines.push(...this.renderOutputSchemaLines(2));
        }

        lines.push("", "Be concise.");

        return lines.join("\n");
    }

    /**
     * Build the list of available intent types based on what the agent has defined.
     * Only includes types that the model can actually use.
     */
    private buildAvailableIntentTypes(): string[] {
        const types: string[] = [];
        
        // Only add tool_call if there are tools
        if (this.ir.tools && this.ir.tools.length > 0) {
            types.push("tool_call");
        }
        
        // Only add workflow if there are workflows
        if (this.ir.workflows && this.ir.workflows.length > 0) {
            types.push("workflow");
        }
        
        // Only add helper if there are helpers
        if (this.ir.helpers && this.ir.helpers.length > 0) {
            types.push("helper");
        }
        
        // If no tooling at all, this shouldn't be called, but fallback
        if (types.length === 0) {
            types.push("tool_call");
        }
        
        return types;
    }

    private hasOutput(): boolean {
        return !!this.ir.output && Object.keys(this.ir.output).length > 0;
    }

    private renderToolsSection(): string[] {
        const tools = this.ir.tools ?? [];
        const helpers = this.ir.helpers ?? [];
        if (tools.length === 0 && helpers.length === 0) {
            return [];
        }
        const lines: string[] = ["Tools:"];
        for (const tool of tools) {
            lines.push(`- name: ${tool.name}`);
            if (tool.description) {
                lines.push(`  description: ${tool.description}`);
            }
            lines.push("  args:");
            lines.push(...this.renderParamSchemaLines(tool.params, 4));
            lines.push(`  returns: ${this.formatInlineType(tool.returns)}`);
        }
        for (const helper of helpers) {
            lines.push(`- name: ${helper.name}`);
            if (helper.description) {
                lines.push(`  description: ${helper.description}`);
            }
            lines.push("  args:");
            lines.push(...this.renderParamSchemaLines(helper.input || {}, 4));
            if (helper.output && Object.keys(helper.output).length > 0) {
                lines.push("  returns:");
                lines.push(...this.renderOutputSchemaLines(4, helper.output));
            }
        }
        return lines;
    }

    private renderWorkflowsSection(): string[] {
        const workflows = this.ir.workflows ?? [];
        if (workflows.length === 0) {
            return [];
        }
        const lines: string[] = ["Workflows:"];
        for (const workflow of workflows) {
            lines.push(`- name: ${workflow.flowName}`);
            if (workflow.description) {
                lines.push(`  description: ${workflow.description}`);
            }
            lines.push("  args:");
            lines.push(...this.renderParamSchemaLines(workflow.flowParams, 4));
            lines.push(`  returns: ${this.formatInlineType(workflow.returns)}`);
        }
        return lines;
    }

    private renderParamSchemaLines(params: Record<string, any>, indent: number): string[] {
        const entries = Object.entries(params);
        if (entries.length === 0) {
            return [`${" ".repeat(indent)}{}`];
        }
        const lines: string[] = [];
        for (const [name, typeInfo] of entries) {
            lines.push(...this.renderFieldLines(name, typeInfo, indent));
        }
        return lines;
    }

    private renderOutputSchemaLines(indent: number, output?: Record<string, any>): string[] {
        const source = output ?? this.ir.output ?? {};
        const lines: string[] = [];
        for (const [name, typeInfo] of Object.entries(source)) {
            lines.push(...this.renderFieldLines(name, typeInfo as any, indent));
        }
        return lines;
    }

    private renderFieldLines(name: string, typeInfo: any, indent: number): string[] {
        const lines: string[] = [];
        const optionalSuffix = typeInfo.optional ? "?" : "";
        const key = `${name}${optionalSuffix}`;
        const desc = typeInfo.description ? ` # ${typeInfo.description}` : "";
        const type = typeInfo.type;
        const arrayItem = this.getArrayItemType(type);
        if (arrayItem) {
            lines.push(`${" ".repeat(indent)}${key}:${desc}`);
            lines.push(...this.renderArrayItemLines(arrayItem, indent + 2));
            return lines;
        }
        const objectProps = this.getObjectProperties(type);
        if (objectProps) {
            lines.push(`${" ".repeat(indent)}${key}:${desc}`);
            for (const [propName, propInfo] of Object.entries(objectProps)) {
                lines.push(...this.renderFieldLines(propName, propInfo, indent + 2));
            }
            return lines;
        }
        lines.push(`${" ".repeat(indent)}${key}: ${this.formatInlineType(type)}${desc}`);
        return lines;
    }

    private renderArrayItemLines(itemType: any, indent: number): string[] {
        const lines: string[] = [];
        const nestedArrayItem = this.getArrayItemType(itemType);
        if (nestedArrayItem) {
            lines.push(`${" ".repeat(indent)}-`);
            lines.push(...this.renderArrayItemLines(nestedArrayItem, indent + 2));
            return lines;
        }
        const objectProps = this.getObjectProperties(itemType);
        if (objectProps) {
            lines.push(`${" ".repeat(indent)}-`);
            for (const [propName, propInfo] of Object.entries(objectProps)) {
                lines.push(...this.renderFieldLines(propName, propInfo, indent + 2));
            }
            return lines;
        }
        lines.push(`${" ".repeat(indent)}- ${this.formatInlineType(itemType)}`);
        return lines;
    }

    private getArrayItemType(irType: any): any | null {
        if (typeof irType === "string" && irType.endsWith("[]")) {
            return irType.slice(0, -2);
        }
        if (typeof irType === "object" && irType.type === "array") {
            return irType.items;
        }
        return null;
    }

    private hasTooling(): boolean {
        return (this.ir.tools?.length ?? 0) > 0 || (this.ir.helpers?.length ?? 0) > 0 || (this.ir.workflows?.length ?? 0) > 0;
    }

    private renderToolSignatures(): string[] {
        const tools = this.ir.tools ?? [];
        const entries: string[] = [];

        for (const tool of tools) {
            const params = this.formatParamSignature(tool.params);
            const suffix = tool.description ? `  # ${tool.description}` : "";
            entries.push(`- ${tool.name}${params ? `(${params})` : "()"}${suffix}`);
        }

        return entries;
    }

    private renderHelperSignatures(): string[] {
        const helpers = this.ir.helpers ?? [];
        const entries: string[] = [];

        for (const helper of helpers) {
            const params = this.formatParamSignature(helper.input ?? {});
            const suffix = helper.description ? `  # ${helper.description}` : "";
            entries.push(`- ${helper.name}${params ? `(${params})` : "()"}${suffix}`);
        }

        return entries;
    }

    private renderWorkflowSignatures(): string[] {
        const workflows = this.ir.workflows ?? [];
        const lines: string[] = [];
        for (const workflow of workflows) {
            const params = this.formatParamSignature(workflow.flowParams ?? {});
            const suffix = workflow.description ? `  # ${workflow.description}` : "";
            lines.push(`- ${workflow.flowName}${params ? `(${params})` : "()"}${suffix}`);
        }
        return lines;
    }

    private formatParamSignature(params: Record<string, any>): string {
        const entries = Object.entries(params ?? {});
        if (entries.length === 0) {
            return "";
        }
        return entries
            .map(([name, info]) => {
                const optional = info?.optional ? "?" : "";
                const typeName = this.formatInlineType(info?.type ?? info);
                return `${name}${optional}: ${typeName}`;
            })
            .join(", ");
    }

    private getObjectProperties(irType: any): Record<string, any> | null {
        if (typeof irType === "object" && irType.type === "object" && irType.properties) {
            return Object.fromEntries(
                Object.entries(irType.properties).map(([key, value]) => [
                    key,
                    { type: value, optional: false }
                ])
            );
        }
        if (typeof irType === "object" && irType.type === "typeRef") {
            const typeDef = this.ir.types?.[irType.name];
            if (typeDef?.properties) {
                return typeDef.properties;
            }
        }
        return null;
    }

    private formatInlineType(irType: any): string {
        if (typeof irType === "string") {
            if (irType.endsWith("[]")) {
                const innerType = irType.slice(0, -2);
                return `array<${this.normalizeType(innerType)}>`;
            }
            return this.normalizeType(irType);
        }
        if (typeof irType === "object" && irType.type === "union" && Array.isArray(irType.options)) {
            return irType.options.map((o: string) => o.replace(/^["']|["']$/g, '')).join(" | ");
        }
        if (typeof irType === "object" && irType.type === "array") {
            return `array<${this.formatInlineType(irType.items)}>`;
        }
        if (typeof irType === "object" && irType.type === "typeRef") {
            return irType.name;
        }
        if (typeof irType === "object" && irType.type === "object") {
            return "object";
        }
        return "string";
    }

    private normalizeType(type: string): string {
        switch (type.toLowerCase()) {
            case "int":
            case "integer":
            case "float":
            case "number":
                return "number";
            case "bool":
            case "boolean":
                return "boolean";
            case "str":
            case "string":
            case "text":
                return "string";
            case "any":
                return "any";
            default:
                return type;
        }
    }

    // ============================================================================
    // JSON Schema methods removed - using YAML output parsing instead
    // Tools and output schema are embedded in the system prompt,
    // model outputs YAML which is parsed by auwgent-yaml-lite
    // ============================================================================
}

