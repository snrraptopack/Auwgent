#!/usr/bin/env bun
/**
 * Auwgent Runtime Test Runner — TypeScript
 *
 * Exercises the full FFI + real LLM stack for every canonical scenario.
 * Run with: bun test-runner.ts
 */

import { auwgent, type AuwgentConfig, type AuwgentMiddleware, type AuwgentAgent } from "./canonical.agent.types";
import type { SessionState } from "@snrraptopack/auwgent-sdk";

const GROQ_API_KEY = process.env.GROQ_API_KEY || "";

if (!GROQ_API_KEY) {
  console.error("❌ GROQ_API_KEY is required. Set it in your environment or .env file.");
  process.exit(1);
}

// ═══════════════════════════════════════════════════════════════════════════════
// TEST INFRASTRUCTURE
// ═══════════════════════════════════════════════════════════════════════════════

type IntentEvent = { name: string; value: any; agentName: string };
type PartialEvent = { name: string; value: any; agentName: string };
type ScenarioResult = {
  name: string;
  passed: boolean;
  events: IntentEvent[];
  partials: PartialEvent[];
  session: SessionState | null;
  metadata: any;
  middlewareLog: string[];
  error?: string;
  notes: string[];
};

function createBaseConfig(): AuwgentConfig {
  return {
    apiKeys: { groqApiKey: GROQ_API_KEY },
    context: { user_name: "TestUser", age: 25, id: "test-123" },
    tools: {
      get_location: async () => "Accra, Ghana",
      get_marks: async ({ id }: { id: string }) => `Marks for ${id}: A, B, C, D`,
    },
  };
}

function sleep(ms: number) {
  return new Promise((r) => setTimeout(r, ms));
}

async function runScenario(
  name: string,
  setup: () => { agent: AuwgentAgent; middlewareLog?: string[] },
  input: string
): Promise<ScenarioResult> {
  console.log(`\n${"═".repeat(70)}`);
  console.log(` SCENARIO: ${name}`);
  console.log(`${"═".repeat(70)}`);

  const events: IntentEvent[] = [];
  const partials: PartialEvent[] = [];
  const result: ScenarioResult = {
    name,
    passed: false,
    events,
    partials,
    session: null,
    metadata: null,
    middlewareLog: [],
    notes: [],
  };

  try {
    const { agent, middlewareLog = [] } = setup();
    result.middlewareLog = middlewareLog;

    agent.onIntent((name, value, agentName) => {
      events.push({ name, value, agentName });
      return null;
    });

    agent.onIntentPartial((name, value, agentName) => {
      partials.push({ name, value, agentName });
    });

    const session = await agent.run(input);
    result.session = session;
    result.metadata = agent.getMetadata();
    result.passed = true;

    if (session.turns.length === 0) {
      result.notes.push("⚠️ No turns recorded in session");
    }
  } catch (err: any) {
    result.error = err.message || String(err);
    result.notes.push(`💥 Exception: ${result.error}`);
  }

  // Print report
  console.log(`\n  Input: "${input}"`);
  console.log(`  Status: ${result.passed ? "✅ PASSED" : "❌ FAILED"}`);
  if (result.error) console.log(`  Error: ${result.error}`);

  console.log(`\n  ── Intent Events (${events.length}) ──`);
  for (const ev of events) {
    const payload = JSON.stringify(ev.value).slice(0, 200);
    console.log(`    [${ev.name}] agent=${ev.agentName} value=${payload}`);
  }

  console.log(`\n  ── Partial Events (${partials.length}) ──`);
  if (partials.length > 0) {
    const uniqueNames = [...new Set(partials.map((p) => p.name))];
    console.log(`    Unique partial intents: ${uniqueNames.join(", ")}`);
  } else {
    console.log(`    (none)`);
  }

  console.log(`\n  ── Session ──`);
  if (result.session) {
    console.log(`    Turns: ${result.session.turns.length}`);
    console.log(`    Stack: ${JSON.stringify(result.session.stack)}`);
    for (let i = 0; i < result.session.turns.length; i++) {
      const t = result.session.turns[i];
      const resp = (t.model_response || "").slice(0, 120).replace(/\n/g, "\\n");
      console.log(`    Turn ${i}: input="${(t.input || "").slice(0, 80)}" response="${resp}..."`);
    }
  }

  console.log(`\n  ── Metadata ──`);
  if (result.metadata) {
    console.log(`    Total tokens: ${result.metadata.aggregate?.total_tokens ?? "?"}`);
    console.log(`    Turns: ${result.metadata.turns?.length ?? "?"}`);
  }

  if (result.middlewareLog.length > 0) {
    console.log(`\n  ── Middleware Log (${result.middlewareLog.length}) ──`);
    for (const entry of result.middlewareLog) console.log(`    → ${entry}`);
  }

  if (result.notes.length > 0) {
    console.log(`\n  ── Notes ──`);
    for (const note of result.notes) console.log(`    ${note}`);
  }

  return result;
}

