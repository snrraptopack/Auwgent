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
    isUnionType,
    isVariableDeclartion,
    isVariableRef,
    isTransferStatement,
    isMemberAccess,
    isBinaryExpression,
    isNamedPrompt,
    isInlinePromptBlock,
    isMultilineStringLiteral,
    isParallelStatement,
    isComparison,
    isLogicalCondition,
    isBooleanCondition,
    isAssignmentStatement,
    isIndexAccess,
    isWorkFlowConfig,
    Model,
    Statement,
    TypeConfigDeclaration,
    Types,
    BaseType,
    isBaseType,
    isContextReference,
    isHelperCall,
    Helper,
    TypeDeclaration,
    isTypeDeclaration,
    createAuwgentServices,
    isPromptCall,
    isExampleBlock,
    Condition
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
    helperHandoff?: Record<string, "user" | "thenContinue">,
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
    const destDir = resolveDestinationDir(destination);
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
            const helperHandoff: Record<string, "user" | "thenContinue"> = {};

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
                            if (helperRef.handoffUser) {
                                helperHandoff[helperName] = helperRef.handoffThenContinue ? "thenContinue" : "user";
                            }
                        }
                    }
                }
            }
            agentIR.helpers = declaredHelpers;
            if (Object.keys(helperToolGrants).length > 0) {
                agentIR.helperToolGrants = helperToolGrants;
            }
            if (Object.keys(helperHandoff).length > 0) {
                agentIR.helperHandoff = helperHandoff;
            }

            fs.writeFileSync(outputPath, JSON.stringify(agentIR, null, 2));
            const typesPath = path.join(destDir, `${baseName}.agent.types.ts`);
            fs.writeFileSync(typesPath, generateTypesFile(agentIR, baseName));
        }

        if (currentElement.$type === "NamedPrompt") continue;
    }
    return outputPath;
}

const resolveDestinationDir = (destination: string): string => {
    if (fs.existsSync(destination)) {
        const stat = fs.statSync(destination);
        if (stat.isDirectory()) {
            return destination;
        }
    }
    if (!path.extname(destination)) {
        return destination;
    }
    return path.dirname(destination);
};


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

/**
 * Extract condition IR from a Condition AST node.
 * Handles both simple comparisons and logical (&&, ||) conditions.
 */
export function extractCondition(condition: Condition): any {
    if (isComparison(condition)) {
        // Simple comparison: left op right
        return {
            type: "comparison",
            left: extractExpression(condition.left),
            operator: condition.op,
            right: extractExpression(condition.right)
        };
    }
    
    if (isLogicalCondition(condition)) {
        // Logical condition: left && right or left || right
        return {
            type: "logical",
            operator: condition.op,
            left: extractCondition(condition.left),
            right: extractCondition(condition.right)
        };
    }

    if (isBooleanCondition(condition)) {
        // Bare boolean expression: if (hasValue) {}
        return {
            type: "boolean",
            value: extractExpression(condition.value)
        };
    }
    
    // Fallback (shouldn't happen)
    return { type: "unknown" };
}

export function extractExpression(express: Expression | Statement): any {

    if (isVariableDeclartion(express)) {
        let value = extractExpression(express.value) as any
        let name = express.name
        return { name: name, value: value, type: "variableDeclaration" }
    }

    if (isAssignmentStatement(express)) {
        let value = extractExpression(express.value) as any
        let name = express.variable.ref?.name
        return { name: name, value: value, type: "assignment" }
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

    if (isIndexAccess(express)) {
        const objectName = express.object.ref?.name ?? "unknown";
        const index = extractExpression(express.index);
        // Handle optional property chain after index access (e.g., array[0].prop)
        if (express.property) {
            const chain = express.chain ?? [];
            return {
                type: "indexAccess",
                object: objectName,
                index: index,
                property: express.property,
                chain: chain
            };
        }
        return { type: "indexAccess", object: objectName, index: index };
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
                value: ref.parts?.map(part => extractPromptStatement(part)).filter((part): part is any => part !== null) ?? []
            }
        }
        return { value: ref?.name, type: "varRef" }
    }

    if (isBooleanLiteral(express)) {
        return { value: express.value, type: "literal" }
    }

    if (isFunctionCall(express)) {
        const ref = express.func.ref;
        const args = express.args.map(arg => extractExpression(arg));
        if (ref && isNamedPrompt(ref)) {
            return {
                type: "promptRef",
                name: ref.name,
                params: ref.params?.map(param => param.name) ?? [],
                args,
                value: ref.parts?.map(part => extractPromptStatement(part)).filter((part): part is any => part !== null) ?? []
            }
        }
        if (ref && isWorkFlowConfig(ref)) {
            // Workflow call - treated like an internal function call
            const name = ref.name;
            return { value: name, type: "workflowCall", args }
        }
        // Tool function call
        const name = ref?.name ?? express.func.$refText;
        return { value: name, type: "functionCall", args }
    }

    if (isPromptCall(express)) {
        const ref = express.prompt.ref;
        return {
            type: "promptRef",
            name: ref?.name ?? express.prompt.$refText,
            params: ref?.params?.map(param => param.name) ?? [],
            args: express.args.map(arg => extractExpression(arg)),
            value: ref?.parts?.map(part => extractPromptStatement(part)).filter((part): part is any => part !== null) ?? []
        }
    }

    if (isIfStatement(express)) {
        const condition = extractCondition(express.condition);

        const thenBlock = express.thenBlock.map(stmt => extractExpression(stmt))
        const elseBlock = express.elseBlock?.map(stmt => extractExpression(stmt)) || []

        return { type: "if", condition, then: thenBlock, else: elseBlock }
    }

    if (isParallelStatement(express)) {
        const body = express.body.map(stmt => extractExpression(stmt))
        return { type: "parallel", body }
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
            type: "binaryOp",
            op: express.op,
            left: extractExpression(express.left),
            right: extractExpression(express.right)
        }
    }

    if (isInlinePromptBlock(express)) {
        return {
            type: "inlinePrompt",
            parts: express.parts.map(part => extractPromptStatement(part)).filter((part): part is any => part !== null)
        }
    }

    return null
}

