import { ExpressionEvaluator, type StreamingHelperExecutor } from "./ExpressionEvaluator";
import type { AgentIR, Statement, Expression, HelperIR, Tool } from "./types/ir";
import type { ToolMap } from "./types/tool";
import type { AgentMiddleware, MiddlewareContext, StreamChunk } from "./types/protocol";
import { WorkflowError } from "./types/errors";
import { runOnWorkflowStart, runOnBeforeStep, runOnAfterStep, runOnWorkflowEnd } from "./IrMiddleware";

export type HelperExecutor = (helper: HelperIR, args: Record<string, any>) => Promise<any>;

export class WorkflowRunner {
    constructor(
        private ir: AgentIR,
        private tools: ToolMap,
        private helperExecutor?: HelperExecutor,
        private streamingHelperExecutor?: StreamingHelperExecutor,
        private middlewares?: AgentMiddleware<any, any, any>[],
        private middlewareCtx?: MiddlewareContext<any, any, any>
    ) { }

    async run(workflowName: string, args: Record<string, any>, context?: Record<string, any>): Promise<any> {
        const flow = this.ir.workflows.find(w => w.flowName === workflowName);
        if (!flow) {
            throw new WorkflowError(workflowName, undefined, new Error('Workflow not found'));
        }

        // Create workflow-specific middleware context
        const workflowCtx = this.middlewareCtx ? {
            ...this.middlewareCtx,
            workflowName
        } : undefined;

        try {
            // Call onWorkflowStart hooks - may return resume point
            let startIndex = 0;
            if (this.middlewares && workflowCtx) {
                const resumeResult = await runOnWorkflowStart(this.middlewares, workflowCtx, workflowName, args);
                if (resumeResult?.resumeFromStep !== undefined) {
                    startIndex = resumeResult.resumeFromStep;
                }
            }

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
            for (let stepIndex = startIndex; stepIndex < flow.body.length; stepIndex++) {
                const stmt = flow.body[stepIndex]!;
                const stepCtx = workflowCtx ? { ...workflowCtx, stepIndex } : undefined;

                try {
                    // Call onBeforeStep hooks - may return cached result
                    if (this.middlewares && stepCtx) {
                        const skipResult = await runOnBeforeStep(this.middlewares, stepCtx, stepIndex, (stmt as any).type || 'expression');
                        if (skipResult?.skip) {
                            // Use cached result - apply to scope if needed
                            if (skipResult.result !== undefined && (stmt as any).name) {
                                scope.set((stmt as any).name, skipResult.result);
                            }
                            continue;
                        }
                    }

                    const result = await evaluator.evaluate(stmt, scope);

                    // Call onAfterStep hooks
                    if (this.middlewares && stepCtx) {
                        await runOnAfterStep(this.middlewares, stepCtx, stepIndex, (stmt as any).type || 'expression', result);
                    }

                    // Handle Return
                    if (result && typeof result === 'object' && result.__type === 'ReturnSignal') {
                        // Call onWorkflowEnd before returning
                        if (this.middlewares && workflowCtx) {
                            await runOnWorkflowEnd(this.middlewares, workflowCtx, workflowName, result.value);
                        }
                        return result.value;
                    }

                    // Handle Transfer - propagate up to the caller (IrInterpreter)
                    if (result && typeof result === 'object' && result.__type === 'TransferSignal') {
                        // Call onWorkflowEnd before returning
                        if (this.middlewares && workflowCtx) {
                            await runOnWorkflowEnd(this.middlewares, workflowCtx, workflowName, result);
                        }
                        return result; // Pass the whole signal up
                    }
                } catch (error: any) {
                    // Get step name if available
                    const stepName = (stmt as any).name || undefined;
                    throw new WorkflowError(workflowName, stepName, error);
                }
            }

            // Call onWorkflowEnd for null return
            if (this.middlewares && workflowCtx) {
                await runOnWorkflowEnd(this.middlewares, workflowCtx, workflowName, null);
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

        // Create workflow-specific middleware context
        const workflowCtx = this.middlewareCtx ? {
            ...this.middlewareCtx,
            workflowName
        } : undefined;

        try {
            // Call onWorkflowStart hooks - may return resume point
            let startIndex = 0;
            if (this.middlewares && workflowCtx) {
                const resumeResult = await runOnWorkflowStart(this.middlewares, workflowCtx, workflowName, args);
                if (resumeResult?.resumeFromStep !== undefined) {
                    startIndex = resumeResult.resumeFromStep;
                }
            }

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
            for (let stepIndex = startIndex; stepIndex < flow.body.length; stepIndex++) {
                const stmt = flow.body[stepIndex]!;
                const stepCtx = workflowCtx ? { ...workflowCtx, stepIndex } : undefined;

                try {
                    // Call onBeforeStep hooks - may return cached result
                    if (this.middlewares && stepCtx) {
                        const skipResult = await runOnBeforeStep(this.middlewares, stepCtx, stepIndex, (stmt as any).type || 'expression');
                        if (skipResult?.skip) {
                            // Use cached result - apply to scope if needed
                            if (skipResult.result !== undefined && (stmt as any).name) {
                                scope.set((stmt as any).name, skipResult.result);
                            }
                            continue;
                        }
                    }

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

                    // Call onAfterStep hooks
                    if (this.middlewares && stepCtx) {
                        await runOnAfterStep(this.middlewares, stepCtx, stepIndex, (stmt as any).type || 'expression', result);
                    }

                    // Handle Return
                    if (result && typeof result === 'object' && result.__type === 'ReturnSignal') {
                        // Call onWorkflowEnd before returning
                        if (this.middlewares && workflowCtx) {
                            await runOnWorkflowEnd(this.middlewares, workflowCtx, workflowName, result.value);
                        }
                        return result.value;
                    }

                    // Handle Transfer - propagate up to the caller (IrInterpreter)
                    if (result && typeof result === 'object' && result.__type === 'TransferSignal') {
                        // Call onWorkflowEnd before returning
                        if (this.middlewares && workflowCtx) {
                            await runOnWorkflowEnd(this.middlewares, workflowCtx, workflowName, result);
                        }
                        return result; // Pass the whole signal up
                    }
                } catch (error: any) {
                    // Get step name if available
                    const stepName = (stmt as any).name || undefined;
                    throw new WorkflowError(workflowName, stepName, error);
                }
            }

            // Call onWorkflowEnd for null return
            if (this.middlewares && workflowCtx) {
                await runOnWorkflowEnd(this.middlewares, workflowCtx, workflowName, null);
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
