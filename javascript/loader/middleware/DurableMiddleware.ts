import type { AgentMiddleware, MiddlewareContext, ModelResponse, ToolResult, ToolUseBlock } from "../types/protocol";

// ═══════════════════════════════════════════════════════════════════════════════
// DURABLE STORE INTERFACE
// ═══════════════════════════════════════════════════════════════════════════════

/**
 * Stored state for a single execution run.
 * Contains all cached results needed to resume from any point.
 */
export interface DurableRunState {
    runId: string;
    agentName: string;
    status: "running" | "completed" | "failed";
    startedAt: number;
    completedAt?: number;
    
    // Input/output
    input: Record<string, any>;
    output?: any;
    error?: { name: string; message: string; stack?: string };
    
    // Cached execution results
    modelCalls: Map<string, ModelResponse>;      // key: hash of messages
    toolCalls: Map<string, ToolResult>;          // key: toolId
    helperCalls: Map<string, any>;               // key: helperName + hash(args)
    workflowSteps: Map<string, Map<number, any>>; // key: workflowName -> stepIndex -> result
    
    // Resume information
    lastCompletedStep?: {
        workflowName?: string;
        stepIndex?: number;
        helperName?: string;
    };
}

/**
 * Interface for durable execution storage backends.
 * Implement this to persist run state to different storage systems.
 */
export interface DurableStore {
    /**
     * Save or update run state.
     */
    save(state: DurableRunState): Promise<void>;
    
    /**
     * Load run state by runId.
     * Returns undefined if not found.
     */
    load(runId: string): Promise<DurableRunState | undefined>;
    
    /**
     * Delete run state.
     */
    delete(runId: string): Promise<void>;
    
    /**
     * List all runs for an agent (for debugging/admin).
     */
    listRuns(agentName: string, options?: {
        status?: DurableRunState["status"];
        limit?: number;
    }): Promise<DurableRunState[]>;
}

// ═══════════════════════════════════════════════════════════════════════════════
// IN-MEMORY STORE (for testing)
// ═══════════════════════════════════════════════════════════════════════════════

/**
 * Simple in-memory implementation of DurableStore for testing.
 * NOT suitable for production - data is lost on process restart.
 */
export class InMemoryDurableStore implements DurableStore {
    private runs = new Map<string, DurableRunState>();
    
    async save(state: DurableRunState): Promise<void> {
        // Deep clone to avoid reference issues
        this.runs.set(state.runId, structuredClone(state));
    }
    
    async load(runId: string): Promise<DurableRunState | undefined> {
        const state = this.runs.get(runId);
        return state ? structuredClone(state) : undefined;
    }
    
    async delete(runId: string): Promise<void> {
        this.runs.delete(runId);
    }
    
    async listRuns(agentName: string, options?: {
        status?: DurableRunState["status"];
        limit?: number;
    }): Promise<DurableRunState[]> {
        let results = Array.from(this.runs.values())
            .filter(r => r.agentName === agentName);
        
        if (options?.status) {
            results = results.filter(r => r.status === options.status);
        }
        
        if (options?.limit) {
            results = results.slice(0, options.limit);
        }
        
        return results.map(r => structuredClone(r));
    }
    
