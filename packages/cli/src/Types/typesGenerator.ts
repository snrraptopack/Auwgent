/**
 * TypeScript Types Generator
 * Generates type-safe interfaces and factory functions from Agent IR
 */

interface HelperType {
    name: string;
    output: Record<string, any> | null;
}

interface AgentIR {
    name: string;
    input: Record<string, any> | null;
    output: Record<string, any> | null;
    context: Record<string, any> | null;
    tools: Array<{ name: string; params: Record<string, any>; returns: any; description: string }>;
    workflows: Array<{ flowName: string; flowParams: Record<string, any>; returns: any; body: any[] }>;
    helpers: HelperType[];
    modelConfig?: Array<{
        defaultConfig?: { model: { type: string; modelName: string; url?: string }; prompt: any };
        namedConfig?: Array<{ configName: string; model: { type: string; modelName: string; url?: string }; prompt: any }>;
    }>;
    lifecycle?: {
        enabled: true;
        maxTokens?: number;
        maxMessages?: number;
    };
    types?: Record<string, {
        isOutput: boolean;
        properties: Record<string, {
            type: any;
            optional: boolean;
            description?: string;
        }>;
    }>;
}

/**
 * Main entry point: generates the complete .agent.types.ts file
 */
export function generateTypesFile(agent: AgentIR): string {
    const hasTools = agent.tools && agent.tools.length > 0;
    const hasContext = agent.context && Object.keys(agent.context).length > 0;
    const hasLifecycle = agent.lifecycle?.enabled === true;
    const requiredProviders = collectRequiredProviders(agent);

    // Collect helpers that are transferred to (their output becomes part of agent output)
    const transferredHelpers = collectTransferredHelpers(agent);

    const sections = [
        `// Auto-generated types for ${agent.name}`,
        `// Do not edit manually`,
        ``,
        `// Core Runtime Imports`,
        `import { Agent, RunConfig } from "../javascript/loader/IrInterpreter";`,
        requiredProviders.has("gemini") ? `import { GoogleDriver } from "../javascript/loader/drivers/GoogleDriver";` : '',
        requiredProviders.has("openai") || requiredProviders.has("custom") ? `import { OpenAIDriver } from "../javascript/loader/drivers/OpenAIDriver";` : '',
        `import type { AgentIR } from "../javascript/loader/types/ir";`,
        `import type { SyntheticMessage, ConversationState, LifecycleHooks } from "../javascript/loader/types/protocol";`,
        ``,
        // Generate custom type interfaces
        agent.types ? generateCustomTypes(agent.types) : '',
        generateInputInterface(agent),
        // Generate output interfaces for transferred helpers
        ...transferredHelpers.map(helper => generateHelperOutputInterface(helper)),
        generateOutputInterface(agent, transferredHelpers),
        generateContextInterface(agent),
        hasTools ? generateToolsInterface(agent) : '',
        hasLifecycle ? generateLifecycleInterface(agent, hasContext ?? false) : '',
        requiredProviders.size > 0 ? generateApiKeysInterface(agent, requiredProviders) : '',
        generateAgentFactory(agent, hasTools, hasContext ?? false, hasLifecycle, requiredProviders, transferredHelpers),
    ];

    return sections.filter(Boolean).join('\n');
}

/**
 * Recursively scan workflow bodies for transfer statements
 * Returns array of helper names that are transferred to
 */
function collectTransferredHelpers(agent: AgentIR): HelperType[] {
    const transferredNames = new Set<string>();

    for (const workflow of (agent.workflows || [])) {
        scanForTransfers(workflow.body || [], transferredNames);
    }

    // Map names to actual helper definitions
    return (agent.helpers || []).filter(h => transferredNames.has(h.name));
}

/**
 * Recursively scan statements for transfer statements
 */
function scanForTransfers(statements: any[], found: Set<string>): void {
    for (const stmt of statements) {
        if (stmt.type === 'transfer' && stmt.target?.value) {
            found.add(stmt.target.value);
        }
        // Recurse into if statements
        if (stmt.type === 'if') {
            if (stmt.then) scanForTransfers(stmt.then, found);
            if (stmt.else) scanForTransfers(stmt.else, found);
        }
    }
}

