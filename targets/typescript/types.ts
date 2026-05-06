export interface IRToolDef {
    name: string;
    description?: string;
    params: unknown;
    returns: unknown;
}

export interface IRCustomIntentDef {
    name: string;
    description?: string;
    fields: unknown;
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
    input?: any;
    modelConfig?: IRModelConfigEntry[] | null;
    customIntents?: readonly IRCustomIntentDef[];
    output?: any;
}

export interface IRComponentChildrenDef {
    kind: 'all' | 'only';
    components?: readonly string[];
}

export interface IRComponentActionTargetDef {
    name: string;
    params?: unknown;
}

export interface IRComponentDef {
    name: string;
    props?: unknown;
    action?: Record<string, readonly IRComponentActionTargetDef[]>;
    children?: IRComponentChildrenDef;
}

/**
 * Custom error thrown/emitted when a tool execution fails.
 */
export class AuwgentToolError extends Error {
    constructor(public toolName: string, message: string) {
        super(`Tool [${toolName}] failed: ${message}`);
        this.name = 'AuwgentToolError';
    }
}

export type AuwgentWarningSource =
    | 'onIntent'
    | 'onIntentPartial'
    | 'onMiddlewareEvent'
    | 'onSubEngineStart'
    | 'onSubEngineComplete'
    | 'middleware'
    | 'run'
    | 'embed'
    | 'embedBatch';

export interface AuwgentWarning {
    timestamp: string;
    source: AuwgentWarningSource;
    message: string;
    detail?: string;
    agentName?: string;
}

export interface AgentIRShape {
    name: string;
    input: any; // Now mandatory in IR for type extraction
    lifecycle?: any;
    tools: readonly IRToolDef[];
    workflows?: readonly IRWorkflowDef[];
    helpers?: readonly IRHelperDef[];
    components?: readonly IRComponentDef[];
    types?: Record<string, any>;
    output?: any;
    modelConfig?: IRModelConfigEntry[];
    customIntents?: readonly IRCustomIntentDef[];
}

export type AuwgentTextPart = { type: 'text'; text: string };
export type AuwgentBinarySource =
    | { data: ArrayBuffer | Uint8Array | string; encoding?: 'base64' | 'utf8' }
    | { path: string }
    | { url: string }
    | { ref: string };
export type AuwgentImagePart = AuwgentBinarySource & { type: 'image'; mimeType?: string; detail?: 'auto' | 'low' | 'high' };
export type AuwgentFilePart = AuwgentBinarySource & { type: 'file'; mimeType?: string; name?: string };
export type AuwgentAudioPart = AuwgentBinarySource & { type: 'audio'; mimeType?: string; transcript?: string };
export type AuwgentVideoPart = AuwgentBinarySource & { type: 'video'; mimeType?: string; transcript?: string; sampledFrames?: AuwgentImagePart[] };
export type AuwgentInputPart =
    | AuwgentTextPart
    | AuwgentImagePart
    | AuwgentFilePart
    | AuwgentAudioPart
    | AuwgentVideoPart;

type PrimitiveTypeName = 'string' | 'Text' | 'text' | 'number' | 'int' | 'float' | 'boolean' | 'bool' | 'image' | 'file' | 'audio' | 'video';
type Simplify<T> = { [K in keyof T]: T[K] } & {};

type NormalizePrimitive<T> =
    T extends 'string' | 'Text' | 'text' ? string :
    T extends 'number' | 'int' | 'float' ? number :
    T extends 'boolean' | 'bool' ? boolean :
    T extends 'image' ? AuwgentImagePart :
    T extends 'file' ? AuwgentFilePart :
    T extends 'audio' ? AuwgentAudioPart :
    T extends 'video' ? AuwgentVideoPart :
    never;

type InputMediaPart<T> =
    T extends 'image' ? AuwgentImagePart :
    T extends 'file' ? AuwgentFilePart :
    T extends 'audio' ? AuwgentAudioPart :
    T extends 'video' ? AuwgentVideoPart :
    never;

type InputMediaArray<T> = readonly (AuwgentTextPart | InputMediaPart<T>)[];

