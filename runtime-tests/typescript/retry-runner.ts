#!/usr/bin/env bun
import { auwgent, type AuwgentConfig, type AuwgentMiddleware, type AuwgentAgent } from "./canonical.agent.types";
import type { SessionState } from "@snrraptopack/auwgent-sdk";

const GROQ_API_KEY = process.env.GROQ_API_KEY || "";
if (!GROQ_API_KEY) { console.error("No GROQ_API_KEY"); process.exit(1); }

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

async function run(name: string, agent: AuwgentAgent, input: string) {
  const events: any[] = [];
  agent.onIntent((name, value, agentName) => { events.push({ name, value, agentName }); return null; });
  try {
    const session = await agent.run(input);
    console.log(`✅ ${name}`);
    console.log(`   Events: ${events.map(e => e.name).join(", ")}`);
    console.log(`   Turns: ${session.turns.length}`);
  } catch (e: any) {
    console.log(`❌ ${name}: ${e.message}`);
  }
}

async function main() {
  // Scenario 6 retry
  const agent6 = auwgent(createBaseConfig());
  await run("6. Helper (User handoff)", agent6, "Tell me a joke about programming.");

  await new Promise(r => setTimeout(r, 4000));

  // Scenario 15 retry
  const log15: string[] = [];
  const stackMw: AuwgentMiddleware = {
    name: "StackMutator",
    onLLMStart: async (prompt, ctx) => {
      log15.push(`llm_start | originalStack=${JSON.stringify(ctx.stack)}`);
      return { prompt, stack: ["RuntimeTest", "Planner"] };
    },
    onIntent: async (name, value, ctx) => {
      log15.push(`intent | stackDuringIntent=${JSON.stringify(ctx.stack)}`);
      return null;
    },
  };
  const cfg15 = createBaseConfig();
  cfg15.middleware = [stackMw];
  const agent15 = auwgent(cfg15);
  await run("15. Middleware Stack Mutation", agent15, "Say hello.");
  console.log("   Middleware log:", log15);
}

main();
