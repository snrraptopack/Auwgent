import { GROQ_API_KEY } from "../secrets"
import { auwgent, type AuwgentConfig } from "./main.agent.types"
import {create_todo,read_todo} from "./tools"


const config: AuwgentConfig = {
  apiKeys: {
    groqApiKey: GROQ_API_KEY || ""
  },
  tools: {
    create_todo,
    read_todo
  }
}
const agent = auwgent(config)


agent.onIntent((intent, value, name) => {
  if (intent === "response_text") {
    console.log("text", value)
  }
  if (intent === "tool_call") {
      console.log("tool_call", value)
  }
})

const session = await agent.run(`Create a new high-priority to-do called 'Fix the benchmark script' due on '2024-05-30'.
  Once it is created, use the ID you received to read the to-do back to me to
  confirm it was saved properly.`)
console.log(JSON.stringify(agent.getMetadata(), null, 2))
console.log("**************** \n \n")
console.log(JSON.stringify(session.turns,null,2))