/**
 * Generate TypeScript interfaces for custom type definitions
 */
function generateCustomTypes(types: Record<string, any>): string {
    const interfaces = Object.entries(types).map(([typeName, typeDef]) => {
        const props = Object.entries(typeDef.properties)
            .map(([propName, propInfo]: [string, any]) => {
                const optional = propInfo.optional ? '?' : '';
                const comment = propInfo.description ? `\n    /** ${propInfo.description} */` : '';
                return `${comment}\n    ${propName}${optional}: ${typeToTsString(propInfo.type)};`;
            })
            .join('\n');

        const comment = typeDef.isOutput ? '\n/** Output type */\n' : '\n';
        return `${comment}export interface ${typeName} {
${props}
}
`;
    });

    return interfaces.join('\n');
}

/**
 * Collect all required provider types from agent model config
 */
function collectRequiredProviders(agent: AgentIR): Set<string> {
    const providers = new Set<string>();

    if (agent.modelConfig && agent.modelConfig.length > 0) {
        const config = agent.modelConfig[0];

        if (config.defaultConfig?.model) {
            providers.add(config.defaultConfig.model.type);
        }

        if (config.namedConfig) {
            for (const named of config.namedConfig) {
                if (named.model) {
                    providers.add(named.model.type);
                }
            }
        }
    }

    return providers;
}

/**
 * Generate ApiKeys interface for required providers
 */
function generateApiKeysInterface(agent: AgentIR, providers: Set<string>): string {
    const keys: string[] = [];

    if (providers.has("gemini")) {
        keys.push("    geminiApiKey: string;");
    }
    if (providers.has("openai")) {
        keys.push("    openaiApiKey: string;");
    }
    if (providers.has("custom")) {
        keys.push("    customApiKey: string;");
        keys.push("    customUrl?: string;  // Optional override for custom provider URL");
    }

    return `/**
 * API keys required for ${agent.name}
 */
export interface ${agent.name}ApiKeys {
${keys.join('\n')}
}
`;
}

/**
 * Generate Input interface
 */
function generateInputInterface(agent: AgentIR): string {
    const props = agent.input
        ? Object.entries(agent.input)
            .map(([name, val]) => {
                const optional = val?.optional ? '?' : '';
                return `    ${name}${optional}: ${typeToTsString(val)};`;
            })
            .join('\n')
        : '';

    return `export interface ${agent.name}Input {
${props}
}
`;
}

/**
 * Generate Helper Output interface (for transferred helpers)
 */
function generateHelperOutputInterface(helper: HelperType): string {
    const props = helper.output
        ? Object.entries(helper.output)
            .map(([name, val]) => {
                const optional = val?.optional ? '?' : '';
                return `    ${name}${optional}: ${typeToTsString(val)};`;
            })
            .join('\n')
        : '';

    return `export interface ${helper.name}Output {
${props}
}
`;
}

/**
 * Generate Output interface (with union types for transfers)
 */
function generateOutputInterface(agent: AgentIR, transferredHelpers: HelperType[]): string {
    const props = agent.output
        ? Object.entries(agent.output)
            .map(([name, val]) => {
                const optional = val?.optional ? '?' : '';
                return `    ${name}${optional}: ${typeToTsString(val)};`;
            })
            .join('\n')
        : '';

    // Base output interface
    const baseInterface = `export interface ${agent.name}BaseOutput {
${props}
}
`;

    // If no transfers, just use the base interface with the normal name
    if (transferredHelpers.length === 0) {
        return `export interface ${agent.name}Output {
${props}
}
`;
    }

    // With transfers: generate union type
    const unionMembers = [
        `${agent.name}BaseOutput`,
        ...transferredHelpers.map(h => `${h.name}Output`)
    ].join(' | ');

    return `${baseInterface}
/** Union of possible output types (includes transfer destinations) */
export type ${agent.name}Output = ${unionMembers};
`;
}

/**
 * Generate Context interface
 */
