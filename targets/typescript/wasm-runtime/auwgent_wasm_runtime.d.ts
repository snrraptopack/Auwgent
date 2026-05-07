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

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_auwgentwasm_free: (a: number, b: number) => void;
    readonly auwgentwasm_clearListeners: (a: number) => void;
    readonly auwgentwasm_clearSession: (a: number) => void;
    readonly auwgentwasm_embed: (a: number, b: number, c: number) => any;
    readonly auwgentwasm_embedBatch: (a: number, b: any) => any;
    readonly auwgentwasm_endStream: (a: number) => [number, number, number, number];
    readonly auwgentwasm_exportSession: (a: number) => [number, number, number, number];
    readonly auwgentwasm_generatePrompt: (a: number, b: number, c: number) => [number, number, number, number];
    readonly auwgentwasm_getMetadata: (a: number) => [number, number, number, number];
    readonly auwgentwasm_getToolNames: (a: number) => any;
    readonly auwgentwasm_getToolSchemas: (a: number) => [number, number, number, number];
    readonly auwgentwasm_importSession: (a: number, b: number, c: number) => [number, number];
    readonly auwgentwasm_new: (a: number, b: number) => [number, number, number];
    readonly auwgentwasm_onIntent: (a: number, b: any) => void;
    readonly auwgentwasm_onIntentPartial: (a: number, b: any) => void;
    readonly auwgentwasm_onMiddlewareEvent: (a: number, b: any) => void;
    readonly auwgentwasm_onSubEngineComplete: (a: number, b: any) => void;
    readonly auwgentwasm_onSubEngineStart: (a: number, b: any) => void;
    readonly auwgentwasm_processIntents: (a: number) => any;
    readonly auwgentwasm_registerTool: (a: number, b: number, c: number, d: any) => void;
    readonly auwgentwasm_run: (a: number, b: number, c: number, d: number, e: number) => any;
    readonly auwgentwasm_setContext: (a: number, b: any) => [number, number];
    readonly auwgentwasm_setCustomDriver: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly auwgentwasm_setGeminiDriver: (a: number, b: number, c: number) => void;
    readonly auwgentwasm_setGroqDriver: (a: number, b: number, c: number) => void;
    readonly auwgentwasm_setOpenaiDriver: (a: number, b: number, c: number) => void;
    readonly auwgentwasm_writeChunk: (a: number, b: number, c: number) => void;
    readonly __wbg_intounderlyingsink_free: (a: number, b: number) => void;
    readonly intounderlyingsink_abort: (a: number, b: any) => any;
    readonly intounderlyingsink_close: (a: number) => any;
    readonly intounderlyingsink_write: (a: number, b: any) => any;
    readonly __wbg_intounderlyingbytesource_free: (a: number, b: number) => void;
    readonly intounderlyingbytesource_autoAllocateChunkSize: (a: number) => number;
    readonly intounderlyingbytesource_cancel: (a: number) => void;
    readonly intounderlyingbytesource_pull: (a: number, b: any) => any;
    readonly intounderlyingbytesource_start: (a: number, b: any) => void;
    readonly intounderlyingbytesource_type: (a: number) => number;
    readonly __wbg_intounderlyingsource_free: (a: number, b: number) => void;
    readonly intounderlyingsource_cancel: (a: number) => void;
    readonly intounderlyingsource_pull: (a: number, b: any) => any;
    readonly wasm_bindgen__convert__closures_____invoke__he19090a95f4dbad6: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen__convert__closures_____invoke__h265d81a46867bea1: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h40911615d7bd6193: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h900c7f03d919a83d: (a: number, b: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
