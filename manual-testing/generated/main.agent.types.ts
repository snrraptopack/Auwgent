// Auto-generated types for Test
// Do not edit manually
// Core Runtime Imports
import { createAuwgent } from "@snrraptopack/auwgent-sdk";
import type { ToolRegistry } from "@snrraptopack/auwgent-sdk";
import _importedIR from './main.agent.json' with { type: 'json' };
type TestIR = Omit<typeof _importedIR, "name" | "workflows" | "helpers"> & {
  name: "Test";
  workflows: ({ flowName: "one"; returns: string })[];
  helpers: undefined;
};
const agentIR = _importedIR as unknown as TestIR;
export type A = {
    wow: string;
}

export type Hey = {
    name: string;
    age: number;
}
export type TestInput = {
    text: string;
}

export type TestOutput =
    | { type: "Hey";
    name: string;
    age: number;
}
    | { type: "A";
    wow: string;
};

export type TestContext = {
    username: string;
    context: string;
}

export type TestTools = {
    one: (args: { id: string }) => Promise<string>;
}

/** Custom intents defined in the DSL (if any) */
export type TestCustomIntents = never;

/**
 * API keys required for Test
 */
export type TestApiKeys = {
    openaiApiKey: string;
    customUrl?: string;  // Optional override for custom provider URL
}

// Defined explicitly (not via ReturnType) so RouterMiddleware can derive from it without circularity
export type TestAgent = import("@snrraptopack/auwgent-sdk").TypedAuwgent<
    typeof agentIR,
    TestCustomIntents,
    TestOutput,
    TestTools
>;

/** Middleware object type — consistent with `TestAgent.onIntent` intent narrowing */
export type TestMiddleware<T extends import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent'] = import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent']> = import("@snrraptopack/auwgent-sdk").Middleware<
    typeof agentIR,
    TestCustomIntents,
    TestOutput,
    TestTools,
    T
>;

export type TestConfig = {
    tools: TestTools;
    middleware?: TestMiddleware[];
    context: TestContext;
    apiKeys: TestApiKeys;
}

export function createTest(config: TestConfig): TestAgent {
    return createAuwgent<
        typeof agentIR,
        TestCustomIntents,
        TestOutput,
        TestTools
    >(agentIR, {
        tools: config.tools,
        middleware: config.middleware as any,
        context: config.context,
        apiKeys: config.apiKeys
    });
}

export const auwgent = createTest;
export type AuwgentTools = TestTools;
export type AuwgentConfig = TestConfig;
export type AuwgentAgent = TestAgent;
export type AuwgentMiddleware = TestMiddleware;
export type AuwgentContext = TestContext;