function generateContextInterface(agent: AgentIR): string {
    const props = agent.context
        ? Object.entries(agent.context)
            .map(([name, val]) => {
                const optional = val?.optional ? '?' : '';
                return `    ${name}${optional}: ${typeToTsString(val)};`;
            })
            .join('\n')
        : '';

    return `export interface ${agent.name}Context {
${props}
}
`;
}

/**
 * Generate Tools interface
 */
function generateToolsInterface(agent: AgentIR): string {
    if (!agent.tools || agent.tools.length === 0) {
        return '';
    }

    const toolMethods = agent.tools.map(tool => {
        const paramType = Object.entries(tool.params)
            .map(([name, typeObj]: [string, any]) => {
                const optional = typeObj?.optional ? '?' : '';
                return `${name}${optional}: ${typeToTsString(typeObj)}`;
            })
            .join(', ');

        return `    ${tool.name}: (args: { ${paramType} }) => Promise<${typeToTsString(tool.returns)}>;`;
    }).join('\n');

    return `export interface ${agent.name}Tools {
    [key: string]: (args: any) => Promise<any>;
${toolMethods}
}
`;
}

/**
 * Generate Lifecycle interface for agents with lifecycle enabled
 */
function generateLifecycleInterface(agent: AgentIR, hasContext: boolean): string {
    const contextType = hasContext ? `${agent.name}Context` : 'Record<string, any>';

    return `/**
 * Lifecycle hooks for ${agent.name}
 * Implement these to manage conversation history and memory
 */
export interface ${agent.name}Lifecycle {
    prune: (args: {
        context: ${contextType};
        agent: any;
        usage: {
            currentTokens: number;
            maxTokens: number;
            currentMessages: number;
            maxMessages: number;
        };
    }) => Promise<ConversationState>;
    
    load: (args: {
        context: ${contextType};
    }) => Promise<ConversationState>;
    
    save: (args: {
        newMessages: SyntheticMessage[];
        context: ${contextType};
        output: ${agent.name}Output;
    }) => Promise<void>;
}
`;
}

/**
 * Generate factory function with unified configuration pattern
 */
