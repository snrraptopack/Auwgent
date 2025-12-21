import {
    AgentConfig,
    Expression,
    InputConfig,
    isAgentConfig,
    isArrayLiteral,
    isArrayType,
    isBooleanLiteral,
    isBooleanType,
    isFunctionCall,
    isIfStatement,
    isInputConfig,
    isNumberLiteral,
    isNumberType,
    isObjectLiteral,
    isObjectType,
    isOutputConfig,
    isReturnStatement,
    isStringLiteral,
    isStringType,
    isTemplateLiteral,
    isTemplateString,
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
    WorkFlowConfig,
    TemplateExpr,
    TemplateString
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
            result[input.name] = { type: extractType(input.t), optional: input.isOptional }
        })
    } else {
        inputConfig.outProperties.map(output => {
            result[output.td.name] = { type: extractType(output.td.t), description: output.description, optional: output.td.isOptional }
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


function extractType(types: Types): any {
    if (types.types) {
        const t = types.types;

        if (isUnionType(t)) {
            return { type: "union", options: t.options }
        }
        if (isBooleanType(t) || isNumberType(t) || isStringType(t)) {
            return t.type
        }

        if (isArrayType(t) && !isNumberType(t)) {
            const innerType = t.type.type;
            return `${innerType}[]`;
        }

        if(isObjectType(t)){
            const props = {} as any
            t.properties.forEach(prop=>{
                props[prop.name] = extractType(prop.type)
            })
            return {type:"object",properties:props}
        }
    }
    return 'unknown';
}

function extractParams(params: TypeConfigDeclaration[]) {
    let param = {} as any
    params.forEach(p => {
        param[p.name] = { type: extractType(p.t), optional: p.isOptional }
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

    if (isArrayLiteral(express)) {
        let elements = express.elements.map(item => extractExpression(item))
        return { type: "array", value: elements }
    }

    if (isUnionLiteral(express)) {
        return { value: express.value.options, type: "union" }
    }

    if (isVariableRef(express)) {
        return { value: express.variable.ref?.name, type: "varRef" }
    }

    if (isBooleanLiteral(express)) {
        return { value: express.value, type: "literal" }
    }

    if (isFunctionCall(express)) {
        let name = express.func.ref?.name
        let args = express.args.map(arg => extractExpression(arg))

        return { value: name, type: "functionCall", args: args }
    }

    if (isIfStatement(express)) {
        const condition = {
            left: extractExpression(express.condition.left),
            operator: express.condition.op,
            right: extractExpression(express.condition.right)
        }

        const thenBlock = express.thenBlock.map(stmt => extractExpression(stmt))
        const elseBlock = express.elseBlock?.map(stmt => extractExpression(stmt)) || []

        return { type: "if", condition, then: thenBlock, else: elseBlock }
    }

    if (isReturnStatement(express)) {
        return { type: "return", value: extractExpression(express.value) }
    }

    if(isObjectLiteral(express)){
        const props: any = {};
        express.properties.forEach(prop => {
        // If value exists, extract it; otherwise it's shorthand (use property name)
            props[prop.name] = prop.value ? extractExpression(prop.value) : { type: "varRef", value: prop.name };
        });
        return {type:"object", value: props};
    }

    if(isTemplateLiteral(express)){
      // const templates = {} as any  
    let result = buildTemplate(express.templates)
    return {type:"template",parts:result}
    }

    return null
}

//for building the template pattern

function buildTemplate(template:(TemplateExpr | TemplateString)[]){
    let stringBuilder = ""

    const parts = [] as any

      for(let i=0; i < template.length;i++){
        let current = template[i]

        if(isTemplateString(current)){
            stringBuilder += " "+ current.value
        }else{
            let expr = extractExpression(current.expr)
            if(stringBuilder.trim().length > 0){
                parts.push({type:"literal",value:stringBuilder})
                parts.push({type:"expression",value:expr})
                stringBuilder = ""
            }else{
               parts.push({type:"expression",value:expr})
            }
        }
    }

    if(stringBuilder.trim().length > 0){
        parts.push({type:"literal",value:stringBuilder})
        stringBuilder = ""
    }  

    return parts

}


// Build TypeScript types
function generateTypes(agent: any): string {

    // Check if input exists
    const inputProps = agent.input
        ? Object.entries(agent.input)
            .map(([name, val]: [string, any]) => {
                const optionalMarker = val?.optional ? '?' : '';
                return `    ${name}${optionalMarker}: ${typeToTsString(val)}`
            })
            .join('\n')
        : '';

    // Check if output exists
    const outputProps = agent.output
        ? Object.entries(agent.output)
            .map(([name, val]: [string, any]) => {
                const optionalMarker = val?.optional ? '?' : '';
                return `    ${name}${optionalMarker}: ${typeToTsString(val)}`
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
            .map(([name, typeObj]: [string, any]) => {
                const optionalMarker = typeObj?.optional ? '?' : '';
                return `${name}${optionalMarker}: ${typeToTsString(typeObj)}`;
            })
            .join(', ');

        return `    ${tool.name}: (args: { ${paramType} }) => Promise<${typeToTsString(tool.returns)}>;`;
    }).join('\n');
    
    return `export interface ${agent.name}Tools {
    [key: string]: (args: any) => Promise<any>;  // Index signature for ToolMap compatibility
${toolMethods}
}`;
}


 // Helper to convert type value to TS string (same as in generateTypes)
function typeToTsString(typeVal: any): string{
        if (typeof typeVal === 'string') {
            return typeVal;
        }
        // Handle union type: { type: "union", options: [...] }
        if (typeVal && typeVal.type === 'union' && Array.isArray(typeVal.options)) {
            return typeVal.options.map((o: string) => `"${o.replace(/^["']|["']$/g, '')}"`).join(' | ');
        }
        // Handle object type: { type: "object", properties: {...} }
        if (typeVal && typeVal.type === 'object' && typeVal.properties) {
            const props = Object.entries(typeVal.properties)
                .map(([key, val]) => `${key}: ${typeToTsString(val)}`)
                .join(', ');
            return `{ ${props} }`;
        }
        // Handle nested type wrapper: { type: {...}, optional: ... }
        if (typeVal && typeof typeVal.type === 'object') {
            return typeToTsString(typeVal.type);
        }
        // Handle simple wrapper: { type: "string", optional: ... }
        if (typeVal && typeof typeVal.type === 'string') {
            return typeVal.type;
        }
        return 'unknown';
    };
