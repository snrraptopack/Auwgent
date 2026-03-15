import { auwgent, type AuwgentConfig, type AuwgentMiddleware } from "./generated/main.agent.types"


const logger: AuwgentMiddleware = {
    name: "logger",
    onRunStart(session, ctx) {
        console.log("start here")
        return session
    },

    onRunComplete(finalSession, ctx) {
        console.log(JSON.stringify(finalSession.turns.at(-1), null, 2))
    },
}


let config: AuwgentConfig = {
    apiKeys: {
        my_groq_providerApiKey: Bun.env.GEMINI_API_KEY || Bun.env.GEMINI || ""
    },
    middleware: [logger]
}

const agent = auwgent(config)

agent.onIntentPartial((name, value) => {
    if (name === "response_schema") {
        process.stdout.write(value.response)
    }
})

const session = await agent.run("when did Ghana gain independence and tell me a story about it")


