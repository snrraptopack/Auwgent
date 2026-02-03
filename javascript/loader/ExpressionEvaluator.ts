import type { AgentIR, Expression, Statement, HelperIR } from "./types/ir";
import type { ToolMap } from "./types/tool";
import type { StreamChunk } from "./types/protocol";
import { logger } from "./Logger";

export type HelperExecutor = (helper: HelperIR, args: Record<string, any>) => Promise<any>;
export type StreamingHelperExecutor = (helper: HelperIR, args: Record<string, any>) => AsyncGenerator<StreamChunk, any, unknown>;

/**
 * Schema context for resolving {{@schema(path)}} directives.
 * This is scoped to the current agent/helper being evaluated.
 */
export interface SchemaContext {
    output?: Record<string, any>;
    input?: Record<string, any>;
    context?: Record<string, any>;
    types?: Record<string, any>;
}

export class ExpressionEvaluator {
    private schemaContext?: SchemaContext;

    constructor(
        private ir?: AgentIR,
        private tools?: ToolMap,
        private helperExecutor?: HelperExecutor,
        private streamingHelperExecutor?: StreamingHelperExecutor,
        private toolDefinitions?: AgentIR["tools"]
    ) { }

    /**
     * Set schema context for {{@schema()}} directive resolution.
     * Call this before evaluating prompts that may contain schema directives.
     */
    setSchemaContext(ctx: SchemaContext): void {
        this.schemaContext = ctx;
    }

    /**
     * Evaluate any expression/statement and return the result
     */
    async evaluate(expr: Expression | Statement, scope: Map<string, any>): Promise<any> {
        if (!expr) return null;

        switch (expr.type) {
            case "literal":
                return expr.value;

            case "varRef":
                if (!scope.has(expr.value)) {
                    throw new Error(`Variable not found: ${expr.value}`);
                }
                return scope.get(expr.value);

            case "contextRef":
                return scope.get(expr.property)

            case "object":
                return this.evaluateObject(expr.value, scope);

            case "array":
                return this.evaluateArray(expr.value, scope);

            case "functionCall":
                return this.evaluateFunctionCall(expr, scope);

            case "workflowCall":
                return this.evaluateWorkflowCall(expr, scope);

            case "helperCall":
                return this.evaluateHelperCall(expr, scope);

            case "if":
                return this.evaluateIf(expr, scope);

            case "parallel":
                return this.evaluateParallel(expr, scope);


            case "variableDeclaration":
                const value = await this.evaluate(expr.value, scope);
                scope.set(expr.name, value);
                return undefined;

            case "assignment":
                const assignValue = await this.evaluate(expr.value, scope);
                if (!scope.has(expr.name)) {
                    throw new Error(`Cannot assign to undeclared variable: ${expr.name}`);
                }
                scope.set(expr.name, assignValue);
                return undefined;

            case "indexAccess":
                return this.evaluateIndexAccess(expr, scope);

            case "return":
                const returnValue = await this.evaluate(expr.value, scope);
                return { __type: "ReturnSignal", value: returnValue };

            case "transfer":
                return this.evaluateTransfer(expr, scope);

            case "memberAccess":
                return this.evaluateMemberAccess(expr, scope);

            case "concat": {
                // Legacy concat - always addition/concatenation
                const left = await this.evaluate(expr.left, scope);
                const right = await this.evaluate(expr.right, scope);
                return (left ?? "") + (right ?? "");
            }

            case "binaryOp": {
                const left = await this.evaluate(expr.left, scope);
                const right = await this.evaluate(expr.right, scope);
                const op = expr.op;
                switch (op) {
                    case '+': return (left ?? "") + (right ?? "");
                    case '-': return (left ?? 0) - (right ?? 0);
                    case '*': return (left ?? 0) * (right ?? 0);
                    case '/': return right !== 0 ? (left ?? 0) / right : 0;
                    default: return (left ?? "") + (right ?? "");
                }
            }

            case "comparison": {
                const left = await this.evaluate(expr.left, scope);
                const right = await this.evaluate(expr.right, scope);
                return this.compare(left, expr.operator, right);
            }

            case "template":
                return this.evaluateTemplate(expr, scope);

            case "expression":
                return this.evaluate(expr.value, scope);

            default:
                throw new Error(`Unknown expression type: ${expr.type}`);
        }
    }

