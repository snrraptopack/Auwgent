import { createRouter, RouterMiddleware, type RouterConfig } from "./main.agent.types"


const Logging: RouterMiddleware = {
    name: "Logging",
    onIntent: (name, value, _ctx) => {
        console.log(name, value)
    },

    onError: (error, session) => {
        console.log(error)
    }
}


const config: RouterConfig = {
    middleware: [Logging],
    apiKeys: {
        geminiApiKey: Bun.env.GEMINI_API_KEY ?? ""
    },
}

const router = createRouter(config)

const session = await router.run("hello")


console.log(JSON.stringify(session, null, 2))



