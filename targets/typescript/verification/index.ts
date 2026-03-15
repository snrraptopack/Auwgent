import { auwgent, type AuwgentConfig } from "./generated/main.agent.types"

let config: AuwgentConfig = {
    apiKeys: {
        geminiApiKey: Bun.env.GEMINI_API_KEY || Bun.env.GEMINI || ""
    }
}

const agent = auwgent(config)

console.log(agent.generatePrompt())

agent.onIntent((name, value) => {
    if (name === "response_schema") {
        console.log(value)
    }
})

const session = await agent.run("when did Ghana gain independence")

console.log(session)


