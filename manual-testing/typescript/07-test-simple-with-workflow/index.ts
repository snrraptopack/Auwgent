import { auwgent, AuwgentConfig, AuwgentMiddleware } from "./generated/main.agent.types"
import { GEMINI_API_KEY, GROQ_API_KEY, KIMI_API_KEY } from "@snrraptopack/auwgent-sdk/secrets"
import { startRepl } from "../loop"
import { tools } from "./tools"


const config: AuwgentConfig = {
  tools,
  context: {
    is_vip: true,
    user_id: "user_1",
    session_id: "session_" + Date.now()
  },
  apiKeys: {
    //geminiApiKey:GEMINI_API_KEY,
    my_groq_apiApiKey: GROQ_API_KEY
  },
}

const agent = auwgent(config)

// console.log(agent.generatePrompt())

agent.onIntentPartial((name, value, agentName) => {
  if (name === "response_text") {
    process.stdout.write(value.delta ?? "")
  }

})



agent.onIntent((intent, value, name) => {
  if (intent === "response_schema") {
    console.log(JSON.stringify(value, null, 2),"agent",name)
  }
})

// agent.onIntent((name, value, agentName) => {

//   console.log("This block has to fire")



//   if (name === "response_schema") {
//     console.log("response_schema", value,"responding",agentName)
//   }

//   if (name === "workflow_call") {
//     console.log("workflow_call", value)
//   }

//   if(name === "workflow_result") {
//     console.log("workflow_result", value)
//   }

//   if (name === "SpeakLoud") {
//     console.log("💭 Thinking:", value.explain)
//   }

//   if (name === "tool_call") {
//     console.log("🔧 Tool call:", value.type, value.args)
//   }

//   if (name === "tool_result") {
//     console.log("🔧 Tool result:", value)
//   }

//   if (name === "helper_call") {
//     console.log("🤝 Helper call:", value)
//   }

//   if (name === "helper_result") {
//     console.log("helper result", value)
//   }

// })

//const session = await agent.run("hello please get me a deatailed analysis")


//console.log(JSON.stringify(session, null, 2))
// Check active handles


startRepl(agent)
