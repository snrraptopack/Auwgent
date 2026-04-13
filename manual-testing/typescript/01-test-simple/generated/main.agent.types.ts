// Auto-generated types for TestAccount
// Do not edit manually
// Core Runtime Imports
import { createAuwgent } from "@snrraptopack/auwgent-sdk";
import type { ToolRegistry } from "@snrraptopack/auwgent-sdk";
import _importedIR from './main.agent.json' with { type: 'json' };
type TestAccountIR = Omit<typeof _importedIR, "name" | "workflows" | "helpers"> & {
  name: "TestAccount";
  workflows: undefined;
  helpers: undefined;
};
const agentIR = _importedIR as unknown as TestAccountIR;
export type TestAccountInput = {

}

export type TestAccountOutput = {

}

export type TestAccountContext = {

}

export type TestAccountTools = {
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
    translate_text: (args: { text: string, target_lang: string }) => Promise<string>;
    detect_language: (args: { text: string }) => Promise<string>;
    summarize_text: (args: { content: string, max_words: number }) => Promise<string>;
    count_words: (args: { text: string }) => Promise<number>;
    is_spam: (args: { text: string }) => Promise<boolean>;
    get_sentiment: (args: { text: string }) => Promise<"positive" | "neutral" | "negative">;
    render_markdown: (args: { markdown: string }) => Promise<string>;
    extract_emails: (args: { text: string }) => Promise<string>;
    extract_urls: (args: { text: string }) => Promise<string>;
    hash_string: (args: { input: string, algorithm: "sha256" | "sha512" | "md5" }) => Promise<string>;
    encode_base64: (args: { value: string }) => Promise<string>;
    decode_base64: (args: { encoded: string }) => Promise<string>;
    generate_uuid: (args: { version: "v4" | "v7" }) => Promise<string>;
    slugify: (args: { text: string }) => Promise<string>;
    truncate_text: (args: { text: string, max_chars: number }) => Promise<string>;
    is_valid_email: (args: { email: string }) => Promise<boolean>;
    is_valid_url: (args: { url: string }) => Promise<boolean>;
    is_valid_json: (args: { raw: string }) => Promise<boolean>;
    get_json_field: (args: { json: string, key_path: string }) => Promise<string>;
    parse_csv_row_count: (args: { csv_content: string }) => Promise<number>;
    get_current_timestamp: (args: { timezone: string }) => Promise<string>;
    format_date: (args: { iso_date: string, format: "short" | "long" | "iso" | "relative" }) => Promise<string>;
    days_between: (args: { date_a: string, date_b: string }) => Promise<number>;
    is_past_date: (args: { iso_date: string }) => Promise<boolean>;
    get_stock_price: (args: { ticker: string }) => Promise<number>;
    get_exchange_rate: (args: { from_currency: string, to_currency: string }) => Promise<number>;
    calculate_compound_interest: (args: { principal: number, annual_rate: number, years: number, compounds_per_year: number }) => Promise<number>;
    get_github_default_branch: (args: { owner: string, repo: string }) => Promise<string>;
    get_github_star_count: (args: { owner: string, repo: string }) => Promise<number>;
    is_github_repo_public: (args: { owner: string, repo: string }) => Promise<boolean>;
    get_github_issue_title: (args: { owner: string, repo: string, issue_number: number }) => Promise<string>;
    run_lint: (args: { source: string, language: "ts" | "js" | "py" | "rs" }) => Promise<string>;
    format_code: (args: { source: string, language: "ts" | "js" | "py" | "rs" }) => Promise<string>;
    count_lint_errors: (args: { source: string, language: "ts" | "js" | "py" | "rs" }) => Promise<number>;
    is_code_valid_syntax: (args: { source: string, language: "ts" | "js" | "py" | "rs" }) => Promise<boolean>;
    geocode_address: (args: { address: string }) => Promise<string>;
    reverse_geocode: (args: { lat: number, lng: number }) => Promise<string>;
    get_distance_km: (args: { lat_a: number, lng_a: number, lat_b: number, lng_b: number }) => Promise<number>;
    get_ip_country: (args: { ip_address: string }) => Promise<string>;
    is_private_ip: (args: { ip_address: string }) => Promise<boolean>;
    dns_resolve_a_record: (args: { hostname: string }) => Promise<string>;
    is_port_open: (args: { host: string, port: number }) => Promise<boolean>;
    get_ssl_expiry_days: (args: { domain: string }) => Promise<number>;
    send_sms: (args: { to_phone: string, message: string }) => Promise<boolean>;
    cache_set: (args: { key: string, value: string, ttl_seconds: number }) => Promise<boolean>;
    cache_get: (args: { key: string }) => Promise<string>;
    cache_delete: (args: { key: string }) => Promise<boolean>;
    get_feature_flag: (args: { flag_key: string, user_id: string }) => Promise<boolean>;
    get_feature_flag_value: (args: { flag_key: string, user_id: string }) => Promise<string>;
    check_permission: (args: { user_id: string, action: string, resource: string }) => Promise<boolean>;
    get_secret: (args: { secret_name: string, vault: "aws" | "vault" | "doppler" }) => Promise<string>;
    set_secret: (args: { secret_name: string, value: string, vault: "aws" | "vault" | "doppler" }) => Promise<boolean>;
    get_job_status: (args: { job_id: string }) => Promise<"pending" | "running" | "done" | "failed">;
    get_job_result: (args: { job_id: string }) => Promise<string>;
    get_queue_depth: (args: { queue_name: string }) => Promise<number>;
    record_metric: (args: { metric_name: string, value: number }) => Promise<boolean>;
    get_metric_average: (args: { metric_name: string, last_n_minutes: number }) => Promise<number>;
    get_metric_max: (args: { metric_name: string, last_n_minutes: number }) => Promise<number>;
    is_metric_above_threshold: (args: { metric_name: string, threshold: number }) => Promise<boolean>;
    render_template: (args: { template: string, context_json: string }) => Promise<string>;
}