    /** Clear all stored runs (for testing) */
    clear(): void {
        this.runs.clear();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// DURABLE MIDDLEWARE STATE
// ═══════════════════════════════════════════════════════════════════════════════

/**
 * Middleware state stored in ctx.state.durable
 */
export interface DurableMiddlewareState {
    store: DurableStore;
    runState: DurableRunState;
    isResuming: boolean;
}

/**
 * State type for middleware - stored in ctx.state
 */
export interface DurableState {
    durable?: DurableMiddlewareState;
}

// ═══════════════════════════════════════════════════════════════════════════════
// HELPER FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════════

/**
 * Create a deterministic hash key for caching.
 * Uses JSON.stringify for simplicity - could use a proper hash for large inputs.
 */
function hashKey(...parts: any[]): string {
    return JSON.stringify(parts);
}

/**
 * Get durable state from middleware context.
 */
function getDurableState(ctx: MiddlewareContext<any, any, DurableState>): DurableMiddlewareState | undefined {
    return ctx.state?.durable;
}

// ═══════════════════════════════════════════════════════════════════════════════
// DURABLE MIDDLEWARE FACTORY
// ═══════════════════════════════════════════════════════════════════════════════

export interface DurableMiddlewareOptions {
    /** The store to persist run state to */
    store: DurableStore;
    /** Optional: existing runId to resume from */
    resumeRunId?: string;
    /** Whether to automatically clean up completed runs (default: false) */
    autoCleanup?: boolean;
    /** Persist interval in ms - how often to save during long runs (default: 5000) */
    persistIntervalMs?: number;
}

/**
 * Creates a durable execution middleware that enables:
 * - Caching of model calls, tool calls, helper calls, and workflow steps
 * - Resuming from failures by skipping already-completed work
 * - Persisting execution state to any storage backend
 * 
 * @example
 * ```typescript
 * const store = new InMemoryDurableStore();
 * const middleware = createDurableMiddleware({ store });
 * 
 * // First run - executes everything
 * await agent.run(input, { middleware: [middleware] });
 * 
 * // Resume from failure
 * const resumeMiddleware = createDurableMiddleware({ 
 *   store, 
 *   resumeRunId: "previous-run-id" 
 * });
 * await agent.run(input, { middleware: [resumeMiddleware] });
 * ```
 */
export function createDurableMiddleware(
    options: DurableMiddlewareOptions
): AgentMiddleware<any, any, DurableState> {
    const { store, resumeRunId, autoCleanup = false } = options;
    
    return {
        name: "durable",
        priority: 100, // Run early to enable caching
        
        // ═══════════════════════════════════════════════════════════════════
        // AGENT LIFECYCLE
        // ═══════════════════════════════════════════════════════════════════
        
        async onAgentStart(ctx: MiddlewareContext<any, any, DurableState>) {
            let runState: DurableRunState;
            let isResuming = false;
            
            if (resumeRunId) {
                // Try to load existing run for resume
                const existing = await store.load(resumeRunId);
                if (existing) {
                    runState = existing;
                    runState.status = "running";
                    isResuming = true;
                } else {
                    // Create new run with the specified ID
                    runState = createNewRunState(ctx.runId, ctx.agentName, ctx.input);
                }
            } else {
                // Create new run
                runState = createNewRunState(ctx.runId, ctx.agentName, ctx.input);
            }
            
            // Store in middleware state
            ctx.state.durable = {
                store,
                runState,
                isResuming
            };
            
            await store.save(runState);
        },
        
        async onAgentEnd(ctx: MiddlewareContext<any, any, DurableState>, result, error) {
            const durable = getDurableState(ctx);
            if (!durable) return;
            
            const { runState } = durable;
            runState.completedAt = Date.now();
            
            if (error) {
                runState.status = "failed";
                runState.error = {
                    name: error.name,
                    message: error.message,
                    stack: error.stack
                };
            } else {
                runState.status = "completed";
                runState.output = result;
            }
            
            await store.save(runState);
            
            // Optionally clean up completed runs
            if (autoCleanup && runState.status === "completed") {
                await store.delete(runState.runId);
            }
        },
        
        // ═══════════════════════════════════════════════════════════════════
        // MODEL CALLS - use wrapModelCall for caching
        // ═══════════════════════════════════════════════════════════════════
        
        async wrapModelCall(ctx: MiddlewareContext<any, any, DurableState>, next) {
            const durable = getDurableState(ctx);
            if (!durable) return next();
            
            const key = hashKey("model", ctx.request.messages);
            
            // Check for cached result when resuming
            if (durable.isResuming) {
                const cached = durable.runState.modelCalls.get(key);
                if (cached) {
                    return cached;
                }
            }
            
            // Execute model call
            const response = await next();
            
            // Cache the result
            durable.runState.modelCalls.set(key, response);
            await store.save(durable.runState);
            
            return response;
        },
        
        // ═══════════════════════════════════════════════════════════════════
        // TOOL CALLS - use wrapToolCall for caching
        // ═══════════════════════════════════════════════════════════════════
        
        async wrapToolCall(ctx: MiddlewareContext<any, any, DurableState>, tool, next) {
            const durable = getDurableState(ctx);
            if (!durable) return next(tool.input);
            
            // Check for cached result when resuming
            if (durable.isResuming) {
                const cached = durable.runState.toolCalls.get(tool.id);
                if (cached) {
                    return cached;
                }
            }
            
            // Execute tool call
            const result = await next(tool.input);
            
            // Cache the result
            durable.runState.toolCalls.set(tool.id, result);
            await store.save(durable.runState);
            
            return result;
        },
        
        // ═══════════════════════════════════════════════════════════════════
        // HELPER CALLS
        // ═══════════════════════════════════════════════════════════════════
        
        async onBeforeHelper(ctx: MiddlewareContext<any, any, DurableState>, helperName, args) {
            const durable = getDurableState(ctx);
            if (!durable?.isResuming) return;
            
            const key = hashKey("helper", helperName, args);
            const cached = durable.runState.helperCalls.get(key);
            
            if (cached) {
                return { skip: true, result: cached };
            }
        },
        
        async onAfterHelper(ctx: MiddlewareContext<any, any, DurableState>, helperName, args, result) {
            const durable = getDurableState(ctx);
            if (!durable) return;
            
            const key = hashKey("helper", helperName, args);
            durable.runState.helperCalls.set(key, result);
            durable.runState.lastCompletedStep = { helperName };
            
            await store.save(durable.runState);
        },
        
        // ═══════════════════════════════════════════════════════════════════
        // WORKFLOW STEPS
        // ═══════════════════════════════════════════════════════════════════
        
        async onWorkflowStart(ctx: MiddlewareContext<any, any, DurableState>, workflowName, args) {
            const durable = getDurableState(ctx);
            if (!durable?.isResuming) return;
            
            // Check if we have cached steps for this workflow
            const workflowSteps = durable.runState.workflowSteps.get(workflowName);
            if (workflowSteps && workflowSteps.size > 0) {
                // Resume from the step after the last completed one
                const completedSteps = Array.from(workflowSteps.keys()).sort((a, b) => a - b);
                const lastCompleted = completedSteps[completedSteps.length - 1];
                if (lastCompleted !== undefined) {
                    return { resumeFromStep: lastCompleted + 1 };
                }
            }
        },
        
        async onBeforeStep(ctx: MiddlewareContext<any, any, DurableState>, stepIndex, stepType) {
            const durable = getDurableState(ctx);
            if (!durable?.isResuming) return;
            
            const workflowName = ctx.workflowName;
            if (!workflowName) return;
            
            const workflowSteps = durable.runState.workflowSteps.get(workflowName);
            const cached = workflowSteps?.get(stepIndex);
            
            if (cached !== undefined) {
                return { skip: true, result: cached };
            }
        },
        
        async onAfterStep(ctx: MiddlewareContext<any, any, DurableState>, stepIndex, stepType, result) {
            const durable = getDurableState(ctx);
            if (!durable) return;
            
            const workflowName = ctx.workflowName;
            if (!workflowName) return;
            
            // Ensure workflow map exists
            if (!durable.runState.workflowSteps.has(workflowName)) {
                durable.runState.workflowSteps.set(workflowName, new Map());
            }
            
            durable.runState.workflowSteps.get(workflowName)!.set(stepIndex, result);
            durable.runState.lastCompletedStep = { workflowName, stepIndex };
            
            await store.save(durable.runState);
        },
        
        async onWorkflowEnd(ctx: MiddlewareContext<any, any, DurableState>, workflowName, result, error) {
            const durable = getDurableState(ctx);
            if (!durable) return;
            
            // Save final workflow state
            await store.save(durable.runState);
        },
        
        // ═══════════════════════════════════════════════════════════════════
        // ERROR HANDLING
        // ═══════════════════════════════════════════════════════════════════
        
        async onError(ctx: MiddlewareContext<any, any, DurableState>, error, phase) {
            const durable = getDurableState(ctx);
            if (!durable) return;
            
            // Save state on error so we can resume later
            durable.runState.error = {
                name: error.name,
                message: error.message,
                stack: error.stack
            };
            
            await store.save(durable.runState);
            
            // Don't retry - let the error propagate so user can resume later
            return { retry: false };
        }
    };
}

// ═══════════════════════════════════════════════════════════════════════════════
// INTERNAL HELPERS
// ═══════════════════════════════════════════════════════════════════════════════

function createNewRunState(
    runId: string,
    agentName: string,
    input: Record<string, any>
): DurableRunState {
    return {
        runId,
        agentName,
        status: "running",
        startedAt: Date.now(),
        input,
        modelCalls: new Map(),
        toolCalls: new Map(),
        helperCalls: new Map(),
        workflowSteps: new Map()
    };
}
