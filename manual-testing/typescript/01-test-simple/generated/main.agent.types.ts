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
    id: string;
    features: Feature[];
    started_at: string;
    tier: PricingTier;
    renews_at: string;
    is_active: boolean;
}

export type ContactInfo = {
    address: Address;
    email: string;
    phone: string;
}

export type Feature = {
    enabled: boolean;
    config: { limit: number; unit: string; overage_rate: number };
    id: string;
    label: string;
}

export type Address = {
    zip: string;
    street: string;
    country: string;
    city: string;
}

export type Organization = {
    subscription: Subscription;
    name: string;
    id: string;
    contact: ContactInfo;
    tags: string[];
    member_count: number;
}

export type PricingTier = {
    annual_cost: number;
    monthly_cost: number;
    name: string;
    max_seats: number;
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