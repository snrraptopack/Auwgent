import type { Type } from './types.js';
import { tArray, tConst, tError, tRecord } from './types.js';
import { TypeEnv } from './env.js';
import { unifyTypes, UnificationError } from './unification.js';
import type {
    Model,
    WorkFlowConfig,
    Statement,
    IfStatement,
    ReturnStatement,
    Expression,
    Types,
    BaseType,
    ObjectType,
    PropertyType,
    TypeDeclaration,
    VariableDeclaration,
    Agent,
    Helper,
    TypeConfigDeclaration,
    NamedPrompt,
    ModelConfig,
    PromptStatement,
    Condition,
    Output,
    TestConfig,
    ToolFunction
} from '../generated/ast.js';

import {
    isArrayLiteral,
    isBooleanLiteral,
    isNumberLiteral,
    isObjectLiteral,
    isStringLiteral,
    isMultilineStringLiteral,
    isVariableRef,
    isMemberAccess,
    isContextReference,
    isFunctionCall,
    isPromptCall,
    isBinaryExpression,
    isReturnStatement,
    isIfStatement,
    isArrayType,
    isBaseType,
    isObjectType,
    isUnionType,
    isNamedPrompt,
    isToolFunction,
    isWorkFlowConfig,
    isAgent,
    isHelper,
    isInputConfig,
    isContextConfig,
    isComparison,
    isLogicalCondition,
    isBooleanCondition,
    isOutputConfig,
    isHelperCall,
    isToolConfig,
    isToolsConfig,
    isIndexAccess
} from '../generated/ast.js';

export type TypeIssue = {
    message: string;
    node: { $type: string };
    property?: string;
};

export class TypeChecker {
    constructor(model: Model) {
        void model;
    }

    public buildEnvForNode(node: any): TypeEnv {
        let container: Agent | Helper | undefined;
        let current = node;
        while (current) {
            if (isAgent(current) || isHelper(current)) {
                container = current;
                break;
            }
            if (isNamedPrompt(current)) {
                // If we hit a named prompt, build its param scope
                const env = new TypeEnv();
                for (const param of current.params ?? []) {
                    env.set(param.name, { vars: [], type: this.mapTypes(param.t) });
                }
                return env; // Named prompts only have access to their params
            }
            current = current.$container;
        }

        return this.buildEnvForContainer(container);
    }

    public inferPathType(path: string[], env: TypeEnv): Type {
        if (!path.length) return tError('empty path');

        const rootName = path[0];
        const scheme = env.get(rootName);
        if (!scheme) return tError('unknown variable');

        let current = scheme.type;
        for (let i = 1; i < path.length; i++) {
            const segment = path[i];
            if (current.kind === 'record' && current.fields[segment]) {
                current = current.fields[segment];
            } else {
                return tError(`property '${segment}' does not exist on type ${this.formatType(current)}`);
            }
        }
        return current;
    }

    checkWorkflow(workflow: WorkFlowConfig): TypeIssue[] {
        const issues: TypeIssue[] = [];
        const container = workflow.$container;
        const env = this.buildEnvForContainer(isAgent(container) || isHelper(container) ? container : undefined);
        const expectedType = this.mapTypes(workflow.return);
        if (container && isHelper(container)) {
            const outputType = this.buildOutputType(container);
            if (outputType) {
                try {
                    unifyTypes(outputType, expectedType, {});
                } catch (error) {
                    const message = error instanceof UnificationError ? error.message : 'Output type mismatch';
                    issues.push({ message: `Workflow return type does not satisfy output config: ${this.formatType(outputType)} vs ${this.formatType(expectedType)} (${message})`, node: workflow, property: 'return' });
                }
            }
        }
        for (const param of workflow.params ?? []) {
            env.set(param.name, { vars: [], type: this.mapTypes(param.t) });
        }
        this.checkStatements(workflow.body, env, expectedType, issues);
        return issues;
    }

