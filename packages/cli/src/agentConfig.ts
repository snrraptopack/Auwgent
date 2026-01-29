import {
    Agent,
    AgentConfig,
    ContextConfig,
    InputConfig,
    isAgentConfig,
    isContextConfig,
    isInputConfig,
    isOutputConfig,
    isToolConfig,
    isToolsConfig,
    isWorkFlowConfig,
    ModelConfig,
    OutputConfig,
    ToolConfig,
    ToolsConfig,
    WorkFlowConfig,
    isGeminiProvider,
    isOpenAIProvider,
    isCustomProvider
} from "auwgent-language";

import {
    extractType,
    extractParams,
    extractExpression
} from "./generator.js";


export function handleAgentConfig(agent: Agent) {
    const agentIR = {
        name: agent.name,
        modelConfig: [] as any,
        input: null,
        output: null,
        context: null,
        tools: [] as any,
        workflows: [] as any,
        helpers: [] as any
    }

    for (let config of agent.configs) {
        if (isAgentConfig(config)) {
            agentIR.modelConfig.push(extractAgentConfig(config))
        }

        if (isInputConfig(config)) {
            agentIR.input = extractInOutConfig(config)
        }

        if (isOutputConfig(config)) {
            agentIR.output = extractInOutConfig(config)
        }

        // Single tool: tool functionName()
        if (isToolConfig(config)) {
            agentIR.tools.push(extractSingleToolConfig(config))
        }

        // Grouped tools: tools { ... }
        if (isToolsConfig(config)) {
            agentIR.tools.push(...extractToolsConfig(config))
        }

        if (isWorkFlowConfig(config)) {
            agentIR.workflows.push(extractWorkflowConfig(config))
        }

        if (isContextConfig(config)) {
            agentIR.context = extractInOutConfig(config)
        }
    }
    return agentIR
}

function extractModelProvider(provider: any) {
    if (isGeminiProvider(provider)) {
        return { type: "gemini", modelName: provider.modelName };
    }
    if (isOpenAIProvider(provider)) {
        return { type: "openai", modelName: provider.modelName };
    }
    if (isCustomProvider(provider)) {
        return { type: "custom", url: provider.url, modelName: provider.modelName };
    }
    throw new Error("Unknown model provider type");
}

function extractAgentConfig(agentConfig: AgentConfig) {
    const result: any = {};

    if (agentConfig.defaultconfig) {
        result.defaultConfig = {
            model: extractModelProvider(agentConfig.defaultconfig.model),
            prompt: extractPrompt(agentConfig.defaultconfig)
        };
    }

    if (agentConfig.nondefaultConfig) {
        result.namedConfig = agentConfig.nondefaultConfig.map(conf => ({
            configName: conf.name,
            model: extractModelProvider(conf.nonConf.model),
            prompt: extractPrompt(conf.nonConf)
        }))
    }

    return result
}

function extractInOutConfig(inputConfig: InputConfig | OutputConfig | ContextConfig) {
    let result = {} as any

    if (inputConfig.$type === "InputConfig") {
        inputConfig.inProperties.map(input => {
            result[input.name] = { type: extractType(input.t), optional: input.isOptional }
        })
    } else if (inputConfig.$type === "OutputConfig") {
        inputConfig.outProperties.map(output => {
            result[output.td.name] = { 
                type: extractType(output.td.t), 
                optional: output.td.isOptional,
                description: output.td.description ?? "no description" 
                //...(output.description && { description: output.description })
            }
        })
    } else if (inputConfig.$type === "ContextConfig") {
        inputConfig.contextProperties.map(context => {
            result[context.name] = { type: extractType(context.t), optional: context.isOptional }
        })
    }

    return result
}

function extractSingleToolConfig(toolConfig: ToolConfig) {
    let tool = toolConfig.tool
    return { description: tool.desc?.[0] ?? "", params: extractParams(tool.params), name: tool.name, returns: extractType(tool.returns) }
}

