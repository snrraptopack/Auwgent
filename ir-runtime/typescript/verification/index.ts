import { createManager, type ManagerConfig } from "./main.agent.types"
import { getStudentDetails } from "./tools";

// agent intilization
const agent = createManager({
    apiKeys: {geminiApiKey: Bun.env.GEMINI_API_KEY ?? ""},
    context: {user_name: "Amihere"},
    tools: { getStudentDetails }
})

agent.onIntent((name, value) => {
    if (name === "response_text") {
        console.log(`answer: ${value.text}`)

    }else if (name === "tool_call") {
        console.log(`[tool call] ${value.type} with args of ${value.args.id}`)
    }
})

const session = await agent.run("what is the deatails for student with id 10")
