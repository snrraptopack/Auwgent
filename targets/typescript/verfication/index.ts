import { GEMINI_API_KEY, GROQ_API_KEY } from "../secrets"
import { auwgent, input, type AuwgentConfig, type AuwgentMiddleware } from "./main.agent.types"
import { create_todo, read_todo } from "./tools"
import { db } from "./db"
import { type SessionState } from "../types"


const logger: AuwgentMiddleware = {
  name: "logger",
  onRunStart: async (session, ctx) => {
    let data = await db.load<SessionState>("data.json", session)
    return data
  },
  onRunComplete: async (session, ctx) => {
    await db.save("data.json", session)
  }
}

const config: AuwgentConfig = {
  apiKeys: {
    geminiApiKey: GEMINI_API_KEY || ""
  },
  // tools: {
  //   get_age: async(args) => 10,
  //   get_name: async(args) => "Amihere Theophilus"
  // }
}
const agent = auwgent(config)


agent.onIntent((intent, value, name) => {
  if (intent === "response_text") {
    console.log("text", value)
  }

  if (intent === "response_schema") {
    console.log(JSON.stringify(value,null,2))
  }

  if (intent === "error") {
    console.log(value)
  }

  // if (intent === "tool_call") {
  //   console.log("tool call",value)
  // }

  // if (intent === "tool_result") {
  //   console.log("tool result",value)
  // }
})

const session = await agent.run([
  input.text("what is in the image? and generate a simple one for me"),
  input.image({
    url: "https://picsum.photos/id/237/200/300",
    mimeType:"image/png"
  })
])
console.log(JSON.stringify(agent.getMetadata(), null, 2))
console.log("**************** \n \n")
console.log(JSON.stringify(session.turns,null,2))
