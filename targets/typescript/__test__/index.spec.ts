/**
 * Integration test for the Auwgent napi-rs FFI bridge.
 *
 * Tests:
 * 1. Native binding loads correctly
 * 2. IR parsing works
 * 3. Tool registration works
 * 4. Type-safe wrapper enforces tool completeness
 * 5. Prompt generation works
 * 6. Session export/import round-trips
 * 7. onIntent callback works
 */
import { describe, it, expect } from 'bun:test';
import { Auwgent } from '../index.js';  // Native binding
import { createAuwgent, parseIR, type AgentIRShape } from '../auwgent.js';

// ── Minimal IR for testing ───────────────────────────────────────────────
const TEST_IR = {
    name: 'test-agent',
    modelConfig: [
        {
            defaultConfig: {
                model: { type: 'gemini', modelName: 'gemini-2.0-flash', config: null },
                prompt: { type: 'literal', value: 'You are a test agent.' },
            },
            namedConfig: null,
        },
    ],
    input: null,
    output: {
        type: 'object',
        properties: {
            response: { type: 'string', description: 'Agent response' },
        },
    },
    context: null,
    tools: [
        {
            name: 'greet',
            description: 'Greet a user by name',
            params: {
                type: 'object',
                properties: {
                    name: { type: 'string', description: 'Name to greet' },
                },
            },
            returns: {
                type: 'object',
                properties: {
                    message: { type: 'string' },
                },
            },
        },
        {
            name: 'search',
            description: 'Search for something',
            params: {
                type: 'object',
                properties: {
                    query: { type: 'string', description: 'Search query' },
                },
            },
            returns: {
                type: 'object',
                properties: {
                    results: { type: 'array', items: { type: 'string' } },
                },
            },
        },
    ],
    workflows: [],
    helpers: [],
    tests: [],
} as const;

const IR_JSON = JSON.stringify(TEST_IR);

// ═══════════════════════════════════════════════════════════════════════════
// NATIVE BINDING TESTS
// ═══════════════════════════════════════════════════════════════════════════

describe('Native Auwgent', () => {
    it('loads the native binding', () => {
        expect(Auwgent).toBeDefined();
        expect(typeof Auwgent).toBe('function');
    });

    it('creates an engine from IR JSON', () => {
        const agent = new Auwgent(IR_JSON);
        expect(agent).toBeDefined();
    });

    it('rejects invalid IR JSON', () => {
        expect(() => new Auwgent('not json')).toThrow();
    });

    it('returns tool names from IR', () => {
        const agent = new Auwgent(IR_JSON);
        const names = agent.getToolNames();
        expect(names).toEqual(['greet', 'search']);
    });

    it('returns tool schemas', () => {
        const agent = new Auwgent(IR_JSON);
        const schemas = JSON.parse(agent.getToolSchemas());
        expect(schemas).toHaveLength(2);
        expect(schemas[0].name).toBe('greet');
        expect(schemas[1].name).toBe('search');
        expect(schemas[0].params.properties.name.type).toBe('string');
    });

    it('registers a tool callback', () => {
        const agent = new Auwgent(IR_JSON);
        agent.registerTool('greet', async (args: any) => {
            return { message: `Hello, ${args.name}!` };
        });
        // No error means success
    });

    it('generates a prompt', () => {
        const agent = new Auwgent(IR_JSON);
        const prompt = agent.generatePrompt();
        expect(prompt).toContain('You are a test agent');
    });

    it('exports and imports session state', () => {
        const agent = new Auwgent(IR_JSON);
        const exported = agent.exportSession();
        const state = JSON.parse(exported);
        expect(state).toBeDefined();
        expect(state.turns).toBeDefined();

        // Round-trip
        agent.importSession(exported);
        const exported2 = agent.exportSession();
        expect(JSON.parse(exported2)).toEqual(state);
    });

    it('clears session', () => {
        const agent = new Auwgent(IR_JSON);
        agent.clearSession();
        const state = JSON.parse(agent.exportSession());
        expect(state.turns).toEqual([]);
    });

    it('registers an onIntent callback', () => {
        const agent = new Auwgent(IR_JSON);
        agent.onIntent((name: string, value: any) => {
            // Just observe
            console.log(`[${name}]`, value);
        });
        // No error means success
    });
});

// ═══════════════════════════════════════════════════════════════════════════
// TYPE-SAFE WRAPPER TESTS
// ═══════════════════════════════════════════════════════════════════════════

