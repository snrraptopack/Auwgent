import { auwgent, type AuwgentConfig } from "./generated/main.agent.types"
import { GROQ_API_KEY } from "@snrraptopack/auwgent-sdk/secrets"

const config: AuwgentConfig = {
  apiKeys: {
    groq_apiApiKey: GROQ_API_KEY || ""
  }
}
const agent = auwgent(config)

console.log(agent.generatePrompt())
agent.onIntent((intent, value, name) => {
  if (intent === "response_text") {
    console.log("text", value)
  }
  if (intent === "response_schema") {
    console.log("json output", JSON.stringify(value.response, null, 2))
  }
})

const session = await agent.run(`Initialize a new Pro account for user 'usr_777' with the email 'shawn@example.com '. The account status should be set to active.`)
console.log(JSON.stringify(session.turns, null, 2))
console.log(agent.getMetadata())
