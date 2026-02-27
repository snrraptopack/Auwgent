import type { AgentIRShape, IntentControl, SessionState, AuwgentIntent } from './types.js';

// ── Middleware Types ───────────────────────────────────────────────────────

/**
 * A shared storage object that naturally lives for the duration of a single `agent.run()` call.
 * Middleware can write trace IDs and metadata here to share between hooks.
 */
export type MiddlewareContext = {
    /** The name of the currently executing agent (e.g. "Router", "FoodWizard", etc) */
    activeAgent?: string;
    /** The raw unparsed YAML block for the current intent (only present during onIntent) */
    rawBlock?: string;
} & Record<string, any>;

export type MiddlewareIntentHandler<
    IR extends AgentIRShape = any,
    Custom = never,
    Output = any,
    Tools = any
> = (
    ...args: {
        [K in AuwgentIntent<IR, Custom, Output, Tools>['name']]: [
            name: K,
            value: Extract<AuwgentIntent<IR, Custom, Output, Tools>, { name: K }>['value'],
            ctx: MiddlewareContext
        ];
    }[AuwgentIntent<IR, Custom, Output, Tools>['name']]
) => IntentControl | Promise<IntentControl>;

/**
 * Interface for Auwgent Middleware Plugins.
 * Middleware intercept the execution lifecycle of the agent, allowing for context
 * compaction, tracing, metrics, and error handling.
 */
export interface Middleware<
    IR extends AgentIRShape = any,
    CustomIntents = never,
    Output = any,
    Tools = any
> {
    /** Name of the middleware (for debug/tracing) */
    name: string;

    /**
     * Fired when `agent.run()` is called, BEFORE the Rust engine executes.
     * Use this to mutate `session.turns` for context summarization/truncation.
     * You MUST return the (mutated) session state.
     */
    onRunStart?: (
        session: SessionState,
        ctx: MiddlewareContext
    ) => SessionState | Promise<SessionState>;

    /**
     * Fired when the engine is about to send a prompt to the underlying Model Provider.
     */
    onLLMStart?: (
        prompt: string,
        ctx: MiddlewareContext
    ) => void | Promise<void>;

    /**
     * Fired when the generic execution intent stream emits an event.
     * You can optionally return an `IntentControl` (e.g. `{skip: true}`) to short-circuit.
     */
    onIntent?: MiddlewareIntentHandler<IR, CustomIntents, Output, Tools>;

    /**
     * Fired when the underlying Model Provider successfully finishes generating.
     */
    onLLMEnd?: (
        response: string,
        ctx: MiddlewareContext
    ) => void | Promise<void>;

    /**
     * Fired when the `agent.run()` loop fully terminates.
     */
    onRunComplete?: (
        finalSession: SessionState,
        ctx: MiddlewareContext
    ) => void | Promise<void>;

    /**
     * Fired if an error triggers panic in the engine or tools.
     * Return `true` to swallow the error and stop propagation.
     */
    onError?: (
        error: Error,
        session: SessionState,
        ctx: MiddlewareContext
    ) => boolean | Promise<boolean> | void | Promise<void>;
}