export function extractPromptStatement(statement: any): any | null {
    if (isExampleBlock(statement)) {
        return null;
    }
    return extractExpression(statement);
}

/**
 * Process multiline string with {{expression}}, {{#if}}...{{/if}}, and {{@schema()}} interpolation.
 * Converts into a template IR structure understood by the runtime.
 */
function processMultilineString(value: string): any {
    // Remove the triple quotes from the value
    const content = value.replace(/^"""/, '').replace(/"""$/, '');
    
    const segments = parseTemplateContent(content);
    
    // If no segments, return empty literal
    if (segments.length === 0) {
        return { type: "literal", value: content };
    }
    
    // If single segment, return it directly
    if (segments.length === 1) {
        return segments[0];
    }
    
    // Wrap in template type for proper evaluation
    return { type: "template", value: segments };
}

/**
 * Parse template content into segments (handles nested {{#if}} blocks)
 */
function parseTemplateContent(content: string): any[] {
    const segments: any[] = [];
    let pos = 0;
    
    while (pos < content.length) {
        // Look for next {{ 
        const nextOpen = content.indexOf('{{', pos);
        
        if (nextOpen === -1) {
            // No more interpolations, add remaining as literal
            const remaining = content.substring(pos);
            if (remaining.length > 0) {
                segments.push({ type: "literal", value: remaining });
            }
            break;
        }
        
        // Add literal text before {{
        if (nextOpen > pos) {
            segments.push({ type: "literal", value: content.substring(pos, nextOpen) });
        }
        
        // Check what kind of block this is
        const afterOpen = content.substring(nextOpen + 2);
        
        if (afterOpen.startsWith('#if ')) {
            // {{#if condition}}...{{/if}} or {{#if condition}}...{{else}}...{{/if}}
            const result = parseIfBlock(content, nextOpen);
            segments.push(result.node);
            pos = result.endPos;
        } else if (afterOpen.startsWith('@schema(')) {
            // {{@schema(path)}}
            const result = parseSchemaDirective(content, nextOpen);
            segments.push(result.node);
            pos = result.endPos;
        } else {
            // Simple {{expression}}
            const closePos = content.indexOf('}}', nextOpen);
            if (closePos === -1) {
                // Malformed, treat rest as literal
                segments.push({ type: "literal", value: content.substring(nextOpen) });
                break;
            }
            
            const exprText = content.substring(nextOpen + 2, closePos).trim();
            segments.push(parseInterpolationExpression(exprText));
            pos = closePos + 2;
        }
    }
    
    return segments;
}

/**
 * Parse {{#if condition}}...{{else}}...{{/if}} block
 * Requires explicit condition: left op right (e.g., user.role == "admin")
 */
