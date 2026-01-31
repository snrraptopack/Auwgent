/**
 * TypeScript Types Generator
 * Generates type-safe interfaces and factory functions from Agent IR
 */

interface HelperType {
    name: string;
    output: Record<string, any> | null;
}

interface ToolDef {
    name: string;
    params: Record<string, any>;
    returns: any;
    description: string;
}

interface AgentIR {
    name: string;
    input: Record<string, any> | null;
    output: Record<string, any> | null;
    context: Record<string, any> | null;
    tools: ToolDef[];
    workflows: Array<{ flowName: string; flowParams: Record<string, any>; returns: any; body: any[]; tools?: ToolDef[] }>;
    helpers: HelperType[];
    helperHandoff?: Record<string, "user" | "thenContinue">;
    modelConfig?: Array<{
        defaultConfig?: { model: { type: string; modelName: string; url?: string; config?: any }; prompt: any };
        namedConfig?: Array<{ configName: string; model: { type: string; modelName: string; url?: string; config?: any }; prompt: any }>;
    }>;
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
export function generateTypesFile(agent: AgentIR, baseName?: string): string {
    const workflowTools = collectWorkflowTools(agent);
    const allTools = mergeToolDefs(agent.tools ?? [], workflowTools);
    const hasTools = allTools.length > 0;
    const hasContext = agent.context && Object.keys(agent.context).length > 0;
    const requiredProviders = collectRequiredProviders(agent);

    // Collect helpers that are transferred to (their output becomes part of agent output)
    const transferredHelpers = collectTransferredHelpers(agent);
    const handoffHelpers = collectHandoffHelpers(agent);
    const outputHelpers = mergeHelpers(transferredHelpers, handoffHelpers);

    // Generate IR import statement using baseName if provided, otherwise fall back to agent.name
    const fileName = baseName || agent.name;
    const irImportStatement = `import _importedIR from './${fileName}.agent.json' with { type: 'json' };\nconst agentIR = _importedIR as unknown as AgentIR;`;

    const sections = [
        `// Auto-generated types for ${agent.name}`,
        `// Do not edit manually`,
        ``,
        `// Core Runtime Imports`,
        `import { Agent } from "../javascript/loader/IrInterpreter";`,
        requiredProviders.has("gemini") ? `import { GoogleDriver } from "../javascript/loader/drivers/GoogleDriver";` : '',
        requiredProviders.has("openai") || requiredProviders.has("custom") ? `import { OpenAIDriver } from "../javascript/loader/drivers/OpenAIDriver";` : '',
        `import type { AgentIR } from "../javascript/loader/types/ir";`,
        `import type { AgentMiddleware } from "../javascript/loader/types/protocol";`,
        ``,
        irImportStatement,
        ``,
        // Generate custom type interfaces
        agent.types ? generateCustomTypes(agent.types) : '',
        generateInputInterface(agent),
        // Generate output interfaces for transferred helpers
        ...outputHelpers.map(helper => generateHelperOutputInterface(helper)),
        generateOutputInterface(agent, outputHelpers),
        generateContextInterface(agent),
        hasTools ? generateToolsInterface(agent.name, allTools) : '',
        requiredProviders.size > 0 ? generateApiKeysInterface(agent, requiredProviders) : '',
        generateAgentFactory(agent, hasTools, hasContext ?? false, requiredProviders, outputHelpers),
    ];

    return sections.filter(Boolean).join('\n');
}

function collectWorkflowTools(agent: AgentIR): ToolDef[] {
    const toolDefs: ToolDef[] = [];
    for (const workflow of agent.workflows || []) {
        if (workflow.tools && workflow.tools.length > 0) {
            toolDefs.push(...workflow.tools);
        }
    }
    return toolDefs;
}

function mergeToolDefs(base: ToolDef[], extra: ToolDef[]): ToolDef[] {
    const toolMap = new Map<string, ToolDef>();
    for (const tool of base) {
        toolMap.set(tool.name, tool);
    }
    for (const tool of extra) {
        toolMap.set(tool.name, tool);
    }
    return Array.from(toolMap.values());
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

function collectHandoffHelpers(agent: AgentIR): HelperType[] {
    const handoff = agent.helperHandoff || {};
    const handoffNames = new Set<string>(Object.keys(handoff));
    return (agent.helpers || []).filter(h => handoffNames.has(h.name));
}

function mergeHelpers(a: HelperType[], b: HelperType[]): HelperType[] {
    const map = new Map<string, HelperType>();
    for (const helper of a) {
        map.set(helper.name, helper);
    }
    for (const helper of b) {
        map.set(helper.name, helper);
    }
    return Array.from(map.values());
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
        return `${comment}export type ${typeName} = {
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
export type ${agent.name}ApiKeys = {
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

    return `export type ${agent.name}Input = {
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

    return `export type ${helper.name}Output = {
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
    const baseInterface = `export type ${agent.name}BaseOutput = {
${props}
}
`;

    // If no transfers, just use the base interface with the normal name
    if (transferredHelpers.length === 0) {
        return `export type ${agent.name}Output = {
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

    return `export type ${agent.name}Context = {
${props}
}
`;
}

/**
 * Generate Tools interface
 */
function generateToolsInterface(agentName: string, tools: ToolDef[]): string {
    if (!tools || tools.length === 0) {
        return '';
    }

    const toolMethods = tools.map(tool => {
        const paramType = Object.entries(tool.params)
            .map(([name, typeObj]: [string, any]) => {
                const optional = typeObj?.optional ? '?' : '';
                return `${name}${optional}: ${typeToTsString(typeObj)}`;
            })
            .join(', ');

        return `    ${tool.name}: (args: { ${paramType} }) => Promise<${typeToTsString(tool.returns)}>;`;
    }).join('\n');

    return `export type ${agentName}Tools = {
${toolMethods}
}
`;
}

/**
 * Generate factory function with unified configuration pattern
 */
function generateAgentFactory(agent: AgentIR, hasTools: boolean, hasContext: boolean, requiredProviders: Set<string>, transferredHelpers: HelperType[]): string {
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
    // IR is now imported automatically, no need for user to provide it
    if (hasContext) {
        configProps.push(`    context: ${agent.name}Context;`);
    }
    if (hasTools) {
        configProps.push(`    tools: ${agent.name}Tools;`);
    }
    configProps.push(`    middleware?: AgentMiddleware<${agent.name}Input, ${agent.name}Context, any>[];`);
    configProps.push(`    middlewareState?: Record<string, any>;`);
    configProps.push(`    runId?: string;`);
    // Build validation checks
    const validationChecks: string[] = [];
    
    if (hasTools) {
        validationChecks.push(`
    // Validate tools against IR
    const toolMap = new Map<string, any>();
    if (agentIR.tools && agentIR.tools.length > 0) {
        for (const toolDef of agentIR.tools) {
            toolMap.set(toolDef.name, toolDef);
        }
    }
    if (agentIR.workflows && agentIR.workflows.length > 0) {
        for (const workflow of agentIR.workflows) {
            if (workflow.tools && workflow.tools.length > 0) {
                for (const toolDef of workflow.tools) {
                    toolMap.set(toolDef.name, toolDef);
                }
            }
        }
    }
    const toolsConfig = config.tools as Record<string, any>;
    for (const toolDef of toolMap.values()) {
        if (!toolsConfig[toolDef.name]) {
            throw new Error(
                \`Missing required tool: \${toolDef.name}\\n\` +
                \`Expected in tools configuration\`
            );
        }
    }`);
    }

    // Build run parameters (just input + optional overrides)
    const runInputParam = `input: ${agent.name}Input`;
    const runOverrideProps: string[] = [];
    if (hasContext) runOverrideProps.push(`context?: ${agent.name}Context`);
    if (hasTools) runOverrideProps.push(`tools?: ${agent.name}Tools`);
    runOverrideProps.push(`modelOverride?: { providerType?: string; modelName?: string; temperature?: number }`);
    runOverrideProps.push(`configName?: ${configNameType}`);
    runOverrideProps.push(`middleware?: AgentMiddleware<${agent.name}Input, ${agent.name}Context, any>[]`);
    runOverrideProps.push(`middlewareState?: Record<string, any>`);
    runOverrideProps.push(`runId?: string`);
    
    const runOverrideParam = runOverrideProps.length > 0 
        ? `, overrides?: { ${runOverrideProps.join('; ')} }`
        : '';

    // Build config merge for run call
    const configMergeParts: string[] = [];
    if (hasTools) configMergeParts.push('tools: overrides?.tools ?? config.tools');
    if (hasContext) configMergeParts.push('context: overrides?.context ?? config.context');
    configMergeParts.push('modelOverride: overrides?.modelOverride');
    configMergeParts.push('configName: overrides?.configName');
    configMergeParts.push('middleware: overrides?.middleware ?? config.middleware');
    configMergeParts.push('middlewareState: overrides?.middlewareState ?? config.middlewareState');
    configMergeParts.push('runId: overrides?.runId ?? config.runId');
    
    const configMerge = `{ ${configMergeParts.join(', ')} }`;

    return `
/**
 * Configuration for ${agent.name} agent
 */
export type ${agent.name}Config = {
${configProps.join('\n')}
}

/**
 * Create a type-safe ${agent.name} agent instance
 * 
 * @example
 * \`\`\`typescript
 * const agent = create${agent.name}({
 *     apiKeys: { geminiApiKey: '...' },${hasContext ? '\n *     context: { sessionId: "123" },' : ''}${hasTools ? '\n *     tools: { ... },' : ''}
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
    
    // Load and validate IR from imported file
    agent.load(agentIR);
${validationChecks.join('\n')}
    
    return {
        /**
         * Run the agent with type-safe parameters
         * @param input - Agent input
         * @param overrides - Optional overrides for context, tools, or configName
         */
        run: (${runInputParam}${runOverrideParam}): Promise<${agent.name}Output> => 
            agent.run(input, ${configMerge}),
        
        /**
         * Fluent streaming API with callbacks
         * @param input - Agent input
         * @param overrides - Optional overrides for context, tools, or configName
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
         * @param overrides - Optional overrides for context, tools, or configName
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
                run: (${runInputParam}, overrides?: { configName?: ${configNameType}; modelOverride?: { providerType?: string; modelName?: string; temperature?: number }; middleware?: AgentMiddleware<${agent.name}Input, ${agent.name}Context, any>[]; middlewareState?: Record<string, any>; runId?: string }) => 
                    agent.run(input, { context: boundContext, configName: overrides?.configName, modelOverride: overrides?.modelOverride, middleware: overrides?.middleware ?? config.middleware, middlewareState: overrides?.middlewareState ?? config.middlewareState, runId: overrides?.runId ?? config.runId }),
                stream: (${runInputParam}, overrides?: { configName?: ${configNameType}; modelOverride?: { providerType?: string; modelName?: string; temperature?: number }; middleware?: AgentMiddleware<${agent.name}Input, ${agent.name}Context, any>[]; middlewareState?: Record<string, any>; runId?: string }) => 
                    agent.stream(input, { context: boundContext, configName: overrides?.configName, modelOverride: overrides?.modelOverride, middleware: overrides?.middleware ?? config.middleware, middlewareState: overrides?.middlewareState ?? config.middlewareState, runId: overrides?.runId ?? config.runId }),
                streamIterable: (${runInputParam}, overrides?: { configName?: ${configNameType}; modelOverride?: { providerType?: string; modelName?: string; temperature?: number }; middleware?: AgentMiddleware<${agent.name}Input, ${agent.name}Context, any>[]; middlewareState?: Record<string, any>; runId?: string }) => 
                    agent.runStream(input, { context: boundContext, configName: overrides?.configName, modelOverride: overrides?.modelOverride, middleware: overrides?.middleware ?? config.middleware, middlewareState: overrides?.middlewareState ?? config.middlewareState, runId: overrides?.runId ?? config.runId })
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
