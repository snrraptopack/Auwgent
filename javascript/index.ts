import { createTypeSystemTest} from "../type-system-test.agent.types"
import data from "../type-system-test.agent.json"
import { TempKey1 } from "./keys";

// ============================================
// EXAMPLE 1: Basic Usage (Fluent API)
// ============================================
const agent = createTypeSystemTest({
    apiKeys: { geminiApiKey: TempKey1 },
    ir: data as any,
    context: { sessionId: "10" }  // Context bound once
})

// Clean execution - no need to pass context again
let result = await agent.stream({ message: "give me syntetic format of how you will give me data" })
    .onChunk((text) => {
        console.log(text)    
    })
    .onToolResult((name, result) => {
        console.log("tool", name)
        console.log("result", result)
    })
    .onToolEnd((name) => {
        console.log("end", name)
    })
    .onToolArgs((name, delta) => {
        console.log("args for", name, "args:", delta)
    })
    .run()

console.log("final", result)

// ============================================
// EXAMPLE 2: Native Async Iteration
// ============================================
console.log("\n=== Using streamIterable (native async iteration) ===\n")

for await (const chunk of agent.streamIterable({ message: "What is 2+2?" })) {
    if (chunk.type === 'text') {
        process.stdout.write(chunk.delta)
    }
    if (chunk.type === 'tool_result') {
        console.log(`\n[Tool Result] ${chunk.name}:`, chunk.result)
    }
}

// ============================================
// EXAMPLE 3: Multi-turn with forContext
// ============================================
console.log("\n=== Using forContext for multi-turn ===\n")

const sessionAgent = agent.forContext({ sessionId: "session-123" })

// Multiple calls with same session
await sessionAgent.run({ message: "Remember my name is John" })
await sessionAgent.run({ message: "What's my name?" })
await sessionAgent.run({ message: "Tell me a joke" })

// ============================================
// EXAMPLE 4: Override context per-call
// ============================================
console.log("\n=== Override context for specific call ===\n")

// Use default context
await agent.run({ message: "Default session" })

// Override for this specific call
await agent.run(
    { message: "Different session" },
    { context: { sessionId: "override-456" } }
)
