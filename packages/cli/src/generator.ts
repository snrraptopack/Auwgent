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
    isContextReference
} from "auwgent-language"


import * as fs from 'node:fs';
import * as path from 'node:path';
import { handleAgentConfig } from "./agentConfig.js";
import { generateTypesFile } from "./Types/typesGenerator.js";


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

        let agentIR = {} as { name: string, modelConfig: [], input: null, output: null, context: null, tools: [], workflows: [] }
        if (currentElement.$type === "Agent") {
            agentIR = handleAgentConfig(currentElement)
        }
        if (currentElement.$type === "NamedPrompt") continue


        fs.writeFileSync(outputPath, JSON.stringify(agentIR, null, 2));
        const typesPath = path.join(destDir, `${baseName}.agent.types.ts`)
        fs.writeFileSync(typesPath, generateTypesFile(agentIR))
    }
    return outputPath;
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
