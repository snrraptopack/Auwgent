import { GROQ_API_KEY } from "../secrets.ts"
import { auwgent, input, type AuwgentConfig, type AuwgentMiddleware, type AuwgentTools } from "./main.agent.types.ts"
import { db } from "./db.ts"
import { type SessionState } from "../types.ts"

let Persist: AuwgentMiddleware = {
  name: "persist",
  onRunStart: async (session, ctx) => {
    let data = await db.load<SessionState>("data.json",session)
    return data
  },

  onRunComplete: async (session, ctx) => {
    await db.save("data.json",session)
  }
}

const tools: AuwgentTools = {
  get_location: async (args) => "Tarkwa/Ghana",
  get_user_marks: async (args) => ['A', 'B', 'C'],
  get_secrete_number: async (args) => "200",
  get_my_school: async(args) => "UMAT"
}

const config: AuwgentConfig = {
  apiKeys: {
    'groqApiKey': GROQ_API_KEY
  },
  context: {
    'user_name': "Theo"
  },
  tools,
  middleware:[Persist]
}
const agent = auwgent(config)

agent.onIntent((intent, value, agentName) => {
  if (intent === "response_text") {
    console.log(value.text)
  }

  if(intent === "tool_call"){
    console.log("tool call", value)
  }

  if(intent === "tool_result"){
    console.log("tool result",value)
  }

})

const session = await agent.run("hi")


console.log(JSON.stringify(agent.getMetadata(),null,2))
