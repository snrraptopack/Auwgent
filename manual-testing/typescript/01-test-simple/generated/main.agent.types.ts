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
export type Address = {
    street: string;
    zip: string;
    city: string;
    country: string;
}

export type ContactInfo = {
    phone: string;
    address: Address;
    email: string;
}

export type Feature = {
    enabled: boolean;
    label: string;
    id: string;
    config: { limit: number; unit: string; overage_rate: number };
}

export type Subscription = {
    renews_at: string;
    id: string;
    tier: PricingTier;
    features: Feature[];
    is_active: boolean;
    started_at: string;
}

export type Organization = {
    id: string;
    member_count: number;
    name: string;
    subscription: Subscription;
    tags: string[];
    contact: ContactInfo;
}

export type PricingTier = {
    name: string;
    monthly_cost: number;
    max_seats: number;
    annual_cost: number;
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
    Record<string, never>
>;

/** Middleware object type — consistent with `MainAgentAgent.onIntent` intent narrowing */
export type MainAgentMiddleware<T extends import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent'] = import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent']> = import("@snrraptopack/auwgent-sdk").Middleware<
    typeof agentIR,
    MainAgentCustomIntents,
    MainAgentOutput,
    Record<string, never>,
    T
>;

export type MainAgentConfig = {
    middleware?: MainAgentMiddleware[];
    apiKeys: MainAgentApiKeys;
}

export function createMainAgent(config: MainAgentConfig): MainAgentAgent {
    return createAuwgent<
        typeof agentIR,
        MainAgentCustomIntents,
        MainAgentOutput,
        Record<string, never>
    >(agentIR, {
        tools: {} as Record<string, never>,
        middleware: config.middleware as any,
        apiKeys: config.apiKeys
    });
}

export const auwgent = createMainAgent;
export type AuwgentTools = Record<string, never>;
export type AuwgentConfig = MainAgentConfig;
export type AuwgentAgent = MainAgentAgent;
export type AuwgentMiddleware = MainAgentMiddleware;
export type AuwgentContext = MainAgentContext;