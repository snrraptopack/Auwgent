// Auto-generated types for TestAccount
// Do not edit manually
// Core Runtime Imports
import { createAuwgent as createAuwgentRuntime } from "@snrraptopack/auwgent-sdk";
import type { ToolRegistry } from "@snrraptopack/auwgent-sdk";
import _importedIR from './main.agent.json' with { type: 'json' };
type TestAccountIR = Omit<typeof _importedIR, "name" | "workflows" | "helpers" | "input"> & {
  name: "TestAccount";
  workflows: undefined;
  helpers: undefined;
  input: "image";
};
const agentIR = _importedIR as unknown as TestAccountIR;
export type TextPart = import("@snrraptopack/auwgent-sdk").AuwgentTextPart;
export type ImagePart = import("@snrraptopack/auwgent-sdk").AuwgentImagePart;
export type FilePart = import("@snrraptopack/auwgent-sdk").AuwgentFilePart;
export type AudioPart = import("@snrraptopack/auwgent-sdk").AuwgentAudioPart;
export type VideoPart = import("@snrraptopack/auwgent-sdk").AuwgentVideoPart;
export type InputPart = import("@snrraptopack/auwgent-sdk").AuwgentInputPart;
export type MediaSource = import("@snrraptopack/auwgent-sdk").AuwgentBinarySource;
export type ImageInput = MediaSource & { mimeType?: string; detail?: "auto" | "low" | "high" };
export type FileInput = MediaSource & { mimeType?: string; name?: string };
export type AudioInput = MediaSource & { mimeType?: string; transcript?: string };
export type VideoInput = MediaSource & { mimeType?: string; transcript?: string; sampledFrames?: ImagePart[] };
export type Input = readonly (TextPart | ImagePart)[]

export const input = {
    text(text: string): TextPart { return { type: "text", text }; },
    image(source: ImageInput): ImagePart { return { type: "image", ...source }; },
};

export type AuwgentOutput = {

}

export type AuwgentContext = {

}

/** Custom intents defined in the DSL (if any) */
export type AuwgentCustomIntents =
    | never;

export interface AuwgentIntentHandler {
    response_text?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "response_text" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
    response_schema?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "response_schema" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
    error?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "error" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
}

export class AuwgentBaseIntentHandler implements AuwgentIntentHandler {
    response_text(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "response_text" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
    response_schema(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "response_schema" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
    error(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "error" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
}

/**
 * API keys required for Auwgent
 */
export type AuwgentApiKeys = {
    geminiApiKey: string;
}

// Defined explicitly (not via ReturnType) so RouterMiddleware can derive from it without circularity
export type AuwgentAgent = import("@snrraptopack/auwgent-sdk").TypedAuwgent<
    typeof agentIR,
    AuwgentCustomIntents,
    AuwgentOutput,
    Record<string, never>
>;

/** Middleware object type — consistent with `AuwgentAgent.onIntent` intent narrowing */
export type AuwgentMiddleware<T extends import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent'] = import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent']> = import("@snrraptopack/auwgent-sdk").Middleware<
    typeof agentIR,
    AuwgentCustomIntents,
    AuwgentOutput,
    Record<string, never>,
    T
>;

export type AuwgentConfig = {
    middleware?: AuwgentMiddleware[];
    apiKeys: AuwgentApiKeys;
}

export function createAuwgent(config: AuwgentConfig): AuwgentAgent {
    return createAuwgentRuntime<
        typeof agentIR,
        AuwgentCustomIntents,
        AuwgentOutput,
        Record<string, never>
    >(agentIR, {
        tools: {} as Record<string, never>,
        middleware: config.middleware as any,
        apiKeys: config.apiKeys
    });
}

export const auwgent = createAuwgent;