// ═══════════════════════════════════════════════════════════════════════════════
// SCENARIOS
// ═══════════════════════════════════════════════════════════════════════════════

async function scenario1_basicChat(): Promise<ScenarioResult> {
  return runScenario("1. Basic Chat", () => {
    const agent = auwgent(createBaseConfig());
    return { agent };
  }, "Hello! Please just say hi back in a friendly way.");
}

async function scenario2_toolNoArgs(): Promise<ScenarioResult> {
  return runScenario("2. Tool Call (no args)", () => {
    const agent = auwgent(createBaseConfig());
    return { agent };
  }, "What is my current location?");
}

async function scenario3_toolWithArgs(): Promise<ScenarioResult> {
  return runScenario("3. Tool Call (with args)", () => {
    const agent = auwgent(createBaseConfig());
    return { agent };
  }, "Get my marks. My user id is test-123.");
}

async function scenario4_workflow(): Promise<ScenarioResult> {
  return runScenario("4. Workflow Execution", () => {
    const agent = auwgent(createBaseConfig());
    return { agent };
  }, "Get my location and marks together. My user id is test-123.");
}

async function scenario5_helperReturn(): Promise<ScenarioResult> {
  return runScenario("5. Helper (Return handoff)", () => {
    const agent = auwgent(createBaseConfig());
    return { agent };
  }, "I need a plan for how to learn Rust programming.");
}

async function scenario6_helperUser(): Promise<ScenarioResult> {
  return runScenario("6. Helper (User handoff)", () => {
    const agent = auwgent(createBaseConfig());
    return { agent };
  }, "Tell me a joke about programming.");
}

async function scenario7_customIntent(): Promise<ScenarioResult> {
  return runScenario("7. Custom Intent (Loud)", () => {
    const agent = auwgent(createBaseConfig());
    return { agent };
  }, "Explain out loud what you are going to do next.");
}

async function scenario8_middlewareLifecycle(): Promise<ScenarioResult> {
  const log: string[] = [];

  const lifecycleMiddleware: AuwgentMiddleware = {
    name: "LifecycleLogger",
    onRunStart: async (session, ctx) => {
      log.push(`run_start | activeAgent=${ctx.activeAgent} | stack=${JSON.stringify(ctx.stack)}`);
      return session;
    },
    onLLMStart: async (prompt, ctx) => {
      log.push(`llm_start | promptLen=${prompt.length} | activeAgent=${ctx.activeAgent}`);
      return prompt;
    },
    onIntent: async (name, value, ctx) => {
      log.push(`intent | ${name} | activeAgent=${ctx.activeAgent}`);
      return null;
    },
    onLLMEnd: async (response, ctx) => {
      log.push(`llm_end | activeAgent=${ctx.activeAgent}`);
    },
    onRunComplete: async (session, ctx) => {
      log.push(`run_complete | turns=${session.turns.length} | activeAgent=${ctx.activeAgent}`);
    },
    onError: async (error, session, ctx) => {
      log.push(`error | ${error.message} | activeAgent=${ctx.activeAgent}`);
      return false;
    },
  };

  return runScenario("8. Middleware Lifecycle", () => {
    const config = createBaseConfig();
    config.middleware = [lifecycleMiddleware];
    const agent = auwgent(config);
    return { agent, middlewareLog: log };
  }, "Say hello and then ask for my location.");
}