    /**
     * STREAMING VERSION: Evaluate expression/statement, yielding chunks for helper calls.
    * For expressions that don't stream (literals, etc.), returns value via generator return.
     * For helperCall/transfer, yields chunks from the streaming helper executor.
     */
    async *evaluateStream(
        expr: Expression | Statement,
        scope: Map<string, any>
    ): AsyncGenerator<StreamChunk, any, unknown> {
        if (!expr) return null;

        switch (expr.type) {
            // Streaming-aware types: yield chunks from helper streams
            case "helperCall":
                return yield* this.evaluateHelperCallStream(expr, scope);

            case "transfer":
                return yield* this.evaluateTransferStream(expr, scope);

            case "if":
                return yield* this.evaluateIfStream(expr, scope);

            case "parallel":
                return await this.evaluateParallel(expr, scope);

            case "variableDeclaration": {
                // Variable declarations might contain streaming helper calls
                const valueGen = this.evaluateStream(expr.value, scope);
                let value: any;
                while (true) {
                    const { value: chunk, done } = await valueGen.next();
                    if (done) {
                        value = chunk;
                        break;
                    }
                    yield chunk;
                }
                scope.set(expr.name, value);
                return undefined;
            }

            case "assignment": {
                // Assignment statements might contain streaming helper calls
                const valueGen = this.evaluateStream(expr.value, scope);
                let assignValue: any;
                while (true) {
                    const { value: chunk, done } = await valueGen.next();
                    if (done) {
                        assignValue = chunk;
                        break;
                    }
                    yield chunk;
                }
                if (!scope.has(expr.name)) {
                    throw new Error(`Cannot assign to undeclared variable: ${expr.name}`);
                }
                scope.set(expr.name, assignValue);
                return undefined;
            }

            case "return": {
                // Return might contain streaming helper call
                const returnGen = this.evaluateStream(expr.value, scope);
                let returnValue: any;
                while (true) {
                    const { value: chunk, done } = await returnGen.next();
                    if (done) {
                        returnValue = chunk;
                        break;
                    }
                    yield chunk;
                }
                return { __type: "ReturnSignal", value: returnValue };
            }

            // Non-streaming types: delegate to regular evaluate()
            default:
                return await this.evaluate(expr, scope);
        }
    }

    /**
     * STREAMING: Evaluate helper call - yields chunks from helper stream
     */
    private async *evaluateHelperCallStream(
        expr: any,
        scope: Map<string, any>
    ): AsyncGenerator<StreamChunk, any, unknown> {
        const helperName = expr.value;
        const args = expr.args || [];

        const helper = this.ir?.helpers?.find(h => h.name === helperName);
        if (!helper) {
            throw new Error(`Helper not found: ${helperName}`);
        }

        if (args.length === 1) {
            const resolvedArgs = await this.evaluate(args[0], scope);

            if (this.streamingHelperExecutor) {
                logger.debug(`[Workflow] Calling helper (streaming): ${helperName}`);
                logger.trackHelperCall(helperName);
                const stream = this.streamingHelperExecutor(helper, resolvedArgs);
                let result: any;

                while (true) {
                    const { value, done } = await stream.next();
                    if (done) {
                        result = value;
                        break;
                    }
                    yield value;  // Yield immediately!
                }
                return result;
            }

            // Fallback to non-streaming
            if (!this.helperExecutor) {
                throw new Error(`No helper executor provided for: ${helperName}`);
            }
            logger.debug(`[Workflow] Calling helper (fallback): ${helperName}`);
            logger.trackHelperCall(helperName);
            return await this.helperExecutor(helper, resolvedArgs);
        }

        throw new Error(`Helper ${helperName} expects exactly 1 object argument`);
    }

