// Auto-generated types for TestProject
// Do not edit manually
// Core Runtime Imports
import { createAuwgent } from "@snrraptopack/auwgent-sdk";
import type { ToolRegistry } from "@snrraptopack/auwgent-sdk";
import _importedIR from './main.agent.json' with { type: 'json' };
type TestProjectIR = Omit<typeof _importedIR, "name" | "workflows" | "helpers"> & {
  name: "TestProject";
  workflows: undefined;
  helpers: undefined;
};
const agentIR = _importedIR as unknown as TestProjectIR;
export type Project = {
    tasks: { title: string; priority: "low" | "medium" | "high"; completed: boolean }[];
    project_name: string;
}
export type TestProjectInput = {

}

export type TestProjectOutput = {
    project_name: string;
    tasks: { title: string; priority: "low" | "medium" | "high"; completed: boolean }[];
}

export type TestProjectContext = {

}

/** Custom intents defined in the DSL (if any) */
export type TestProjectCustomIntents =
    | never;

/**
 * API keys required for TestProject
 */
export type TestProjectApiKeys = {
    groq_apiApiKey: string;  // API key for custom provider 'groq-api'
}

// Defined explicitly (not via ReturnType) so RouterMiddleware can derive from it without circularity
export type TestProjectAgent = import("@snrraptopack/auwgent-sdk").TypedAuwgent<
    typeof agentIR,
    TestProjectCustomIntents,
    TestProjectOutput,
    Record<string, never>
>;

/** Middleware object type — consistent with `TestProjectAgent.onIntent` intent narrowing */
export type TestProjectMiddleware<T extends import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent'] = import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent']> = import("@snrraptopack/auwgent-sdk").Middleware<
    typeof agentIR,
    TestProjectCustomIntents,
    TestProjectOutput,
    Record<string, never>,
    T
>;

export type TestProjectConfig = {
    middleware?: TestProjectMiddleware[];
    apiKeys: TestProjectApiKeys;
}

export function createTestProject(config: TestProjectConfig): TestProjectAgent {
    return createAuwgent<
        typeof agentIR,
        TestProjectCustomIntents,
        TestProjectOutput,
        Record<string, never>
    >(agentIR, {
        tools: {} as Record<string, never>,
        middleware: config.middleware as any,
        apiKeys: config.apiKeys
    });
}

export const auwgent = createTestProject;
export type AuwgentTools = Record<string, never>;
export type AuwgentConfig = TestProjectConfig;
export type AuwgentAgent = TestProjectAgent;
export type AuwgentMiddleware = TestProjectMiddleware;
export type AuwgentContext = TestProjectContext;