import { createHello, type HelloConfig } from "./main.agent.types"

let config: HelloConfig = {
    apiKeys: {
        geminiApiKey: Bun.env.GEMINI_API_KEY || Bun.env.GEMINI || ""
    }
}

const agent = createHello(config)

console.log(agent.generatePrompt())

agent.onIntent((name, value) => {
    if (name === "response_schema") {
        console.log(value)
    }
})


