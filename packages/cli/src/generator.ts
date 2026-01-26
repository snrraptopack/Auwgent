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
    isTransferStatement,
    isMemberAccess,
    isUseLifecycle,
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
    helpers: HelperType[],
    helperToolGrants?: Record<string, string[] | "all">,
    lifecycle?: {
        enabled: true,
        maxTokens?: number,
        maxMessages?: number
    }
}

type HelperType = {
    name: string,
    description: string,
    modelConfig: any[],
    input: any,
    output: any,
    context: any,
    tools: any[],
    workflows: any[]
} /// returns: string | undefined, has be removed requires changes in the loader too


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
            // Also extract tool grants for each helper
            const declaredHelpers: HelperType[] = [];
            const helperToolGrants: Record<string, string[] | "all"> = {};

            for (const config of currentElement.configs) {
                if (config.$type === "HelpersConfig" && config.helpers) {
                    for (const helperRef of config.helpers) {
                        const helperName = helperRef.helper?.ref?.name;
                        if (helperName && helperMap.has(helperName)) {
                            declaredHelpers.push(helperMap.get(helperName)!);

                            // Extract tool grants
                            if (helperRef.withAllTools) {
                                helperToolGrants[helperName] = "all";
                            } else if (helperRef.grantedTools && helperRef.grantedTools.length > 0) {
                                helperToolGrants[helperName] = helperRef.grantedTools
                                    .map(t => t.ref?.name)
                                    .filter((n): n is string => !!n);
                            }
                        }
                    }
                }
            }
            agentIR.helpers = declaredHelpers;
            if (Object.keys(helperToolGrants).length > 0) {
                agentIR.helperToolGrants = helperToolGrants;
            }

            // Extract lifecycle configuration
            for (const config of currentElement.configs) {
                if (isUseLifecycle(config)) {
                    agentIR.lifecycle = {
                        enabled: true,
                        maxTokens: config.maxTokens,
                        maxMessages: config.maxMessages
                    };
                    break;
                }
            }

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

    if (isTransferStatement(express)) {
        const helperName = express.call.helper.ref?.name
        const args = express.call.args.map(arg => extractExpression(arg))
        const mode = express.thenContinue ? "thenContinue" : "direct"
        return {
            type: "transfer",
            target: { type: "helperCall", value: helperName, args: args },
            mode: mode
        }
    }

    if (isMemberAccess(express)) {
        const objectName = express.object.ref?.name;
        const properties = [express.property, ...(express.chain || [])];
        return {
            type: "memberAccess",
            object: { type: "varRef", value: objectName },
            properties: properties
        };
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