    /**
     * STREAMING: Evaluate transfer - yields chunks and returns TransferSignal
     */
    private async *evaluateTransferStream(
        expr: any,
        scope: Map<string, any>
    ): AsyncGenerator<StreamChunk, any, unknown> {
        const target = expr.target;
        const helperName = target.value;
        const args = target.args || [];
        const mode = expr.mode;

        const helper = this.ir?.helpers?.find(h => h.name === helperName);
        if (!helper) {
            throw new Error(`Helper not found for transfer: ${helperName}`);
        }

        if (args.length === 1) {
            const resolvedArgs = await this.evaluate(args[0], scope);
            let result: any;

            if (this.streamingHelperExecutor) {
                logger.debug(`[Workflow] Transfer to helper (streaming): ${helperName} (mode: ${mode})`);
                logger.trackHelperCall(helperName);
                const stream = this.streamingHelperExecutor(helper, resolvedArgs);

                while (true) {
                    const { value, done } = await stream.next();
                    if (done) {
                        result = value;
                        break;
                    }
                    yield value;  // Yield immediately!
                }
            } else {
                if (!this.helperExecutor) {
                    throw new Error(`No helper executor provided for transfer to: ${helperName}`);
                }
                logger.debug(`[Workflow] Transfer to helper (fallback): ${helperName} (mode: ${mode})`);
                logger.trackHelperCall(helperName);
                result = await this.helperExecutor(helper, resolvedArgs);
            }

            return {
                __type: "TransferSignal",
                value: result,
                mode: mode,
                helperName: helperName
            };
        }

        throw new Error(`Transfer to ${helperName} expects exactly 1 object argument`);
    }

    /**
     * STREAMING: Evaluate if statement - yields chunks from streaming blocks
     */
    private async *evaluateIfStream(
        expr: any,
        scope: Map<string, any>
    ): AsyncGenerator<StreamChunk, any, unknown> {
        const conditionMet = await this.evaluateCondition(expr.condition, scope);

        const block = conditionMet ? expr.then : expr.else;
        if (block && block.length > 0) {
            for (const stmt of block) {
                const stmtGen = this.evaluateStream(stmt, scope);
                let result: any;

                while (true) {
                    const { value: chunk, done } = await stmtGen.next();
                    if (done) {
                        result = chunk;
                        break;
                    }
                    yield chunk;
                }

                // Propagate signals
                if (result && typeof result === "object" &&
                    (result.__type === "ReturnSignal" || result.__type === "TransferSignal")) {
                    return result;
                }
            }
        }

        return undefined;
    }

    /**
     * Evaluate object literal
     */
    private async evaluateObject(properties: Record<string, any>, scope: Map<string, any>): Promise<Record<string, any>> {
        const result: Record<string, any> = {};

        for (const [key, valueExpr] of Object.entries(properties)) {
            result[key] = await this.evaluate(valueExpr, scope);
        }

        return result;
    }

    /**
     * Evaluate member access (e.g., design.propose)
     * Traverses the object and returns the nested property value
     */
    private async evaluateMemberAccess(expr: any, scope: Map<string, any>): Promise<any> {
        // First, get the base object
        const baseObject = await this.evaluate(expr.object, scope);

        if (baseObject === null || baseObject === undefined) {
            throw new Error(`Cannot access property '${expr.properties[0]}' of ${baseObject}`);
        }

        // Traverse through the property chain
        let current = baseObject;
        for (const prop of expr.properties) {
            if (current === null || current === undefined) {
                throw new Error(`Cannot access property '${prop}' of ${current}`);
            }
            current = current[prop];
        }

        return current;
    }

