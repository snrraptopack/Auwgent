// Auto-generated types for TestCompany
// Do not edit manually
// Core Runtime Imports
import { createAuwgent } from "@snrraptopack/auwgent-sdk";
import type { ToolRegistry } from "@snrraptopack/auwgent-sdk";
import _importedIR from './main.agent.json' with { type: 'json' };
type TestCompanyIR = Omit<typeof _importedIR, "name" | "workflows" | "helpers"> & {
  name: "TestCompany";
  workflows: undefined;
  helpers: undefined;
};
const agentIR = _importedIR as unknown as TestCompanyIR;
export type Company = {
    company: { name: string; departments: { dept_name: string; employees: { name: string; role: string; salary: number }[] }[] };
}
export type TestCompanyInput = {

}

export type TestCompanyOutput = {
    company: { name: string; departments: { dept_name: string; employees: { name: string; role: string; salary: number }[] }[] };
}

export type TestCompanyContext = {

}

/** Custom intents defined in the DSL (if any) */
export type TestCompanyCustomIntents =
    | never;

/**
 * API keys required for TestCompany
 */
export type TestCompanyApiKeys = {
    groq_apiApiKey: string;  // API key for custom provider 'groq-api'
}

// Defined explicitly (not via ReturnType) so RouterMiddleware can derive from it without circularity
export type TestCompanyAgent = import("@snrraptopack/auwgent-sdk").TypedAuwgent<
    typeof agentIR,
    TestCompanyCustomIntents,
    TestCompanyOutput,
    Record<string, never>
>;

/** Middleware object type — consistent with `TestCompanyAgent.onIntent` intent narrowing */
export type TestCompanyMiddleware<T extends import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent'] = import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent']> = import("@snrraptopack/auwgent-sdk").Middleware<
    typeof agentIR,
    TestCompanyCustomIntents,
    TestCompanyOutput,
    Record<string, never>,
    T
>;

export type TestCompanyConfig = {
    middleware?: TestCompanyMiddleware[];
    apiKeys: TestCompanyApiKeys;
}

export function createTestCompany(config: TestCompanyConfig): TestCompanyAgent {
    return createAuwgent<
        typeof agentIR,
        TestCompanyCustomIntents,
        TestCompanyOutput,
        Record<string, never>
    >(agentIR, {
        tools: {} as Record<string, never>,
        middleware: config.middleware as any,
        apiKeys: config.apiKeys
    });
}

export const auwgent = createTestCompany;
export type AuwgentTools = Record<string, never>;
export type AuwgentConfig = TestCompanyConfig;
export type AuwgentAgent = TestCompanyAgent;
export type AuwgentMiddleware = TestCompanyMiddleware;
export type AuwgentContext = TestCompanyContext;