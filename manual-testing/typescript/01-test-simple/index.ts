import { auwgent, AuwgentConfig } from "./generated/main.agent.types"
import {GEMINI_API_KEY} from "@snrraptopack/auwgent-sdk/secrets"

const config: AuwgentConfig = {
  apiKeys: {
    geminiApiKey:GEMINI_API_KEY
  }
}

const agent = auwgent(config)

console.log(agent.generatePrompt())

agent.onIntent((name, value, agent) => {
  if (name === "response_schema") {
    value
  }
})

const session = await agent.run("hello")
