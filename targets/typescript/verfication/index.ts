import { GEMINI_API_KEY, GROQ_API_KEY } from "../secrets"
import { auwgent, type AuwgentConfig, type AuwgentMiddleware } from "./main.agent.types"
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
  },
  onError: async (error,session,ctx)=>{
    return {swallow:true}
  }
}

const config: AuwgentConfig = {
  apiKeys: {
    groqApiKey: GROQ_API_KEY || ""
  },
  // tools: {
  //   get_age: async(args) => 10,
  //   get_name: async(args) => "Amihere Theophilus"
  // },
  middleware:[logger]
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

const session = await agent.run("hello my name is Theo i am 10 I'm from Ghana")
console.log(JSON.stringify(session.turns,null,2))
