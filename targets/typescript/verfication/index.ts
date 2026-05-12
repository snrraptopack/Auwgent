import { GROQ_API_KEY } from "../secrets"
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
  }
}

const config: AuwgentConfig = {
  apiKeys: {
    groqApiKey: GROQ_API_KEY || ""
  },
  // tools: {
  //   create_todo,
  //   read_todo
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
})

const session = await agent.run(`hello my name is Theo I am 22 please present it in the output format`)
console.log(JSON.stringify(agent.getMetadata(), null, 2))
console.log("**************** \n \n")
console.log(JSON.stringify(session.turns,null,2))