type NormalizeUnionOption<T> =
    T extends PrimitiveTypeName ? NormalizePrimitive<T> :
    T extends string ? T :
    IRSchemaToType<T>;

type IsRawPropertyMap<T> =
    T extends Record<string, any>
    ? [keyof T] extends [never]
        ? false
        : T[keyof T] extends { type?: any }
            ? true
            : false
    : false;

type RequiredPropertyKeys<T extends Record<string, any>> = {
    [K in keyof T]-?: T[K] extends { optional: true } ? never : K;
}[keyof T];

type OptionalPropertyKeys<T extends Record<string, any>> = {
    [K in keyof T]-?: T[K] extends { optional: true } ? K : never;
}[keyof T];

type PropertyValueType<T> =
    T extends { type: infer U } ? IRSchemaToType<U> : IRSchemaToType<T>;

type PropertyMapToType<T extends Record<string, any>> = Simplify<
    { [K in RequiredPropertyKeys<T>]: PropertyValueType<T[K]> } &
    { [K in OptionalPropertyKeys<T>]?: PropertyValueType<T[K]> }
>;

export type IRSchemaToType<T> =
    [T] extends [null | undefined] ? never :
    T extends string ? (NormalizePrimitive<T> extends never ? T : NormalizePrimitive<T>) :
    T extends { kind: 'properties', fields: infer F extends Record<string, any> } ? PropertyMapToType<F> :
    T extends { type: infer U }
    ? U extends 'array'
        ? T extends { items: infer I } ? IRSchemaToType<I>[] : unknown[]
        : U extends 'object'
            ? T extends { properties: infer P extends Record<string, any> } ? PropertyMapToType<P> : Record<string, unknown>
            : U extends 'union'
                ? T extends { options: infer O extends readonly any[] } ? NormalizeUnionOption<O[number]> : unknown
                : U extends 'typeRef'
                    ? unknown
                    : U extends PrimitiveTypeName
                        ? NormalizePrimitive<U>
                        : U extends object
                            ? IRSchemaToType<U>
                            : unknown
    : IsRawPropertyMap<T> extends true
        ? PropertyMapToType<Extract<T, Record<string, any>>>
        : T;

type ResolveShape<T, NullFallback> =
    [T] extends [null | undefined] ? NullFallback : IRSchemaToType<T>;

type ResolveInputShape<T> =
    [T] extends [null | undefined] ? string :
    T extends 'image' | 'file' | 'audio' | 'video' ? InputMediaArray<T> :
    T extends { type: 'union'; options: infer O extends readonly any[] }
        ? InputMediaArray<Extract<O[number], 'image' | 'file' | 'audio' | 'video'>>
        : IRSchemaToType<T>;

export type ExtractToolNames<IR extends AgentIRShape> =
    IR['tools'][number]['name'];

export type ExtractHelperNames<IR extends AgentIRShape> =
    IR['helpers'] extends readonly any[]
    ? IR['helpers'][number]['name']
    : never;

type ExtractToolDef<IR extends AgentIRShape, K extends ExtractToolNames<IR>> =
    Extract<IR['tools'][number], { name: K }>;

type ExtractToolArgsFromIR<IR extends AgentIRShape, K extends ExtractToolNames<IR>> =
    ExtractToolDef<IR, K> extends { params: infer P } ? ResolveShape<P, Record<string, never>> : Record<string, never>;

type ExtractToolResultFromIR<IR extends AgentIRShape, K extends ExtractToolNames<IR>> =
    ExtractToolDef<IR, K> extends { returns: infer R } ? ResolveShape<R, unknown> : unknown;

type ExtractWorkflowArgs<W> =
    W extends { flowParams: infer P } ? ResolveShape<P, Record<string, never>> : Record<string, never>;

type ExtractWorkflowResult<W> =
    W extends { returns: infer R } ? ResolveShape<R, unknown> : unknown;

type ExtractHelperInput<H> =
    H extends { input: infer I } ? ResolveShape<I, string> : string;

type ExtractHelperOutput<H> =
    H extends { output: infer O } ? ResolveShape<O, { text: string }> : { text: string };

/**
 * Collects all possible output shapes from the root agent AND all its helpers.
 * Used to ensure `response_schema` autocompletion works for everything.
 */
