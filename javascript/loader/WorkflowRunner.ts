import { ExpressionEvaluator, type StreamingHelperExecutor } from "./ExpressionEvaluator";
import type { AgentIR, Statement, Expression, HelperIR } from "./types/ir";
import type { ToolMap } from "./types/tool";
import type { StreamChunk } from "./types/protocol";

export type HelperExecutor = (helper: HelperIR, args: Record<string, any>) => Promise<any>;

export class WorkflowRunner {
    constructor(
        private ir: AgentIR,
        private tools: ToolMap,
        private helperExecutor?: HelperExecutor,
        private streamingHelperExecutor?: StreamingHelperExecutor
    ) { }

    async run(workflowName: string, args: Record<string, any>, context?: Record<string, any>): Promise<any> {
        const evaluator = new ExpressionEvaluator(this.ir, this.tools, this.helperExecutor);
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
            if (result && typeof result === 'object' && result.__type === 'ReturnSignal') {
                return result.value;
            }

            // Handle Transfer - propagate up to the caller (IrInterpreter)
            if (result && typeof result === 'object' && result.__type === 'TransferSignal') {
                return result; // Pass the whole signal up
            }
        }

        return null; // Void return
    }

    /**
     * Run workflow with streaming helper support.
     * Yields chunks immediately as they arrive from streaming helpers.
     * Returns final result when complete.
     */
    async *runStream(
        workflowName: string,
        args: Record<string, any>,
        context?: Record<string, any>
    ): AsyncGenerator<StreamChunk, any, unknown> {
        const evaluator = new ExpressionEvaluator(
            this.ir,
            this.tools,
            this.helperExecutor,
            this.streamingHelperExecutor
        );
        const scope = new Map<string, any>([
            ...Object.entries(context ?? {}),
            ...Object.entries(args)
        ]);

        const flow = this.ir.workflows.find(w => w.flowName === workflowName);
        if (!flow) {
            throw new Error(`Workflow not found: ${workflowName}`);
        }

        // Execute Body Statements using streaming evaluator
        for (const stmt of flow.body) {
            const stmtGen = evaluator.evaluateStream(stmt, scope);
            let result: any;

            // Consume generator, yielding all chunks
            while (true) {
                const { value: chunk, done } = await stmtGen.next();
                if (done) {
                    result = chunk;
                    break;
                }
                yield chunk;  // Forward chunks immediately!
            }

            // Handle Return
            if (result && typeof result === 'object' && result.__type === 'ReturnSignal') {
                return result.value;
            }

            // Handle Transfer - propagate up to the caller (IrInterpreter)
            if (result && typeof result === 'object' && result.__type === 'TransferSignal') {
                return result; // Pass the whole signal up
            }
        }

        return null; // Void return
    }
}