function extractToolsConfig(toolsConfig: ToolsConfig) {
    return toolsConfig.tools.map(tool => ({
        description: tool.desc?.[0] ?? "",
        params: extractParams(tool.params),
        name: tool.name,
        returns: extractType(tool.returns)
    }))
}

function extractPrompt(modelConfig: ModelConfig) {

    // Case 1: Inline parts - prompt { ... }
    if (modelConfig.parts && modelConfig.parts.length > 0) {
        return { type: "parts", value: modelConfig.parts.map(part => extractExpression(part)) }
    }

    // Case 2: Expression (concatenation, reference, string, etc.)
    if (modelConfig.promptExpr) {
        return extractExpression(modelConfig.promptExpr);
    }

    return null;
}

function extractWorkflowConfig(workflowConfig: WorkFlowConfig) {
    let flowName = workflowConfig.name
    let description = workflowConfig.desc
    let flowParams = extractParams(workflowConfig.params)
    let returns = extractType(workflowConfig.return)
    const workflowTools = [
        ...(workflowConfig.workflowToolConfigs ?? []).map(extractSingleToolConfig),
        ...(workflowConfig.workflowToolsConfigs ?? []).flatMap(extractToolsConfig)
    ]

    // Extract all statements
    let allStatements = workflowConfig.body.map(bdy => extractExpression(bdy))

    // Find used variables (starting from return statement)
    const usedVars = findUsedVariables(allStatements)

    // Filter to only include used statements (but keep side-effect statements)
    let body = allStatements.filter(stmt => {
        if (stmt.type === 'return') return true
        if (stmt.type === 'variableDeclaration') {
            // Keep if referenced OR if it has side effects (function/helper calls)
            if (usedVars.has(stmt.name)) return true
            if (hasSideEffects(stmt.value)) return true
            return false
        }
        return true // keep other statements like if
    })

    return { flowName, flowParams, returns, body, description, tools: workflowTools }
}

function findUsedVariables(statements: any[]): Set<string> {
    const used = new Set<string>()
    const varDeps = new Map<string, Set<string>>() // var -> vars it depends on

    // Build dependency graph
    for (const stmt of statements) {
        if (stmt.type === 'variableDeclaration') {
            varDeps.set(stmt.name, collectVarRefs(stmt.value))
        }
    }

    // Find return statement and collect its refs
    const returnStmt = statements.find(s => s.type === 'return')
    if (returnStmt) {
        const returnRefs = collectVarRefs(returnStmt.value)

        // Recursively mark all used vars
        const markUsed = (varName: string) => {
            if (used.has(varName)) return
            used.add(varName)
            const deps = varDeps.get(varName)
            if (deps) deps.forEach(markUsed)
        }

        returnRefs.forEach(markUsed)
    }

    return used
}

function collectVarRefs(expr: any): Set<string> {
    const refs = new Set<string>()

    const walk = (node: any) => {
        if (!node) return
        if (node.type === 'varRef') refs.add(node.value)
        if (node.value && typeof node.value === 'object') walk(node.value)
        if (node.args) node.args.forEach(walk)
        if (node.parts) node.parts.forEach((p: any) => walk(p.value))
        if (Array.isArray(node)) node.forEach(walk)
        if (typeof node === 'object') {
            Object.values(node).forEach(v => {
                if (typeof v === 'object') walk(v)
            })
        }
    }

    walk(expr)
    return refs
}

function hasSideEffects(expr: any): boolean {
    if (!expr) return false

    // Direct side-effect calls
    if (expr.type === 'functionCall') return true
    if (expr.type === 'helperCall') return true

    // Check nested expressions
    if (expr.value && typeof expr.value === 'object') {
        if (hasSideEffects(expr.value)) return true
    }
    if (expr.args && Array.isArray(expr.args)) {
        for (const arg of expr.args) {
            if (hasSideEffects(arg)) return true
        }
    }

    return false
}
