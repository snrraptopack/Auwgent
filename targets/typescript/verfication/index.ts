import { GROQ_API_KEY } from "../secrets.ts"
import { auwgent, type AuwgentConfig, type AuwgentMiddleware, type AuwgentTools } from "./main.agent.types.ts"


let log: AuwgentMiddleware = {
  name: "one",
  onRunStart: (session, ctx) => {
    ctx.one = "hello"
    return session
  },

  onLLMStart: (prompt, ctx) => {
    ctx.setContext({marks:['A','B','C'], location:"Tarkwa/Accra"})
    console.log(ctx.one)
  }
}

const tools: AuwgentTools = {
  get_location: async (args) => "Tarkwa/Ghana",
  get_user_marks: async (args) => ['A', 'B', 'C']
}

const config: AuwgentConfig = {
  apiKeys: {
    'groqApiKey': GROQ_API_KEY
  },
  context: {
    'user_name': "Theo"
  },
  tools,
  middleware:[log]
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


const session = await agent.run("hello in the system prompt do you see anything related to location and marks")
