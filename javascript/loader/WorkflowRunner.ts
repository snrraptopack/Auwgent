import type { AgentIR, Statement, Expression } from "./types/ir";
import type { ToolMap } from "./types/tool";

export class WorkflowRunner {
    constructor(
        private ir: AgentIR,
        private tools: ToolMap
    ) { }

    async run(workflowName: string, args: Record<string, any>): Promise<any> {
        const flow = this.ir.workflows.find(w => w.flowName === workflowName);
        if (!flow) {
            throw new Error(`Workflow not found: ${workflowName}`);
        }

        // Initialize Scope with arguments
        const scope = new Map<string, any>(Object.entries(args));

        // Execute Body Statements
        for (const stmt of flow.body) {
            const result = await this.executeStatement(stmt, scope);

            // Handle Return
            // (In a real interpreter we'd have a wrapper for ReturnValue, 
            // but here we can just check if result is defined if we strict)
            // Actually, let's use a special object for Return to distinguish from generic values
            if (result && typeof result === 'object' && result.__type === 'ReturnSignal') {
                return result.value;
            }
        }

        return null; // Void return
    }

    private async executeStatement(stmt: Statement, scope: Map<string, any>): Promise<any> {
        switch (stmt.type) {
            case "variableDeclaration": {
                const value = await this.evaluateExpression(stmt.value, scope);
                scope.set(stmt.name, value);
                return undefined;
            }
            case "return": {
                const value = await this.evaluateExpression(stmt.value, scope);
                return { __type: 'ReturnSignal', value };
            }
            case "if": {
                // Evaluate condition
                const left = await this.evaluateExpression((stmt as any).condition.left, scope);
                const right = await this.evaluateExpression((stmt as any).condition.right, scope);
                const operator = (stmt as any).condition.operator;

                let conditionResult = false;
                switch (operator) {
                    case "==": conditionResult = left == right; break;
                    case "!=": conditionResult = left != right; break;
                    case "<": conditionResult = left < right; break;
                    case ">": conditionResult = left > right; break;
                    case "<=": conditionResult = left <= right; break;
                    case ">=": conditionResult = left >= right; break;
                }

                // Execute appropriate block
                const block = conditionResult ? (stmt as any).then : (stmt as any).else;
                if (block && block.length > 0) {
                    for (const innerStmt of block) {
                        const result = await this.executeStatement(innerStmt, scope);
                        if (result && typeof result === 'object' && result.__type === 'ReturnSignal') {
                            return result; // Propagate return up
                        }
                    }
                }
                return undefined;
            }
            case "functionCall": // Function call as a statement (ignore return value)
                await this.evaluateExpression(stmt as any, scope); // casting because Expression is subset
                return undefined;

            default:
                // It might be just an expression used as statement
                if ((stmt as any).type) {
                    await this.evaluateExpression(stmt as any, scope);
                }
                return undefined;
        }
    }

    private async evaluateExpression(expr: Expression, scope: Map<string, any>): Promise<any> {
        switch (expr.type) {
            case "literal":
                return expr.value;

            case "union":
                // Unions are type constraints, not runtime values
                throw new Error(`Union types cannot be used as runtime values. Options: ${expr.value.join(' | ')}`);

            case "varRef": {
                if (!scope.has(expr.value)) {
                    throw new Error(`Variable not found: ${expr.value}`);
                }
                return scope.get(expr.value);
            }

            case "object": {
                const result: Record<string, any> = {};
                
                // expr.value contains the properties object from the IR
                // Format: { name: { type: "varRef", value: "name" }, age: { type: "literal", value: 25 } }
                for (const [key, valueExpr] of Object.entries((expr as any).value)) {
                    result[key] = await this.evaluateExpression(valueExpr as Expression, scope);
                }
                
                return result;
            }

            case "array": {
                const elements = (expr as any).value || [];
                const result = [];
                for (const elemExpr of elements) {
                    result.push(await this.evaluateExpression(elemExpr, scope));
                }
                return result;
            }

            case "functionCall": {
                const funcName = expr.value;
                const args = expr.args;

                // 1. Resolve Arguments
                // Note: IR doesn't give us named args mapped to values :( 
                // It gives an array of Expressions.
                // We assume the tool implementation expects a single object "args".
                // ERROR DETECTED: The Generator output for FunctionCall args is just `Expression[]`. 
                // But `Tool` signature expects `(args: { x: ... })`.

                // CRITICAL FIX: The Generator must map positional args to named args based on definition!
                // For now, let's assume the Workflow DSL passes values in order 
                // and we need to look up the Tool Definition to match names.

                const toolDef = this.ir.tools.find(t => t.name === funcName);
                if (!toolDef) {
                    throw new Error(`Tool not found: ${funcName}`);
                }

                const paramNames = Object.keys(toolDef.params);
                const resolvedArgs: Record<string, any> = {};

                for (let i = 0; i < args.length; i++) {
                    const expr = args[i];
                    if (!expr) continue;

                    const argValue = await this.evaluateExpression(expr, scope);
                    const paramName = paramNames[i];
                    if (paramName) {
                        resolvedArgs[paramName] = argValue;
                    }
                }

                // 2. Execute Tool
                if (!this.tools[funcName]) {
                    throw new Error(`Tool implementation missing for: ${funcName}`);
                }

                return await this.tools[funcName](resolvedArgs);
            }

            default:
                throw new Error(`Unknown expression type: ${(expr as any).type}`);
        }
    }
}