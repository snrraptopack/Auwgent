import type { StreamChunk, ToolResult } from "./types/protocol";
import { StreamError } from "./types/errors";

/**
 * Handler types for each stream event
 */
export interface StreamHandlers {
    onText?: (delta: string, meta?: { format?: 'yaml' | 'json'; raw?: string }) => void;
    onToolStart?: (name: string, id: string) => void;
    onToolArgs?: (name: string, id: string, delta: string) => void;
    onToolEnd?: (name: string, id: string) => void;
    onToolResult?: (name: string, result: ToolResult) => void;
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
    private toolNames = new Map<string, string>(); // Track tool names by id

    constructor(
        private streamGenerator: () => AsyncGenerator<StreamChunk, TOutput, unknown>
    ) { }

    /**
     * Called when text tokens are streamed from the LLM
     */
    onText(handler: (delta: string, meta?: { format?: 'yaml' | 'json'; raw?: string }) => void): this {
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
    onToolArgs(handler: (name: string, id: string, delta: string) => void): this {
        this.handlers.onToolArgs = handler;
        return this;
    }

    /**
     * Called when a tool invocation ends
     */
    onToolEnd(handler: (name: string, id: string) => void): this {
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
        try {
            const stream = this.streamGenerator();
            let result: TOutput;

            while (true) {
                const { value, done } = await stream.next();
                if (done) {
                    result = value;
                    break;
                }

                // Dispatch to handlers (with error recovery)
                this.dispatch(value);
            }

            return result;
        } catch (error: any) {
            throw new StreamError('streaming', error);
        }
    }

    /**
     * Dispatch a chunk to the appropriate handler(s)
     * Handlers are wrapped in try-catch to prevent single handler from crashing stream
     */
    private dispatch(chunk: StreamChunk): void {
        // Always call catch-all if registered (with error recovery)
        if (this.handlers.onChunk) {
            try {
                this.handlers.onChunk(chunk);
            } catch (error: any) {
                console.error('[StreamBuilder] Error in onChunk handler:', error);
                // Continue processing other handlers
            }
        }

        // Call specific handler based on chunk type (with error recovery)
        try {
            switch (chunk.type) {
                case 'text':
                    this.handlers.onText?.(chunk.delta, { format: chunk.format, raw: chunk.raw });
                    break;

                case 'tool_start':
                    this.toolNames.set(chunk.id, chunk.name); // Track tool name
                    this.handlers.onToolStart?.(chunk.name, chunk.id);
                    break;

                case 'tool_args':
                    const toolNameForArgs = this.toolNames.get(chunk.id) || 'unknown';
                    this.handlers.onToolArgs?.(toolNameForArgs, chunk.id, chunk.delta);
                    break;

                case 'tool_end':
                    const toolNameForEnd = this.toolNames.get(chunk.id) || 'unknown';
                    this.handlers.onToolEnd?.(toolNameForEnd, chunk.id);
                    this.toolNames.delete(chunk.id); // Clean up
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
        } catch (error: any) {
            console.error(`[StreamBuilder] Error in handler for chunk type "${chunk.type}":`, error);
            // Continue streaming despite handler error
        }
    }
}
