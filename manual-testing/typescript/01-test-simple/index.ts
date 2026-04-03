import { auwgent, AuwgentConfig, AuwgentMiddleware } from "./generated/main.agent.types"
import { GROQ_API_KEY } from "@snrraptopack/auwgent-sdk/secrets"


const config: AuwgentConfig = {
  apiKeys: {
    my_groq_apiApiKey: GROQ_API_KEY
  },

  tools: {
    user_name: async(args:{id})=> "Theo"
  }

}

const agent = auwgent(config)
agent.onIntentPartial((name, value, age) => {
  if (name == "response_text") {
    process.stdout.write(value.delta ?? "")
  }

  if (name === "tool_call") {
      console.log(value.args)
  }

})

agent.onIntent((name, value, agentname) => {
  console.log(JSON.stringify(value, null, 2))
   console.log("\n")
})

agent.onWarning((warning) => {
  console.log(warning)
})

console.log(agent.generatePrompt())

const session = await agent.run(`render one component for me to se`)

console.log(JSON.stringify(session,null,2))


console.log(agent.getMetadata().aggregate)
