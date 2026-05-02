export default function init(input?: unknown): Promise<void>;

export class AuwgentWasm {
    constructor(irJson: string);
    setContext(context: unknown): void;
    setGeminiDriver(apiKey: string): void;
    setGroqDriver(apiKey: string): void;
    setOpenaiDriver(apiKey: string): void;
    setCustomDriver(id: string, apiKey: string, baseUrl: string): void;
    registerTool(name: string, callback: (args: unknown) => Promise<unknown> | unknown): void;
    onIntent(callback: (name: string, value: unknown, agentName: string) => Promise<unknown> | unknown): void;
    onIntentPartial(callback: (name: string, value: unknown, agentName: string) => void): void;
    onSubEngineStart(callback: (helperName: string, emptySessionJson: string) => Promise<string | undefined> | string | undefined): void;
    onSubEngineComplete(callback: (helperName: string, completedSessionJson: string) => Promise<void> | void): void;
    onMiddlewareEvent(callback: (eventJson: string) => Promise<string | undefined> | string | undefined): void;
    clearListeners(): void;
    run(input?: string | null, initialStackJson?: string | null): Promise<string>;
    exportSession(): string;
    importSession(json: string): void;
    clearSession(): void;
    getMetadata(): string;
    generatePrompt(helperName?: string): string;
    getToolNames(): string[];
    getToolSchemas(): string;
    writeChunk(chunk: string): void;
    endStream(): string;
    processIntents(): Promise<string>;
    embed(text: string): Promise<number[]>;
    embedBatch(texts: string[]): Promise<number[][]>;
}

