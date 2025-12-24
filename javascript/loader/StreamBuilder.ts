import type { StreamChunk } from "./types/protocol";

/**
 * Handler types for each stream event
 */
export interface StreamHandlers {
    onText?: (delta: string) => void;
    onToolStart?: (name: string, id: string) => void;
    onToolArgs?: (id: string, delta: string) => void;
    onToolEnd?: (id: string) => void;
    onToolResult?: (name: string, result: any) => void;
    onHelperStart?: (name: string) => void;
    onHelperEnd?: (name: string, result: any) => void;
    onHelperChunk?: (name: string, chunk: StreamChunk) => void;
    onTransfer?: (helperName: string, mode: 'direct' | 'thenContinue') => void;
    onChunk?: (chunk: StreamChunk) => void;
}

/**
 * Fluent builder for streaming agent execution.
 * Provides a clean API for subscribing to stream events.
 * 
 * @example
 * ```typescript
 * const result = await agent
 *   .stream({ request: "Create a login page" })
 *   .onText(delta => process.stdout.write(delta))
 *   .onHelperStart(name => console.log(`>>> ${name}`))
 *   .run();
 * ```
 */
export class StreamBuilder<TOutput> {
    private handlers: StreamHandlers = {};

    constructor(
        private streamGenerator: () => AsyncGenerator<StreamChunk, TOutput, unknown>
    ) { }

    /**
     * Called when text tokens are streamed from the LLM
     */
    onText(handler: (delta: string) => void): this {
        this.handlers.onText = handler;
        return this;
    }

    /**
     * Called when a tool invocation starts
     */
    onToolStart(handler: (name: string, id: string) => void): this {
        this.handlers.onToolStart = handler;
        return this;
    }

    /**
     * Called when tool arguments are streamed
     */
    onToolArgs(handler: (id: string, delta: string) => void): this {
        this.handlers.onToolArgs = handler;
        return this;
    }

    /**
     * Called when a tool invocation ends
     */
    onToolEnd(handler: (id: string) => void): this {
        this.handlers.onToolEnd = handler;
        return this;
    }

    /**
     * Called when a tool returns its result
     */
    onToolResult(handler: (name: string, result: any) => void): this {
        this.handlers.onToolResult = handler;
        return this;
    }

    /**
     * Called when a helper (sub-agent) starts executing
     */
    onHelperStart(handler: (name: string) => void): this {
        this.handlers.onHelperStart = handler;
        return this;
    }

    /**
     * Called when a helper (sub-agent) finishes executing
     */
    onHelperEnd(handler: (name: string, result: any) => void): this {
        this.handlers.onHelperEnd = handler;
        return this;
    }

    /**
     * Called for each chunk streamed from a helper (recursive streaming)
     */
    onHelperChunk(handler: (name: string, chunk: StreamChunk) => void): this {
        this.handlers.onHelperChunk = handler;
        return this;
    }

    /**
     * Called when control is transferred to a helper
     */
    onTransfer(handler: (helperName: string, mode: 'direct' | 'thenContinue') => void): this {
        this.handlers.onTransfer = handler;
        return this;
    }

    /**
     * Catch-all handler for any chunk type (for custom handling or debugging)
     */
    onChunk(handler: (chunk: StreamChunk) => void): this {
        this.handlers.onChunk = handler;
        return this;
    }

    /**
     * Execute the stream and return the final result.
     * All registered handlers will be called as chunks arrive.
     */
    async run(): Promise<TOutput> {
        const stream = this.streamGenerator();
        let result: TOutput;

        while (true) {
            const { value, done } = await stream.next();
            if (done) {
                result = value;
                break;
            }

            // Dispatch to handlers
            this.dispatch(value);
        }

        return result;
    }

    /**
     * Dispatch a chunk to the appropriate handler(s)
     */
    private dispatch(chunk: StreamChunk): void {
        // Always call catch-all if registered
        this.handlers.onChunk?.(chunk);

        // Call specific handler based on chunk type
        switch (chunk.type) {
            case 'text':
                this.handlers.onText?.(chunk.delta);
                break;

            case 'tool_start':
                this.handlers.onToolStart?.(chunk.name, chunk.id);
                break;

            case 'tool_args':
                this.handlers.onToolArgs?.(chunk.id, chunk.delta);
                break;

            case 'tool_end':
                this.handlers.onToolEnd?.(chunk.id);
                break;

            case 'tool_result':
                this.handlers.onToolResult?.(chunk.name, chunk.result);
                break;

            case 'helper_start':
                this.handlers.onHelperStart?.(chunk.name);
                break;

            case 'helper_end':
                this.handlers.onHelperEnd?.(chunk.name, chunk.result);
                break;

            case 'helper_chunk':
                this.handlers.onHelperChunk?.(chunk.name, chunk.chunk);
                // Also dispatch the nested chunk for convenience
                this.dispatch(chunk.chunk);
                break;

            case 'transfer':
                this.handlers.onTransfer?.(chunk.helperName, chunk.mode);
                break;
        }
    }
}
