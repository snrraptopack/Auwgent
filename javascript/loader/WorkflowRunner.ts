import { ExpressionEvaluator } from "./ExpressionEvaluator";
import type { AgentIR, Statement, Expression } from "./types/ir";
import type { ToolMap } from "./types/tool";

export class WorkflowRunner {
    constructor(
        private ir: AgentIR,
        private tools: ToolMap
    ) { }

    async run(workflowName: string, args: Record<string, any>, context?: Record<string, any>): Promise<any> {
        const evaluator = new ExpressionEvaluator(this.ir, this.tools);
        const scope = new Map<string, any>([
            ...Object.entries(context ?? {}),  // Context first
            ...Object.entries(args)             // Workflow args override
        ]);

        const flow = this.ir.workflows.find(w => w.flowName === workflowName);
        if (!flow) {
            throw new Error(`Workflow not found: ${workflowName}`);
        }

        // Execute Body Statements
        for (const stmt of flow.body) {
            const result = await evaluator.evaluate(stmt, scope);

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

}