function generateAgentFactory(agent: AgentIR, hasTools: boolean, hasContext: boolean, hasLifecycle: boolean, requiredProviders: Set<string>, transferredHelpers: HelperType[]): string {
    // Extract named config names for type-safe configName
    const namedConfigs = agent.modelConfig?.[0]?.namedConfig ?? [];
    const configNames = namedConfigs
        .map((c: any) => c.configName)
        .filter((name: string | undefined) => name);

    const configNameType = configNames.length > 0
        ? configNames.map((n: string) => `"${n}"`).join(' | ')
        : 'never';

    // Type parameters for Agent generic
    const typeParams = [
        `${agent.name}Input`,
        `${agent.name}Output`,
        hasContext ? `${agent.name}Context` : 'Record<string, never>',
        hasTools ? `${agent.name}Tools` : 'Record<string, never>'
    ].join(', ');

    // Generate drivers object
    const driverEntries: string[] = [];
    if (requiredProviders.has("gemini")) {
        driverEntries.push(`        gemini: new GoogleDriver(config.apiKeys.geminiApiKey)`);
    }
    if (requiredProviders.has("openai")) {
        driverEntries.push(`        openai: new OpenAIDriver(config.apiKeys.openaiApiKey)`);
    }
    if (requiredProviders.has("custom")) {
        driverEntries.push(`        custom: new OpenAIDriver(config.apiKeys.customApiKey, config.apiKeys.customUrl ?? "https://api.openai.com/v1")`);
    }

    const hasApiKeys = requiredProviders.size > 0;
    const driversObject = driverEntries.length > 0
        ? `{\n${driverEntries.join(',\n')}\n    }`
        : '{}';

    // Build config interface properties
    const configProps: string[] = [];
    if (hasApiKeys) {
        configProps.push(`    apiKeys: ${agent.name}ApiKeys;`);
    }
    configProps.push(`    ir: AgentIR;`);
    if (hasContext) {
        configProps.push(`    context?: ${agent.name}Context;`);
    }
    if (hasTools) {
        configProps.push(`    tools?: ${agent.name}Tools;`);
    }
    if (hasLifecycle) {
        configProps.push(`    lifecycle?: ${agent.name}Lifecycle;`);
    }

    // Build validation checks
    const validationChecks: string[] = [];
    
    if (hasTools) {
        validationChecks.push(`
    // Validate tools match IR requirements
    if (config.ir.tools && config.ir.tools.length > 0) {
        for (const toolDef of config.ir.tools) {
            if (!config.tools?.[toolDef.name]) {
                throw new Error(
                    \`Missing required tool: \${toolDef.name}\\n\` +
                    \`Expected in tools configuration\`
                );
            }
        }
    }`);
    }

    if (hasLifecycle) {
        validationChecks.push(`
    // Validate lifecycle hooks if required
    if (config.ir.lifecycle?.enabled && !config.lifecycle) {
        throw new Error(
            \`Agent "\${config.ir.name}" requires lifecycle hooks.\\n\` +
            \`Provide: { prune, load, save }\`
        );
    }`);
    }

    // Build run parameters (just input + optional overrides)
    const runInputParam = `input: ${agent.name}Input`;
    const runOverrideProps: string[] = [];
    if (hasContext) runOverrideProps.push(`context?: ${agent.name}Context`);
    if (hasTools) runOverrideProps.push(`tools?: ${agent.name}Tools`);
    if (hasLifecycle) runOverrideProps.push(`lifecycle?: ${agent.name}Lifecycle`);
    runOverrideProps.push(`modelOverride?: { providerType?: string; modelName?: string; temperature?: number }`);
    runOverrideProps.push(`configName?: ${configNameType}`);
    
    const runOverrideParam = runOverrideProps.length > 0 
        ? `, overrides?: { ${runOverrideProps.join('; ')} }`
        : '';

    // Build config merge for run call
    const configMergeParts: string[] = [];
    if (hasTools) configMergeParts.push('tools: overrides?.tools ?? config.tools');
    if (hasContext) configMergeParts.push('context: overrides?.context ?? config.context');
    if (hasLifecycle) configMergeParts.push('lifecycle: overrides?.lifecycle ?? config.lifecycle');
    configMergeParts.push('modelOverride: overrides?.modelOverride');
    configMergeParts.push('configName: overrides?.configName');
    
    const configMerge = `{ ${configMergeParts.join(', ')} }`;

    return `
/**
 * Configuration for ${agent.name} agent
 */
export interface ${agent.name}Config {
${configProps.join('\n')}
}

/**
 * Create a type-safe ${agent.name} agent instance
 * 
 * @example
 * \`\`\`typescript
 * const agent = create${agent.name}({
 *     apiKeys: { geminiApiKey: '...' },
 *     ir: agentIR,${hasContext ? '\n *     context: { sessionId: "123" },' : ''}${hasTools ? '\n *     tools: { ... },' : ''}${hasLifecycle ? '\n *     lifecycle: { prune, load, save }' : ''}
 * });
 * 
 * // Clean execution - config bound at creation
 * const result = await agent.run({ ... });
 * const stream = await agent.stream({ ... });
 * \`\`\`
 */
export function create${agent.name}(config: ${agent.name}Config) {
    // Create agent with drivers
    const agent = new Agent<${typeParams}>(${driversObject});
    
    // Load and validate IR immediately
    agent.load(config.ir);
${validationChecks.join('\n')}
    
    return {
        /**
         * Run the agent with type-safe parameters
         * @param input - Agent input
         * @param overrides - Optional overrides for context, tools, lifecycle, or configName
         */
        run: (${runInputParam}${runOverrideParam}): Promise<${agent.name}Output> => 
            agent.run(input, ${configMerge}),
        
        /**
         * Fluent streaming API with callbacks
         * @param input - Agent input
         * @param overrides - Optional overrides for context, tools, lifecycle, or configName
         * 
         * @example
         * \`\`\`typescript
         * const result = await agent
         *   .stream({ request: "..." })
         *   .onChunk(delta => console.log(delta))
         *   .onToolResult((name, result) => console.log(name, result))
         *   .run();
         * \`\`\`
         */
        stream: (${runInputParam}${runOverrideParam}) => 
            agent.stream(input, ${configMerge}),
        
        /**
         * Native async iteration over stream chunks
         * @param input - Agent input
         * @param overrides - Optional overrides for context, tools, lifecycle, or configName
         * 
         * @example
         * \`\`\`typescript
         * for await (const chunk of agent.streamIterable({ request: "..." })) {
         *     if (chunk.type === 'text') console.log(chunk.delta);
         * }
         * \`\`\`
         */
        streamIterable: (${runInputParam}${runOverrideParam}) => 
            agent.runStream(input, ${configMerge}),${hasContext ? `
        
        /**
         * Create a new agent instance with bound context
         * Useful for multi-turn conversations with the same session
         * 
         * @example
         * \`\`\`typescript
         * const sessionAgent = agent.forContext({ sessionId: '123' });
         * await sessionAgent.run({ message: "First" });
         * await sessionAgent.run({ message: "Second" });
         * \`\`\`
         */
        forContext: (context: ${agent.name}Context) => {
            const boundContext = context;
            return {
                run: (${runInputParam}, overrides?: { configName?: ${configNameType}; modelOverride?: { providerType?: string; modelName?: string; temperature?: number } }) => 
                    agent.run(input, { context: boundContext, configName: overrides?.configName, modelOverride: overrides?.modelOverride }),
                stream: (${runInputParam}, overrides?: { configName?: ${configNameType}; modelOverride?: { providerType?: string; modelName?: string; temperature?: number } }) => 
                    agent.stream(input, { context: boundContext, configName: overrides?.configName, modelOverride: overrides?.modelOverride }),
                streamIterable: (${runInputParam}, overrides?: { configName?: ${configNameType}; modelOverride?: { providerType?: string; modelName?: string; temperature?: number } }) => 
                    agent.runStream(input, { context: boundContext, configName: overrides?.configName, modelOverride: overrides?.modelOverride })
            };
        }` : ''}
    };
}

/** Type for the created agent instance */
export type ${agent.name}Agent = ReturnType<typeof create${agent.name}>;
`;
}

