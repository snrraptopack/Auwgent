import { createRouter, RouterMiddleware, type RouterConfig } from "./main.agent.types"


const Logging: RouterMiddleware = {
    name: "Logging",
    onIntent: (name, value, _ctx) => {
        console.log("intent run")
        console.log(name, value)
    },

    onError: (error, session) => {
        console.log(error)
    },
}


const config: RouterConfig = {
    middleware: [Logging],
    apiKeys: {
        geminiApiKey: Bun.env.GEMINI_API_KEY ?? ""
    },
}

const router = createRouter(config)

router.onIntent((name, value) => {
    console.log("intent", name, value)
})

console.log(router.generatePrompt())
const session = await router.run("suggest a food")

//console.log(JSON.stringify(session, null, 2))



