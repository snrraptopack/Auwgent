import type { AgentIRShape, IntentControl, SessionState, AuwgentIntent, AuwgentModelIntent, AuwgentModelValue, AuwgentResponseValue, AuwgentTargetedResponseValue } from './types.js';

// ── Middleware Types ───────────────────────────────────────────────────────

/**
 * A shared storage object that naturally lives for the duration of a single `agent.run()` call.
 * Middleware can write trace IDs and metadata here to share between hooks.
 */
export type MiddlewareContext<IR extends AgentIRShape = any> = (
    | { /** The name of the root agent */ activeAgent: (string extends IR['name'] ? never : IR['name']) }
    | { /** The name of a helper being executed */ activeAgent: (IR['helpers'] extends readonly any[] ? IR['helpers'][number]['name'] : never) }
    | { /** Fallback for generic string IRs */ activeAgent: (string extends IR['name'] ? string : never) }
) & {
    /** The full execution stack (breadcrumbs). Index 0 is rootAgent. */
    stack: string[];
    /** The name of the root orchestrator for this entire session */
    rootAgent: string;
    /** The raw unparsed YAML block for the current intent (only present during onIntent) */
    rawBlock?: string;
    /** The system prompt for the currently executing agent */
    systemPrompt?: string;
    /** Generate an embedding for the given text using the configured model */
    embed: (text: string) => Promise<number[]>;
    /** Generate embeddings for a batch of texts */
    embedBatch: (texts: string[]) => Promise<number[][]>;
  setContext: (data: any) => void;
} & Record<string, any>;

/**
 * Internal base hook definitions for targeted scoping.
 */
interface _MiddlewareHooks<
    IR extends AgentIRShape = any,
    CustomIntents = never,
    Output = any,
    Tools = any,
    T extends MiddlewareContext<IR>['activeAgent'] = any
> {
    /**
     * Fired when `agent.run()` is called, BEFORE the Rust engine executes.
     * Use this to mutate `session.turns` for context summarization/truncation.
     * You MUST return the (mutated) session state.
     */
    onRunStart?: (
        session: SessionState,
        ctx: Extract<MiddlewareContext<IR>, { activeAgent: T }>
    ) => SessionState | Promise<SessionState>;

    /**
     * Fired when the engine is about to send a prompt to the underlying Model Provider.
     * Return a string to replace the entire prompt JSON (RAG / prompt injection).
     */
    onLLMStart?: (
        prompt: string,
        ctx: Extract<MiddlewareContext<IR>, { activeAgent: T }>
    ) => void | Promise<void> | string | Promise<string>;

    /**
     * Fired when the generic execution intent stream emits an event.
     * You can optionally return an `IntentControl` (e.g. `{skip: true}`) to short-circuit.
     */
    onIntent?: (
        ...args: {
            [K in AuwgentIntent<IR, CustomIntents, Output, Tools>['name']]: [
                name: K,
                value: Extract<AuwgentIntent<IR, CustomIntents, Output, Tools>, { name: K }>['value'],
                ctx: Extract<MiddlewareContext<IR>, { activeAgent: T }>
            ];
        }[AuwgentIntent<IR, CustomIntents, Output, Tools>['name']]
    ) => IntentControl | Promise<IntentControl>;

    /**
     * Fired when the model emits a terminal response (text or schema).
     */
    onLLMEnd?: (
        response: AuwgentTargetedResponseValue<IR, T, CustomIntents, Output, Tools>,
        ctx: Extract<MiddlewareContext<IR>, { activeAgent: T }>
    ) => void | Promise<void>;

    /**
     * Fired when the `agent.run()` loop fully terminates.
     */
    onRunComplete?: (
        finalSession: SessionState,
        ctx: Extract<MiddlewareContext<IR>, { activeAgent: T }>
    ) => void | Promise<void>;

    /**
     * Fired if an error triggers panic in the engine or tools.
     * Return `true` to swallow the error and stop propagation.
     */
    onError?: (
        error: Error,
        session: SessionState | undefined,
        ctx: Extract<MiddlewareContext<IR>, { activeAgent: T }>
    ) => boolean | Promise<boolean> | void | Promise<void>;
}

/**
 * Interface for Auwgent Middleware Plugins.
 * Middleware intercept the execution lifecycle of the agent, allowing for context
 * compaction, tracing, metrics, and error handling.
 * 
 * Narrowing: If a 'target' is specified, hooks automatically narrow their 'ctx.activeAgent'.
 */
export type Middleware<
    IR extends AgentIRShape = any,
    CustomIntents = never,
    Output = any,
    Tools = any,
    Target extends MiddlewareContext<IR>['activeAgent'] = MiddlewareContext<IR>['activeAgent']
> =
    | ({ name: string; target?: undefined } & _MiddlewareHooks<IR, CustomIntents, Output, Tools, MiddlewareContext<IR>['activeAgent']>)
    | {
        [T in MiddlewareContext<IR>['activeAgent']]: {
            name: string;
            target: T | T[] | ReadonlyArray<T>;
        } & _MiddlewareHooks<IR, CustomIntents, Output, Tools, T>
    }[MiddlewareContext<IR>['activeAgent'] & Target];

/** @deprecated Use Middleware union directly */
export type MiddlewareIntentHandler = any;
