// Auto-generated types for MainAgent
// Do not edit manually
// Core Runtime Imports
import { createAuwgent } from "@snrraptopack/auwgent-sdk";
import type { ToolRegistry } from "@snrraptopack/auwgent-sdk";
import _importedIR from './main.agent.json' with { type: 'json' };
type MainAgentIR = Omit<typeof _importedIR, "name" | "workflows" | "helpers"> & {
  name: "MainAgent";
  workflows: undefined;
  helpers: undefined;
};
const agentIR = _importedIR as unknown as MainAgentIR;
export type Subscription = {
    tier: PricingTier;
    renews_at: string;
    is_active: boolean;
    id: string;
    started_at: string;
    features: Feature[];
}

export type Address = {
    country: string;
    street: string;
    city: string;
    zip: string;
}

export type PricingTier = {
    name: string;
    monthly_cost: number;
    annual_cost: number;
    max_seats: number;
}

export type Organization = {
    name: string;
    id: string;
    subscription: Subscription;
    member_count: number;
    tags: string[];
    contact: ContactInfo;
}

export type ContactInfo = {
    email: string;
    address: Address;
    phone: string;
}

export type Feature = {
    config: { limit: number; unit: string; overage_rate: number };
    label: string;
    id: string;
    enabled: boolean;
}
export type MainAgentInput = {

}

export type MainAgentOutput = {
    id: string;
    name: string;
    contact: ContactInfo;
    subscription: Subscription;
    member_count: number;
    tags: string[];
}

export type MainAgentContext = {

}

export type MainAgentTools = {
    user_name: (args: {  }) => Promise<string>;
}

/** Custom intents defined in the DSL (if any) */
export type MainAgentCustomIntents =
    | never;

/**
 * API keys required for MainAgent
 */
export type MainAgentApiKeys = {
    my_groq_apiApiKey: string;  // API key for custom provider 'my-groq-api'
}

// Defined explicitly (not via ReturnType) so RouterMiddleware can derive from it without circularity
export type MainAgentAgent = import("@snrraptopack/auwgent-sdk").TypedAuwgent<
    typeof agentIR,
    MainAgentCustomIntents,
    MainAgentOutput,
    MainAgentTools
>;

/** Middleware object type — consistent with `MainAgentAgent.onIntent` intent narrowing */
export type MainAgentMiddleware<T extends import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent'] = import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent']> = import("@snrraptopack/auwgent-sdk").Middleware<
    typeof agentIR,
    MainAgentCustomIntents,
    MainAgentOutput,
    MainAgentTools,
    T
>;

export type MainAgentConfig = {
    tools: MainAgentTools;
    middleware?: MainAgentMiddleware[];
    apiKeys: MainAgentApiKeys;
}

export function createMainAgent(config: MainAgentConfig): MainAgentAgent {
    return createAuwgent<
        typeof agentIR,
        MainAgentCustomIntents,
        MainAgentOutput,
        MainAgentTools
    >(agentIR, {
        tools: config.tools,
        middleware: config.middleware as any,
        apiKeys: config.apiKeys
    });
}

export const auwgent = createMainAgent;
export type AuwgentTools = MainAgentTools;
export type AuwgentConfig = MainAgentConfig;
export type AuwgentAgent = MainAgentAgent;
export type AuwgentMiddleware = MainAgentMiddleware;
export type AuwgentContext = MainAgentContext;