    checkTestConfig(testConfig: TestConfig, container?: Agent | Helper): TypeIssue[] {
        const issues: TypeIssue[] = [];
        const target = container ?? testConfig.$container;
        if (!target) return issues;
        const env = this.buildEnvForContainer(target);
        const tools = this.collectToolFunctions(target);
        for (const stub of testConfig.toolStubs ?? []) {
            const tool = tools.get(stub.name);
            if (!tool) {
                issues.push({ message: `Unknown tool '${stub.name}'`, node: stub, property: 'name' });
                continue;
            }
            if (stub.value) {
                const valueType = this.inferExpression(stub.value, env, issues);
                const returnType = this.mapTypes(tool.returns);
                try {
                    unifyTypes(returnType, valueType, {});
                } catch (error) {
                    const message = error instanceof UnificationError ? error.message : 'Return type mismatch';
                    issues.push({ message: `Tool stub return type mismatch: ${this.formatType(valueType)} vs ${this.formatType(returnType)} (${message})`, node: stub, property: 'value' });
                }
            }
        }
        return issues;
    }

    checkPrompt(prompt: NamedPrompt): TypeIssue[] {
        const issues: TypeIssue[] = [];
        const env = new TypeEnv();
        for (const param of prompt.params ?? []) {
            env.set(param.name, { vars: [], type: this.mapTypes(param.t) });
        }
        this.checkPromptStatements(prompt.parts, env, issues);
        return issues;
    }

    checkModelConfig(modelConfig: ModelConfig, container?: Agent | Helper): TypeIssue[] {
        const issues: TypeIssue[] = [];
        const env = this.buildEnvForContainer(container);
        if (modelConfig.promptExpr) {
            this.inferExpression(modelConfig.promptExpr, env, issues);
        }
        if (modelConfig.parts?.length) {
            this.checkPromptStatements(modelConfig.parts, env, issues);
        }
        return issues;
    }

    private checkStatements(statements: Statement[], env: TypeEnv, expectedType: Type, issues: TypeIssue[]): void {
        for (const statement of statements) {
            if ((statement as VariableDeclaration).$type === 'VariableDeclaration') {
                const decl = statement as VariableDeclaration;
                const valueType = this.inferExpression(decl.value, env, issues);

                // Use declared type annotation if present, otherwise infer from value
                let varType: Type;
                if (decl.varType) {
                    varType = this.mapTypes(decl.varType);
                    // Check that value is compatible with declared type (unless it's an empty array)
                    const isEmptyArray = valueType.kind === 'array' && valueType.element.kind === 'error';
                    if (!isEmptyArray) {
                        try {
                            unifyTypes(varType, valueType, {});
                        } catch (error) {
                            const message = error instanceof UnificationError ? error.message : 'Type mismatch';
                            issues.push({ message: `Variable type mismatch: declared ${this.formatType(varType)} but got ${this.formatType(valueType)} (${message})`, node: decl, property: 'value' });
                        }
                    }
                } else {
                    varType = valueType;
                }

                env.set(decl.name, { vars: [], type: varType });
                continue;
            }
            if (isReturnStatement(statement)) {
                this.checkReturnStatement(statement, env, expectedType, issues);
                continue;
            }
            if (isIfStatement(statement)) {
                this.checkIfStatement(statement, env, expectedType, issues);
            }
        }
    }

    private checkIfStatement(statement: IfStatement, env: TypeEnv, expectedType: Type, issues: TypeIssue[]): void {
        this.checkCondition(statement.condition, env, issues);
        this.checkStatements(statement.thenBlock, env.extend(), expectedType, issues);
        if (statement.elseBlock) {
            this.checkStatements(statement.elseBlock, env.extend(), expectedType, issues);
        }
    }

    private checkReturnStatement(statement: ReturnStatement, env: TypeEnv, expectedType: Type, issues: TypeIssue[]): void {
        const actual = this.inferExpression(statement.value, env, issues);
        const missingFields = this.getMissingRequiredFields(expectedType, actual);
        if (missingFields.length) {
            issues.push({
                message: `Return type mismatch: Expected ${this.formatType(expectedType)} but got ${this.formatType(actual)} (Missing fields: ${missingFields.join(', ')})`,
                node: statement,
                property: 'value'
            });
            return;
        }
        try {
            unifyTypes(expectedType, actual, {});
        } catch (error) {
            const message = error instanceof UnificationError ? error.message : 'Return type mismatch';
            issues.push({ message: `Return type mismatch: Expected ${this.formatType(expectedType)} but got ${this.formatType(actual)} (${message})`, node: statement, property: 'value' });
        }
    }