export type CollectAllOutputs<IR extends AgentIRShape> =
    | ResolveShape<IR['output'], never>
    | (IR['helpers'] extends readonly any[]
        ? ExtractHelperOutput<IR['helpers'][number]>
        : never);

export type ToolRegistry<IR extends AgentIRShape> = {
    [K in ExtractToolNames<IR>]: (args: ExtractToolArgsFromIR<IR, K>) => Promise<ExtractToolResultFromIR<IR, K>>;
};

export type ExtractInputShape<IR extends AgentIRShape> =
    ResolveInputShape<IR['input']>;


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

/** Extract custom ID from a single model config */
type ExtractCustomId<T> = T extends { model?: { type: 'custom'; id: infer I } }
    ? I extends string ? `${I}ApiKey` : never
    : never;

/** Extract all custom IDs from an array of model config entries */
type ExtractAllCustomIds<T> = T extends readonly (infer E)[]
    ? (E extends { defaultConfig?: infer D } ? ExtractCustomId<D> : never)
    | (E extends { namedConfig?: readonly (infer NC)[] } ? ExtractCustomId<NC> : never)
    : never;

/** Extract custom IDs from helpers */
type ExtractHelperCustomIds<T> = T extends readonly (infer H)[]
    ? H extends { modelConfig?: infer MC } ? ExtractAllCustomIds<MC> : never
    : never;

/** All custom provider IDs used across the entire IR */
type CollectCustomIds<IR extends AgentIRShape> =
    ExtractAllCustomIds<IR['modelConfig']> | ExtractHelperCustomIds<IR['helpers']>;

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
  ? { [key: string]: string | undefined } & {
    geminiApiKey?: string;
    openaiApiKey?: string;
    groqApiKey?: string;
    customUrl?: string
  }
    : { [K in RequiredKeyFields<IR>]: string } & { [K in CollectCustomIds<IR>]: string };

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

export interface ToolCallIntent<K extends string = string, A = any> { name: 'tool_call'; value: { type: K; args: A } }
export interface ToolResultIntent<K extends string = string, A = any, R = any> { name: 'tool_result'; value: { name: K; args: A; result: R; overridden?: boolean } }
export interface ToolErrorIntent<K extends string = string> { name: 'tool_error'; value: { tool: K; message: string } }
export interface ToolSkippedIntent<K extends string = string, A = any> { name: 'tool_skipped'; value: { type: K; args: A } }

export interface WorkflowCallIntent<K = string, A = any> { name: 'workflow_call'; value: { type: K; args: A } }
export interface WorkflowResultIntent<K = string, A = any, R = any> { name: 'workflow_result'; value: { name: K; args: A; result: R } }

export interface HelperCallIntent<K = string, A = any> { name: 'helper_call'; value: { type: K; args: A } }
export interface HelperResultIntent<K = string, A = any, R = any> { name: 'helper_result'; value: { name: K; args: A; result: R } }

export interface ComponentIntent<K = string, P = any, A = any> {
    name: 'component';
    value: {
        type: K;
        c_id: string;
        props: P;
        action?: A;
        children?: readonly string[];
    };
}

export interface RenderComponentIntent {
    name: 'render_component';
    value: {
        root?: string;
        roots?: readonly string[];
        components?: Record<string, any>;
        tree?: any;
        trees?: readonly any[];
    };
}

export interface ResponseTextIntent { name: 'response_text'; value: { text: string } }

type RootOutputSchemaName<IR extends AgentIRShape> =
    string extends IR['name'] ? never : `${IR['name']}Output`;

type HelperOutputSchemaNames<IR extends AgentIRShape> =
    IR['helpers'] extends readonly any[]
    ? IR['helpers'][number] extends { name: infer N extends string }
        ? `${N}Output`
        : never
    : never;

type DeclaredSchemaNames<IR extends AgentIRShape> =
    IR['types'] extends Record<string, any>
    ? keyof IR['types'] & string
    : never;

type ResponseSchemaTypeName<IR extends AgentIRShape> =
    string extends IR['name']
    ? string
    : RootOutputSchemaName<IR> | HelperOutputSchemaNames<IR> | DeclaredSchemaNames<IR>;

