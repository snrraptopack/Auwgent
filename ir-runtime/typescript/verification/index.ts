import { createManager, type ManagerConfig } from "./main.agent.types"
import { getStudentDetails } from "./tools";


const config: ManagerConfig = {
    apiKeys: {
        geminiApiKey: Bun.env.GEMINI_API_KEY ?? ""
    },
    context: {
        user_name: "Amihere"
    },
    tools: { getStudentDetails }
}

const router = createManager(config)


router.onIntent((name, value) => {
    if (name === "response_text") {
        console.log(`answer: ${value.text}`)
    }

    if (name === "tool_call") {
        console.log(`[tool call] ${value.type} with args of ${value.args}`)
    }
})

const session = await router.run("what is the deatails for student with id 10")



//console.log(router.generatePrompt())


//console.log("session", JSON.stringify(session, null, 2))



