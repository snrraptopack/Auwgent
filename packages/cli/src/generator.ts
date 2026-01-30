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
    isBinaryExpression,
    isNamedPrompt,
    isInlinePromptBlock,
    isMultilineStringLiteral,
    Model,
    Statement,
    TypeConfigDeclaration,
    Types,
    BaseType,
    isBaseType,
    TemplateExpr,
    TemplateString,
    isContextReference,
    isHelperCall,
    Helper,
    TypeDeclaration,
    isTypeDeclaration,
    createAuwgentServices,
    isPromptCall
} from "auwgent-language"


import * as fs from 'node:fs';
import * as path from 'node:path';
import { handleAgentConfig } from "./agentConfig.js";
import { generateTypesFile } from "./Types/typesGenerator.js";
import { CrossFileResolver } from "./cross-file-resolver.js";
import { NodeFileSystem } from 'langium/node';
import { URI } from 'langium';


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
    },
    types?: Record<string, TypeDefinition>
}

type TypeDefinition = {
    isOutput: boolean,
    properties: Record<string, PropertyInfo>
}

type PropertyInfo = {
    type: any,
    optional: boolean,
    description?: string
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


export async function generateOutput(model: Model, source: string, destination: string) {

    const destDir = path.dirname(destination);
    if (!fs.existsSync(destDir)) {
        fs.mkdirSync(destDir, { recursive: true });
    }

    // Build output filename
    const baseName = path.basename(source, '.agent');
    const outputPath = path.join(destDir, `${baseName}.agent.json`);

    // Initialize cross-file resolver
    const resolver = new CrossFileResolver();
    const services = createAuwgentServices(NodeFileSystem).Auwgent;
    
    // Create a parser function for the resolver
    const parseFile = async (filePath: string): Promise<Model | null> => {
        try {
            const document = await services.shared.workspace.LangiumDocuments.getOrCreateDocument(URI.file(filePath));
            await services.shared.workspace.DocumentBuilder.build([document], { validation: false });
            return document.parseResult?.value as Model;
        } catch (error) {
            console.error(`Error parsing file ${filePath}:`, error);
            return null;
        }
    };

    // Resolve all imports and collect dependencies
    const absoluteSourcePath = path.resolve(source);
    const { helpers: importedHelpers, types: importedTypes } = 
        await resolver.resolveImports(model, absoluteSourcePath, parseFile);

    // Collect all type definitions (local + imported)
    const typeMap = new Map<string, TypeDeclaration>();
    
    // Add imported types
    for (const [name, type] of importedTypes) {
        typeMap.set(name, type);
    }
    
    // Add local types (override imported if same name)
    for (const element of model.elements) {
        if (isTypeDeclaration(element)) {
            typeMap.set(element.name, element);
        }
    }

    // First pass: collect all helpers (local + imported)
    const helperMap = new Map<string, HelperType>();
    
    // Add imported helpers
    for (const [name, helper] of importedHelpers) {
        const helperIR = handleHelper(helper);
        helperMap.set(name, helperIR);
    }
    
    // Add local helpers (override imported if same name)
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

            // Handle direct output type flattening
            if (agentIR.output && typeof agentIR.output === 'object' && '__directType' in agentIR.output) {
                const directTypeInfo = agentIR.output as any;
                const typeName = directTypeInfo.__directType;
                
                // Look up the type definition
                if (typeMap.has(typeName)) {
                    const typeDef = typeMap.get(typeName)!;
                    const flattened: any = {};
                    
                    // Flatten the type properties
                    for (const prop of typeDef.types) {
                        flattened[prop.name] = {
                            type: extractType(prop.t),
                            optional: prop.isOptional,
                            ...(prop.description && { description: prop.description })
                        };
                    }
                    
                    agentIR.output = flattened;
                }
            }

            // Add type definitions to IR (includes imported types)
            if (typeMap.size > 0) {
                agentIR.types = extractTypeDefinitions(typeMap);
            }

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
            fs.writeFileSync(typesPath, generateTypesFile(agentIR, baseName));
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

/**
 * Extract type definitions from the type map into IR format
 */
function extractTypeDefinitions(typeMap: Map<string, TypeDeclaration>): Record<string, TypeDefinition> {
    const types: Record<string, TypeDefinition> = {};
    
    for (const [name, typeDecl] of typeMap.entries()) {
        const properties: Record<string, PropertyInfo> = {};
        
        for (const prop of typeDecl.types) {
            properties[prop.name] = {
                type: extractType(prop.t),
                optional: prop.isOptional,
                ...(prop.description && { description: prop.description })
            };
        }
        
        types[name] = {
            isOutput: typeDecl.isOutput,
            properties
        };
    }
    
    return types;
}


export function extractType(types: Types): any {
    // Handle ArrayType
    if (isArrayType(types)) {
        const elementType = extractBaseType(types.elementType);
        return { type: "array", items: elementType };
    }
    
    // Handle BaseType
    if (isBaseType(types)) {
        return extractBaseType(types);
    }
    
    return 'unknown';
}

function extractBaseType(baseType: BaseType): any {
    // Handle type reference
    if (baseType.typeRef) {
        return { type: "typeRef", name: baseType.typeRef.ref?.name || "unknown" };
    }
    
    // Handle inline types
    if (baseType.type) {
        const t = baseType.type;
        
        if (isUnionType(t)) {
            return { type: "union", options: t.options };
        }
        
        if (isBooleanType(t)) {
            return "boolean";
        }
        
        if (isNumberType(t)) {
            return "number";
        }
        
        if (isStringType(t)) {
            return "string";
        }
        
        if (isObjectType(t)) {
            const props = {} as any;
            t.properties.forEach(prop => {
                props[prop.name] = extractType(prop.type);
            });
            return { type: "object", properties: props };
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

    if (isMultilineStringLiteral(express)) {
        // Process multiline string with {{}} interpolation
        return processMultilineString(express.value);
    }

    if (isArrayLiteral(express)) {
        let elements = express.elements.map(item => extractExpression(item))
        return { type: "array", value: elements }
    }

    if (isVariableRef(express)) {
        const ref = express.variable.ref;
        // Check if this is a reference to a NamedPrompt
        if (ref && isNamedPrompt(ref)) {
            return {
                type: "promptRef",
                name: ref.name,
                params: ref.params?.map(param => param.name) ?? [],
                args: [],
                value: ref.parts?.map(part => extractExpression(part)) ?? []
            }
        }
        return { value: ref?.name, type: "varRef" }
    }

    if (isBooleanLiteral(express)) {
        return { value: express.value, type: "literal" }
    }

    if (isFunctionCall(express)) {
        let name = express.func.ref?.name
        let args = express.args.map(arg => extractExpression(arg))

        return { value: name, type: "functionCall", args: args }
    }

    if (isPromptCall(express)) {
        const ref = express.prompt.ref;
        return {
            type: "promptRef",
            name: ref?.name ?? express.prompt.$refText,
            params: ref?.params?.map(param => param.name) ?? [],
            args: express.args.map(arg => extractExpression(arg)),
            value: ref?.parts?.map(part => extractExpression(part)) ?? []
        }
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
        let result = buildTemplate(express.templates)
        return { type: "template", value: result }
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

    if (isBinaryExpression(express)) {
        return {
            type: "concat",
            left: extractExpression(express.left),
            right: extractExpression(express.right)
        }
    }

    if (isInlinePromptBlock(express)) {
        return {
            type: "inlinePrompt",
            parts: express.parts.map(part => extractExpression(part))
        }
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

/**
 * Process multiline string with {{expression}} interpolation
 * Returns an IR structure similar to template literals
 */
function processMultilineString(value: string): any {
    // Remove the triple quotes from the value
    const content = value.replace(/^"""/, '').replace(/"""$/, '');
    
    // Pattern to match {{...}} expressions
    const interpolationPattern = /\{\{([^}]+)\}\}/g;
    
    const parts: any[] = [];
    let lastIndex = 0;
    let match: RegExpExecArray | null;
    
    // Find all {{expression}} patterns
    while ((match = interpolationPattern.exec(content)) !== null) {
        // Add literal text before the expression
        if (match.index > lastIndex) {
            const literalText = content.substring(lastIndex, match.index);
            if (literalText.length > 0) {
                parts.push({ type: "literal", value: literalText });
            }
        }
        
        // Parse the expression inside {{}}
        const expressionText = match[1].trim();
        
        // Create a simple expression parser for the interpolation
        // This handles basic cases: variable refs, member access, etc.
        const parsedExpr = parseInterpolationExpression(expressionText);
        parts.push({ type: "expression", value: parsedExpr });
        
        lastIndex = match.index + match[0].length;
    }
    
    // Add remaining literal text after last expression
    if (lastIndex < content.length) {
        const literalText = content.substring(lastIndex);
        if (literalText.length > 0) {
            parts.push({ type: "literal", value: literalText });
        }
    }
    
    // If no interpolations found, return as simple literal
    if (parts.length === 0) {
        return { type: "literal", value: content };
    }
    
    // If only one part and it's a literal, return it directly
    if (parts.length === 1 && parts[0].type === "literal") {
        return parts[0];
    }
    
    // Return as template-like structure
    return { type: "template", value: parts };
}

/**
 * Parse an interpolation expression from {{...}}
 * Handles: variable refs, member access, simple literals
 */
function parseInterpolationExpression(expr: string): any {
    // Handle member access: user.name or user.profile.email
    if (expr.includes('.')) {
        const parts = expr.split('.');
        const objectName = parts[0];
        const properties = parts.slice(1);
        
        return {
            type: "memberAccess",
            object: { type: "varRef", value: objectName },
            properties: properties
        };
    }
    
    // Handle simple variable reference
    return { type: "varRef", value: expr };
}

