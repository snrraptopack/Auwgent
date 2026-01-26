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
        defaultConfig?: { modelName: string; prompt: any };
        namedConfig?: Array<{ configName: string; modelName: string; prompt: any }>;
    }>;
}

/**
 * Main entry point: generates the complete .agent.types.ts file
 */
export function generateTypesFile(agent: AgentIR): string {
    const hasTools = agent.tools && agent.tools.length > 0;
    const hasContext = agent.context && Object.keys(agent.context).length > 0;

    // Collect helpers that are transferred to (their output becomes part of agent output)
    const transferredHelpers = collectTransferredHelpers(agent);

    const sections = [
        `// Auto-generated types for ${agent.name}`,
        `// Do not edit manually`,
        ``,
        `// Core Runtime Imports`,
        `import { Agent } from "../javascript/loader/IrInterpreter";`,
        `import { DriverRegistry } from "../javascript/loader/DriverRegistry";`,
        `import type { AgentIR } from "../javascript/loader/types/ir";`,
        ``,
        generateInputInterface(agent),
        // Generate output interfaces for transferred helpers
        ...transferredHelpers.map(helper => generateHelperOutputInterface(helper)),
        generateOutputInterface(agent, transferredHelpers),
        generateContextInterface(agent),
        hasTools ? generateToolsInterface(agent) : '',
        generateAgentFactory(agent, hasTools, hasContext ?? false, transferredHelpers),
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
 * Generate factory function with conditional parameters
 */
function generateAgentFactory(agent: AgentIR, hasTools: boolean, hasContext: boolean, transferredHelpers: HelperType[]): string {
    // Build parameter list for user-facing API
    const runParams: string[] = [`input: ${agent.name}Input`];

    if (hasTools) {
        runParams.push(`tools: ${agent.name}Tools`);
    }

    if (hasContext) {
        runParams.push(`context: ${agent.name}Context`);
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
    configParts.push('configName');
    const configObject = `{ ${configParts.join(', ')} }`;

    // Type parameters for Agent generic
    const typeParams = [
        `${agent.name}Input`,
        `${agent.name}Output`,
        hasContext ? `${agent.name}Context` : 'Record<string, never>',
        hasTools ? `${agent.name}Tools` : 'Record<string, never>'
    ].join(', ');

    return `
/**
 * Create a type-safe ${agent.name} agent instance
 */
export function create${agent.name}(registry: DriverRegistry) {
    const agent = new Agent<${typeParams}>(registry);
    
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