    private inferExpression(node: Expression, env: TypeEnv, issues: TypeIssue[]): Type {
        if (isStringLiteral(node) || isMultilineStringLiteral(node)) {
            return tConst('string');
        }
        if (isNumberLiteral(node)) {
            return tConst('number');
        }
        if (isBooleanLiteral(node)) {
            return tConst('boolean');
        }
        if (isArrayLiteral(node)) {
            if (node.elements.length === 0) {
                return tArray(tError('unknown'));
            }
            const elementTypes = node.elements.map(el => this.inferExpression(el, env, issues));
            let current = elementTypes[0];
            for (let i = 1; i < elementTypes.length; i += 1) {
                try {
                    unifyTypes(current, elementTypes[i], {});
                } catch {
                    issues.push({ message: 'Array elements must have the same type', node, property: 'elements' });
                    return tArray(tError('mixed'));
                }
            }
            return tArray(current);
        }
        if (isObjectLiteral(node)) {
            const fields: Record<string, Type> = {};
            const optional: Record<string, boolean> = {};
            for (const prop of node.properties) {
                const valueType = prop.value ? this.inferExpression(prop.value, env, issues) : tError('unknown');
                fields[prop.name] = valueType;
                optional[prop.name] = false;
            }
            return tRecord(fields, optional);
        }
        if (isVariableRef(node)) {
            const ref = node.variable.ref;
            if (ref && (ref as any).$type === 'TypeConfigDeclaration') {
                return this.mapTypes((ref as any).t);
            }
            if (ref && isNamedPrompt(ref)) {
                return tConst('string');
            }
            if (ref && (ref as any).$type === 'VariableDeclaration') {
                const scheme = env.get((ref as any).name);
                if (scheme) {
                    return scheme.type;
                }
            }
            return tError('unknown');
        }
        if (isMemberAccess(node)) {
            const baseRef = node.object?.ref;
            let current = this.inferReferenceType(baseRef, env);
            const segments = [node.property, ...(node.chain ?? [])];
            for (const segment of segments) {
                if (current.kind === 'record' && current.fields[segment]) {
                    current = current.fields[segment];
                    continue;
                }
                issues.push({ message: `Unknown property '${segment}'`, node, property: 'property' });
                return tError('unknown');
            }
            return current;
        }
        if (isContextReference(node)) {
            const ref = node.property?.ref;
            if (ref) {
                return this.mapTypes(ref.t);
            }
            return tError('unknown');
        }
        if (isHelperCall(node)) {
            const helper = node.helper?.ref;
            if (helper) {
                const inputType = this.buildInputType(helper);
                if (inputType) {
                    if (node.args.length !== 1) {
                        issues.push({ message: `Helper call expects exactly 1 argument`, node, property: 'args' });
                    } else {
                        const argType = this.inferExpression(node.args[0], env, issues);
                        try {
                            unifyTypes(argType, inputType, {});
                        } catch (error) {
                            const message = error instanceof UnificationError ? error.message : 'Argument type mismatch';
                            issues.push({ message: `Helper argument type mismatch: ${this.formatType(argType)} vs ${this.formatType(inputType)} (${message})`, node, property: 'args' });
                        }
                    }
                } else if (node.args.length > 0) {
                    issues.push({ message: `Helper call expects no arguments`, node, property: 'args' });
                }
                const outputType = this.buildOutputType(helper);
                if (outputType) {
                    return outputType;
                }
            }
            return tError('unknown');
        }
        if (isFunctionCall(node)) {
            const ref = node.func?.ref;
            if (ref && isToolFunction(ref)) {
                this.checkCallArgs(node.args, ref.params, node, env, issues);
                return this.mapTypes(ref.returns);
            }
            if (ref && isNamedPrompt(ref)) {
                this.checkCallArgs(node.args, ref.params ?? [], node, env, issues);
                return tConst('string');
            }
            if (ref && isWorkFlowConfig(ref)) {
                // Workflow call - check args match workflow params
                this.checkCallArgs(node.args, ref.params ?? [], node, env, issues);
                return this.mapTypes(ref.return);
            }
            return tError('unknown');
        }
        if (isPromptCall(node)) {
            const ref = node.prompt?.ref;
            if (ref && isNamedPrompt(ref)) {
                this.checkCallArgs(node.args, ref.params ?? [], node, env, issues);
            }
            return tConst('string');
        }
        if (isBinaryExpression(node)) {
            const left = this.inferExpression(node.left, env, issues);
            const right = this.inferExpression(node.right, env, issues);
            const op = node.op;

            // String concatenation only for +
            if (op === '+') {
                if (left.kind === 'const' && left.name === 'string') {
                    return tConst('string');
                }
                if (right.kind === 'const' && right.name === 'string') {
                    return tConst('string');
                }
            }

            // All arithmetic ops require numbers
            if (left.kind === 'const' && left.name === 'number' && right.kind === 'const' && right.name === 'number') {
                return tConst('number');
            }

            // For +, allow string or number
            if (op === '+') {
                issues.push({ message: 'Operator + expects number or string operands', node, property: 'op' });
            } else {
                issues.push({ message: `Operator ${op} expects number operands`, node, property: 'op' });
            }
            return tError('invalid');
        }
        if (isIndexAccess(node)) {
            // Get the type of the array being indexed
            const arrayRef = node.object?.ref;
            const arrayType = this.inferReferenceType(arrayRef, env);

            // If it's an array type, get the element type
            if (arrayType.kind === 'array') {
                let elementType = arrayType.element;

                // If there's a property access after the index (e.g., findings[0].claim)
                if (node.property) {
                    if (elementType.kind === 'record' && elementType.fields[node.property]) {
                        elementType = elementType.fields[node.property];
                    } else {
                        issues.push({ message: `Unknown property '${node.property}' on array element`, node, property: 'property' });
                        return tError('unknown');
                    }
                }

                // If there's a chain of properties (e.g., findings[0].nested.field)
                if (node.chain && node.chain.length > 0) {
                    for (const segment of node.chain) {
                        if (elementType.kind === 'record' && elementType.fields[segment]) {
                            elementType = elementType.fields[segment];
                        } else {
                            issues.push({ message: `Unknown property '${segment}' in chain`, node, property: 'chain' });
                            return tError('unknown');
                        }
                    }
                }

                return elementType;
            }

            issues.push({ message: 'Index access requires an array type', node, property: 'object' });
            return tError('unknown');
        }
        return tError('unknown');
    }

