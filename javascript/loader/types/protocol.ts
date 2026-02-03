/**
 * specific implementation of JSON Schema for our runtime.
 * This is the "Assembly Language" for structured outputs.
 */
export interface JsonSchema {
    type?: string;
    description?: string;
    properties?: Record<string, JsonSchema>;
    required?: string[];
    items?: JsonSchema;
    enum?: string[];
    anyOf?: JsonSchema[];
}

/**
 * JSON-safe value used in tool arguments, results, and schemas.
 */
export type JsonValue =
    | string
    | number
    | boolean
    | null
    | JsonValue[]
    | { [k: string]: JsonValue };

/**
 * Tool argument map passed to tool implementations.
 */
export type ToolArgs = Record<string, JsonValue>;
/**
 * Tool result returned to the runtime and the model.
 */
export type ToolResult = JsonValue;

/**
 * Normalized content blocks returned by model drivers.
 * Each block represents a single semantic unit in the assistant output.
 */
export type ContentBlock =
    | { type: 'text'; text: string }
    | { type: 'thinking'; text?: string; summary?: string; redacted?: boolean; tokenCount?: number }
    | { type: 'tool_use'; id: string; name: string; input: ToolArgs }
    | { type: 'tool_result'; toolUseId: string; content: ContentBlock[] | string; isError?: boolean };

/**
 * Narrowed tool-use block for middleware hook typing.
 */
export type ToolUseBlock = Extract<ContentBlock, { type: 'tool_use' }>;

/**
 * Intent parsed from YAML output - represents a tool call, workflow, or response.
 */
export interface YamlIntent {
    type: 'tool_call' | 'workflow' | 'helper' | 'respond' | 'question';
    name: string;
    args?: Record<string, any>;
}

/**
 * Parsed YAML output from model response.
 * This is the core structured output format for the YAML-based architecture.
 */
export interface YamlOutput {
    /** Text response to the user */
    text?: string;
    /** When true, execute intents in parallel */
    parallel?: boolean;
    /** List of intents (tool calls, workflows, etc.) */
    intents?: YamlIntent[];
    /** Structured output matching the agent's output schema */
    output?: Record<string, any>;
    /** Follow-up question for the user */
    question?: string;
}

/**
 * Tool definition provided to model drivers.
 */
export interface SyntheticToolDef {
    name: string;
    description: string;
    parameters: JsonSchema;
    _meta?: Record<string, JsonValue>;
}

/**
 * A normalized message format.
 * Drivers map this to their specific SDK message types.
 */
export type SyntheticMessage =
    | { role: 'system' | 'user'; content: ContentBlock[] | string }
    | { role: 'assistant'; content: ContentBlock[]; toolCalls?: ToolCall[] }
    | { role: 'tool'; content: ContentBlock[] | string; toolCallId: string; name: string };

/**
 * Structured response requirements for the model.
 */
export type ResponseFormat =
    | { type: 'text' }
    | { type: 'json_object' }
    | { type: 'json_schema'; schema: JsonSchema };

/**
 * Reasoning configuration for supported model providers.
 */
export type ReasoningConfig = {
    enabled: boolean;
    budgetTokens?: number;
    effort?: 'low' | 'medium' | 'high';
    visible?: boolean;
};

/**
 * Tool selection policy sent to the model.
 */
export type ToolChoice = 'auto' | 'required' | 'none' | { name: string };

/**
 * The Normalized LLM Interaction Object (NLIO).
 * This contains EVERYTHING a driver needs to execute a request.
 */
export interface SyntheticRequest {
    /** The strict conversation history */
    messages: SyntheticMessage[];

    /**
     * The schema for the expected response.
     * If present, the driver MUST enforce this structure.
     */
    responseSchema?: JsonSchema;

    /**
     * Structured response requirements for the final model response.
     */
    responseFormat?: ResponseFormat;

    /**
     * Reasoning configuration for compatible providers.
     */
    reasoning?: ReasoningConfig;

    /**
     * Tool selection policy used by the provider.
     */
    toolChoice?: ToolChoice;


    /**
     * Tool definitions the model can call.
     */
    tools?: SyntheticToolDef[];



    /** Model configuration hints */
    config: {
        model?: string;  // Provider type: "gemini", "openai", "custom"
        modelName?: string;  // Actual model name
        temperature?: number;
        providerConfig?: Record<string, any>;
    };
}

