

// --- In-Memory State Stores for Agentic Persistence ---
const state = {
    todos: new Map<string, any>(),
    users: new Map<string, any>([
        ["usr_123", { name: "Alice", email: "alice@example.com", role: "admin", active: true }],
        ["usr_456", { name: "Bob", email: "bob@example.com", role: "viewer", active: true }]
    ]),
    emails: new Map<string, any>(),
    meetings: new Map<string, any>(),
    files: new Map<string, string>(),
    cache: new Map<string, string>()
};

// --- TODO TOOLS ---
export async function create_todo(args: { title: string, due_date: string, priority: "low" | "medium" | "high" }): Promise<string> {
    const id = "todo_" + Math.random().toString(36).substr(2, 6);
    state.todos.set(id, { ...args, done: false });
    return id;
}

export async function read_todo(args: { id: string }): Promise<string> {
    const todo = state.todos.get(args.id);
    if (!todo) return "Error: Todo not found";
    return JSON.stringify(todo);
}

// --- USER TOOLS ---
export async function deactivate_user(args: { user_id: string }): Promise<boolean> {
    const user = state.users.get(args.user_id);
    if (!user) return false;
    user.active = false;
    return true;
}

export async function check_permission(args: { user_id: string, action: string, resource: string }): Promise<boolean> {
    const user = state.users.get(args.user_id);
    if (!user) return false;
    if (user.role === "admin") return true;
    return args.action === "read"; // Viewers can only read
}

// --- COMMS & CALENDAR ---
export async function send_email(args: { to: string, subject: string, body: string }): Promise<boolean> {
    const id = "msg_" + Date.now();
    state.emails.set(id, args);
    return true;
}

export async function schedule_meeting(args: { title: string, start_iso: string, duration_minutes: number }): Promise<string> {
    const id = "evt_" + Date.now();
    state.meetings.set(id, args);
    return id;
}

// --- FILESYSTEM & OS ---
export async function write_file(args: { path: string, content: string }): Promise<boolean> {
    state.files.set(args.path, args.content);
    return true;
}

export async function run_shell_command(args: { command: string, working_dir: string }): Promise<string> {
    if (args.command.startsWith("echo")) {
        return args.command.replace("echo ", "").replace(/["']/g, "");
    }
    return `Mock execution output for: ${args.command}`;
}

// --- DATA UTILS ---
export async function search_web(args: { query: string, num_results: number }): Promise<string> {
    return JSON.stringify([
        { title: `${args.query} Overview`, url: `https://example.com/search?q=${encodeURI(args.query)}` }
    ]);
}

export async function get_weather_summary(args: { city: string, unit: "celsius" | "fahrenheit" }): Promise<string> {
    const t = args.unit === "celsius" ? 22 : 72;
    return `Currently ${t} degrees and sunny in ${args.city}.`;
}

export async function run_read_query(args: { sql: string, db_name: string }): Promise<string> {
    return `[{ "mock_row_1": "data" }]`;
}

export async function get_stock_price(args: { ticker: string }): Promise<number> {
    return args.ticker.toUpperCase() === "AAPL" ? 175.50 : 100.00;
}

export async function get_github_issue_title(args: { owner: string, repo: string, issue_number: number }): Promise<string> {
    return `Bug report #${args.issue_number} in ${args.owner}/${args.repo}`;
}

export async function translate_text(args: { text: string, target_lang: string }): Promise<string> {
    return `[Mock translation of "${args.text}" into ${args.target_lang}]`;
}

// --- DEV & CRYPTO TOOLS ---
export async function hash_string(args: { input: string, algorithm: "sha256" | "sha512" | "md5" }): Promise<string> {
    // In a real environment, node:crypto could be used. Mocking here.
    return `hashed_output_${Buffer.from(args.input).toString('base64')}`;
}

export async function format_date(args: { iso_date: string, format: "short" | "long" | "iso" | "relative" }): Promise<string> {
    return new Date(args.iso_date).toDateString();
}

export async function run_lint(args: { source: string, language: "ts" | "js" | "py" | "rs" }): Promise<string> {
    return args.source.includes("var ") ? '["Prefer let/const over var"]' : "[]";
}

// --- MAPS & DNS ---
export async function geocode_address(args: { address: string }): Promise<string> {
    return "37.7749,-122.4194"; // Hardcode SF for mock
}

export async function dns_resolve_a_record(args: { hostname: string }): Promise<string> {
    return "104.21.25.10"; // Cloudflare dummy IP
}

// --- KV CACHE ---
export async function cache_set(args: { key: string, value: string, ttl_seconds: number }): Promise<boolean> {
    state.cache.set(args.key, args.value);
    return true;
}