/** Custom intents defined in the DSL (if any) */
export type TestAccountCustomIntents =
    | never;

/**
 * API keys required for TestAccount
 */
export type TestAccountApiKeys = {
    groq_apiApiKey: string;  // API key for custom provider 'groq-api'
}

// Defined explicitly (not via ReturnType) so RouterMiddleware can derive from it without circularity
export type TestAccountAgent = import("@snrraptopack/auwgent-sdk").TypedAuwgent<
    typeof agentIR,
    TestAccountCustomIntents,
    TestAccountOutput,
    TestAccountTools
>;

/** Middleware object type — consistent with `TestAccountAgent.onIntent` intent narrowing */
export type TestAccountMiddleware<T extends import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent'] = import("@snrraptopack/auwgent-sdk").MiddlewareContext<typeof agentIR>['activeAgent']> = import("@snrraptopack/auwgent-sdk").Middleware<
    typeof agentIR,
    TestAccountCustomIntents,
    TestAccountOutput,
    TestAccountTools,
    T
>;

export type TestAccountConfig = {
    tools: TestAccountTools;
    middleware?: TestAccountMiddleware[];
    apiKeys: TestAccountApiKeys;
}

export function createTestAccount(config: TestAccountConfig): TestAccountAgent {
    return createAuwgent<
        typeof agentIR,
        TestAccountCustomIntents,
        TestAccountOutput,
        TestAccountTools
    >(agentIR, {
        tools: config.tools,
        middleware: config.middleware as any,
        apiKeys: config.apiKeys
    });
}

export const auwgent = createTestAccount;
export type AuwgentTools = TestAccountTools;
export type AuwgentConfig = TestAccountConfig;
export type AuwgentAgent = TestAccountAgent;
export type AuwgentMiddleware = TestAccountMiddleware;
export type AuwgentContext = TestAccountContext;