export interface ResponseSchemaIntent<Output = any, SchemaType extends string = string> {
    name: 'response_schema';
    value: {
        type: SchemaType;
        response: Output;
    };
}
export interface PendingStreamValue { $state: 'pending' }
export interface PartialIntentEnvelope {
    partial: true
    complete: false
    mode: 'text' | 'structured'
    segment: number
    raw: string
}
export type PartialTextIntentValue = PartialIntentEnvelope & {
    mode: 'text'
    text: string
    delta?: string
}
export type PartialStructuredIntentValue<T> = PartialIntentEnvelope & Omit<T, keyof PartialIntentEnvelope | 'mode'> & {
    mode: 'structured'
}

export interface ErrorIntent { name: 'error'; value: { message: string } }

export type ToolIntents<Tools> =
    IsAny<Tools> extends true ? (ToolCallIntent | ToolResultIntent | ToolErrorIntent | ToolSkippedIntent) :
    [Tools] extends [never] ? never :
    [Tools] extends [Record<string, never>] ? never :
    string extends keyof Tools
    ? (ToolCallIntent | ToolResultIntent | ToolErrorIntent | ToolSkippedIntent)
    : {
        [K in keyof Tools & string]:
        | ToolCallIntent<K, GetToolArgs<Tools[K]>>
        | ToolResultIntent<K, GetToolArgs<Tools[K]>, GetToolResult<Tools[K]>>
        | ToolErrorIntent<K>
        | ToolSkippedIntent<K, GetToolArgs<Tools[K]>>
    }[keyof Tools & string];

export type WorkflowIntents<IR extends AgentIRShape> =
    IR['workflows'] extends readonly any[]
    ? (IR['workflows'][number] extends never ? never :
        IR['workflows'][number] extends infer W extends IRWorkflowDef
        ? W extends { flowName: infer N extends string }
        ? (WorkflowCallIntent<N, ExtractWorkflowArgs<W>> | WorkflowResultIntent<N, ExtractWorkflowArgs<W>, ExtractWorkflowResult<W>>)
        : (WorkflowCallIntent | WorkflowResultIntent)
        : never
    )
    : never;

export type HelperIntents<IR extends AgentIRShape> =
    IR['helpers'] extends readonly any[]
    ? (IR['helpers'][number] extends never ? never :
        IR['helpers'][number] extends infer H extends IRHelperDef
        ? H extends { name: infer N extends string }
        ? (HelperCallIntent<N, ExtractHelperInput<H>> | HelperResultIntent<N, ExtractHelperInput<H>, ExtractHelperOutput<H>>)
        : (HelperCallIntent | HelperResultIntent)
        : never
    )
    : never;

export type CoreIntents<IR extends AgentIRShape, Output = any, Tools = any> = (
    | ToolIntents<Tools>
    | WorkflowIntents<IR>
    | HelperIntents<IR>
    | (IR['components'] extends readonly any[] ? (ComponentIntent | RenderComponentIntent) : never)
    | (IsAny<Output> extends true ? (ResponseTextIntent | ResponseSchemaIntent<any, ResponseSchemaTypeName<IR>>) :
        [Output] extends [never] ? ResponseTextIntent : (ResponseTextIntent | ResponseSchemaIntent<Output, ResponseSchemaTypeName<IR>>))
    | ErrorIntent
);

export type AuwgentIntent<
    IR extends AgentIRShape,
    Custom extends { name: string; value: any } = never,
    Output = any,
    Tools = any
> =
    | CoreIntents<IR, Output, Tools>
    | Custom;

/** Subset of intents that are generated by the LLM (model -> engine) */
export type AuwgentModelIntent<
    IR extends AgentIRShape,
    Custom extends { name: string; value: any } = never,
    Output = any,
    Tools = any
> =
    | Extract<AuwgentIntent<IR, Custom, Output, Tools>, { name: 'tool_call' | 'workflow_call' | 'helper_call' | 'component' | 'render_component' | 'response_text' | 'response_schema' }>
    | Custom;

/** Values generated by the LLM that are intended as terminal outputs (text or schema) */
export type AuwgentResponseValue<
    IR extends AgentIRShape,
    Custom extends { name: string; value: any } = never,
    Output = any,
    Tools = any