    private checkPromptStatements(statements: PromptStatement[], env: TypeEnv, issues: TypeIssue[]): void {
        for (const statement of statements ?? []) {
            if (isIfStatement(statement)) {
                this.checkCondition(statement.condition, env, issues);
                this.checkStatementsLoose(statement.thenBlock, env.extend(), issues);
                if (statement.elseBlock) {
                    this.checkStatementsLoose(statement.elseBlock, env.extend(), issues);
                }
                continue;
            }
            this.inferExpression(statement as Expression, env, issues);
        }
    }

    private checkStatementsLoose(statements: Statement[], env: TypeEnv, issues: TypeIssue[]): void {
        for (const statement of statements) {
            if ((statement as VariableDeclaration).$type === 'VariableDeclaration') {
                const decl = statement as VariableDeclaration;
                const valueType = this.inferExpression(decl.value, env, issues);
                env.set(decl.name, { vars: [], type: valueType });
                continue;
            }
            if (isReturnStatement(statement)) {
                this.inferExpression(statement.value, env, issues);
                continue;
            }
            if (isIfStatement(statement)) {
                this.checkCondition(statement.condition, env, issues);
                this.checkStatementsLoose(statement.thenBlock, env.extend(), issues);
                if (statement.elseBlock) {
                    this.checkStatementsLoose(statement.elseBlock, env.extend(), issues);
                }
            }
        }
    }

