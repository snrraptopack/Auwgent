import { auwgent, type AuwgentConfig, type AuwgentMiddleware } from "./generated/stack_test.agent.types";

import { GEMINI_API_KEY } from "@snrraptopack/auwgent-sdk/secrets"
import { startRepl } from "../loop"

let one: AuwgentMiddleware = {
    name: "one",
    onLLMStart: (prompt, ctx) => {
        //console.log(ctx.systemPrompt)
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





let agent = await auwgent(config)


agent.onIntent((name, value) => {
    if (name === "response_text") {
        console.log("[response_text] : ", value.text ?? value)
        console.log("\n")
    }
    if (name === "thought") {
        console.log("[explain] : ", value.explain ?? value)
        console.log("\n")
    }

    if (name === "questions") {
        console.log("[quesion] : ", value.questions ?? value)
        console.log("\n")
    }

    if (name === "helper_call") {
        console.log("[calling heloer] : ", value)
        console.log("\n")
    }

})


startRepl(agent)