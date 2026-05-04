// Auto-generated types for Asisstant
// Do not edit manually
// Core Runtime Imports
import { createAuwgent as createAuwgentRuntime } from "../auwgent.ts";
import type { ToolRegistry } from "../auwgent.ts";
import _importedIR from './main.agent.json' with { type: 'json' };
type AsisstantIR = Omit<typeof _importedIR, "name" | "workflows" | "helpers"> & {
  name: "Asisstant";
  workflows: undefined;
  helpers: undefined;
};
const agentIR = _importedIR as unknown as AsisstantIR;
export type AuwgentInput = {

}

export type AuwgentOutput = {

}

export type AuwgentContext = {
    user_name: string;
}

export type AuwgentTools = {
    get_user_marks: (args: {  }) => Promise<string[]>;
    get_location: (args: {  }) => Promise<string>;
}

/** Custom intents defined in the DSL (if any) */
export type AuwgentCustomIntents =
    | never;

export interface AuwgentIntentHandler {
    tool_call?(value: Extract<import("../auwgent.ts").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_call" }>["value"], agentName: string): import("../auwgent.ts").IntentControl | Promise<import("../auwgent.ts").IntentControl> | void | Promise<void>;
    tool_result?(value: Extract<import("../auwgent.ts").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_result" }>["value"], agentName: string): import("../auwgent.ts").IntentControl | Promise<import("../auwgent.ts").IntentControl> | void | Promise<void>;
    tool_error?(value: Extract<import("../auwgent.ts").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_error" }>["value"], agentName: string): import("../auwgent.ts").IntentControl | Promise<import("../auwgent.ts").IntentControl> | void | Promise<void>;
    tool_skipped?(value: Extract<import("../auwgent.ts").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_skipped" }>["value"], agentName: string): import("../auwgent.ts").IntentControl | Promise<import("../auwgent.ts").IntentControl> | void | Promise<void>;
    response_text?(value: Extract<import("../auwgent.ts").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "response_text" }>["value"], agentName: string): import("../auwgent.ts").IntentControl | Promise<import("../auwgent.ts").IntentControl> | void | Promise<void>;
    response_schema?(value: Extract<import("../auwgent.ts").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "response_schema" }>["value"], agentName: string): import("../auwgent.ts").IntentControl | Promise<import("../auwgent.ts").IntentControl> | void | Promise<void>;
    error?(value: Extract<import("../auwgent.ts").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "error" }>["value"], agentName: string): import("../auwgent.ts").IntentControl | Promise<import("../auwgent.ts").IntentControl> | void | Promise<void>;
}

export class AuwgentBaseIntentHandler implements AuwgentIntentHandler {
    tool_call(value: Extract<import("../auwgent.ts").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_call" }>["value"], agentName: string): import("../auwgent.ts").IntentControl | Promise<import("../auwgent.ts").IntentControl> | void | Promise<void> {}
    tool_result(value: Extract<import("../auwgent.ts").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_result" }>["value"], agentName: string): import("../auwgent.ts").IntentControl | Promise<import("../auwgent.ts").IntentControl> | void | Promise<void> {}
    tool_error(value: Extract<import("../auwgent.ts").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_error" }>["value"], agentName: string): import("../auwgent.ts").IntentControl | Promise<import("../auwgent.ts").IntentControl> | void | Promise<void> {}
    tool_skipped(value: Extract<import("../auwgent.ts").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_skipped" }>["value"], agentName: string): import("../auwgent.ts").IntentControl | Promise<import("../auwgent.ts").IntentControl> | void | Promise<void> {}
    response_text(value: Extract<import("../auwgent.ts").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "response_text" }>["value"], agentName: string): import("../auwgent.ts").IntentControl | Promise<import("../auwgent.ts").IntentControl> | void | Promise<void> {}
    response_schema(value: Extract<import("../auwgent.ts").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "response_schema" }>["value"], agentName: string): import("../auwgent.ts").IntentControl | Promise<import("../auwgent.ts").IntentControl> | void | Promise<void> {}
    error(value: Extract<import("../auwgent.ts").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "error" }>["value"], agentName: string): import("../auwgent.ts").IntentControl | Promise<import("../auwgent.ts").IntentControl> | void | Promise<void> {}
}

/**
 * API keys required for Auwgent
 */
export type AuwgentApiKeys = {
    groqApiKey: string;
}

// Defined explicitly (not via ReturnType) so RouterMiddleware can derive from it without circularity
export type AuwgentAgent = import("../auwgent.ts").TypedAuwgent<
    typeof agentIR,
    AuwgentCustomIntents,
    AuwgentOutput,
    AuwgentTools
>;

/** Middleware object type — consistent with `AuwgentAgent.onIntent` intent narrowing */
export type AuwgentMiddleware<T extends import("../auwgent.ts").MiddlewareContext<typeof agentIR>['activeAgent'] = import("../auwgent.ts").MiddlewareContext<typeof agentIR>['activeAgent']> = import("../auwgent.ts").Middleware<
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
