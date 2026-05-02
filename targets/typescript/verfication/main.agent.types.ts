// Auto-generated types for SimpleTool
// Do not edit manually
// Core Runtime Imports
import { createAuwgent as createAuwgentRuntime } from "@snrraptopack/auwgent-sdk";
import type { ToolRegistry } from "@snrraptopack/auwgent-sdk";
import _importedIR from './main.agent.json' with { type: 'json' };
type SimpleToolIR = Omit<typeof _importedIR, "name" | "workflows" | "helpers"> & {
  name: "SimpleTool";
  workflows: ({ flowName: "marks_and_location"; flowParams: { user_id: string }; returns: string })[];
  helpers: ({ name: "Joker"; input: null; output: null } | { name: "Plan"; input: null; output: { steps: string[]; motivation: string } } | { name: "Fact"; input: null; output: { is_fact: boolean; confidence: number; reasons: string } })[];
};
const agentIR = _importedIR as unknown as SimpleToolIR;
export type AuwgentInput = {

}

export type PlanOutput = {
    type: { steps: string[]; motivation: string };
}

export type FactOutput = {
    type: { is_fact: boolean; confidence: number; reasons: string };
}

export type AuwgentBaseOutput = {

}

/** Union of possible output types (includes transfer destinations) */
export type AuwgentOutput = AuwgentBaseOutput | PlanOutput | FactOutput;

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