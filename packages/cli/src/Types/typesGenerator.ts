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
 * Generate factory function with conditional parameters
 */
function generateAgentFactory(agent: AgentIR, hasTools: boolean, hasContext: boolean, hasLifecycle: boolean, requiredProviders: Set<string>, transferredHelpers: HelperType[]): string {
    // Build parameter list for user-facing API
    const runParams: string[] = [`input: ${agent.name}Input`];

    if (hasTools) {
        runParams.push(`tools: ${agent.name}Tools`);
    }

    if (hasContext) {
        runParams.push(`context: ${agent.name}Context`);
    }

    if (hasLifecycle) {
        runParams.push(`lifecycle: ${agent.name}Lifecycle`);
    }

    // Extract named config names for type-safe configName
    const namedConfigs = agent.modelConfig?.[0]?.namedConfig ?? [];
    const configNames = namedConfigs
        .map((c: any) => c.configName)
        .filter((name: string | undefined) => name);

    const configNameType = configNames.length > 0
        ? configNames.map((n: string) => `"${n}"`).join(' | ')
        : 'never';

    runParams.push(`configName?: ${configNameType}`);

    // Build config object construction for run() call
    const configParts: string[] = [];
    if (hasTools) {
        configParts.push('tools');
    }
    if (hasContext) {
        configParts.push('context');
    }
    if (hasLifecycle) {
        configParts.push('lifecycle');
    }
    configParts.push('configName');
    const configObject = `{ ${configParts.join(', ')} }`;

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
        driverEntries.push(`        gemini: new GoogleDriver(apiKeys.geminiApiKey)`);
    }
    if (requiredProviders.has("openai")) {
        driverEntries.push(`        openai: new OpenAIDriver(apiKeys.openaiApiKey)`);
    }
    if (requiredProviders.has("custom")) {
        driverEntries.push(`        custom: new OpenAIDriver(apiKeys.customApiKey, apiKeys.customUrl ?? "https://api.openai.com/v1")`);
    }

    const hasApiKeys = requiredProviders.size > 0;
    const factoryParam = hasApiKeys ? `apiKeys: ${agent.name}ApiKeys` : '';
    const driversObject = driverEntries.length > 0
        ? `{\n${driverEntries.join(',\n')}\n    }`
        : '{}';

    return `
/**
 * Create a type-safe ${agent.name} agent instance${hasApiKeys ? '\n * Auto-creates drivers based on required providers' : ''}
 */
export function create${agent.name}(${factoryParam}) {
    const agent = new Agent<${typeParams}>(${driversObject});
    
    return {
        /**
         * Load the agent IR configuration
         */
        load: (ir: AgentIR) => agent.load(ir),
        
        /**
         * Run the agent with type-safe parameters
         */
        run: (${runParams.join(', ')}): Promise<${agent.name}Output> => 
            agent.run(input, ${configObject}),
        
        /**
         * Fluent streaming API with callbacks
         * @example
         * const result = await agent
         *   .stream({ request: "..." })
         *   .onText(delta => console.log(delta))
         *   .run();
         */
        stream: (${runParams.join(', ')}) => 
            agent.stream(input, ${configObject})
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

    // Handle array type
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
