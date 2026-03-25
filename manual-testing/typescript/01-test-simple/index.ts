import { auwgent, AuwgentConfig, AuwgentMiddleware } from "./generated/main.agent.types"


let one: AuwgentMiddleware = {
  name: "one",
  target: "one",

}

const config: AuwgentConfig = {
  apiKeys: {
    GeminiApiKey:YOUR API KEY
  },
  context: {
    is_vip: false,
    name: "auwgent"
  }
}

const agent = auwgent(config)

agent.onIntent((name, value, agent) => {
  if (name === "response_text") {
      console.log(value.text)
  }
})

await agent.run("hello")
