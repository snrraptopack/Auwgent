/**
 * A general function signature for a tool implementation.
 * It takes an object of arguments and returns any value.
 */
export type ToolImplementation<TArgs = any, TResult = any> = (args: TArgs) => Promise<TResult> | TResult;

/**
 * A map of tool names to their implementations.
 * This is what the user passes to Agent.run().
 */
export type ToolMap = Record<string, ToolImplementation>;


/**
 * Represents a request from the model to execute a tool.
 */
export interface ToolCall {
    id: string;      // Unique ID for this call
    name: string;    // Name of the tool to call
    args: any;       // Arguments parsed from JSON
}

/**
 * Represents the result of a tool execution.
 * We send this back to the model.
 */
export interface ToolResult {
    toolCallId: string;
    result: any;
    isError?: boolean;
}