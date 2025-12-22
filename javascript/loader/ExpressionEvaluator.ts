import type { AgentIR, Expression, Statement } from "./types/ir";
import type { ToolMap } from "./types/tool";

export class ExpressionEvaluator {
    constructor(
        private ir?: AgentIR,
        private tools?: ToolMap
    ) { }

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

            case "template":
                return this.evaluateTemplate(expr.value, scope);

            case "object":
                return this.evaluateObject(expr.value, scope);

            case "array":
                return this.evaluateArray(expr.value, scope);

            case "functionCall":
                return this.evaluateFunctionCall(expr, scope);

            case "if":
                return this.evaluateIf(expr, scope);


            case "variableDeclaration":
                const value = await this.evaluate(expr.value, scope);
                scope.set(expr.name, value);
                return undefined;

            case "return":
                const returnValue = await this.evaluate(expr.value, scope);
                return { __type: "ReturnSignal", value: returnValue };

            default:
                throw new Error(`Unknown expression type: ${expr.type}`);
        }
    }

    /**
     * Evaluate template literal parts into a string
     */
    private async evaluateTemplate(parts: any[], scope: Map<string, any>): Promise<string> {
        let result = "";

        for (const part of parts) {
            if (part.type === "literal") {
                result += part.value;
            } else if (part.type === "expression") {
                const value = await this.evaluate(part.value, scope);
                result += String(value ?? "");
            }
        }

        return result;
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
        const toolDef = this.ir?.tools.find(t => t.name === funcName);
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
     * Evaluate if statement
     */
    private async evaluateIf(expr: any, scope: Map<string, any>): Promise<any> {
        const left = await this.evaluate(expr.condition.left, scope);
        const right = await this.evaluate(expr.condition.right, scope);
        const operator = expr.condition.operator;

        const conditionMet = this.compare(left, operator, right);

        const block = conditionMet ? expr.then : expr.else;
        if (block && block.length > 0) {
            for (const stmt of block) {
                const result = await this.evaluate(stmt, scope);
                // Propagate return signal
                if (result && typeof result === "object" && result.__type === "ReturnSignal") {
                    return result;
                }
            }
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
