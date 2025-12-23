import {
    Expression,
    isArrayLiteral,
    isArrayType,
    isBooleanLiteral,
    isBooleanType,
    isFunctionCall,
    isIfStatement,
    isNumberLiteral,
    isNumberType,
    isObjectLiteral,
    isObjectType,
    isReturnStatement,
    isStringLiteral,
    isStringType,
    isTemplateLiteral,
    isTemplateString,
    isUnionType,
    isVariableDeclartion,
    isVariableRef,
    Model,
    Statement,
    TypeConfigDeclaration,
    Types,
    TemplateExpr,
    TemplateString,
    isContextReference,
    isHelperCall,
    Helper
} from "auwgent-language"


import * as fs from 'node:fs';
import * as path from 'node:path';
import { handleAgentConfig } from "./agentConfig.js";
import { generateTypesFile } from "./Types/typesGenerator.js";


type AgentIr = {
    name: string,
    modelConfig: any[],
    input: any,
    output: any,
    context: any,
    tools: any[],
    workflows: any[],
    helpers: HelperType[]
}

type HelperType = {
    name: string,
    description: string,
    returns: string | undefined,
    modelConfig: any[],
    input: any,
    output: any,
    context: any,
    tools: any[],
    workflows: any[]
}


export function generateOutput(model: Model, source: string, destination: string) {

    const destDir = path.dirname(destination);
    if (!fs.existsSync(destDir)) {
        fs.mkdirSync(destDir, { recursive: true });
    }

    // Build output filename
    const baseName = path.basename(source, '.agent');
    const outputPath = path.join(destDir, `${baseName}.agent.json`);


    // First pass: collect all helpers into a map by name
    const helperMap = new Map<string, HelperType>();
    for (const element of model.elements) {
        if (element.$type === "Helper") {
            const helperIR = handleHelper(element);
            helperMap.set(element.name, helperIR);
        }
    }

    // Second pass: process agents
    for (let i = 0; i < model.elements.length; i++) {
        let currentElement = model.elements[i];

        if (currentElement.$type === "Agent") {
            const agentIR = handleAgentConfig(currentElement) as AgentIr;

            // Filter helpers: only include those declared in agent's helpers { } block
            const declaredHelpers: HelperType[] = [];
            for (const config of currentElement.configs) {
                if (config.$type === "HelpersConfig" && config.helpers) {
                    for (const helperRef of config.helpers) {
                        if (helperRef.ref && helperMap.has(helperRef.ref.name)) {
                            declaredHelpers.push(helperMap.get(helperRef.ref.name)!);
                        }
                    }
                }
            }
            agentIR.helpers = declaredHelpers;

            fs.writeFileSync(outputPath, JSON.stringify(agentIR, null, 2));
            const typesPath = path.join(destDir, `${baseName}.agent.types.ts`);
            fs.writeFileSync(typesPath, generateTypesFile(agentIR));
        }

        if (currentElement.$type === "NamedPrompt") continue;
    }
    return outputPath;
}


/**
 * Handle Helper element - extract its configs into HelperType
 */
function handleHelper(helper: Helper): HelperType {
    // Extract helper configs using the same logic as agents
    const baseConfig = handleAgentConfig(helper as any); // Reuse agent config extraction

    return {
        name: helper.name,
        description: helper.desc,
        returns: helper.returnMode,
        modelConfig: baseConfig.modelConfig || [],
        input: baseConfig.input,
        output: baseConfig.output,
        context: baseConfig.context,
        tools: baseConfig.tools || [],
        workflows: baseConfig.workflows || []
    };
}



export function extractType(types: Types): any {
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

        if (isObjectType(t)) {
            const props = {} as any
            t.properties.forEach(prop => {
                props[prop.name] = extractType(prop.type)
            })
            return { type: "object", properties: props }
        }
    }
    return 'unknown';
}

export function extractParams(params: TypeConfigDeclaration[]) {
    let param = {} as any
    params.forEach(p => {
        param[p.name] = { type: extractType(p.t), optional: p.isOptional }
    })
    return param
}

export function extractExpression(express: Expression | Statement): any {

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

    if (isObjectLiteral(express)) {
        const props: any = {};
        express.properties.forEach(prop => {
            // If value exists, extract it; otherwise it's shorthand (use property name)
            props[prop.name] = prop.value ? extractExpression(prop.value) : { type: "varRef", value: prop.name };
        });
        return { type: "object", value: props };
    }

    if (isTemplateLiteral(express)) {
        // const templates = {} as any  
        let result = buildTemplate(express.templates)
        return { type: "template", parts: result }
    }

    if (isContextReference(express)) {
        return { type: "contextRef", property: express.property.ref?.name }
    }

    if (isHelperCall(express)) {
        const helperName = express.helper.ref?.name
        const args = express.args.map(arg => extractExpression(arg))
        return { type: "helperCall", value: helperName, args: args }
    }

    return null
}

//for building the template pattern

export function buildTemplate(template: (TemplateExpr | TemplateString)[]) {
    let stringBuilder = ""

    const parts = [] as any

    for (let i = 0; i < template.length; i++) {
        let current = template[i]

        if (isTemplateString(current)) {
            stringBuilder += " " + current.value
        } else {
            let expr = extractExpression(current.expr)
            if (stringBuilder.trim().length > 0) {
                parts.push({ type: "literal", value: stringBuilder })
                parts.push({ type: "expression", value: expr })
                stringBuilder = ""
            } else {
                parts.push({ type: "expression", value: expr })
            }
        }
    }

    if (stringBuilder.trim().length > 0) {
        parts.push({ type: "literal", value: stringBuilder })
        stringBuilder = ""
    }

    return parts

}