/**
 * Normalized model response emitted by drivers.
 */
export interface DriverResult {
    content?: ContentBlock[];
    toolCalls?: ToolCall[];
    usage?: ModelUsage;
    thinking?: ThinkingBlock;
    stopReason?: string;
    raw?: unknown;
}

/**
 * Alias for proposal naming.
 */
export type ModelRequest = SyntheticRequest;
/**
 * Alias for proposal naming.
 */
export type ModelResponse = DriverResult;

/**
 * Stream chunk types for async generator streaming
 */
export type StreamChunk =
    | { type: 'text'; delta: string; format?: 'yaml' | 'json'; raw?: string }
    | { type: 'tool_start'; name: string; id: string }
    | { type: 'tool_args'; id: string; delta: string }
    | { type: 'tool_end'; id: string }
    | { type: 'tool_result'; name: string; result: ToolResult }
    | { type: 'transfer'; mode: 'direct' | 'thenContinue'; helperName: string }
    | { type: 'helper_start'; name: string }
    | { type: 'helper_end'; name: string; result: any }
    | { type: 'helper_chunk'; name: string; chunk: StreamChunk };

/**
 * The interface every provider driver must implement.
 */
export interface AgentDriver {
    name: string;
    /**
     * Execute the synthetic request and return the complete result.
     */
    execute(request: SyntheticRequest): Promise<DriverResult>;

    /**
     * Execute with streaming - yields chunks as they arrive.
     * Returns final DriverResult when complete.
     */
    executeStream?(request: SyntheticRequest): AsyncGenerator<StreamChunk, DriverResult, unknown>;
}

/**
 * Token usage metadata reported by providers.
 */
export interface ModelUsage {
    input: number;
    response: number;
    thinking?: number;
    total: number;
    /** Tokens served from cache (reduces cost) */
    cachedInput?: number;
}

/**
 * Provider-agnostic reasoning block.
 * This is emitted only when reasoning is enabled by the provider.
 */
export interface ThinkingBlock {
    text?: string;
    summary?: string;
    redacted?: boolean;
    tokenCount?: number;
}

/**
 * Tool call emitted by the model driver.
 * The runtime uses this to execute the named tool.
 */
export interface ToolCall {
    id: string;
    name: string;
    args: ToolArgs;
}

/**
 * Runtime context passed to every middleware hook.
 * Context fields are mutable unless otherwise specified.
 */
export interface MiddlewareContext<TInput, TContext, TState> {
    /** Agent name from the compiled workflow. */
    agentName: string;
    /** Unique run identifier for tracing and correlation. */
    runId: string;
    /** Current attempt counter used by retry logic. */
    attempt: number;
    /** Start time in milliseconds since epoch. */
    startedAt: number;
    /** Mutable middleware state shared across hooks. */
    state: TState;
    /** Original agent input payload. */
    input: TInput;
    /** Optional user context supplied at run-time. */
    userContext?: TContext;
    /** Normalized request sent to the model provider. */
    request: ModelRequest;
    /** Normalized response after model execution, if available. */
    response?: ModelResponse;
    /** Current workflow name if executing inside a workflow. */
    workflowName?: string;
    /** Current step index within a workflow (0-based). */
    stepIndex?: number;
}

/**
 * Result returned by onBeforeHelper to skip execution and return cached result.
 */
export interface HelperSkipResult {
    skip: true;
    result: any;
}

/**
 * Result returned by onBeforeStep to skip execution and return cached result.
 */
export interface StepSkipResult {
    skip: true;
    result: any;
}

/**
 * Result returned by onWorkflowStart to resume from a specific step.
 */
export interface WorkflowResumeResult {
    resumeFromStep?: number;
    scopeSnapshot?: Record<string, any>;
}

/**
 * Middleware hooks for observing and controlling agent execution.
 * Hooks run in middleware order unless otherwise noted.
 */
export interface AgentMiddleware<TInput, TContext, TState> {
    /** Human-readable identifier for the middleware. */
    name: string;
    /** Order precedence for middleware execution (lower runs first). */
    priority?: number;
    
    // ═══════════════════════════════════════════════════════════════════
    // AGENT PHASE - Core agent/model interaction hooks
    // ═══════════════════════════════════════════════════════════════════
    
