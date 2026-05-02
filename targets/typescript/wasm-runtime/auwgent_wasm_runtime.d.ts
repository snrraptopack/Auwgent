/* tslint:disable */
/* eslint-disable */
/**
 * The `ReadableStreamType` enum.
 *
 * *This API requires the following crate features to be activated: `ReadableStreamType`*
 */

type ReadableStreamType = "bytes";

export class AuwgentWasm {
    free(): void;
    [Symbol.dispose](): void;
    clearListeners(): void;
    clearSession(): void;
    embed(text: string): Promise<any>;
    embedBatch(texts: any): Promise<any>;
    endStream(): string;
    exportSession(): string;
    generatePrompt(helper_name?: string | null): string;
    getMetadata(): string;
    getToolNames(): Array<any>;
    getToolSchemas(): string;
    importSession(json: string): void;
    constructor(ir_json: string);
    onIntent(callback: Function): void;
    onIntentPartial(callback: Function): void;
    onMiddlewareEvent(callback: Function): void;
    onSubEngineComplete(callback: Function): void;
    onSubEngineStart(callback: Function): void;
    processIntents(): Promise<any>;
    registerTool(name: string, callback: Function): void;
    run(input?: string | null, initial_stack_json?: string | null): Promise<any>;
    setContext(context: any): void;
    setCustomDriver(id: string, api_key: string, base_url: string): void;
    setGeminiDriver(api_key: string): void;
    setGroqDriver(api_key: string): void;
    setOpenaiDriver(api_key: string): void;
    writeChunk(chunk: string): void;
}

export class IntoUnderlyingByteSource {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    cancel(): void;
    pull(controller: ReadableByteStreamController): Promise<any>;
    start(controller: ReadableByteStreamController): void;
    readonly autoAllocateChunkSize: number;
    readonly type: ReadableStreamType;
}

export class IntoUnderlyingSink {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    abort(reason: any): Promise<any>;
    close(): Promise<any>;
    write(chunk: any): Promise<any>;
}

export class IntoUnderlyingSource {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    cancel(): void;
    pull(controller: ReadableStreamDefaultController): Promise<any>;
}
