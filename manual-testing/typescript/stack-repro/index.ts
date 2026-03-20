import { auwgent, type AuwgentConfig, type AuwgentMiddleware } from "./generated/stack_test.agent.types";

import { GEMINI_API_KEY } from "@snrraptopack/auwgent-sdk/secrets"
import { startRepl } from "../loop"

let one: AuwgentMiddleware = {
    name: "one",
    onLLMStart: (prompt, ctx) => {
        //console.log(ctx.systemPrompt)
    },

    onError: (error, ctx) => {
        console.error(error)
        return true
    }
}

const config: AuwgentConfig = {
    apiKeys: {
        geminiApiKey: GEMINI_API_KEY,
    },
    context: {
        user_name: "Theophilus"
    },
    middleware: [one]
}

let agent = auwgent(config)

console.log(agent.generatePrompt())


agent.onIntent((name, value) => {
    console.log("***********************************\n")
    console.log(`line 35 Intent: ${name}`, (value as any)?.text ?? value)
    console.log("***********************************\n")

})

startRepl(agent)