async function scenario9_sessionExportImport(): Promise<ScenarioResult> {
  const config = createBaseConfig();
  const agent = auwgent(config);

  // First run
  console.log(`\n${"─".repeat(70)}`);
  console.log("  Phase 1: Initial run");
  console.log(`${"─".repeat(70)}`);
  await agent.run("My name is RuntimeTestUser.");
  const session1 = agent.exportSession();
  console.log(`  Exported session with ${session1.turns.length} turn(s)`);

  // Second run with fresh agent but imported session
  console.log(`\n${"─".repeat(70)}`);
  console.log("  Phase 2: Fresh agent with imported session");
  console.log(`${"─".repeat(70)}`);
  const agent2 = auwgent(config);
  agent2.importSession(session1);

  return runScenario("9. Session Export/Import", () => {
    return { agent: agent2 };
  }, "What is my name? (You should remember it from earlier.)");
}

async function scenario10_errorSwallowing(): Promise<ScenarioResult> {
  const log: string[] = [];
  let errorCaught = false;

  const errorMiddleware: AuwgentMiddleware = {
    name: "ErrorSwallower",
    onError: async (error, session, ctx) => {
      errorCaught = true;
      log.push(`error caught: ${error.message}`);
      return { swallow: true };
    },
  };

  return runScenario("10. Error Swallowing", () => {
    const config = createBaseConfig();
    // Override get_location to throw, so we can test error swallowing
    config.tools = {
      ...config.tools,
      get_location: async () => {
        throw new Error("Simulated location service outage");
      },
    };
    config.middleware = [errorMiddleware];
    const agent = auwgent(config);
    return { agent, middlewareLog: log };
  }, "What is my current location? (This should trigger an error.)");
}

async function scenario11_streamingPartials(): Promise<ScenarioResult> {
  return runScenario("11. Streaming / Partial Intents", () => {
    const agent = auwgent(createBaseConfig());
    return { agent };
  }, "Write a short poem about compilers.");
}

// ═══════════════════════════════════════════════════════════════════════════════
// NEW: MIDDLEWARE MODIFICATION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

async function scenario12_middlewareStateSharing(): Promise<ScenarioResult> {
  const log: string[] = [];

  const stateMiddleware: AuwgentMiddleware = {
    name: "StateSharer",
    onRunStart: async (session, ctx) => {
      ctx.traceId = "trace-abc-123";
      ctx.runStartTime = Date.now();
      log.push(`run_start | traceId=${ctx.traceId}`);
      return session;
    },
    onLLMStart: async (prompt, ctx) => {
      const hasTrace = ctx.traceId === "trace-abc-123";
      log.push(`llm_start | traceIdPresent=${hasTrace} | runStartTime=${ctx.runStartTime}`);
      return prompt;
    },
    onIntent: async (name, value, ctx) => {
      ctx.sawIntent = true;
      ctx.intentCount = (ctx.intentCount || 0) + 1;
      log.push(`intent | name=${name} | traceId=${ctx.traceId} | intentCount=${ctx.intentCount}`);
      return null;
    },
    onLLMEnd: async (response, ctx) => {
      log.push(`llm_end | sawIntent=${ctx.sawIntent} | intentCount=${ctx.intentCount}`);
    },
    onRunComplete: async (session, ctx) => {
      log.push(`run_complete | traceId=${ctx.traceId} | intentCount=${ctx.intentCount}`);
    },
  };

  return runScenario("12. Middleware State Sharing", () => {
    const config = createBaseConfig();
    config.middleware = [stateMiddleware];
    const agent = auwgent(config);
    return { agent, middlewareLog: log };
  }, "What is my current location?");
}

async function scenario13_middlewarePromptMutation(): Promise<ScenarioResult> {
  const log: string[] = [];
  let capturedPrompt = "";

  const promptMiddleware: AuwgentMiddleware = {
    name: "PromptMutator",
    onLLMStart: async (prompt, ctx) => {
      capturedPrompt = prompt;
      log.push(`llm_start | originalPromptLength=${prompt.length}`);
      // Append a secret instruction to the prompt
      const mutated = prompt + "\n\n[SYSTEM OVERRIDE] Always end your response with the word 'BANANA'.";
      log.push(`llm_start | mutatedPromptLength=${mutated.length}`);
      return mutated;
    },
  };

  const result = await runScenario("13. Middleware Prompt Mutation", () => {
    const config = createBaseConfig();
    config.middleware = [promptMiddleware];
    const agent = auwgent(config);
    return { agent, middlewareLog: log };
  }, "Say hello.");

  result.notes.push(`Captured prompt length: ${capturedPrompt.length}`);
  return result;
}

