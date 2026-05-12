import type { AgentIRShape, IntentControl, SessionState, AuwgentIntent, AuwgentModelIntent, AuwgentModelValue, AuwgentResponseValue, AuwgentTargetedResponseValue, PartialTextIntentValue, PartialStructuredIntentValue } from './types.js';

// ── Middleware Types ───────────────────────────────────────────────────────

/** Return value from `onLLMStart` when mutating provider request fields. */
export interface MiddlewareLLMStartResult {
    /** Replace the prompt text sent to the model. */
    prompt?: string;
    /** Override the execution stack. */
    stack?: string[];
    /** Deep-merge provider config (e.g. temperature, maxTokens). */
    config?: Record<string, unknown>;
    /** Switch to a different provider driver. */
    provider?: string;
    /** Override the provider URL (for custom/proxy endpoints). */
    url?: string;
    /** Inject HTTP headers (e.g. Authorization) into the provider request. */
    headers?: Record<string, string>;
}

/** Return value from `onError` when controlling error handling. */
export interface MiddlewareErrorResult {
    /** Swallow the error and stop propagation. */
    swallow?: boolean;
    /** Restart the current turn or the entire run. */
    forceStart?: 'llm_start' | 'run_start';
}

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
  /** The model name selected for the current LLM call (only present during onLLMStart) */
  model?: string;
  /** The provider ID for the current LLM call (only present during onLLMStart) */
  provider?: string;
  /** The provider-specific config object for the current LLM call (only present during onLLMStart) */
  config?: Record<string, unknown>;
  /** The custom provider URL, if applicable (only present during onLLMStart) */
  url?: string;
  /** HTTP headers injected into the provider request (only present during onLLMStart) */
  headers?: Record<string, string>;
} & Record<string, any>;

/**
 * Internal base hook definitions for targeted scoping.
 */
interface _MiddlewareHooks<
    IR extends AgentIRShape = any,
    CustomIntents extends { name: string; value: any } = never,
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
     * Return a string to replace the entire prompt, or an object to mutate config/provider/headers.
     */
    onLLMStart?: (
        prompt: string,
        ctx: Extract<MiddlewareContext<IR>, { activeAgent: T }>
    ) => void | Promise<void>
      | string | Promise<string>
      | MiddlewareLLMStartResult | Promise<MiddlewareLLMStartResult>;

    /**
     * Fired when the generic execution intent stream emits an event.
     * You can optionally return an `IntentControl` (e.g. `{skip: true}`) to short-circuit.
     */
    onIntent?: (
        ...args: {
            [K in AuwgentIntent<IR, CustomIntents, Output, Tools>['name'] & string]: [
                name: K,
                value: Extract<AuwgentIntent<IR, CustomIntents, Output, Tools>, { name: K }>['value'],
                ctx: Extract<MiddlewareContext<IR>, { activeAgent: T }>
            ];
        }[AuwgentIntent<IR, CustomIntents, Output, Tools>['name'] & string]
    ) => IntentControl | Promise<IntentControl>;

    /**
     * Fired for streaming partial intent updates.
     * Observational only; cannot return control signals.
     */
    onIntentPartial?: (
        ...args: {
            [K in AuwgentIntent<IR, CustomIntents, Output, Tools>['name'] & string]: [
                name: K,
                value:
                    | Extract<AuwgentIntent<IR, CustomIntents, Output, Tools>, { name: K }>['value']
                    | PartialTextIntentValue
                    | PartialStructuredIntentValue<any>,
                ctx: Extract<MiddlewareContext<IR>, { activeAgent: T }>
            ];
        }[AuwgentIntent<IR, CustomIntents, Output, Tools>['name'] & string]
    ) => void | Promise<void>;

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
     * Return `true` or `{ swallow: true }` to swallow the error and stop propagation.
     * Return `{ forceStart: 'llm_start' | 'run_start' }` to restart the turn or run.
     */
    onError?: (
        error: Error,
        session: SessionState | undefined,
        ctx: Extract<MiddlewareContext<IR>, { activeAgent: T }>
    ) => boolean | Promise<boolean>
      | MiddlewareErrorResult | Promise<MiddlewareErrorResult>
      | void | Promise<void>;
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
    CustomIntents extends { name: string; value: any } = never,
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