    /**
     * Evaluate index access (e.g., results[0] or results[0].field)
     * Supports array indexing with optional property chain
     */
    private async evaluateIndexAccess(expr: any, scope: Map<string, any>): Promise<any> {
        // Get the base object from the variable name
        const objectName = expr.object;
        if (!scope.has(objectName)) {
            throw new Error(`Variable not found for index access: ${objectName}`);
        }
        const baseObject = scope.get(objectName);

        if (baseObject === null || baseObject === undefined) {
            throw new Error(`Cannot index into ${baseObject}`);
        }

        // Evaluate the index expression
        const index = await this.evaluate(expr.index, scope);
        
        // Access the indexed element
        let current = baseObject[index];

        // If there's a property, access it
        if (expr.property && current !== null && current !== undefined) {
            current = current[expr.property];
        }

        // If there's a chain of properties, traverse them
        if (expr.chain && expr.chain.length > 0 && current !== null && current !== undefined) {
            for (const prop of expr.chain) {
                if (current === null || current === undefined) {
                    throw new Error(`Cannot access property '${prop}' of ${current}`);
                }
                current = current[prop];
            }
        }

        return current;
    }

    /**
     * Evaluate array literal
     */
    private async evaluateArray(elements: any[], scope: Map<string, any>): Promise<any[]> {
        const result: any[] = [];

        for (const elem of elements) {
            result.push(await this.evaluate(elem, scope));
        }

        return result;
    }

    /**
     * Evaluate function call (tool invocation)
     */
    private async evaluateFunctionCall(expr: any, scope: Map<string, any>): Promise<any> {
        const funcName = expr.value;
        const args = expr.args || [];

        // Find tool definition to map positional args to named params
        const availableTools = this.toolDefinitions ?? this.ir?.tools ?? [];
        const toolDef = availableTools.find(t => t.name === funcName);
        if (!toolDef) {
            throw new Error(`Tool not found: ${funcName}`);
        }

        const paramNames = Object.keys(toolDef.params);
        const resolvedArgs: Record<string, any> = {};

        for (let i = 0; i < args.length; i++) {
            const argValue = await this.evaluate(args[i], scope);
            const paramName = paramNames[i];
            if (paramName) {
                resolvedArgs[paramName] = argValue;
            }
        }

        // Execute tool with graceful error handling
        if (!this.tools || !this.tools[funcName]) {
            return {
                __toolError: true,
                tool: funcName,
                message: `Tool implementation missing for: ${funcName}`
            };
        }

        try {
            return await this.tools[funcName](resolvedArgs);
        } catch (e: any) {
            // Don't throw — return error as data so workflow can continue
            console.warn(`[Tool Error] ${funcName}: ${e.message}`);
            return {
                __toolError: true,
                tool: funcName,
                args: resolvedArgs,
                message: e.message
            };
        }
    }

    /**
     * Evaluate workflow call (internal workflow-to-workflow call)
     * This allows workflows to call other workflows within the same agent/helper.
     */
    private async evaluateWorkflowCall(expr: any, scope: Map<string, any>): Promise<any> {
        const workflowName = expr.value;
        const args = expr.args || [];

        // Find workflow definition
        const workflow = this.ir?.workflows?.find(w => w.flowName === workflowName);
        if (!workflow) {
            throw new Error(`Workflow not found: ${workflowName}`);
        }

        // Create a new scope for the workflow execution
        const workflowScope = new Map<string, any>();

        // Map positional arguments to workflow parameters
        const paramNames = Object.keys(workflow.flowParams);
        for (let i = 0; i < args.length; i++) {
            const argValue = await this.evaluate(args[i], scope);
            const paramName = paramNames[i];
            if (paramName) {
                workflowScope.set(paramName, argValue);
            }
        }

        // Execute workflow body statements
        for (const stmt of workflow.body) {
            const result = await this.evaluate(stmt, workflowScope);
            
            // Handle return signal
            if (result && typeof result === "object" && result.__type === "ReturnSignal") {
                return result.value;
            }
            // Handle transfer signal - propagate it up
            if (result && typeof result === "object" && result.__type === "TransferSignal") {
                return result;
            }
        }

        return undefined;
    }

