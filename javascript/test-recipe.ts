import { GoogleDriver } from "./loader/drivers/GoogleDriver";
import { OpenAIDriver } from "./loader/drivers/OpenAIDriver";
import { DriverRegistry } from "./loader/DriverRegistry";
import { createChef } from "../output/recipe-test.agent.types";
import data from "../output/recipe-test.agent.json";
import { kimi, TempKey } from "./keys";

// Setup drivers
const googleDriver = new GoogleDriver(TempKey);
const kimiDriver = new OpenAIDriver(kimi, "https://api.moonshot.ai/v1");

// Setup registry
const registry = new DriverRegistry();
registry.registerProvider("google", googleDriver);  // For helpers
registry.registerProvider("kimi", kimiDriver);      // For main agent

// Create agent
const agent = createChef(registry);
agent.load(data as any);

// Test: Ask for a recipe - should trigger the workflow
console.log("\n=== Recipe Test: Should use createRecipe workflow ===");
console.log("Request: Create a complete meal using chicken, rice, and bell peppers\n");

const result = await agent.run({
    request: "Create a complete meal using chicken, rice, and bell peppers"
});
console.log("Result:", result);
