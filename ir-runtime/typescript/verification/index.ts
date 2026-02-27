import { createManager, ManagerMiddleware, type ManagerConfig, Student } from "./main.agent.types"


const student: Student = {
    user_name: "Amihere",
    age: 10,
    id: "100",
    grades: ["A", "B"]
}

const jokerMiddleware: ManagerMiddleware = {
    name: "JokerLogger",
    target: "Joker", // <--- Targeted!
    onLLMEnd(response, ctx) {
        ctx.activeAgent
        console.log("[Jokey] response", response)
    },
    onRunComplete: (session, ctx) => {
        console.log("*********************** turns **************")
        console.log("turns", session)
        console.log("*********************** turns **************")
        console.log("*********************** ctx **************")
        console.log("ctx", ctx)
        console.log("*********************** ctx **************")
    },
};

const loggingMiddleware: ManagerMiddleware = {
    name: "Logger",
    onLLMStart: (prompt, ctx) => {
        console.log("llm", prompt)
    },
};

const config: ManagerConfig = {
    apiKeys: {
        geminiApiKey: Bun.env.GEMINI_API_KEY ?? ""
    },
    context: {
        user_name: "Amihere"
    },
    tools: {
        getStudentDetails: async (id) => student
    },
    middleware: [loggingMiddleware, jokerMiddleware]
}

const router = createManager(config)


router.onIntent((name, value) => {
    if (name === "response_text") {
        console.log(`\n[AGENT SAYS] ${value.text}\n`)
    } else if (name === "tool_call") {
        console.log(`\n[TOOL CALL] ${value.type}`)
    } else if (name === "helper_call") {
        console.log(`\n[HELPER CALL] ${value.type}`)
    }
    if (name === "helper_result") {
        console.log("result", value.result)
    }
})


//console.log(router.generatePrompt())
const session = await router.run("hello please get the details for student with id 100, and then ask the Joker helper to tell me a school joke")


//console.log("session", JSON.stringify(session, null, 2))



