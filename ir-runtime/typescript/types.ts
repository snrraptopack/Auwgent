export interface IRToolDef {
    name: string;
    description?: string;
    params: unknown;
    returns: unknown;
}

export interface IRModelProvider {
    type: string;
}

export interface IRModelConfig {
    model?: IRModelProvider;
}

export interface IRNamedModelConfig {
    model?: IRModelProvider;
}

export interface IRModelConfigEntry {
    defaultConfig?: IRModelConfig;
    namedConfig?: IRNamedModelConfig[] | null;
}

export interface IRWorkflowDef {
    flowName: string;
    description?: string;
    flowParams?: unknown;
    returns?: unknown;
}

export interface IRHelperDef {
    name: string;
    modelConfig?: IRModelConfigEntry[] | null;
}

export interface AgentIRShape {
    name: string;
    tools: readonly IRToolDef[];
    workflows?: readonly IRWorkflowDef[];
    helpers?: readonly IRHelperDef[];
    output?: Record<string, unknown> | null;
    modelConfig?: IRModelConfigEntry[];
}

export type ExtractToolNames<IR extends AgentIRShape> =
    IR['tools'][number]['name'];

export type ToolRegistry<IR extends AgentIRShape> = {
    [K in ExtractToolNames<IR>]: (args: Record<string, unknown>) => Promise<unknown>;
};

// ── Provider type extraction ─────────────────────────────────────────────

/** Map from provider type string → the key field name the user must supply */
type ProviderKeyMap = {
    gemini: 'geminiApiKey';
    openai: 'openaiApiKey';
    custom: 'openaiApiKey'; // custom uses OpenAI-compatible key
};

/** Extract provider `type` from a single model config */
type ExtractProviderType<T> = T extends { model?: { type: infer P } }
    ? P extends string ? P : never
    : never;

/** Extract all provider types from an array of named configs */
type ExtractNamedProviders<T> = T extends readonly (infer E)[]
    ? ExtractProviderType<E>
    : never;

/** Extract all provider types from a single model config entry */
type ExtractEntryProviders<T> = T extends { defaultConfig?: infer D; namedConfig?: infer N }
    ? ExtractProviderType<D> | (N extends readonly (infer NC)[] ? ExtractProviderType<NC> : never)
    : never;

/** Extract all provider types from an array of model config entries */
type ExtractAllProviders<T> = T extends readonly (infer E)[] ? ExtractEntryProviders<E> : never;

/** Extract provider types from helpers */
type ExtractHelperProviders<T> = T extends readonly (infer H)[]
    ? H extends { modelConfig?: infer MC } ? ExtractAllProviders<MC> : never
    : never;

/** All providers used across the entire IR (agents + helpers) */
type CollectProviders<IR extends AgentIRShape> =
    ExtractAllProviders<IR['modelConfig']> | ExtractHelperProviders<IR['helpers']>;

/** Pick only the key fields needed for the providers the IR actually uses */
type RequiredKeyFields<IR extends AgentIRShape> =
    CollectProviders<IR> extends infer P
    ? P extends keyof ProviderKeyMap
    ? ProviderKeyMap[P]
    : never
    : never;

/** Dynamic ApiKeys – only demands the keys the IR needs */
export type ApiKeys<IR extends AgentIRShape = AgentIRShape> =
    string extends CollectProviders<IR>
    ? { geminiApiKey?: string; openaiApiKey?: string }  // fallback when IR is not const
    : [RequiredKeyFields<IR>] extends [never]
    ? { geminiApiKey?: string; openaiApiKey?: string }  // no providers found
    : { [K in RequiredKeyFields<IR>]: string };

// ── Intent Types ─────────────────────────────────────────────────────────

export type IntentControl =
    | { skip: true }
    | { result: unknown }
    | void
    | null
    | undefined;

export type GetToolArgs<T> = T extends (args: infer A) => any ? A : Record<string, any>;
export type IsAny<T> = 0 extends (1 & T) ? true : false;
export type GetToolResult<T> = T extends (args: any) => Promise<infer R> ? R : any;

export interface ToolCallIntent<K = string, A = any> { name: 'tool_call'; value: { type: K; args: A } }
export interface ToolResultIntent<K = string, R = any> { name: 'tool_result'; value: { name: K; result: R; overridden?: boolean } }
export interface ToolErrorIntent<K = string> { name: 'tool_error'; value: { tool: K; message: string } }
export interface ToolSkippedIntent<K = string, A = any> { name: 'tool_skipped'; value: { type: K; args: A } }

export interface WorkflowCallIntent<K = string, A = any> { name: 'workflow_call'; value: { type: K; args: A } }
export interface WorkflowResultIntent<K = string, R = any> { name: 'workflow_result'; value: { name: K; result: R } }

export interface HelperCallIntent<K = string, A = any> { name: 'helper_call'; value: { type: K; args: A } }
export interface HelperResultIntent<K = string, R = any> { name: 'helper_result'; value: { name: K; result: R } }

export interface ResponseTextIntent { name: 'response_text'; value: { text: string } }
export interface ResponseSchemaIntent<Output = any> { name: 'response_schema'; value: Output }

export interface ErrorIntent { name: 'error'; value: { message: string } }

