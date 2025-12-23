import { GoogleDriver } from "./loader/drivers/GoogleDriver";
import { OpenAIDriver } from "./loader/drivers/OpenAIDriver";
import { DriverRegistry } from "./loader/DriverRegistry";
import { createManager } from "../output/helper-test.agent.types";
import data from "../output/helper-test.agent.json";
import { kimi, TempKey } from "./keys";

// Setup drivers
const googleDriver = new GoogleDriver(TempKey);
const kimiDriver = new OpenAIDriver(kimi, "https://api.moonshot.ai/v1");

// Setup registry with providers
const registry = new DriverRegistry();
registry.registerProvider("google", googleDriver);  // For helper (gemini-2.5-flash)
registry.registerProvider("kimi", kimiDriver);      // For manager (kimi-k2-0905-preview)

// Create agent
const agent = createManager(registry);
agent.load(data as any);

// Test 1: Simple question (should NOT use helper)
console.log("\n=== Test 1: Simple Question ===");
const simple = await agent.run({ request: "What is 2 + 2?" });
console.log("Result:", simple);

// // Test 2: Complex question (should delegate to DeepThink helper)
// console.log("\n=== Test 2: Complex Question (should use DeepThink) ===");
// const complex = await agent.run({
//     request: "Analyze the philosophical implications of artificial intelligence achieving consciousness"
// });
// console.log("Result:", complex);

// // Test 3: Another complex question (should use CACHED helper)
// console.log("\n=== Test 3: Another Complex Question (should use CACHED helper) ===");
// const complex2 = await agent.run({
//     request: "What are the ethical implications of AI making life-or-death decisions?"
// });
// console.log("Result:", complex2);

// Test 4: Ask LLM to use the analyze workflow (which calls helper internally)
console.log("\n=== Test 4: Workflow calling Helper (LLM triggers workflow) ===");
const workflowResult = await agent.run({
    request: "Use your analyze workflow to analyze quantum computing"
});
console.log("Result:", workflowResult);
