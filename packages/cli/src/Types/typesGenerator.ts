/**
 * TypeScript Types Generator
 * Generates type-safe interfaces and factory functions from Agent IR
 */

interface HelperType {
    name: string;
    output: Record<string, any> | null;
    modelConfig?: Array<{
        defaultConfig?: { model: { type: string; modelName: string; url?: string; config?: any }; prompt: any };
        namedConfig?: Array<{ configName: string; model: { type: string; modelName: string; url?: string; config?: any }; prompt: any }>;
    }>;
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
    tests?: Array<{ name: string }>;
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

    // Generate literal types for workflows & helpers so auwgent.ts can infer exact string intents from the IR
    const workflowTypes = agent.workflows?.length
        ? agent.workflows.map(w => `{ flowName: "${w.flowName}"; returns: ${typeToTsString(w.returns)} }`).join(' | ')
        : 'undefined';

    const helperTypes = agent.helpers?.length
        ? agent.helpers.map(h => `{ name: "${h.name}" }`).join(' | ')
        : 'undefined';

    const fileName = baseName || agent.name;
    const irImportStatement = `import _importedIR from './${fileName}.agent.json' with { type: 'json' };\n` +
        `type ${agent.name}IR = Omit<typeof _importedIR, "workflows" | "helpers"> & {\n` +
        `  workflows: ${workflowTypes === 'undefined' ? 'undefined' : `(${workflowTypes})[]`};\n` +
        `  helpers: ${helperTypes === 'undefined' ? 'undefined' : `(${helperTypes})[]`};\n` +
        `};\n` +
        `const agentIR = _importedIR as unknown as ${agent.name}IR;`;

    const sections = [
        `// Auto-generated types for ${agent.name}`,
        `// Do not edit manually`,
        ``,
        `// Core Runtime Imports`,
        `import { createAuwgent } from "@auwgent/runtime";`,
        `import type { ToolRegistry } from "@auwgent/runtime";`,
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
        generateCustomIntents(agent),
        requiredProviders.size > 0 ? generateApiKeysInterface(agent, requiredProviders) : '',
        generateAgentFactory(agent, hasTools, hasContext ?? false, requiredProviders),
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
function collectProvidersFromModelConfig(modelConfig?: AgentIR["modelConfig"]): Set<string> {
    const providers = new Set<string>();
    if (!modelConfig) {
        return providers;
    }

    for (const config of modelConfig) {
        if (config.defaultConfig?.model?.type) {
            providers.add(config.defaultConfig.model.type);
        }
        if (config.namedConfig) {
            for (const named of config.namedConfig) {
                if (named.model?.type) {
                    providers.add(named.model.type);
                }
            }
        }
    }

    return providers;
}

function collectRequiredProviders(agent: AgentIR): Set<string> {
    const providers = collectProvidersFromModelConfig(agent.modelConfig);

    for (const helper of agent.helpers || []) {
        const helperProviders = collectProvidersFromModelConfig(helper.modelConfig);
        for (const provider of helperProviders) {
            providers.add(provider);
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
 * Generate CustomIntents union (presently placeholder for future DSL intents)
 */
function generateCustomIntents(agent: AgentIR): string {
    return `/** Custom intents defined in the DSL (if any) */
export type ${agent.name}CustomIntents = never;
`;
}

/**
 * Generate factory function with unified configuration pattern
 */
function generateAgentFactory(agent: AgentIR, hasTools: boolean, hasContext: boolean, requiredProviders: Set<string>): string {
    const toolsType = hasTools ? `${agent.name}Tools` : 'Record<string, never>';
    const hasApiKeys = requiredProviders.size > 0;

    const configProps: string[] = [];
    configProps.push(`    tools: ${toolsType};`);
    if (hasContext) {
        configProps.push(`    context: ${agent.name}Context;`);
    }
    if (hasApiKeys) {
        configProps.push(`    apiKeys: ${agent.name}ApiKeys;`);
    }

    return `
export type ${agent.name}Config = {
${configProps.join('\n')}
}

export function create${agent.name}(config: ${agent.name}Config) {
    return createAuwgent<
        typeof agentIR,
        ${agent.name}CustomIntents,
        ${agent.output ? `${agent.name}Output` : 'never'},
        ${agent.name}Tools
    >(agentIR, {
        tools: config.tools,
        ${hasContext ? 'context: config.context,' : ''}
        ${hasApiKeys ? 'apiKeys: config.apiKeys' : ''}
    });
}

export type ${agent.name}Agent = ReturnType<typeof create${agent.name}>;
export const auwgent = create${agent.name};
export type AuwgentTools = ${toolsType};
export type AuwgentConfig = ${agent.name}Config;
export type AuwgentAgent = ${agent.name}Agent;
export type AuwgentContext = ${agent.name}Context;
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
