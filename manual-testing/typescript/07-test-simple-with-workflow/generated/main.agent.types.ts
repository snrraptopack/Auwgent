// Auto-generated types for Main
// Do not edit manually
// Core Runtime Imports
import { createAuwgent } from "@snrraptopack/auwgent-sdk";
import type { ToolRegistry } from "@snrraptopack/auwgent-sdk";
import _importedIR from './main.agent.json' with { type: 'json' };
type MainIR = Omit<typeof _importedIR, "name" | "workflows" | "helpers"> & {
  name: "Main";
  workflows: ({ flowName: "get_user_orders"; returns: string } | { flowName: "calculate_revenue"; returns: string } | { flowName: "process_bulk_order"; returns: string })[];
  helpers: ({ name: "DataAnalyzer" } | { name: "ReportGenerator" })[];
};
const agentIR = _importedIR as unknown as MainIR;
export type Order = {
    total: number;
    product_id: string;
    status: "pending" | "completed" | "cancelled";
    id: string;
    quantity: number;
    user_id: string;
}

export type QueryResult = {
    success: boolean;
    message: string;
    data: string;
}

export type User = {
    id: string;
    email: string;
    created_at: string;
    name: string;
}

export type AnalysisReport = {
    total_products: number;
    total_orders: number;
    revenue: number;
    insights: string[];
    total_users: number;
}

export type Product = {
    name: string;
    price: number;
    stock: number;
    id: string;
}
export type MainInput = {

}

export type DataAnalyzerOutput = {
    total_users: number;
    total_products: number;
    total_orders: number;
    revenue: number;
    insights: string[];
}

export type ReportGeneratorOutput = {
    type: { report_title: string; summary: string; sections: string[]; generated_at: string };
}

export type MainBaseOutput = {
    success: boolean;
    data: string;
    message: string;
}

/** Union of possible output types (includes transfer destinations) */
export type MainOutput = MainBaseOutput | DataAnalyzerOutput | ReportGeneratorOutput;

export type MainContext = {
    is_vip: boolean;
    user_id: string;
    session_id: string;
}

export type MainTools = {
    db_query_users: (args: { filter: string }) => Promise<User[]>;
    db_query_products: (args: { filter: string }) => Promise<Product[]>;
    db_query_orders: (args: { filter: string }) => Promise<Order[]>;
    db_create_user: (args: { name: string, email: string }) => Promise<User>;
    db_create_product: (args: { name: string, price: number, stock: number }) => Promise<Product>;
    db_create_order: (args: { user_id: string, product_id: string, quantity: number }) => Promise<Order>;
    sum_order_totals: (args: { orders_json: string }) => Promise<number>;
    validate_stock: (args: { product_id: string, quantity: number }) => Promise<boolean>;
    parse_csv: (args: { csv_string: string }) => Promise<string>;
    analyze_user_behavior: (args: { user_id: string }) => Promise<string>;
    detect_low_stock: (args: {  }) => Promise<string>;
    calculate_average: (args: { numbers: string }) => Promise<number>;
    find_outliers: (args: { data: string }) => Promise<string>;
    format_table: (args: { data: string }) => Promise<string>;
    generate_chart_description: (args: { data: string, chart_type: string }) => Promise<string>;
    aggregate_by_status: (args: { orders: string }) => Promise<string>;
    calculate_metrics: (args: { orders: string }) => Promise<string>;
}

/** Custom intents defined in the DSL (if any) */
export type MainCustomIntents =
    | { name: "SpeakLoud"; value: { explain: string } };

/**
 * API keys required for Main
 */
export type MainApiKeys = {
    my_groq_apiApiKey: string;  // API key for custom provider 'my-groq-api'
}

// Defined explicitly (not via ReturnType) so RouterMiddleware can derive from it without circularity
export type MainAgent = import("@snrraptopack/auwgent-sdk").TypedAuwgent<
    typeof agentIR,
    MainCustomIntents,
    MainOutput,
    MainTools
>;

/** Middleware object type — consistent with `MainAgent.onIntent` intent narrowing */
export type MainMiddleware<T extends import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent'] = import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent']> = import("@snrraptopack/auwgent-sdk").Middleware<
    typeof agentIR,
    MainCustomIntents,
    MainOutput,
    MainTools,
    T
>;

export type MainConfig = {
    tools: MainTools;
    middleware?: MainMiddleware[];
    context: MainContext;
    apiKeys: MainApiKeys;
}

export function createMain(config: MainConfig): MainAgent {
    return createAuwgent<
        typeof agentIR,
        MainCustomIntents,
        MainOutput,
        MainTools
    >(agentIR, {
        tools: config.tools,
        middleware: config.middleware as any,
        context: config.context,
        apiKeys: config.apiKeys
    });
}

export const auwgent = createMain;
export type AuwgentTools = MainTools;
export type AuwgentConfig = MainConfig;
export type AuwgentAgent = MainAgent;
export type AuwgentMiddleware = MainMiddleware;
export type AuwgentContext = MainContext;