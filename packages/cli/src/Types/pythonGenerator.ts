/**
 * Python Type Stubs (.pyi) Generator
 * Generates type-safe TypedDicts and Protocols from Agent IR
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
 * Main entry point: generates the complete _types.pyi file
 */
export function generatePythonTypesFile(agent: AgentIR, baseName?: string): string {
    const workflowTools = collectWorkflowTools(agent);
    const allTools = mergeToolDefs(agent.tools ?? [], workflowTools);

    // Collect helpers that are transferred to
    const transferredHelpers = collectTransferredHelpers(agent);

    // Collect API keys
    const requiredProviders = collectRequiredProviders(agent);

    // Standard Python typing imports for stubs (with NotRequired fallback)
    const imports = [
        "import os",
        "import json",
        "from typing import TypedDict, Callable, Awaitable, Any, List, Dict, Union, Optional, Protocol",
        "",
        "# NotRequired is 3.11+; fall back to typing_extensions for 3.9/3.10",
        "try:",
        "    from typing import NotRequired",
        "except ImportError:",
        "    from typing_extensions import NotRequired",
        "",
        "try:",
        "    from auwgent import TypedAuwgent, create_auwgent, Middleware, MiddlewareContext, SessionState, AuwgentToolError",
        "except ImportError:",
        "    # For local testing if auwgent is not installed via pip",
        "    import sys",
        "    sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))",
        "    from auwgent import TypedAuwgent, create_auwgent, Middleware, MiddlewareContext, SessionState, AuwgentToolError",
        ""
    ];

    const sections = [
        `# Auto-generated types for ${agent.name}`,
        `# Do not edit manually`,
        ``,
        ...imports,
        // Generate custom type definitions
        agent.types ? generateCustomTypes(agent.types) : '',
        generateInputInterface(agent),
        // Generate output interfaces for transferred helpers
        ...transferredHelpers.map(helper => generateHelperOutputInterface(helper)),
        generateOutputInterface(agent, transferredHelpers),
        generateContextInterface(agent),
        generateToolsProtocol(agent.name, allTools),
        requiredProviders.size > 0 ? generateApiKeysInterface(agent, requiredProviders) : '',
        generateFactoryFunction(agent, requiredProviders, baseName)
    ];

    return sections.filter(Boolean).join('\n');
}

function generateFactoryFunction(agent: AgentIR, providers: Set<string>, baseName?: string): string {
    const configClassName = `${agent.name}Config`;
    const configKeys: string[] = [];

    configKeys.push(`    tools: NotRequired['${agent.name}Tools']`);
    configKeys.push(`    middleware: NotRequired[List['${agent.name}Middleware']]`);
    if (agent.context) {
        configKeys.push(`    context: NotRequired['${agent.name}Context']`);
    }
    if (providers.size > 0) {
        configKeys.push(`    apiKeys: NotRequired['${agent.name}ApiKeys']`);
    }

    const configClass = `class ${configClassName}(TypedDict, total=False):\n${configKeys.join('\n')}\n`;

    // Agent type alias
    const agentType = `${agent.name}Agent = TypedAuwgent\n`;

    // Middleware type alias
    const middlewareType = `${agent.name}Middleware = Middleware\n`;

    // Fall back to agent.name.toLowerCase() if no baseName is provided
    const jsonFileName = baseName ? `${baseName}.agent.json` : `${agent.name.toLowerCase()}.agent.json`;

    const factory = `def create${agent.name}(config: ${configClassName}) -> '${agent.name}Agent':
    """Create a fully configured ${agent.name} agent from config."""
    ir_path = os.path.join(os.path.dirname(__file__), "${jsonFileName}")
    with open(ir_path, "r", encoding="utf-8") as f:
        ir_dict = json.load(f)
    return create_auwgent(ir_dict, config)
`;

    // Convenience aliases (matching TS output)
    const aliases = [
        `auwgent = create${agent.name}`,
        `AuwgentTools = ${agent.name}Tools`,
        `AuwgentConfig = ${agent.name}Config`,
        `AuwgentAgent = ${agent.name}Agent`,
        `AuwgentMiddleware = ${agent.name}Middleware`,
        `AuwgentContext = ${agent.name}Context`,
    ].join('\n');

    return `${agentType}\n${middlewareType}\n${configClass}\n${factory}\n${aliases}\n`;
}

function collectProvidersFromModelConfig(modelConfig?: AgentIR["modelConfig"]): Set<string> {
    const providers = new Set<string>();
    if (!modelConfig) return providers;

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
        const helperProviders = collectProvidersFromModelConfig(helper.modelConfig as any);
        for (const provider of helperProviders) {
            providers.add(provider);
        }
    }

    return providers;
}