/**
 * Convert IR type value to TypeScript type string
 */
function typeToTsString(typeVal: any): string {
    if (typeof typeVal === 'string') {
        return normalizeType(typeVal);
    }

    // Handle type reference: { type: "typeRef", name: "Point" }
    if (typeVal?.type === 'typeRef' && typeVal.name) {
        return typeVal.name;
    }

    // Handle array type: { type: "array", items: {...} }
    if (typeVal?.type === 'array' && typeVal.items) {
        return `${typeToTsString(typeVal.items)}[]`;
    }

    // Handle union type: { type: "union", options: [...] }
    if (typeVal?.type === 'union' && Array.isArray(typeVal.options)) {
        return typeVal.options
            .map((o: string) => `"${o.replace(/^["']|["']$/g, '')}"`)
            .join(' | ');
    }

    // Handle object type: { type: "object", properties: {...} }
    if (typeVal?.type === 'object' && typeVal.properties) {
        const props = Object.entries(typeVal.properties)
            .map(([key, val]) => `${key}: ${typeToTsString(val)}`)
            .join('; ');
        return `{ ${props} }`;
    }

    // Handle array type (legacy string format)
    if (typeof typeVal === 'string' && typeVal.endsWith('[]')) {
        const inner = typeVal.slice(0, -2);
        return `${normalizeType(inner)}[]`;
    }

    // Handle nested type wrapper: { type: {...}, optional: ... }
    if (typeVal && typeof typeVal.type === 'object') {
        return typeToTsString(typeVal.type);
    }

    // Handle simple wrapper: { type: "string", optional: ... }
    if (typeVal && typeof typeVal.type === 'string') {
        return normalizeType(typeVal.type);
    }

    return 'unknown';
}

/**
 * Normalize type names to TypeScript equivalents
 */
function normalizeType(t: string): string {
    switch (t.toLowerCase()) {
        case 'int':
        case 'float':
        case 'number':
            return 'number';
        case 'bool':
        case 'boolean':
            return 'boolean';
        case 'string':
            return 'string';
        default:
            return t;
    }
}
