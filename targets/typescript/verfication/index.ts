import { GEMINI_API_KEY, GROQ_API_KEY } from "../secrets.ts"
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
  },
  onError: async (error, session) => {
    await db.save("data.json", session)
    console.log(error)
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
    'geminiApiKey': GEMINI_API_KEY
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

// const imgResp = await fetch("https://upload.wikimedia.org/wikipedia/commons/f/f2/LPU-v1-die.jpg");
// const buffer = await imgResp.arrayBuffer();

const session = await agent.run([
  input.text("what is in the image?"),
  input.image({
    url: "https://upload.wikimedia.org/wikipedia/commons/f/f2/LPU-v1-die.jpg",
    mimeType: "image/jpeg"
  })
])

const lastTurn = session.turns.at(-1)

console.log(JSON.stringify(agent.getMetadata(),null,2))