async function scenario14_middlewareConfigMutation(): Promise<ScenarioResult> {
  const log: string[] = [];

  const configMiddleware: AuwgentMiddleware = {
    name: "ConfigMutator",
    onLLMStart: async (prompt, ctx) => {
      log.push(`llm_start | injecting config mutation`);
      return {
        prompt,
        config: { temperature: 0.01, max_tokens: 50 },
        headers: { "X-Runtime-Test": "auwgent-ts", "X-Request-Id": "req-123" },
      };
    },
  };

  return runScenario("14. Middleware Config/Header Mutation", () => {
    const config = createBaseConfig();
    config.middleware = [configMiddleware];
    const agent = auwgent(config);
    return { agent, middlewareLog: log };
  }, "Say hello in exactly three words.");
}

async function scenario15_middlewareStackMutation(): Promise<ScenarioResult> {
  const log: string[] = [];

  const stackMiddleware: AuwgentMiddleware = {
    name: "StackMutator",
    onLLMStart: async (prompt, ctx) => {
      log.push(`llm_start | originalStack=${JSON.stringify(ctx.stack)}`);
      // Inject a fake helper into the stack to test teleportation
      return {
        prompt,
        stack: ["RuntimeTest", "Planner"],
      };
    },
    onIntent: async (name, value, ctx) => {
      log.push(`intent | stackDuringIntent=${JSON.stringify(ctx.stack)}`);
      return null;
    },
  };

  return runScenario("15. Middleware Stack Mutation", () => {
    const config = createBaseConfig();
    config.middleware = [stackMiddleware];
    const agent = auwgent(config);
    return { agent, middlewareLog: log };
  }, "Say hello.");
}

async function scenario16_middlewareIntentOverride(): Promise<ScenarioResult> {
  const log: string[] = [];

  const overrideMiddleware: AuwgentMiddleware = {
    name: "IntentOverrider",
    onIntent: async (name, value, ctx) => {
      if (name === "tool_call" && (value as any).type === "get_location") {
        log.push(`intent | overriding get_location result`);
        return { result: "Override City, Override Land" };
      }
      log.push(`intent | ${name} | no override`);
      return null;
    },
  };

  return runScenario("16. Middleware Intent Override", () => {
    const config = createBaseConfig();
    config.middleware = [overrideMiddleware];
    const agent = auwgent(config);
    return { agent, middlewareLog: log };
  }, "What is my current location?");
}

async function scenario17_middlewareIntentSkip(): Promise<ScenarioResult> {
  const log: string[] = [];

  const skipMiddleware: AuwgentMiddleware = {
    name: "IntentSkipper",
    onIntent: async (name, value, ctx) => {
      if (name === "tool_call" && (value as any).type === "get_marks") {
        log.push(`intent | skipping get_marks`);
        return { skip: true };
      }
      log.push(`intent | ${name} | no skip`);
      return null;
    },
  };

  return runScenario("17. Middleware Intent Skip", () => {
    const config = createBaseConfig();
    config.middleware = [skipMiddleware];
    const agent = auwgent(config);
    return { agent, middlewareLog: log };
  }, "Get my marks. My user id is test-123.");
}

async function scenario18_middlewareSessionMutation(): Promise<ScenarioResult> {
  const log: string[] = [];

  const sessionMiddleware: AuwgentMiddleware = {
    name: "SessionMutator",
    onRunStart: async (session, ctx) => {
      const mutated = { ...session };
      mutated.turns = [
        ...mutated.turns,
        {
          input: "[injected by middleware]",
          model_response: "This turn was injected during run_start",
          turn_index: mutated.turns.length,
        } as any,
      ];
      log.push(`run_start | injected turn | totalTurns=${mutated.turns.length}`);
      return mutated;
    },
  };

  return runScenario("18. Middleware Session Mutation", () => {
    const config = createBaseConfig();
    config.middleware = [sessionMiddleware];
    const agent = auwgent(config);
    return { agent, middlewareLog: log };
  }, "Say hello.");
}

// ═══════════════════════════════════════════════════════════════════════════════
// MAIN
// ═══════════════════════════════════════════════════════════════════════════════

