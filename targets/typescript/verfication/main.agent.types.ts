// Auto-generated types for TestAccount
// Do not edit manually
// Core Runtime Imports
import { createAuwgent as createAuwgentRuntime } from "@snrraptopack/auwgent-sdk";
import type { ToolRegistry } from "@snrraptopack/auwgent-sdk";
import _importedIR from './main.agent.json' with { type: 'json' };
type TestAccountIR = Omit<typeof _importedIR, "name" | "workflows" | "helpers"> & {
  name: "TestAccount";
  workflows: undefined;
  helpers: undefined;
};
const agentIR = _importedIR as unknown as TestAccountIR;
export type TextPart = import("@snrraptopack/auwgent-sdk").AuwgentTextPart;
export type ImagePart = import("@snrraptopack/auwgent-sdk").AuwgentImagePart;
export type FilePart = import("@snrraptopack/auwgent-sdk").AuwgentFilePart;
export type AudioPart = import("@snrraptopack/auwgent-sdk").AuwgentAudioPart;
export type VideoPart = import("@snrraptopack/auwgent-sdk").AuwgentVideoPart;
export type InputPart = import("@snrraptopack/auwgent-sdk").AuwgentInputPart;
export type MediaSource = import("@snrraptopack/auwgent-sdk").AuwgentBinarySource;
export type ImageInput = MediaSource & { mimeType?: string; detail?: "auto" | "low" | "high" };
export type FileInput = MediaSource & { mimeType?: string; name?: string };
export type AudioInput = MediaSource & { mimeType?: string; transcript?: string };
export type VideoInput = MediaSource & { mimeType?: string; transcript?: string; sampledFrames?: ImagePart[] };
export type Input = string

export type AuwgentOutput = {

}

export type AuwgentContext = {

}

export type AuwgentTools = {
    read_todo: (args: { id: string }) => Promise<string>;
    create_todo: (args: { title: string, due_date: string, priority: "low" | "medium" | "high" }) => Promise<string>;
    delete_todo: (args: { id: string }) => Promise<boolean>;
    mark_todo_done: (args: { id: string }) => Promise<boolean>;
    update_todo_title: (args: { id: string, new_title: string }) => Promise<boolean>;
    set_todo_priority: (args: { id: string, priority: "low" | "medium" | "high" }) => Promise<boolean>;
    count_todos: (args: { filter: "all" | "done" | "pending" }) => Promise<number>;
    get_user_name: (args: { user_id: string }) => Promise<string>;
    get_user_email: (args: { user_id: string }) => Promise<string>;
    update_user_email: (args: { user_id: string, new_email: string }) => Promise<boolean>;
    check_user_exists: (args: { user_id: string }) => Promise<boolean>;
    deactivate_user: (args: { user_id: string }) => Promise<boolean>;
    get_user_role: (args: { user_id: string }) => Promise<string>;
    assign_user_role: (args: { user_id: string, role: "admin" | "editor" | "viewer" }) => Promise<boolean>;
    send_email: (args: { to: string, subject: string, body: string }) => Promise<boolean>;
    get_email_subject: (args: { message_id: string }) => Promise<string>;
    mark_email_read: (args: { message_id: string }) => Promise<boolean>;
    delete_email: (args: { message_id: string }) => Promise<boolean>;
    schedule_meeting: (args: { title: string, start_iso: string, duration_minutes: number }) => Promise<string>;
    cancel_meeting: (args: { event_id: string, reason: string }) => Promise<boolean>;
    get_meeting_title: (args: { event_id: string }) => Promise<string>;
    check_meeting_exists: (args: { event_id: string }) => Promise<boolean>;
    get_weather_summary: (args: { city: string, unit: "celsius" | "fahrenheit" }) => Promise<string>;
    get_temperature: (args: { city: string, unit: "celsius" | "fahrenheit" }) => Promise<number>;
    is_raining: (args: { city: string }) => Promise<boolean>;
    get_uv_index: (args: { city: string }) => Promise<number>;
    search_web: (args: { query: string, num_results: number }) => Promise<string>;
    fetch_page_text: (args: { url: string }) => Promise<string>;
    check_url_reachable: (args: { url: string }) => Promise<boolean>;
    get_page_title: (args: { url: string }) => Promise<string>;
    run_read_query: (args: { sql: string, db_name: string }) => Promise<string>;
    count_rows: (args: { table: string, db_name: string }) => Promise<number>;
    check_table_exists: (args: { table: string, db_name: string }) => Promise<boolean>;
    write_file: (args: { path: string, content: string }) => Promise<boolean>;
    read_file: (args: { path: string }) => Promise<string>;
    delete_file: (args: { path: string, force: boolean }) => Promise<boolean>;
    file_exists: (args: { path: string }) => Promise<boolean>;
    get_file_size_bytes: (args: { path: string }) => Promise<number>;
    run_shell_command: (args: { command: string, working_dir: string }) => Promise<string>;
    get_exit_code: (args: { command: string }) => Promise<number>;
}

/** Custom intents defined in the DSL (if any) */
export type AuwgentCustomIntents =
    | never;

export interface AuwgentIntentHandler {
    tool_call?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_call" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
    tool_result?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_result" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
    tool_error?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_error" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
    tool_skipped?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_skipped" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
    response_text?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "response_text" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
    response_schema?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "response_schema" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
    error?(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "error" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void>;
}

export class AuwgentBaseIntentHandler implements AuwgentIntentHandler {
    tool_call(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_call" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
    tool_result(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_result" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
    tool_error(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_error" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
    tool_skipped(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "tool_skipped" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
    response_text(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "response_text" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
    response_schema(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "response_schema" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
    error(value: Extract<import("@snrraptopack/auwgent-sdk").AuwgentIntent<typeof agentIR, AuwgentCustomIntents, AuwgentOutput, AuwgentTools>, { name: "error" }>["value"], agentName: string): import("@snrraptopack/auwgent-sdk").IntentControl | Promise<import("@snrraptopack/auwgent-sdk").IntentControl> | void | Promise<void> {}
}

/**
 * API keys required for Auwgent
 */
export type AuwgentApiKeys = {
    groqApiKey: string;
}

// Defined explicitly (not via ReturnType) so RouterMiddleware can derive from it without circularity
export type AuwgentAgent = import("@snrraptopack/auwgent-sdk").TypedAuwgent<
    typeof agentIR,
    AuwgentCustomIntents,
    AuwgentOutput,
    AuwgentTools
>;

/** Middleware object type — consistent with `AuwgentAgent.onIntent` intent narrowing */
export type AuwgentMiddleware<T extends import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent'] = import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent']> = import("@snrraptopack/auwgent-sdk").Middleware<
    typeof agentIR,
    AuwgentCustomIntents,
    AuwgentOutput,
    AuwgentTools,
    T
>;

export type AuwgentConfig = {
    tools: AuwgentTools;
    middleware?: AuwgentMiddleware[];
    apiKeys: AuwgentApiKeys;
}

export function createAuwgent(config: AuwgentConfig): AuwgentAgent {
    return createAuwgentRuntime<
        typeof agentIR,
        AuwgentCustomIntents,
        AuwgentOutput,
        AuwgentTools
    >(agentIR, {
        tools: config.tools,
        middleware: config.middleware as any,
        apiKeys: config.apiKeys
    });
}

export const auwgent = createAuwgent;