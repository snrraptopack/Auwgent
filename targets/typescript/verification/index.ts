import { auwgent, type AuwgentConfig, type AuwgentMiddleware } from "./generated/main.agent.types"


const logger: AuwgentMiddleware = {
    name: "logger",
    onRunStart(session, ctx) {
      console.log("start here")

      ctx.
        return session
    },

    async onLLMStart(prompt, ctx) {
        const result = await ctx.embed(prompt)
        console.log("model start")
        console.log("result", result)
    },


    onError(error, session, ctx) {
        console.log("error", error.message)
        return true
    },
}


let config: AuwgentConfig = {
    apiKeys: {
        my_kimi_apiApiKey: Bun.env.KIMI_API_KEY || "",
        geminiApiKey: Bun.env.GEMINI_API_KEY || ""
    },
    middleware: [logger]
}

const agent = auwgent(config)

console.log(agent.generatePrompt())

agent.onIntent((name, value) => {

    if (name === "question") {
        console.log("question:", value)
    } else {
        console.log("others", value)
    }

})


const session = await agent.run("do you have the cup")
