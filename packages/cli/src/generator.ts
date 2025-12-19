import {
    AgentConfig,
    Expression,
    InputConfig,
    isAgentConfig,
    isBooleanType,
    isFunctionCall,
    isInputConfig,
    isNumberLiteral,
    isNumberType,
    isOutputConfig,
    isReturnStatement,
    isStringLiteral,
    isStringType,
    isToolConfig,
    isUnionLiteral,
    isUnionType,
    isVariableDeclartion,
    isVariableRef,
    isWorkFlowConfig,
    Model,
    OutputConfig,
    Statement,
    ToolConfig,
    TypeConfigDeclaration,
    Types,
    WorkFlowConfig
} from "auwgent-language"

import * as fs from 'node:fs';
import * as path from 'node:path';




export function generateOutput(model: Model, source: string, destination: string) {

    const destDir = path.dirname(destination);
    if (!fs.existsSync(destDir)) {
        fs.mkdirSync(destDir, { recursive: true });
    }


    // Build output filename
    const baseName = path.basename(source, '.agent');
    const outputPath = path.join(destDir, `${baseName}.agent.json`);


    for (let i = 0; i < model.elements.length; i++) {

        let currentElement = model.elements[i]

        for (let agent of currentElement.agents) {

            const agentIR = {
                name: agent.name,
                modelConfig: [] as any,
                input: null,
                output: null,
                tools: [] as any,
                workflows: [] as any
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

                if (isToolConfig(config)) {
                    agentIR.tools.push(extractToolConfig(config))
                }

                if (isWorkFlowConfig(config)) {
                    agentIR.workflows.push(extractWorkflowConfig(config))
                }
            }

            fs.writeFileSync(outputPath, JSON.stringify(agentIR, null, 2));
            const typesPath = path.join(destDir, `${baseName}.agent.types.ts`)
            fs.writeFileSync(typesPath, generateTypes(agentIR))
        }
    }

    return outputPath;
}

function extractAgentConfig(agentConfig: AgentConfig) {
    const result: any = {};

    if (agentConfig.defaultconfig) {
        result.defaultConfig = {
            modelName: agentConfig.defaultconfig.ModelName,
            prompt: agentConfig.defaultconfig.prompt ?? null
        };
    }

    if (agentConfig.nondefaultConfig) {
        result.namedConfig = agentConfig.nondefaultConfig.map(conf => ({
            configName: conf.name,
            modelName: conf.nonConf.ModelName,
            prompt: conf.nonConf.prompt ?? null
        }))
    }

    return result
}

function extractInOutConfig(inputConfig: InputConfig | OutputConfig) {
    let result = {} as any

    if (inputConfig.$type === "InputConfig") {
        inputConfig.inProperties.map(input => {
            result[input.name] = extractType(input.t)
        })
    } else {
        inputConfig.outProperties.map(output => {
            result[output.td.name] = { type: extractType(output.td.t), description: output.description }
        })
    }

    return result
}

function extractToolConfig(toolConfig: ToolConfig) {
    let tool = toolConfig.tool
    return { description: tool.desc, params: extractParams(tool.params), name: tool.name, returns: extractType(tool.returns) }

}

function extractWorkflowConfig(workflowConfig: WorkFlowConfig) {

    let flowName = workflowConfig.name
    let description = workflowConfig.desc
    let flowParams = extractParams(workflowConfig.params)
    let returns = extractType(workflowConfig.return)

    let body = workflowConfig.body.map(bdy => {
        return extractExpression(bdy)
    })

    return { flowName, flowParams, returns, body, description }
}


function extractType(types: Types): string {
    if (types.types) {
        const t = types.types;

        if (isUnionType(t)) {
            return t.options.join("|")
        }
        if (isBooleanType(t) || isNumberType(t) || isStringType(t)) {
            return t.type
        }
    }
    return 'unknown';
}

function extractParams(params: TypeConfigDeclaration[]) {
    let param = {} as any
    params.forEach(p => {
        param[p.name] = extractType(p.t)
    })
    return param
}

function extractExpression(express: Expression | Statement): any {

    if (isVariableDeclartion(express)) {
        let value = extractExpression(express.value) as any
        let name = express.name
        return { name: name, value: value, type: "variableDeclaration" }
    }

    if (isNumberLiteral(express) || isStringLiteral(express)) {
        return { value: express.value, type: "literal" }
    }

    if (isUnionLiteral(express)) {
        return { value: express.value.options, type: "union" }
    }

    if (isVariableRef(express)) {
        return { value: express.variable.ref?.name, type: "varRef" }
    }

    if (isFunctionCall(express)) {
        let name = express.func.ref?.name
        let args = express.args.map(arg => extractExpression(arg))

        return { value: name, type: "functionCall", args: args }
    }

    if (isReturnStatement(express)) {
        return { type: "return", value: extractExpression(express.value) }
    }

    return null
}


// Build TypeScript types
function generateTypes(agent: any): string {
    // Check if input exists
    const inputProps = agent.input
        ? Object.entries(agent.input)
            .map(([name, type]) => `    ${name}: ${type}`)
            .join('\n')
        : '';

    // Check if output exists
    const outputProps = agent.output
        ? Object.entries(agent.output)
            .map(([name, value]: [string, any]) => {
                const type = typeof value === 'string' ? value : value.type
                return `    ${name}: ${type}`
            })
            .join('\n')
        : '';

    const toolInterface = generateToolsInterface(agent);

    return `// Auto-generated from ${agent.name}

export interface ${agent.name}Input {
${inputProps}
}

export interface ${agent.name}Output {
${outputProps}
}

${toolInterface}
`
}


function generateToolsInterface(agent: any): string {
    if (!agent.tools || agent.tools.length === 0) {
        return "";
    }
    const toolMethods = agent.tools.map((tool: any) => {
        const paramType = Object.entries(tool.params)
            .map(([name, type]) => `${name}: ${type}`)
            .join(', ');

        // e.g. calculate: (args: {a: number, b: number}) => Promise<number>;
        return `    ${tool.name}: (args: { ${paramType} }) => Promise<${tool.returns}>;`;
    }).join('\n');
    return `export interface ${agent.name}Tools {
${toolMethods}
}`;
}
