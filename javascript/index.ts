
import { GoogleDriver } from "./loader/drivers/GoogleDriver";
import data from "../output/t.agent.json"
import { createUIMaker } from "../output/t.agent.types"
import { OpenAIDriver } from "./loader/drivers/OpenAIDriver";
import { kimi, openAiKey, TempKey } from "./keys";
import { DriverRegistry } from "./loader/DriverRegistry";
import { writeFileSync } from "fs";
import { logger, LogLevel } from "./loader/Logger";

// ===== Configure logging level =====
// LogLevel.NONE   = silence everything
// LogLevel.ERROR  = only errors
// LogLevel.WARN   = errors + warnings  
// LogLevel.INFO   = normal output (default for production)
// LogLevel.DEBUG  = verbose (default for development)
// LogLevel.TRACE  = everything

logger.setLevel(LogLevel.INFO);  // Change to DEBUG to see all internal logs

const driver = new GoogleDriver(TempKey);
const driver1 = new OpenAIDriver(kimi, "https://api.moonshot.ai/v1")
const openAi = new OpenAIDriver(openAiKey)

const registry = new DriverRegistry()
registry.registerProvider("kimi", driver1)
registry.registerProvider("google", driver)
registry.registerProvider("openai", openAi)

const agent = createUIMaker(registry)
agent.load(data as any);

console.log("🚀 Starting agent...\n");

// Track code from helpers
let capturedCode = "";

const finalResult = await agent
    .stream({ request: "hello" })

    // Tool lifecycle
    .onToolStart((name) => console.log(`🔧 [Tool] ${name} started`))

    // Helper (sub-agent) lifecycle
    .onHelperStart(name => console.log(`>>> [Helper] ${name} starting...`))
    .onHelperEnd((name, result) => {
        console.log(`<<< [Helper] ${name} completed`);
        // Capture code from UIProgrammer
        if (name === "UIProgrammer" && result?.code) {
            capturedCode = result.code;
        }
    })

    // Stream helper chunks (shows the actual streaming output from helpers)
    .onHelperChunk((name, chunk) => {
        if (chunk.type === 'text') {
            process.stdout.write(chunk.delta);
        }
    })

    // Transfer events
    .onTransfer((helperName, mode) => {
        console.log(`🔀 [Transfer] to ${helperName} (mode: ${mode})`);
    })

    // Execute and get result
    .run();

// Write code to file if captured
if (capturedCode) {
    writeFileSync("output.html", capturedCode);
    console.log("\n✅ Code saved to output.html");
}

// Show final acknowledgment
if (finalResult && typeof finalResult === 'object' && 'result' in finalResult) {
    console.log(`\n💬 Model: ${(finalResult as any).result}`);
}

console.log("\n🏁 Done!");

// Show stats (token usage, call counts)
logger.finalize();
logger.printStats();