export type ToolIntents<Tools> =
    IsAny<Tools> extends true ? (ToolCallIntent | ToolResultIntent | ToolErrorIntent | ToolSkippedIntent) :
    [Tools] extends [never] ? never :
    [Tools] extends [Record<string, never>] ? never :
    string extends keyof Tools
    ? (ToolCallIntent | ToolResultIntent | ToolErrorIntent | ToolSkippedIntent)
    : {
        [K in keyof Tools]:
        | ToolCallIntent<K, GetToolArgs<Tools[K]>>
        | ToolResultIntent<K, GetToolResult<Tools[K]>>
        | ToolErrorIntent<K>
        | ToolSkippedIntent<K, GetToolArgs<Tools[K]>>
    }[keyof Tools];

export type WorkflowIntents<IR extends AgentIRShape> =
    IR['workflows'] extends readonly any[]
    ? (IR['workflows'][number] extends never ? never :
        IR['workflows'][number] extends infer W extends IRWorkflowDef
        ? W extends { flowName: infer N extends string }
        ? (WorkflowCallIntent<N, any> | WorkflowResultIntent<N, W['returns']>)
        : (WorkflowCallIntent | WorkflowResultIntent)
        : never
    )
    : never;

export type HelperIntents<IR extends AgentIRShape> =
    IR['helpers'] extends readonly any[]
    ? (IR['helpers'][number] extends never ? never :
        IR['helpers'][number] extends infer H extends IRHelperDef
        ? H extends { name: infer N extends string }
        ? (HelperCallIntent<N, any> | HelperResultIntent<N, any>)
        : (HelperCallIntent | HelperResultIntent)
        : never
    )
    : never;

export type CoreIntents<IR extends AgentIRShape, Output = any, Tools = any> = (
    | ToolIntents<Tools>
    | WorkflowIntents<IR>
    | HelperIntents<IR>
    | (IR['output'] extends Record<string, any>
        ? ([Output] extends [never] ? ResponseTextIntent : ResponseSchemaIntent<Output>)
        : ResponseTextIntent)
    | ErrorIntent
);

export type AuwgentIntent<IR extends AgentIRShape, Custom = never, Output = any, Tools = any> =
    | CoreIntents<IR, Output, Tools>
    | (Custom extends never ? never : { name: string; value: any });

/** Subset of intents that are generated by the LLM (model -> engine) */
export type AuwgentModelIntent<IR extends AgentIRShape, Custom = never, Output = any, Tools = any> =
    Extract<AuwgentIntent<IR, Custom, Output, Tools>, { name: 'tool_call' | 'workflow_call' | 'helper_call' | 'response_text' | 'response_schema' }>;

/** Values generated by the LLM that are intended as terminal outputs (text or schema) */
export type AuwgentResponseValue<IR extends AgentIRShape, Custom = never, Output = any, Tools = any> =
    Extract<AuwgentIntent<IR, Custom, Output, Tools>, { name: 'response_text' | 'response_schema' }>['value'];

export type AuwgentModelValue<IR extends AgentIRShape, Custom = never, Output = any, Tools = any> =
    AuwgentResponseValue<IR, Custom, Output, Tools>;

/** 
 * Narrowed response value for a specific target agent.
 * If target is the root agent, returns full Output-aware value.
 * If target is a helper, returns helper's specific output or {text: string}.
 */
export type AuwgentTargetedResponseValue<
    IR extends AgentIRShape,
    T extends string,
    Custom = never,
    Output = any,
    Tools = any
> =
    T extends (string extends IR['name'] ? string : IR['name'])
    ? AuwgentResponseValue<IR, Custom, Output, Tools>
    : (IR['helpers'] extends readonly any[]
        ? (Extract<IR['helpers'][number], { name: T }> extends { output: infer O }
            ? (O extends null ? { text: string } : O)
            : { text: string })
        : { text: string });

export type IntentHandler<IR extends AgentIRShape = any, Custom = never, Output = any, Tools = any> = (
    ...args: {
        [K in AuwgentIntent<IR, Custom, Output, Tools>['name']]: [
            name: K,
            value: Extract<AuwgentIntent<IR, Custom, Output, Tools>, { name: K }>['value'],
        ];
    }[AuwgentIntent<IR, Custom, Output, Tools>['name']]
) => IntentControl | Promise<IntentControl>;

export type IntentHandlers<IR extends AgentIRShape = any, Custom = never, Output = any, Tools = any> = {
    [K in AuwgentIntent<IR, Custom, Output, Tools>['name']]?: (
        value: Extract<AuwgentIntent<IR, Custom, Output, Tools>, { name: K }>['value']
    ) => IntentControl | Promise<IntentControl> | void | Promise<void>;
};

export type PartialIntentHandlers<IR extends AgentIRShape = any, Custom = never, Output = any, Tools = any> = {
    [K in AuwgentIntent<IR, Custom, Output, Tools>['name']]?: (
        value: Extract<AuwgentIntent<IR, Custom, Output, Tools>, { name: K }>['value']
    ) => void;
};

export type PartialIntentHandler<
    IR extends AgentIRShape = any,
    Custom = never,
    Output = any,
    Tools = any
> = (
    ...args: {
        [K in AuwgentIntent<IR, Custom, Output, Tools>['name']]: [
            name: K,
            value: Extract<AuwgentIntent<IR, Custom, Output, Tools>, { name: K }>['value'],
        ];
    }[AuwgentIntent<IR, Custom, Output, Tools>['name']]
) => void;

// ── Session Types ────────────────────────────────────────────────────────

export type Role = 'system' | 'user' | 'model' | 'toolResult';

export interface Message {
    role: Role;
    content: string;
}

export interface Turn {
    input: string;
    model_response: string;
}

export interface SessionState {
    systemPrompt?: string;
    turns: Turn[];
}
