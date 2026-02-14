import { createCHEF, type CHEFConfig } from "./main.agent.types";

const geminiApiKey = Bun.env.GEMINI_API_KEY

let config:CHEFConfig ={
    tools:{
        tool_get_user_name : async ()=> "Theophilus"
    },
    apiKeys:{
        geminiApiKey: geminiApiKey ?? ""
    }
}

const chef = createCHEF(config)

if (!geminiApiKey) {
    console.log(chef.generatePrompt())
} else {
    let sessions = await chef.run("hello what my name")
    console.log(sessions.systemPrompt)
    console.log(sessions.steps)
}