    /** Called once when a run begins. */
    onAgentStart?: (ctx: MiddlewareContext<TInput, TContext, TState>) => void | Promise<void>;
    /** Observe or modify the request before the model call. */
    onBeforeModel?: (ctx: MiddlewareContext<TInput, TContext, TState>) => void | ModelRequest | Promise<void | ModelRequest>;
    /** Wrap the model call to add retry, caching, or instrumentation. */
    wrapModelCall?: (
        ctx: MiddlewareContext<TInput, TContext, TState>,
        next: () => Promise<ModelResponse>
    ) => Promise<ModelResponse>;
    /** Observe or transform reasoning blocks when emitted. */
    onThinking?: (
        ctx: MiddlewareContext<TInput, TContext, TState>,
        thinking: ThinkingBlock
    ) => void | ThinkingBlock | Promise<void | ThinkingBlock>;
    /** Observe or modify the model response. */
    onAfterModel?: (ctx: MiddlewareContext<TInput, TContext, TState>, res: ModelResponse) => void | ModelResponse | Promise<void | ModelResponse>;
    /** Approve or block tool execution. */
    onBeforeTool?: (ctx: MiddlewareContext<TInput, TContext, TState>, tool: ToolUseBlock) => boolean | void | Promise<boolean | void>;
    /** Wrap the tool call to add instrumentation or modify results. */
    wrapToolCall?: (
        ctx: MiddlewareContext<TInput, TContext, TState>,
        tool: ToolUseBlock,
        next: (args: ToolArgs) => Promise<ToolResult>
    ) => Promise<ToolResult>;
    /** Observe tool results after execution. */
    onAfterTool?: (ctx: MiddlewareContext<TInput, TContext, TState>, tool: ToolUseBlock, result: ToolResult) => void | Promise<void>;
    /** Handle errors and optionally request a retry. */
    onError?: (
        ctx: MiddlewareContext<TInput, TContext, TState>,
        error: Error,
        phase: 'model' | 'tool' | 'thinking' | 'helper' | 'workflow'
    ) => { retry: boolean; delayMs?: number } | void | Promise<{ retry: boolean; delayMs?: number } | void>;
    /** Called once when a run completes or fails. */
    onAgentEnd?: (ctx: MiddlewareContext<TInput, TContext, TState>, result?: ModelResponse, error?: Error) => void | Promise<void>;
    
    // ═══════════════════════════════════════════════════════════════════
    // HELPER PHASE - Hooks for helper (sub-agent) execution
    // ═══════════════════════════════════════════════════════════════════
    
    /** Called before executing a helper. Return { skip: true, result } to use cached result. */
    onBeforeHelper?: (
        ctx: MiddlewareContext<TInput, TContext, TState>,
        helperName: string,
        args: Record<string, any>
    ) => void | HelperSkipResult | Promise<void | HelperSkipResult>;
    
    /** Called after a helper completes successfully. */
    onAfterHelper?: (
        ctx: MiddlewareContext<TInput, TContext, TState>,
        helperName: string,
        args: Record<string, any>,
        result: any
    ) => void | Promise<void>;
    
    // ═══════════════════════════════════════════════════════════════════
    // WORKFLOW PHASE - Hooks for workflow step execution
    // ═══════════════════════════════════════════════════════════════════
    
    /** Called when a workflow begins. Return resumeFromStep to skip completed steps. */
    onWorkflowStart?: (
        ctx: MiddlewareContext<TInput, TContext, TState>,
        workflowName: string,
        args: Record<string, any>
    ) => void | WorkflowResumeResult | Promise<void | WorkflowResumeResult>;
    
    /** Called before each workflow step. Return { skip: true, result } to use cached result. */
    onBeforeStep?: (
        ctx: MiddlewareContext<TInput, TContext, TState>,
        stepIndex: number,
        stepType: string
    ) => void | StepSkipResult | Promise<void | StepSkipResult>;
    
    /** Called after each workflow step completes. */
    onAfterStep?: (
        ctx: MiddlewareContext<TInput, TContext, TState>,
        stepIndex: number,
        stepType: string,
        result: any
    ) => void | Promise<void>;
    
    /** Called when a workflow completes or fails. */
    onWorkflowEnd?: (
        ctx: MiddlewareContext<TInput, TContext, TState>,
        workflowName: string,
        result: any,
        error?: Error
    ) => void | Promise<void>;
}
