// Auto-generated types for JokeBot
// Do not edit manually
// Core Runtime Imports
import { createAuwgent } from "@snrraptopack/auwgent-sdk";
import type { ToolRegistry } from "@snrraptopack/auwgent-sdk";
import _importedIR from './intent_test.agent.json' with { type: 'json' };
type JokeBotIR = Omit<typeof _importedIR, "name" | "workflows" | "helpers"> & {
  name: "JokeBot";
  workflows: undefined;
  helpers: ({ name: "Teller" })[];
};
const agentIR = _importedIR as unknown as JokeBotIR;
export type JokeBotInput = {

}

export type JokeBotOutput = {

}

export type JokeBotContext = {

}

/** Custom intents defined in the DSL (if any) */
export type JokeBotCustomIntents =
    | { name: "bail"; value: { reason: string } }
    | { name: "question"; value: { text: string; options: string[] } }
    | { name: "feedback"; value: { rating: number } };

/**
 * API keys required for JokeBot
 */
export type JokeBotApiKeys = {
    my_botApiKey: string;  // API key for custom provider 'my-bot'
}

// Defined explicitly (not via ReturnType) so RouterMiddleware can derive from it without circularity
export type JokeBotAgent = import("@snrraptopack/auwgent-sdk").TypedAuwgent<
    typeof agentIR,
    JokeBotCustomIntents,
    never,
    Record<string, never>
>;

/** Middleware object type — consistent with `JokeBotAgent.onIntent` intent narrowing */
export type JokeBotMiddleware<T extends import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent'] = import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent']> = import("@snrraptopack/auwgent-sdk").Middleware<
    typeof agentIR,
    JokeBotCustomIntents,
    never,
    Record<string, never>,
    T
>;

export type JokeBotConfig = {
    middleware?: JokeBotMiddleware[];
    apiKeys: JokeBotApiKeys;
}

export function createJokeBot(config: JokeBotConfig): JokeBotAgent {
    return createAuwgent<
        typeof agentIR,
        JokeBotCustomIntents,
        never,
        Record<string, never>
    >(agentIR, {
        tools: {} as Record<string, never>,
        middleware: config.middleware as any,
        apiKeys: config.apiKeys
    });
}

export const auwgent = createJokeBot;
export type AuwgentTools = Record<string, never>;
export type AuwgentConfig = JokeBotConfig;
export type AuwgentAgent = JokeBotAgent;
export type AuwgentMiddleware = JokeBotMiddleware;
export type AuwgentContext = JokeBotContext;