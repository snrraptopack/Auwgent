import { auwgent, type AuwgentConfig, type AuwgentMiddleware } from "./generated/main.agent.types"


const logger: AuwgentMiddleware = {
    name: "logger",
    onRunStart(session, ctx) {
        console.log("start here")
        return session
    },

    onRunComplete(finalSession, ctx) {
        // console.log(JSON.stringify(finalSession.turns.at(-1), null, 2))
    },

    onError(error, session, ctx) {
        console.log("error", error.message)
        return true
    },
}


let config: AuwgentConfig = {
    apiKeys: {
        my_groq_providerApiKey: Bun.env.GROQ_API_KEY || ""
    },
    middleware: [logger]
}

const agent = auwgent(config)


agent.onIntent((name, value) => {
    if (name === "response_schema") {
        console.log(value)
    }
})

const session = await agent.run("when did Ghana gain independence and tell me a story about it")


console.log("*****************session**************")
console.log(JSON.stringify(session, null, 2))