import { createSchool,type SchoolTools,type Student} from "../test-autoreg.agent.types"

import data from "../test-autoreg.agent.json"
import { TempKey1 } from "./keys";

let high:Student = {
    name:"Amihere",
    class: "year 1",
    age: "10"
}

let low:Student = {
    name:"Theophilus",
    class: "year 2",
    age: "100"
}

const schoolTools:SchoolTools = {
    get_student_with_higher_grade: async()=>{
        return high
    },
    get_student_with_lower_grade: async ()=>{
        return low
    }
}

// ============================================
// EXAMPLE 1: Basic Usage (Fluent API)
// ============================================
const agent = createSchool({
    apiKeys: { geminiApiKey: TempKey1 },
    ir: data as any,
    tools:schoolTools
    // context: { sessionId: "10" }  // Context bound once
})

// Clean execution - no need to pass context again
let result = await agent.stream({ text: "who has the higher grades?" })
    .onText((text)=>{
        console.log(text)
    })
    .run()

console.log("final", result)

// // ============================================
// // EXAMPLE 2: Native Async Iteration
// // ============================================
// console.log("\n=== Using streamIterable (native async iteration) ===\n")

// for await (const chunk of agent.streamIterable({ message: "What is 2+2?" })) {
//     if (chunk.type === 'text') {
//         process.stdout.write(chunk.delta)
//     }
//     if (chunk.type === 'tool_result') {
//         console.log(`\n[Tool Result] ${chunk.name}:`, chunk.result)
//     }
// }

// // ============================================
// // EXAMPLE 3: Multi-turn with forContext
// // ============================================
// console.log("\n=== Using forContext for multi-turn ===\n")

// const sessionAgent = agent.forContext({ sessionId: "session-123" })

// // Multiple calls with same session
// await sessionAgent.run({ message: "Remember my name is John" })
// await sessionAgent.run({ message: "What's my name?" })
// await sessionAgent.run({ message: "Tell me a joke" })

// // ============================================
// // EXAMPLE 4: Override context per-call
// // ============================================
// console.log("\n=== Override context for specific call ===\n")

// // Use default context
// await agent.run({ message: "Default session" })

// // Override for this specific call
// await agent.run(
//     { message: "Different session" },
//     { context: { sessionId: "override-456" } }
// )
