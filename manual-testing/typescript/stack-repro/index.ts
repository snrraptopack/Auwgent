import { auwgent, type AuwgentConfig, type AuwgentMiddleware } from "./generated/stack_test.agent.types";

import { GEMINI_API_KEY, GROQ_API_KEY } from "@snrraptopack/auwgent-sdk/secrets"
import { startRepl } from "../loop"

const stack = ["Router", "Analyzer"]

let one: AuwgentMiddleware = {
    name: "one",
  onLLMStart: (prompt, ctx) => {
    ctx.stack = stack
      console.log(ctx.stack)
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

//console.log(agent.generatePrompt("Analyzer"))



agent.onIntent((intent, value,agent) => {

  if (intent === "response_text") {
    console.log("response_text",value.text ?? value)
  }

  if (intent === "helper_call") {
    console.log("helper_call",value)
  }

  if (intent === "thought") {
    console.log("thought", value.explain ?? value)
  }

  if (intent === "error") {
    console.log("an error occured", value.message)
  }

  if (intent === "response_schema") {

  }

})

startRepl(agent)