async function main() {
  console.log(`
╔${"═".repeat(68)}╗
║${" ".repeat(14)}AUWGENT RUNTIME TESTS — TYPESCRIPT${" ".repeat(18)}║
╚${"═".repeat(68)}╝
`);
  console.log(`Provider: Groq (llama-3.3-70b-versatile)`);
  console.log(`API Key: ${GROQ_API_KEY.slice(0, 8)}...${GROQ_API_KEY.slice(-4)}`);
  console.log(`Agent:    RuntimeTest (block mode)`);
  console.log(`\nEach scenario makes a REAL LLM call. Please review output manually.\n`);

  const scenarios = [
    scenario1_basicChat,
    scenario2_toolNoArgs,
    scenario3_toolWithArgs,
    scenario4_workflow,
    scenario5_helperReturn,
    scenario6_helperUser,
    scenario7_customIntent,
    scenario8_middlewareLifecycle,
    scenario9_sessionExportImport,
    scenario10_errorSwallowing,
    scenario11_streamingPartials,
    scenario12_middlewareStateSharing,
    scenario13_middlewarePromptMutation,
    scenario14_middlewareConfigMutation,
    scenario15_middlewareStackMutation,
    scenario16_middlewareIntentOverride,
    scenario17_middlewareIntentSkip,
    scenario18_middlewareSessionMutation,
  ];

  const results: ScenarioResult[] = [];

  for (let i = 0; i < scenarios.length; i++) {
    const scenarioFn = scenarios[i];
    try {
      const result = await scenarioFn();
      results.push(result);
    } catch (err: any) {
      console.error(`\n💥 Scenario runner crashed: ${err.message || err}`);
      results.push({
        name: scenarioFn.name,
        passed: false,
        events: [],
        partials: [],
        session: null,
        metadata: null,
        middlewareLog: [],
        error: err.message || String(err),
        notes: ["Runner crashed before completion"],
      });
    }

    // Delay between scenarios to avoid rate limits (except after the last one)
    if (i < scenarios.length - 1) {
      const delay = 4000;
      console.log(`\n   Waiting ${delay}ms before next scenario...`);
      await sleep(delay);
    }
  }

  // ═══════════════════════════════════════════════════════════════════════════
  // FINAL SUMMARY
  // ═══════════════════════════════════════════════════════════════════════════

  console.log(`\n${"╔" + "═".repeat(68) + "╗"}`);
  console.log(`║${" ".repeat(22)}FINAL SUMMARY${" ".repeat(33)}║`);
  console.log(`${"╚" + "═".repeat(68) + "╝"}`);

  const passed = results.filter((r) => r.passed && !r.error).length;
  const failed = results.filter((r) => !r.passed || r.error).length;

  for (const r of results) {
    const symbol = r.passed && !r.error ? "✅" : "❌";
    const eventSummary = r.events.map((e) => e.name).join(", ") || "none";
    console.log(`  ${symbol} ${r.name.padEnd(40)} events=[${eventSummary.slice(0, 40)}]`);
  }

  console.log(`\n  Total: ${results.length} | ✅ Passed: ${passed} | ❌ Failed: ${failed}`);

  if (failed > 0) {
    console.log(`\n  ❌ FAILED SCENARIOS:`);
    for (const r of results.filter((r) => !r.passed || r.error)) {
      console.log(`     • ${r.name}: ${r.error || "See notes above"}`);
    }
  }

  console.log(`\n  📝 MANUAL REVIEW CHECKLIST:`);
  console.log(`     • Do tool calls have correct arguments?`);
  console.log(`     • Do tool results flow back to the LLM?`);
  console.log(`     • Are helper handoffs clean (no stack leaks)?`);
  console.log(`     • Does middleware fire in the right order?`);
  console.log(`     • Is session state preserved across export/import?`);
  console.log(`     • Does prompt mutation actually reach the LLM?`);
  console.log(`     • Does config/header mutation affect the provider request?`);
  console.log(`     • Does intent override skip execution and use provided result?`);
  console.log(`     • Does intent skip prevent execution entirely?`);
  console.log(`     • Is middleware state shared across all hooks in a run?`);
  console.log(`     • Any duplicate or missing intent events?`);

  console.log(`\n${"═".repeat(70)}\n`);

  process.exit(failed > 0 ? 1 : 0);
}

main();
