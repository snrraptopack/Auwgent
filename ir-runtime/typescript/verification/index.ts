import { createManager, ManagerMiddleware, type ManagerConfig, Student } from "./main.agent.types"


const student: Student = {
    user_name: "Amihere",
    age: 10,
    id: "100",
    grades: ["A", "B"]
}

const config: ManagerConfig = {
    apiKeys: {
        geminiApiKey: Bun.env.GEMINI_API_KEY ?? ""
    },

    context: {
        user_name: "Amihere"
    },
    tools: {
        getStudentDetails: async (id) => student
    }
}

const router = createManager(config)

router.onIntent((name, value) => {

    if (name === "response_text") {
        console.log(value.text)
    }
})


console.log(router.generatePrompt())
const session = await router.run("hello please get the details for student with id 100")

console.log("session", JSON.stringify(session, null, 2))



