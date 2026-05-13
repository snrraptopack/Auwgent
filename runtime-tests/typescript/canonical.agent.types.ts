// Auto-generated types for RuntimeTest
// Do not edit manually
// Core Runtime Imports
import { createAuwgent as createAuwgentRuntime } from "@snrraptopack/auwgent-sdk";
import type { ToolRegistry } from "@snrraptopack/auwgent-sdk";
import _importedIR from './canonical.agent.json' with { type: 'json' };
type RuntimeTestIR = Omit<typeof _importedIR, "name" | "workflows" | "helpers" | "input"> & {
  name: "RuntimeTest";
  workflows: ({ flowName: "marks_and_location"; flowParams: { user_id: string }; returns: string })[];
  helpers: ({ name: "Planner"; input: null; output: { steps: string[]; motivation: string } } | { name: "Joker"; input: null; output: null })[];
  input: null;
};
const agentIR = _importedIR as unknown as RuntimeTestIR;
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
export type Input = string

export type JokerOutput = {

}

export type AuwgentBaseOutput = {

}

/** Union of possible output types (includes transfer destinations) */
export type AuwgentOutput = AuwgentBaseOutput | JokerOutput;

export type AuwgentContext = {
    user_name: string;
    age: number;
    id: string;
}

export type AuwgentTools = {
    get_location: (args: {  }) => Promise<string>;
    get_marks: (args: { id: string }) => Promise<string>;
}

/** Custom intents defined in the DSL (if any) */
export type AuwgentCustomIntents =
    | { name: "Loud"; value: { actions: string; reason: string } };

export interface AuwgentIntentHandler {
    tool_call?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_call" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
    tool_result?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_result" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
    tool_error?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_error" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
    tool_skipped?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_skipped" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
    response_text?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "response_text" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
    response_schema?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "response_schema" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
    error?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "error" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
    Loud?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "Loud" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
    workflow_call?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "workflow_call" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
    workflow_result?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "workflow_result" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
    helper_call?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "helper_call" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
    helper_result?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "helper_result" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
}

export class AuwgentBaseIntentHandler implements AuwgentIntentHandler {
    tool_call(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_call" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
    tool_result(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_result" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
    tool_error(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_error" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
    tool_skipped(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_skipped" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
    response_text(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "response_text" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
    response_schema(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "response_schema" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
    error(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "error" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
    Loud(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "Loud" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
    workflow_call(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "workflow_call" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
    workflow_result(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "workflow_result" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
    helper_call(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "helper_call" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
    helper_result(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "helper_result" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
}

/**
 * API keys required for Auwgent
 */
export type AuwgentApiKeys = {
    groqApiKey: string;
}

// Defined explicitly (not via ReturnType) so RouterMiddleware can derive from it without circularity
export type AuwgentAgent = import("@snrraptopack/auwgent-sdk").TypedAuwgent<
    typeof agentIR,
    AuwgentCustomIntents,
    AuwgentOutput,
    AuwgentTools
>;

/** Middleware object type — consistent with `AuwgentAgent.onIntent` intent narrowing */
export type AuwgentMiddleware<T extends import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent'] = import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent']> = import("@snrraptopack/auwgent-sdk").Middleware<
    typeof agentIR,
    AuwgentCustomIntents,
    AuwgentOutput,
    AuwgentTools,
    T
>;

export type AuwgentConfig = {
    tools: AuwgentTools;
    middleware?: AuwgentMiddleware[];
    context: AuwgentContext;
    apiKeys: AuwgentApiKeys;
}

export function createAuwgent(config: AuwgentConfig): AuwgentAgent {
    return createAuwgentRuntime<
        typeof agentIR,
        AuwgentCustomIntents,
        AuwgentOutput,
        AuwgentTools
    >(agentIR, {
        tools: config.tools,
        middleware: config.middleware as any,
        context: config.context,
        apiKeys: config.apiKeys
    });
}

export const auwgent = createAuwgent;