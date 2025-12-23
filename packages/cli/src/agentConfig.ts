import { Agent, AgentConfig, ContextConfig, InputConfig, isAgentConfig, isContextConfig, isInputConfig, isOutputConfig, isToolConfig, isToolsConfig, isWorkFlowConfig, ModelConfig, OutputConfig, ToolConfig, ToolsConfig, WorkFlowConfig } from "auwgent-language";
import { extractType, extractParams, extractExpression } from "./generator.js";


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

function extractAgentConfig(agentConfig: AgentConfig) {
    const result: any = {};

    if (agentConfig.defaultconfig) {
        result.defaultConfig = {
            modelName: agentConfig.defaultconfig.ModelName,
            prompt: extractPrompt(agentConfig.defaultconfig)
        };
    }

    if (agentConfig.nondefaultConfig) {
        result.namedConfig = agentConfig.nondefaultConfig.map(conf => ({
            configName: conf.name,
            modelName: conf.nonConf.ModelName,
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
            result[output.td.name] = { type: extractType(output.td.t), description: output.description, optional: output.td.isOptional }
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

    // Case 2: Reference - prompt: SomeNamedPrompt
    if (modelConfig.refPrompt?.ref) {
        const namedPrompt = modelConfig.refPrompt.ref;
        return {
            type: "ref",
            name: namedPrompt.name,
            // Optionally resolve the parts from the referenced prompt
            value: namedPrompt.parts?.map(part => extractExpression(part)) ?? []
        };
    }

    // Case 3: Simple string - prompt: "some string"
    if (modelConfig.simplePrompt) {
        return {
            type: "simple",
            value: modelConfig.simplePrompt
        };
    }
    return null;
}

function extractWorkflowConfig(workflowConfig: WorkFlowConfig) {
    let flowName = workflowConfig.name
    let description = workflowConfig.desc
    let flowParams = extractParams(workflowConfig.params)
    let returns = extractType(workflowConfig.return)

    // Extract all statements
    let allStatements = workflowConfig.body.map(bdy => extractExpression(bdy))

    // Find used variables (starting from return statement)
    const usedVars = findUsedVariables(allStatements)

    // Filter to only include used statements
    let body = allStatements.filter(stmt => {
        if (stmt.type === 'return') return true
        if (stmt.type === 'variableDeclaration') {
            return usedVars.has(stmt.name)
        }
        return true // keep other statements like if
    })

    return { flowName, flowParams, returns, body, description }
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