    private checkCondition(condition: Condition, env: TypeEnv, issues: TypeIssue[]): void {
        if (isComparison(condition)) {
            // Simple comparison: left op right
            const left = this.inferExpression(condition.left, env, issues);
            const right = this.inferExpression(condition.right, env, issues);
            try {
                unifyTypes(left, right, {});
            } catch (error) {
                const message = error instanceof UnificationError ? error.message : 'Condition type mismatch';
                issues.push({ message: `Condition type mismatch: ${this.formatType(left)} vs ${this.formatType(right)} (${message})`, node: condition, property: 'op' });
            }
        } else if (isLogicalCondition(condition)) {
            // Logical condition: left && right or left || right
            this.checkCondition(condition.left, env, issues);
            this.checkCondition(condition.right, env, issues);
        } else if (isBooleanCondition(condition)) {
            // Bare boolean expression: if (hasValue) {}
            const exprType = this.inferExpression(condition.value, env, issues);
            if (exprType.kind !== 'const' || exprType.name !== 'boolean') {
                issues.push({ message: `Condition expects boolean but got ${this.formatType(exprType)}`, node: condition, property: 'value' });
            }
        }
    }

    private inferReferenceType(ref: any, env: TypeEnv): Type {
        if (!ref) return tError('unknown');
        if (ref.$type === 'TypeConfigDeclaration') {
            // This handles both:
            // 1. Direct type config references (like type aliases)
            // 2. Workflow/helper parameters (which are TypeConfigDeclaration)
            // First check if it's in the env (as a parameter)
            const scheme = env.get(ref.name);
            if (scheme) {
                return scheme.type;
            }
            // Otherwise map the type directly
            return this.mapTypes(ref.t);
        }
        if (ref.$type === 'VariableDeclaration') {
            const scheme = env.get(ref.name);
            return scheme ? scheme.type : tError('unknown');
        }
        if (ref.$type === 'NamedPrompt') {
            return tConst('string');
        }
        return tError('unknown');
    }

    private mapTypes(node: Types): Type {
        if (isArrayType(node)) {
            return tArray(this.mapBaseType(node.elementType));
        }
        if (isBaseType(node)) {
            return this.mapBaseType(node);
        }
        return tError('unknown');
    }

    private mapBaseType(node: BaseType): Type {
        if (node.typeRef?.ref) {
            return this.mapTypeDeclaration(node.typeRef.ref);
        }
        const concrete = node.type;
        if (!concrete) return tError('unknown');
        if ((concrete as any).$type === 'StringType') return tConst('string');
        if ((concrete as any).$type === 'NumberType') return tConst('number');
        if ((concrete as any).$type === 'BooleanType') return tConst('boolean');
        if (isObjectType(concrete)) return this.mapObjectType(concrete);
        if (isUnionType(concrete)) return tConst('string');
        return tError('unknown');
    }

    private mapObjectType(node: ObjectType): Type {
        const fields: Record<string, Type> = {};
        const optional: Record<string, boolean> = {};
        for (const prop of node.properties as PropertyType[]) {
            fields[prop.name] = this.mapTypes(prop.type);
            optional[prop.name] = prop.isOptional ?? false;
        }
        return tRecord(fields, optional);
    }

    private mapTypeDeclaration(node: TypeDeclaration): Type {
        const fields: Record<string, Type> = {};
        const optional: Record<string, boolean> = {};
        for (const prop of node.types) {
            fields[prop.name] = this.mapTypes(prop.t);
            optional[prop.name] = prop.isOptional ?? false;
        }
        return tRecord(fields, optional);
    }

    private formatType(type: Type): string {
        if (type.kind === 'const') return type.name;
        if (type.kind === 'array') return `${this.formatType(type.element)}[]`;
        if (type.kind === 'record') {
            const fields = Object.entries(type.fields).map(([name, fieldType]) => {
                const optional = type.optional?.[name] ? '?' : '';
                return `${name}${optional}: ${this.formatType(fieldType)}`;
            });
            return fields.length ? `{ ${fields.join(', ')} }` : '{}';
        }
        if (type.kind === 'union') return type.options.map(t => this.formatType(t)).join(' | ');
        if (type.kind === 'func') return `(${type.params.map(t => this.formatType(t)).join(', ')}) -> ${this.formatType(type.returns)}`;
        if (type.kind === 'var') return type.id;
        return type.kind;
    }