    /**
     * Evaluate helper call (delegate to helper agent) - NON-STREAMING
     * For streaming, use evaluateStream() instead.
     */
    private async evaluateHelperCall(expr: any, scope: Map<string, any>): Promise<any> {
        const helperName = expr.value;
        const args = expr.args || [];

        // Find helper definition
        const helper = this.ir?.helpers?.find(h => h.name === helperName);
        if (!helper) {
            throw new Error(`Helper not found: ${helperName}`);
        }

        // Resolve args - for now we expect a single object arg
        if (args.length === 1) {
            const resolvedArgs = await this.evaluate(args[0], scope);

            if (!this.helperExecutor) {
                throw new Error(`No helper executor provided for: ${helperName}`);
            }

            logger.debug(`[Workflow] Calling helper: ${helperName}`);
            logger.trackHelperCall(helperName);
            return await this.helperExecutor(helper, resolvedArgs);
        }

        throw new Error(`Helper ${helperName} expects exactly 1 object argument`);
    }

    /**
     * Evaluate if statement
     */
    private async evaluateIf(expr: any, scope: Map<string, any>): Promise<any> {
        const conditionMet = await this.evaluateCondition(expr.condition, scope);

        const block = conditionMet ? expr.then : expr.else;
        if (block && block.length > 0) {
            for (const stmt of block) {
                const result = await this.evaluate(stmt, scope);
                // Propagate return signal
                if (result && typeof result === "object" && result.__type === "ReturnSignal") {
                    return result;
                }
                // Propagate transfer signal
                if (result && typeof result === "object" && result.__type === "TransferSignal") {
                    return result;
                }
            }
        }

        return undefined;
    }

    /**
     * Evaluate a condition (supports simple comparison, logical operators, and bare booleans)
     */
    private async evaluateCondition(condition: any, scope: Map<string, any>): Promise<boolean> {
        // Handle logical conditions (&&, ||)
        if (condition.type === "logical") {
            const left = await this.evaluateCondition(condition.left, scope);
            if (condition.operator === "||") {
                // Short-circuit: if left is true, return true immediately
                if (left) return true;
                return await this.evaluateCondition(condition.right, scope);
            } else if (condition.operator === "&&") {
                // Short-circuit: if left is false, return false immediately
                if (!left) return false;
                return await this.evaluateCondition(condition.right, scope);
            }
        }

        // Handle bare boolean expression: if (hasValue) {}
        if (condition.type === "boolean") {
            const value = await this.evaluate(condition.value, scope);
            return Boolean(value);
        }

        // Handle comparison conditions (==, !=, >, <, >=, <=)
        if (condition.type === "comparison" || condition.left !== undefined) {
            const left = await this.evaluate(condition.left, scope);
            const right = await this.evaluate(condition.right, scope);
            return this.compare(left, condition.operator, right);
        }

        throw new Error(`Unknown condition type: ${condition.type}`);
    }

    private async evaluateParallel(expr: any, scope: Map<string, any>): Promise<any> {
        const baseScope = new Map(scope);
        const updates = new Map<string, any>();

        await Promise.all(expr.body.map(async (stmt: any) => {
            const localScope = new Map(baseScope);
            const result = await this.evaluate(stmt, localScope);

            if (result && typeof result === "object") {
                if (result.__type === "ReturnSignal") {
                    throw new Error("Return is not supported inside parallel blocks");
                }
                if (result.__type === "TransferSignal") {
                    throw new Error("Transfer is not supported inside parallel blocks");
                }
            }

            for (const [key, value] of localScope.entries()) {
                const baseValue = baseScope.get(key);
                const changed = !baseScope.has(key) || !Object.is(baseValue, value);
                if (changed) {
                    if (updates.has(key)) {
                        throw new Error(`Parallel block writes to "${key}" more than once`);
                    }
                    updates.set(key, value);
                }
            }

            return result;
        }));

        for (const [key, value] of updates.entries()) {
            scope.set(key, value);
        }

        return undefined;
    }