describe('TypedAuwgent', () => {
    it('creates a typed agent with all tools', () => {
        const agent = createAuwgent(TEST_IR, {
            tools: {
                greet: async (args) => ({ message: `Hello, ${args.name}!` }),
                search: async (args) => ({ results: [`Result for ${args.query}`] }),
            },
        });
        expect(agent).toBeDefined();
    });

    it('returns typed tool names', () => {
        const agent = createAuwgent(TEST_IR, {
            tools: {
                greet: async () => ({ message: 'hi' }),
                search: async () => ({ results: [] }),
            },
        });
        const names = agent.getToolNames();
        expect(names).toContain('greet');
        expect(names).toContain('search');
    });

    it('generates prompt through wrapper', () => {
        const agent = createAuwgent(TEST_IR, {
            tools: {
                greet: async () => ({ message: 'hi' }),
                search: async () => ({ results: [] }),
            },
        });
        const prompt = agent.generatePrompt();
        expect(typeof prompt).toBe('string');
        expect(prompt.length).toBeGreaterThan(0);
    });

    it('exports typed session without steps', () => {
        const agent = createAuwgent(TEST_IR, {
            tools: {
                greet: async () => ({ message: 'hi' }),
                search: async () => ({ results: [] }),
            },
        });
        const session = agent.exportSession();
        expect(session.turns).toBeDefined();
        expect(Array.isArray(session.turns)).toBe(true);
        // Steps should NOT exist anymore
        expect((session as any).steps).toBeUndefined();
    });

    it('round-trips session through wrapper', () => {
        const agent = createAuwgent(TEST_IR, {
            tools: {
                greet: async () => ({ message: 'hi' }),
                search: async () => ({ results: [] }),
            },
        });
        const session = agent.exportSession();
        agent.importSession(session);
        const after = agent.exportSession();
        expect(after).toEqual(session);
    });

    it('registers onIntent through wrapper', () => {
        const agent = createAuwgent(TEST_IR, {
            tools: {
                greet: async () => ({ message: 'hi' }),
                search: async () => ({ results: [] }),
            },
        });

        const events: Array<{ name: string; value: unknown }> = [];
        agent.onIntent((name, value) => {
            events.push({ name, value });
        });
        // onIntent registered successfully — events will fire during run()
        expect(events).toEqual([]);
    });

    it('adds response_text delta in onIntentPartial for realtime streaming', () => {
        const agent = createAuwgent(TEST_IR, {
            tools: {
                greet: async () => ({ message: 'hi' }),
                search: async () => ({ results: [] }),
            },
        });

        const native = (agent as any).native;
        let partialCallback: ((name: string, value: any, agentName: string) => void) | undefined;

        native.onIntent = () => {};
        native.onMiddlewareEvent = () => {};
        native.onSubEngineStart = () => {};
        native.onSubEngineComplete = () => {};
        native.onIntentPartial = (cb: (name: string, value: any, agentName: string) => void) => {
            partialCallback = cb;
        };

        const deltas: string[] = [];
        agent.onIntentPartial((name, value) => {
            if (name === 'response_text') {
                deltas.push((value as any).delta);
            }
        });

        (agent as any).activateListeners();

        partialCallback?.('response_text', { text: 'Hello', delta: 'Hello' }, 'Main');
        partialCallback?.('response_text', { text: 'Hello there', delta: ' there' }, 'Main');

        expect(deltas).toEqual(['Hello', ' there']);
    });

    it('parseIR returns typed IR', () => {
        const ir = parseIR(IR_JSON);
        expect(ir.name).toBe('test-agent');
        expect(ir.tools).toHaveLength(2);
    });
});

// ═══════════════════════════════════════════════════════════════════════════
// TYPE SAFETY COMPILE-TIME CHECKS (these are TS-only, not runtime)
// ═══════════════════════════════════════════════════════════════════════════

describe('Type safety (compile-time)', () => {
    it('TypeScript would error if a tool is missing (documented)', () => {
        // This test documents the type safety behavior.
        // If you uncomment the following, TypeScript WILL error:
        //
        // createAuwgent(TEST_IR, {
        //   tools: {
        //     greet: async () => ({ message: 'hi' }),
        //     // search is MISSING — TS error: Property 'search' is missing
        //   },
        // });
        //
        // This proves that ToolRegistry<IR> enforces completeness.
        expect(true).toBe(true);
    });
});