> =
    Extract<AuwgentIntent<IR, Custom, Output, Tools>, { name: 'response_text' | 'response_schema' }>['value'];

export type AuwgentModelValue<
    IR extends AgentIRShape,
    Custom extends { name: string; value: any } = never,
    Output = any,
    Tools = any
> =
    Extract<AuwgentModelIntent<IR, Custom, Output, Tools>, { name: AuwgentModelIntent<IR, Custom, Output, Tools>['name'] }>['value'];

/**
 * Narrowed response value for a specific target agent.
 * If target is the root agent, returns full Output-aware value.
 * If target is a helper, returns helper's specific output or {text: string}.
 */
export type AuwgentTargetedResponseValue<
    IR extends AgentIRShape,
    T extends string,
    Custom extends { name: string; value: any } = never,
    Output = any,
    Tools = any
> =
    T extends (string extends IR['name'] ? string : IR['name'])
    ? AuwgentResponseValue<IR, Custom, Output, Tools>
    : (IR['helpers'] extends readonly any[]
        ? (Extract<IR['helpers'][number], { name: T }> extends { output: infer O }
            ? ExtractHelperOutput<{ output: O }>
            : { text: string })
        : { text: string });

export type IntentHandler<
    IR extends AgentIRShape = any,
    Custom extends { name: string; value: any } = never,
    Output = any,
    Tools = any
> = (
    ...args: {
        [K in AuwgentIntent<IR, Custom, Output, Tools>['name'] & string]: [
            name: K,
            value: Extract<AuwgentIntent<IR, Custom, Output, Tools>, { name: K }>['value'],
            agentName: string,
        ];
    }[AuwgentIntent<IR, Custom, Output, Tools>['name'] & string]
) => IntentControl | Promise<IntentControl>;

export type IntentHandlers<
    IR extends AgentIRShape = any,
    Custom extends { name: string; value: any } = never,
    Output = any,
    Tools = any
> = {
        [K in AuwgentIntent<IR, Custom, Output, Tools>['name'] & string]?: (
            value: Extract<AuwgentIntent<IR, Custom, Output, Tools>, { name: K }>['value'],
            agentName: string
        ) => IntentControl | Promise<IntentControl> | void | Promise<void>;
    };

export type PartialIntentHandlers<
    IR extends AgentIRShape = any,
    Custom extends { name: string; value: any } = never,
    Output = any,
    Tools = any
> = {
        [K in AuwgentIntent<IR, Custom, Output, Tools>['name'] & string]?: (
            value: K extends 'response_text'
                ? PartialTextIntentValue
                : PartialStructuredIntentValue<Extract<AuwgentIntent<IR, Custom, Output, Tools>, { name: K }>['value']>,
            agentName: string
        ) => void;
    };

export type PartialIntentHandler<
    IR extends AgentIRShape = any,
    Custom extends { name: string; value: any } = never,
    Output = any,
    Tools = any
> = (
    ...args: {
        [K in AuwgentIntent<IR, Custom, Output, Tools>['name'] & string]: [
            name: K,
            value: K extends 'response_text'
                ? PartialTextIntentValue
                : PartialStructuredIntentValue<Extract<AuwgentIntent<IR, Custom, Output, Tools>, { name: K }>['value']>,
            agentName: string,
        ];
    }[AuwgentIntent<IR, Custom, Output, Tools>['name'] & string]
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
    stack: string[];
    initialInput?: any;
    bindingCursor?: BindingCursor | null;
}

export interface TokenUsage {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
    reasoning_tokens: number;
    cached_tokens: number;
}

export type FinishReason =
    | 'stop'
    | 'length'
    | 'tool_calls'
    | 'content_filter'
    | { other: string };

export interface TurnMetadata {
    turn_index: number;
    usage: TokenUsage;
    finish_reason: FinishReason | null;
    model: string;
}

export interface AggregateUsage {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
    reasoning_tokens: number;
    cached_tokens: number;
}

export interface BindingCursor {
    turnIndex: number | null;
    role: string;
    input: string | null;
}

export interface RunMetadata {
    aggregate: AggregateUsage;
    turns: TurnMetadata[];
}