    /**
     * Compare two values with an operator
     */
    private compare(left: any, operator: string, right: any): boolean {
        switch (operator) {
            case "==": return left == right;
            case "!=": return left != right;
            case ">": return left > right;
            case "<": return left < right;
            case ">=": return left >= right;
            case "<=": return left <= right;
            default: return false;
        }
    }

    /**
     * Evaluate transfer statement - NON-STREAMING
     * For streaming, use evaluateStream() instead.
     */
    private async evaluateTransfer(expr: any, scope: Map<string, any>): Promise<any> {
        const target = expr.target;
        const helperName = target.value;
        const args = target.args || [];
        const mode = expr.mode; // "direct" or "thenContinue"

        // Find helper definition
        const helper = this.ir?.helpers?.find(h => h.name === helperName);
        if (!helper) {
            throw new Error(`Helper not found for transfer: ${helperName}`);
        }

        // Resolve args - expect a single object arg
        if (args.length === 1) {
            const resolvedArgs = await this.evaluate(args[0], scope);

            if (!this.helperExecutor) {
                throw new Error(`No helper executor provided for transfer to: ${helperName}`);
            }

            logger.debug(`[Workflow] Transfer to helper: ${helperName} (mode: ${mode})`);
            logger.trackHelperCall(helperName);
            const result = await this.helperExecutor(helper, resolvedArgs);

            // Return TransferSignal with mode
            return {
                __type: "TransferSignal",
                value: result,
                mode: mode,
                helperName: helperName
            };
        }

        throw new Error(`Transfer to ${helperName} expects exactly 1 object argument`);
    }

    private async evaluateTemplate(expr: any, scope: Map<string, any>): Promise<string> {
        if (!expr || !Array.isArray(expr.value)) {
            return "";
        }

        let rendered = "";
        for (const part of expr.value) {
            rendered += await this.evaluateTemplatePart(part, scope);
        }
        return rendered;
    }

    /**
     * Evaluate a single template part (literal, expression, inlineIf, schemaDirective)
     */
    private async evaluateTemplatePart(part: any, scope: Map<string, any>): Promise<string> {
        if (!part) return "";

        switch (part.type) {
            case "literal":
                return String(part.value ?? "");

            case "expression":
                const evaluated = await this.evaluate(part.value, scope);
                return evaluated !== undefined && evaluated !== null ? String(evaluated) : "";

            case "inlineIf":
                return this.evaluateInlineIf(part, scope);

            case "schemaDirective":
                return this.evaluateSchemaDirective(part.path);

            default:
                // Fallback: try to evaluate as expression
                const fallback = await this.evaluate(part, scope);
                return fallback !== undefined && fallback !== null ? String(fallback) : "";
        }
    }

    /**
     * Evaluate {{#if condition}}...{{else}}...{{/if}} inline conditionals
     */
    private async evaluateInlineIf(expr: any, scope: Map<string, any>): Promise<string> {
        const conditionMet = await this.evaluateCondition(expr.condition, scope);

        const block = conditionMet ? expr.then : expr.else;
        if (!block || block.length === 0) {
            return "";
        }

        let result = "";
        for (const part of block) {
            result += await this.evaluateTemplatePart(part, scope);
        }
        return result;
    }

