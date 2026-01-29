import { ExpressionEvaluator, type StreamingHelperExecutor } from "./ExpressionEvaluator";
import type { AgentIR, Statement, Expression, HelperIR, Tool } from "./types/ir";
import type { ToolMap } from "./types/tool";
import type { StreamChunk } from "./types/protocol";
import { WorkflowError } from "./types/errors";

export type HelperExecutor = (helper: HelperIR, args: Record<string, any>) => Promise<any>;

export class WorkflowRunner {
    constructor(
        private ir: AgentIR,
        private tools: ToolMap,
        private helperExecutor?: HelperExecutor,
        private streamingHelperExecutor?: StreamingHelperExecutor
    ) { }

    async run(workflowName: string, args: Record<string, any>, context?: Record<string, any>): Promise<any> {
        const flow = this.ir.workflows.find(w => w.flowName === workflowName);
        if (!flow) {
            throw new WorkflowError(workflowName, undefined, new Error('Workflow not found'));
        }

        try {
            const evaluator = new ExpressionEvaluator(
                this.ir,
                this.tools,
                this.helperExecutor,
                undefined,
                this.getToolDefinitions(flow.tools)
            );
            const ctx = context ?? {};
            const scope = new Map<string, any>([
                ...Object.entries({ ...ctx, ctx }),
                ...Object.entries(args)
            ]);

            // Execute Body Statements
            for (const stmt of flow.body) {
                try {
                    const result = await evaluator.evaluate(stmt, scope);

                    // Handle Return
                    if (result && typeof result === 'object' && result.__type === 'ReturnSignal') {
                        return result.value;
                    }

                    // Handle Transfer - propagate up to the caller (IrInterpreter)
                    if (result && typeof result === 'object' && result.__type === 'TransferSignal') {
                        return result; // Pass the whole signal up
                    }
                } catch (error: any) {
                    // Get step name if available
                    const stepName = (stmt as any).name || undefined;
                    throw new WorkflowError(workflowName, stepName, error);
                }
            }

            return null; // Void return
        } catch (error: any) {
            // Re-throw WorkflowError as-is, wrap others
            if (error instanceof WorkflowError) {
                throw error;
            }
            throw new WorkflowError(workflowName, undefined, error);
        }
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
        const flow = this.ir.workflows.find(w => w.flowName === workflowName);
        if (!flow) {
            throw new WorkflowError(workflowName, undefined, new Error('Workflow not found'));
        }

        try {
            const evaluator = new ExpressionEvaluator(
                this.ir,
                this.tools,
                this.helperExecutor,
                this.streamingHelperExecutor,
                this.getToolDefinitions(flow.tools)
            );
            const ctx = context ?? {};
            const scope = new Map<string, any>([
                ...Object.entries({ ...ctx, ctx }),
                ...Object.entries(args)
            ]);

            // Execute Body Statements using streaming evaluator
            for (const stmt of flow.body) {
                try {
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
                } catch (error: any) {
                    // Get step name if available
                    const stepName = (stmt as any).name || undefined;
                    throw new WorkflowError(workflowName, stepName, error);
                }
            }

            return null; // Void return
        } catch (error: any) {
            // Re-throw WorkflowError as-is, wrap others
            if (error instanceof WorkflowError) {
                throw error;
            }
            throw new WorkflowError(workflowName, undefined, error);
        }
    }

    private getToolDefinitions(workflowTools?: Tool[]): Tool[] {
        if (!workflowTools || workflowTools.length === 0) {
            return this.ir.tools ?? [];
        }
        const toolMap = new Map<string, Tool>();
        for (const tool of this.ir.tools ?? []) {
            toolMap.set(tool.name, tool);
        }
        for (const tool of workflowTools) {
            toolMap.set(tool.name, tool);
        }
        return Array.from(toolMap.values());
    }
}
