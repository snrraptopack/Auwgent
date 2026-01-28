/**
 * Examples of the new unified configuration API
 */

import { createTypeSystemTest } from "../type-system-test.agent.types"
import data from "../type-system-test.agent.json"
import { TempKey1 } from "./keys";

// ============================================
// 1. BASIC SETUP - Unified Configuration
// ============================================

const agent = createTypeSystemTest({
    apiKeys: { geminiApiKey: TempKey1 },
    ir: data as any,
    context: { sessionId: "10" }  // Bound once at creation
})

// ============================================
// 2. FLUENT STREAMING API (Original)
// ============================================

const result1 = await agent.stream({ message: "Hello" })
    .onChunk(delta => console.log(delta))
    .onToolResult((name, result) => console.log(name, result))
    .run()

// ============================================
// 3. NATIVE ASYNC ITERATION (New!)
// ============================================

for await (const chunk of agent.streamIterable({ message: "Hello" })) {
    if (chunk.type === 'text') {
        process.stdout.write(chunk.delta)
    }
    if (chunk.type === 'tool_result') {
        console.log(`Tool: ${chunk.name}`, chunk.result)
    }
    if (chunk.type === 'helper_start') {
        console.log(`Helper started: ${chunk.name}`)
    }
}

// ============================================
// 4. SIMPLE RUN (No streaming)
// ============================================

const result2 = await agent.run({ message: "What is 2+2?" })
console.log(result2)

// ============================================
// 5. MULTI-TURN CONVERSATIONS
// ============================================

// Bind session once
const sessionAgent = agent.forContext({ sessionId: "session-123" })

// Multiple calls with same context
await sessionAgent.run({ message: "My name is John" })
await sessionAgent.run({ message: "What's my name?" })
await sessionAgent.run({ message: "Tell me about yourself" })

// ============================================
// 6. OVERRIDE CONTEXT PER-CALL
// ============================================

// Use default context (sessionId: "10")
await agent.run({ message: "Default session" })

// Override for specific call
await agent.run(
    { message: "Different session" },
    { context: { sessionId: "override-456" } }
)

// ============================================
// 7. DIFFERENT SESSIONS FROM SAME AGENT
// ============================================

const user1 = agent.forContext({ sessionId: "user-1" })
const user2 = agent.forContext({ sessionId: "user-2" })

await user1.run({ message: "I like pizza" })
await user2.run({ message: "I like burgers" })

// Each maintains separate context