function parseIfBlock(content: string, startPos: number): { node: any, endPos: number } {
    // Find the condition (between {{#if and }})
    const conditionStart = startPos + 5; // {{#if 
    const conditionEnd = content.indexOf('}}', conditionStart);
    if (conditionEnd === -1) {
        return { node: { type: "literal", value: "" }, endPos: content.length };
    }
    
    const conditionText = content.substring(conditionStart, conditionEnd).trim();
    const condition = parseCondition(conditionText);
    
    // Find matching {{/if}} (handling nested ifs)
    let depth = 1;
    let searchPos = conditionEnd + 2;
    let elsePos = -1;
    let endIfPos = -1;
    
    while (depth > 0 && searchPos < content.length) {
        const nextIf = content.indexOf('{{#if ', searchPos);
        const nextElse = content.indexOf('{{else}}', searchPos);
        const nextEndIf = content.indexOf('{{/if}}', searchPos);
        
        // Find the closest tag
        const positions = [
            { type: 'if', pos: nextIf },
            { type: 'else', pos: nextElse },
            { type: 'endif', pos: nextEndIf }
        ].filter(p => p.pos !== -1).sort((a, b) => a.pos - b.pos);
        
        if (positions.length === 0) break;
        
        const closest = positions[0];
        
        if (closest.type === 'if') {
            depth++;
            searchPos = closest.pos + 6;
        } else if (closest.type === 'else' && depth === 1) {
            elsePos = closest.pos;
            searchPos = closest.pos + 8;
        } else if (closest.type === 'endif') {
            depth--;
            if (depth === 0) {
                endIfPos = closest.pos;
            }
            searchPos = closest.pos + 7;
        }
    }
    
    if (endIfPos === -1) {
        // Malformed, return empty
        return { node: { type: "literal", value: "" }, endPos: content.length };
    }
    
    // Extract then and else blocks
    const thenStart = conditionEnd + 2;
    const thenEnd = elsePos !== -1 ? elsePos : endIfPos;
    const thenContent = content.substring(thenStart, thenEnd);
    const thenBlock = parseTemplateContent(thenContent);
    
    let elseBlock: any[] = [];
    if (elsePos !== -1) {
        const elseStart = elsePos + 8; // {{else}}
        const elseContent = content.substring(elseStart, endIfPos);
        elseBlock = parseTemplateContent(elseContent);
    }
    
    return {
        node: {
            type: "inlineIf",
            condition,
            then: thenBlock,
            else: elseBlock
        },
        endPos: endIfPos + 7 // {{/if}}
    };
}

/**
 * Parse a condition string: "left op right"
 * Supports: ==, !=, >, <
 */
function parseCondition(condText: string): any {
    // Match: expression operator expression
    const operators = ['==', '!=', '>=', '<=', '>', '<'];
    
    for (const op of operators) {
        const opIndex = condText.indexOf(op);
        if (opIndex !== -1) {
            const left = condText.substring(0, opIndex).trim();
            const right = condText.substring(opIndex + op.length).trim();
            
            return {
                left: parseInterpolationExpression(left),
                operator: op,
                right: parseConditionValue(right)
            };
        }
    }
    
    // No operator found - this is an error, but return a falsy condition
    // We require explicit comparisons for cross-language compatibility
    console.warn(`Warning: Condition "${condText}" has no explicit operator. Use "value == true" instead.`);
    return {
        left: parseInterpolationExpression(condText),
        operator: "==",
        right: { type: "literal", value: true }
    };
}

/**
 * Parse a condition value (right side of comparison)
 */
function parseConditionValue(value: string): any {
    // String literal
    if ((value.startsWith('"') && value.endsWith('"')) || 
        (value.startsWith("'") && value.endsWith("'"))) {
        return { type: "literal", value: value.slice(1, -1) };
    }
    
    // Boolean
    if (value === 'true') return { type: "literal", value: true };
    if (value === 'false') return { type: "literal", value: false };
    
    // Number
    if (!isNaN(Number(value))) {
        return { type: "literal", value: Number(value) };
    }
    
    // Variable reference
    return parseInterpolationExpression(value);
}

/**
 * Parse {{@schema(path)}} directive
 * Path can be: output, output.property, input, context, types.TypeName
 */
function parseSchemaDirective(content: string, startPos: number): { node: any, endPos: number } {
    const directiveStart = startPos + 2; // {{
    const closePos = content.indexOf('}}', directiveStart);
    
    if (closePos === -1) {
        return { node: { type: "literal", value: "" }, endPos: content.length };
    }
    
    // Extract: @schema(path)
    const directive = content.substring(directiveStart, closePos).trim();
    const match = directive.match(/^@schema\(([^)]+)\)$/);
    
    if (!match) {
        return { node: { type: "literal", value: "" }, endPos: closePos + 2 };
    }
    
    const schemaPath = match[1].trim();
    
    return {
        node: {
            type: "schemaDirective",
            path: schemaPath
        },
        endPos: closePos + 2
    };
}

/**
 * Parse an interpolation expression from {{...}}
 * Handles: variable refs, member access, simple literals
 */
function parseInterpolationExpression(expr: string): any {
    // Handle string literals
    if ((expr.startsWith('"') && expr.endsWith('"')) || 
        (expr.startsWith("'") && expr.endsWith("'"))) {
        return { type: "literal", value: expr.slice(1, -1) };
    }
    
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

