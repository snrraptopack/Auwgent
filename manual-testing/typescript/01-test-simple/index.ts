import { auwgent, AuwgentConfig } from "./generated/main.agent.types"
import {GEMINI_API_KEY, KIMI_API_KEY} from "@snrraptopack/auwgent-sdk/secrets"

const config: AuwgentConfig = {
  apiKeys: {
    kimiApiKey:KIMI_API_KEY
  }
}

const agent = auwgent(config)

console.log(agent.generatePrompt())

agent.onIntent((name, value, agent) => {
  if (name === "response_text") {
      console.log(value.text)
  }
})

const session = await agent.run("hello")