    private getMissingRequiredFields(expected: Type, actual: Type): string[] {
        if (expected.kind !== 'record' || actual.kind !== 'record') return [];
        const missing: string[] = [];
        for (const key of Object.keys(expected.fields)) {
            const isOptional = expected.optional?.[key] ?? false;
            if (!isOptional && !actual.fields[key]) {
                missing.push(key);
            }
        }
        return missing;
    }

    private buildEnvForContainer(container?: Agent | Helper): TypeEnv {
        const env = new TypeEnv();
        if (!container) return env;
        for (const config of container.configs) {
            if (isInputConfig(config)) {
                this.addTypeConfigToEnv(config.inProperties, env);
            }
            if (isContextConfig(config)) {
                this.addTypeConfigToEnv(config.contextProperties, env);
            }
        }
        return env;
    }

    private buildOutputType(container: Agent | Helper): Type | undefined {
        for (const config of container.configs) {
            if (isOutputConfig(config)) {
                if (config.directType) {
                    return this.mapTypes(config.directType);
                }
                if (config.outProperties?.length) {
                    return this.mapOutputProperties(config.outProperties);
                }
            }
        }
        return undefined;
    }

    private buildInputType(container: Agent | Helper): Type | undefined {
        for (const config of container.configs) {
            if (isInputConfig(config)) {
                if (config.inProperties?.length) {
                    return this.mapInputProperties(config.inProperties);
                }
            }
        }
        return undefined;
    }

    private mapOutputProperties(properties: Output[]): Type {
        const fields: Record<string, Type> = {};
        const optional: Record<string, boolean> = {};
        for (const prop of properties) {
            const decl = prop.td;
            fields[decl.name] = this.mapTypes(decl.t);
            optional[decl.name] = decl.isOptional ?? false;
        }
        return tRecord(fields, optional);
    }

    private mapInputProperties(properties: TypeConfigDeclaration[]): Type {
        const fields: Record<string, Type> = {};
        const optional: Record<string, boolean> = {};
        for (const prop of properties) {
            fields[prop.name] = this.mapTypes(prop.t);
            optional[prop.name] = prop.isOptional ?? false;
        }
        return tRecord(fields, optional);
    }

    private collectToolFunctions(container: Agent | Helper): Map<string, ToolFunction> {
        const tools = new Map<string, ToolFunction>();
        for (const config of container.configs) {
            if (isToolConfig(config)) {
                tools.set(config.tool.name, config.tool);
            }
            if (isToolsConfig(config)) {
                for (const tool of config.tools ?? []) {
                    tools.set(tool.name, tool);
                }
            }
        }
        return tools;
    }

    private addTypeConfigToEnv(properties: TypeConfigDeclaration[], env: TypeEnv): void {
        for (const prop of properties ?? []) {
            env.set(prop.name, { vars: [], type: this.mapTypes(prop.t) });
        }
    }

    private checkCallArgs(args: Expression[], params: TypeConfigDeclaration[], node: { $type: string }, env: TypeEnv, issues: TypeIssue[]): void {
        if (args.length !== params.length) {
            issues.push({ message: `Argument count mismatch: expected ${params.length} but got ${args.length}`, node, property: 'args' });
            return;
        }
        for (let i = 0; i < args.length; i += 1) {
            const argType = this.inferExpression(args[i], env, issues);
            const paramType = this.mapTypes(params[i].t);
            try {
                unifyTypes(argType, paramType, {});
            } catch (error) {
                const message = error instanceof UnificationError ? error.message : 'Argument type mismatch';
                issues.push({ message: `Argument ${i + 1} type mismatch: ${this.formatType(argType)} vs ${this.formatType(paramType)} (${message})`, node, property: 'args' });
            }
        }
    }
}
