// Auto-generated types for Test
// Do not edit manually
// Core Runtime Imports
import { createAuwgent } from "@auwgent/runtime";
import type { ToolRegistry } from "@auwgent/runtime";
import _importedIR from './main.agent.json' with { type: 'json' };
type TestIR = Omit<typeof _importedIR, "name" | "workflows" | "helpers"> & {
  name: "Test";
  workflows: undefined;
  helpers: undefined;
};
const agentIR = _importedIR as unknown as TestIR;

export type Hey = {

    name: string;

    age: number;
}


export type A = {

    wow: string;
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
export type TestAgent = import("@auwgent/runtime").TypedAuwgent<
    typeof agentIR,
    TestCustomIntents,
    TestOutput,
    Record<string, never>
>;

/** Middleware object type — consistent with `TestAgent.onIntent` intent narrowing */
export type TestMiddleware<T extends import("@auwgent/runtime").MiddlewareContext<typeof agentIR>['activeAgent'] = import("@auwgent/runtime").MiddlewareContext<typeof agentIR>['activeAgent']> = import("@auwgent/runtime").Middleware<
    typeof agentIR,
    TestCustomIntents,
    TestOutput,
    Record<string, never>,
    T
>;

export type TestConfig = {
    middleware?: TestMiddleware[];
    apiKeys: TestApiKeys;
}

export function createTest(config: TestConfig): TestAgent {
    return createAuwgent<
        typeof agentIR,
        TestCustomIntents,
        TestOutput,
        Record<string, never>
    >(agentIR, {
        tools: {} as Record<string, never>,
        middleware: config.middleware as any,
        
        apiKeys: config.apiKeys
    });
}

export const auwgent = createTest;
export type AuwgentTools = Record<string, never>;
export type AuwgentConfig = TestConfig;
export type AuwgentAgent = TestAgent;
export type AuwgentMiddleware = TestMiddleware;
export type AuwgentContext = TestContext;