    /**
     * Evaluate {{@schema(path)}} directive.
     * Paths: output, output.property, input, context, types.TypeName
     */
    private evaluateSchemaDirective(path: string): string {
        if (!this.schemaContext) {
            return `[schema: ${path} - no context]`;
        }

        const parts = path.split('.');
        const root = parts[0];

        switch (root) {
            case 'output':
                return this.renderSchemaYAML(
                    parts.length > 1 
                        ? this.getNestedProperty(this.schemaContext.output, parts.slice(1))
                        : this.schemaContext.output,
                    0
                );

            case 'input':
                return this.renderSchemaYAML(
                    parts.length > 1
                        ? this.getNestedProperty(this.schemaContext.input, parts.slice(1))
                        : this.schemaContext.input,
                    0
                );

            case 'context':
                return this.renderSchemaYAML(
                    parts.length > 1
                        ? this.getNestedProperty(this.schemaContext.context, parts.slice(1))
                        : this.schemaContext.context,
                    0
                );

            case 'types':
                if (parts.length < 2) {
                    return '[schema: types requires type name]';
                }
                const typeName = parts[1]!;  // Safe: length check above guarantees this exists
                const typeDef = this.schemaContext.types?.[typeName];
                if (!typeDef) {
                    return `[schema: type "${typeName}" not found]`;
                }
                return this.renderSchemaYAML(typeDef.properties ?? typeDef, 0);

            default:
                return `[schema: unknown path "${root}"]`;
        }
    }

    /**
     * Get nested property from object using path array
     */
    private getNestedProperty(obj: any, path: string[]): any {
        if (!obj) return undefined;
        let current = obj;
        for (const key of path) {
            if (current === undefined || current === null) return undefined;
            current = current[key];
        }
        return current;
    }

    /**
     * Render a schema object as YAML-like string for prompt injection
     */
    private renderSchemaYAML(schema: any, indent: number): string {
        if (!schema) return "";

        const lines: string[] = [];
        const pad = " ".repeat(indent);

        for (const [name, info] of Object.entries(schema)) {
            const typeInfo = info as any;
            const optional = typeInfo.optional ? "?" : "";
            const desc = typeInfo.description ? ` # ${typeInfo.description}` : "";
            const type = typeInfo.type;

            // Handle arrays
            if (typeof type === 'object' && type.type === 'array') {
                lines.push(`${pad}${name}${optional}:${desc}`);
                const itemType = type.items;
                if (typeof itemType === 'object' && itemType.properties) {
                    lines.push(`${pad}  -`);
                    lines.push(this.renderSchemaYAML(itemType.properties, indent + 4));
                } else {
                    lines.push(`${pad}  - ${this.formatType(itemType)}`);
                }
                continue;
            }

            // Handle nested objects
            if (typeof type === 'object' && type.properties) {
                lines.push(`${pad}${name}${optional}:${desc}`);
                lines.push(this.renderSchemaYAML(type.properties, indent + 2));
                continue;
            }

            // Handle type references
            if (typeof type === 'object' && type.type === 'typeRef') {
                lines.push(`${pad}${name}${optional}: ${type.name}${desc}`);
                continue;
            }

            // Simple types
            lines.push(`${pad}${name}${optional}: ${this.formatType(type)}${desc}`);
        }

        return lines.join('\n');
    }

    /**
     * Format a type for YAML schema output
     */
    private formatType(type: any): string {
        if (typeof type === 'string') return type;
        if (typeof type === 'object') {
            if (type.type === 'union') return type.options?.join(' | ') ?? 'string';
            if (type.type === 'typeRef') return type.name;
            if (type.type === 'array') return `${this.formatType(type.items)}[]`;
        }
        return 'any';
    }

    /**
     * Evaluate a list of statements (for workflow body or prompt parts)
     * Returns the final value if there's a return, otherwise concatenates string results
     */
    async evaluateStatements(statements: any[], scope: Map<string, any>, asString = false): Promise<any> {
        let stringResult = "";

        for (const stmt of statements) {
            const result = await this.evaluate(stmt, scope);

            // Handle return signal
            if (result && typeof result === "object" && result.__type === "ReturnSignal") {
                return asString ? String(result.value ?? "") : result.value;
            }

            // For string mode (prompts), accumulate string results
            if (asString && result !== undefined) {
                stringResult += String(result);
            }
        }

        return asString ? stringResult : null;
    }
}
