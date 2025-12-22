/**
 * TypeScript Types Generator
 * Generates type-safe interfaces and factory functions from Agent IR
 */

interface AgentIR {
    name: string;
    input: Record<string, any> | null;
    output: Record<string, any> | null;
    context: Record<string, any> | null;
    tools: Array<{ name: string; params: Record<string, any>; returns: any; description: string }>;
    workflows: Array<{ flowName: string; flowParams: Record<string, any>; returns: any }>;
}

/**
 * Main entry point: generates the complete .agent.types.ts file
 */
export function generateTypesFile(agent: AgentIR): string {
    const hasTools = agent.tools && agent.tools.length > 0;
    const hasContext = agent.context && Object.keys(agent.context).length > 0;

    const sections = [
        `// Auto-generated types for ${agent.name}`,
        `// Do not edit manually`,
        ``,
        `// To use the factory function, import Agent and AgentDriver from your loader:`,
        `// import { Agent } from "../javascript/loader/IrInterpreter";`,
        `// import type { AgentDriver } from "../javascript/loader/types/protocol";`,
        `// import type { AgentIR } from "../javascript/loader/types/ir";`,
        ``,
        generateInputInterface(agent),
        generateOutputInterface(agent),
        generateContextInterface(agent),
        hasTools ? generateToolsInterface(agent) : '',
        generateAgentFactory(agent, hasTools, hasContext ?? false),
    ];

    return sections.filter(Boolean).join('\n');
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
 * Generate Output interface
 */
function generateOutputInterface(agent: AgentIR): string {
    const props = agent.output
        ? Object.entries(agent.output)
            .map(([name, val]) => {
                const optional = val?.optional ? '?' : '';
                return `    ${name}${optional}: ${typeToTsString(val)};`;
            })
            .join('\n')
        : '';

    return `export interface ${agent.name}Output {
${props}
}
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
function generateAgentFactory(agent: AgentIR, hasTools: boolean, hasContext: boolean): string {
    // Build parameter list based on what's defined
    const runParams: string[] = [`input: ${agent.name}Input`];
    const runArgs: string[] = ['input'];

    if (hasTools) {
        runParams.push(`tools: ${agent.name}Tools`);
        runArgs.push('tools');
    } else {
        runArgs.push('undefined');
    }

    if (hasContext) {
        runParams.push(`context: ${agent.name}Context`);
        runArgs.push('context');
    } else {
        runArgs.push('{}');
    }

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
export function create${agent.name}(driver: AgentDriver) {
    const agent = new Agent<${typeParams}>(driver);
    
    return {
        /**
         * Load the agent IR configuration
         */
        load: (ir: AgentIR) => agent.load(ir),
        
        /**
         * Run the agent with type-safe parameters
         */
        run: (${runParams.join(', ')}): Promise<${agent.name}Output> => 
            agent.run(${runArgs.join(', ')})
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
