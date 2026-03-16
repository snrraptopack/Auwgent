import { auwgent, type AuwgentConfig, type AuwgentMiddleware } from "./generated/main.agent.types"


const logger: AuwgentMiddleware = {
    name: "logger",
    onRunStart(session, ctx) {
        console.log("start here")
        return session
    },

    onRunComplete(finalSession, ctx) {

        console.log("final session",finalSession)
    },

    onLLMEnd(session,ctx){
        console.log(ctx.rawBlock)
    },

    onError(error, session, ctx) {
        console.log("error", error.message)
        return true
    },
}


let config: AuwgentConfig = {
    apiKeys: {
        my_groq_apiApiKey: Bun.env.GROQ_API_KEY || ""
    },
    middleware: [logger]
}

const agent = auwgent(config)

let lastLength = 0
agent.onIntent((name, value) => {
    if (name === "response_schema") {
        console.log("final value",value)
    }
})

const session = await agent.run("when did Ghana gain independence and tell me a story about it")


