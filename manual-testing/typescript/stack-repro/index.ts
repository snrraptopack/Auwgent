import { auwgent, type AuwgentConfig, type AuwgentMiddleware } from "./generated/stack_test.agent.types";

import { GEMINI_API_KEY, GROQ_API_KEY } from "@snrraptopack/auwgent-sdk/secrets"
import { startRepl } from "../loop"

let one: AuwgentMiddleware = {
    name: "one",
    onLLMStart: (prompt, ctx) => {
        console.log(ctx.systemPrompt)
    },

}

const config: AuwgentConfig = {
    apiKeys: {
        my_groq_apiApiKey: GROQ_API_KEY,
        geminiApiKey: GEMINI_API_KEY,
    },
    context: {
        user_name: "Theophilus"
    },
    middleware: [one]
}

let agent = auwgent(config)

//console.log(agent.generatePrompt())


agent.onIntent((name, value) => {

  if (name === "response_text") {
    console.log("response_text",value.text ?? value)
  }

  if (name === "helper_call") {
    console.log("helper_call",value)
  }

  if (name === "thought") {
    console.log("thought", value.explain ?? value)
  }

})

startRepl(agent)
