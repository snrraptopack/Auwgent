import { auwgent, AuwgentConfig, AuwgentMiddleware } from "./generated/main.agent.types"
import { GROQ_API_KEY } from "@snrraptopack/auwgent-sdk/secrets"


const config: AuwgentConfig = {
  apiKeys: {
    my_groq_apiApiKey: GROQ_API_KEY
  },

  tools: {
    user_name: async()=> "Theo"
  }

}

const agent = auwgent(config)
agent.onIntentPartial((name, value, age) => {
  if (name == "response_text") {
    value
  }

})

agent.onIntent((name, value, agentname) => {
  console.log(JSON.stringify(value, null, 2))

})

agent.onWarning((warning) => {
  console.log(warning)
})

console.log(agent.generatePrompt())

await agent.run(`what my name`)

console.log(agent.getMetadata().aggregate)
