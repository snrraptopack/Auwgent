import { auwgent, type AuwgentConfig, type AuwgentMiddleware } from "./generated/main.agent.types"


const logger: AuwgentMiddleware = {
    name: "logger",
    onRunStart(session, ctx) {
        console.log("start here")
        return session
    },

    onRunComplete(finalSession, ctx) {

        console.log("final session", finalSession)
    },

    onIntent(...args) {

        if (args[0] === "response_schema") {
            console.log(args[1].response)
        }
    },

    onLLMEnd(session, ctx) {
        console.log(ctx.rawBlock)
    },

    onError(error, session, ctx) {
        console.log("error", error.message)
        return true
    },
}


let config: AuwgentConfig = {
    apiKeys: {
        my_kimi_apiApiKey: Bun.env.KIMI_API_KEY || ""
    },
    middleware: [logger]
}

const agent = auwgent(config)


const session = await agent.run("when did Ghana gain independence and tell me a story about it")


