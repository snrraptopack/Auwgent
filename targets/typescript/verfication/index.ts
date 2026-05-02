import { GEMINI_API_KEY,GROQ_API_KEY } from "../secrets";
import {
  auwgent,
  type AuwgentApiKeys,
  type AuwgentConfig,
  type AuwgentContext,
  type AuwgentTools,
  AuwgentBaseIntentHandler,
  type AuwgentMiddleware
} from "./main.agent.types";

const logger: AuwgentMiddleware = {
  name: "simple",

  onRunStart: (session, ctx) => {
    console.log(ctx)
    return session
  }
}


let apiKeys: AuwgentApiKeys = {
  'groqApiKey': GROQ_API_KEY
}

let context: AuwgentContext = {
  'age': 100,
  'id': "100",
  'user_name':"Amihere"
}

let tools: AuwgentTools = {
  'get_location': async (args) => "Tarkwa",
  'get_marks': async (args) => "A,B,C"
}

let config: AuwgentConfig = {
  apiKeys,
  context,
  tools,
  middleware:[logger]
}

console.log("from the index.ts")

const agent = auwgent(config)

agent.onIntent((intent, value, agent) => {

  if (intent === "response_text") {
    console.log(value.text)
  }

  if(intent === "tool_call"){
    console.log("type", value.type, "args",value.args)
  }

  if(intent === "tool_result"){
    console.log("result",value)
  }

  if(intent === "workflow_call"){
     console.log("workflow call",value)
  }

  if(intent === "workflow_result"){
     console.log("result",value)
  }

  if (intent === "Loud") {
    console.log("loud",value)
  }

})

let session = await agent.run("hello get my marks for me")

console.log("meta",agent.getMetadata())
console.log(JSON.stringify(session.turns, null, 2))