function generateApiKeysInterface(agent: AgentIR, providers: Set<string>): string {
    const keys: string[] = [];

    if (providers.has("gemini")) {
        keys.push("    geminiApiKey: str");
    }
    if (providers.has("openai")) {
        keys.push("    openaiApiKey: str");
    }
    if (providers.has("custom")) {
        keys.push("    customApiKey: str");
        keys.push("    customUrl: NotRequired[str]  # type: ignore") // standard Python 3.11+ TypedDict
    }

    if (keys.length === 0) return '';
    return `class ${agent.name}ApiKeys(TypedDict, total=False):\n${keys.join('\n')}\n`;
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

function collectTransferredHelpers(agent: AgentIR): HelperType[] {
    const transferredNames = new Set<string>();

    for (const workflow of (agent.workflows || [])) {
        scanForTransfers(workflow.body || [], transferredNames);
    }

    return (agent.helpers || []).filter(h => transferredNames.has(h.name));
}

function scanForTransfers(statements: any[], found: Set<string>): void {
    for (const stmt of statements) {
        if (stmt.type === 'transfer' && stmt.target?.value) {
            found.add(stmt.target.value);
        }
        if (stmt.type === 'if') {
            if (stmt.then) scanForTransfers(stmt.then, found);
            if (stmt.else) scanForTransfers(stmt.else, found);
        }
    }
}

function generateCustomTypes(types: Record<string, any>): string {
    const interfaces = Object.entries(types).map(([typeName, typeDef]) => {
        const props = Object.entries(typeDef.properties)
            .map(([propName, propInfo]: [string, any]) => {
                const comment = propInfo.description ? `    # ${propInfo.description}\n` : '';
                // TypedDict handles optionality differently depending on python version,
                // but for stubs we can use NotRequired in Py3.11+, or just Optional[Type].
                // Using Optional for standard generic compatibility.
                let pythonType = typeToPythonString(propInfo.type);
                if (propInfo.optional) {
                    pythonType = `Optional[${pythonType}]`;
                }
                return `${comment}    ${propName}: ${pythonType}`;
            })
            .join('\n');

        const propertiesBody = props.length > 0 ? props : '    pass';

        return `class ${typeName}(TypedDict, total=False):
${propertiesBody}
`;
    });

    return interfaces.join('\n');
}

function generateInputInterface(agent: AgentIR): string {
    const props = agent.input
        ? Object.entries(agent.input)
            .map(([name, val]: [string, any]) => {
                let pythonType = typeToPythonString(val);
                if (val?.optional) pythonType = `Optional[${pythonType}]`;
                return `    ${name}: ${pythonType}`;
            })
            .join('\n')
        : '';

    const propertiesBody = props.length > 0 ? props : '    pass';

    return `class ${agent.name}Input(TypedDict, total=False):
${propertiesBody}
`;
}

function generateHelperOutputInterface(helper: HelperType): string {
    const props = helper.output
        ? Object.entries(helper.output)
            .map(([name, val]: [string, any]) => {
                let pythonType = typeToPythonString(val);
                if (val?.optional) pythonType = `Optional[${pythonType}]`;
                return `    ${name}: ${pythonType}`;
            })
            .join('\n')
        : '';

    const propertiesBody = props.length > 0 ? props : '    pass';

    return `class ${helper.name}Output(TypedDict, total=False):
${propertiesBody}
`;
}

function generateOutputInterface(agent: AgentIR, transferredHelpers: HelperType[]): string {
    if (agent.output && '__variants' in agent.output) {
        const variants = agent.output.__variants as Record<string, Record<string, any>>;

        const variantClassNames: string[] = [];
        let variantClasses = "";

        for (const [variantName, variantProps] of Object.entries(variants)) {
            const className = `${agent.name}Output_${variantName}`;
            variantClassNames.push(className);

            const props = Object.entries(variantProps)
                .map(([name, val]: [string, any]) => {
                    let pythonType = typeToPythonString(val);
                    // Force the discriminator field to be Literal for proper type narrowing if we could use Literal
                    // But for simplicity in TypedDict stubs
                    if (val?.optional) pythonType = `Optional[${pythonType}]`;
                    return `    ${name}: ${pythonType}`;
                })
                .join('\n');

            variantClasses += `class ${className}(TypedDict, total=False):\n${props.length > 0 ? props : '    pass'}\n\n`;
        }

        const unionType = `Union[${variantClassNames.join(', ')}]`;

        return `${variantClasses}${agent.name}Output = ${unionType}\n`;
    }

    const props = agent.output
        ? Object.entries(agent.output)
            .map(([name, val]: [string, any]) => {
                let pythonType = typeToPythonString(val);
                if (val?.optional) pythonType = `Optional[${pythonType}]`;
                return `    ${name}: ${pythonType}`;
            })
            .join('\n')
        : '';

    const propertiesBody = props.length > 0 ? props : '    pass';
    const baseClass = `class ${agent.name}BaseOutput(TypedDict, total=False):\n${propertiesBody}\n`;

    if (transferredHelpers.length === 0) {
        return `class ${agent.name}Output(TypedDict, total=False):\n${propertiesBody}\n`;
    }

    const unionMembers = [
        `${agent.name}BaseOutput`,
        ...transferredHelpers.map(h => `${h.name}Output`)
    ].join(', ');

    return `${baseClass}
${agent.name}Output = Union[${unionMembers}]
`;
}

function generateContextInterface(agent: AgentIR): string {
    const props = agent.context
        ? Object.entries(agent.context)
            .map(([name, val]: [string, any]) => {
                let pythonType = typeToPythonString(val);
                if (val?.optional) pythonType = `Optional[${pythonType}]`;
                return `    ${name}: ${pythonType}`;
            })
            .join('\n')
        : '';

    const propertiesBody = props.length > 0 ? props : '    pass';

    return `class ${agent.name}Context(TypedDict, total=False):
${propertiesBody}
`;
}

function generateToolsProtocol(agentName: string, tools: ToolDef[]): string {
    if (!tools || tools.length === 0) {
        return `class ${agentName}Tools(TypedDict, total=False):\n    pass\n`;
    }

    const toolFields = tools.map(tool => {
        // Build the parameter type hints for the Callable signature
        const paramTypes = Object.entries(tool.params)
            .map(([name, typeObj]: [string, any]) => {
                let pythonType = typeToPythonString(typeObj);
                if (typeObj?.optional) pythonType = `Optional[${pythonType}]`;
                return pythonType;
            });

        const methodComment = tool.description ? `    # ${tool.description}\n` : '';
        const returns = typeToPythonString(tool.returns);

        // Generate as Callable with keyword args hint via Protocol isn't needed;
        // use a simple Callable[[param_types], Awaitable[ReturnType]]
        const paramList = paramTypes.length > 0 ? paramTypes.join(', ') : '';
        return `${methodComment}    ${tool.name}: Callable[[${paramList}], Awaitable[${returns}]]`;
    }).join('\n\n');

    return `class ${agentName}Tools(TypedDict, total=False):\n${toolFields}\n`;
}

function typeToPythonString(typeVal: any): string {
    if (typeof typeVal === 'string') {
        return normalizeType(typeVal);
    }

    if (typeVal?.type === 'typeRef' && typeVal.name) {
        // Enclose in quotes to handle forward references
        return `"${typeVal.name}"`;
    }

    if (typeVal?.type === 'array' && typeVal.items) {
        return `List[${typeToPythonString(typeVal.items)}]`;
    }

    if (typeVal?.type === 'union' && Array.isArray(typeVal.options)) {
        // Convert JS union strings to Literal equivalent behavior or just generic Union
        // Actually since options are strings let's map to Literal['foo', 'bar']
        // Wait, Python needs from typing import Literal for Literal['x']
        // We will just simplify to str for now if it's string enums
        return `str`;
    }

    if (typeVal?.type === 'object' && typeVal.properties) {
        // Inline TypedDict is not directly possible natively like TS { a: string }
        // For stubs we can map to Dict[str, Any]
        return `Dict[str, Any]`;
    }

    if (typeof typeVal === 'string' && typeVal.endsWith('[]')) {
        const inner = typeVal.slice(0, -2);
        return `List[${normalizeType(inner)}]`;
    }

    if (typeVal && typeof typeVal.type === 'object') {
        return typeToPythonString(typeVal.type);
    }

    if (typeVal && typeof typeVal.type === 'string') {
        return normalizeType(typeVal.type);
    }

    return 'Any';
}

function normalizeType(t: string): string {
    switch (t.toLowerCase()) {
        case 'int':
        case 'number':
        case 'float':
            return 'float';
        case 'bool':
        case 'boolean':
            return 'bool';
        case 'string':
            return 'str';
        default:
            return t; // Might be a custom type, return raw
    }
}
