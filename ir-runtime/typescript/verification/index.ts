import { createCHEF, type CHEFConfig } from "./main.agent.types";

const geminiApiKey = Bun.env.GEMINI_API_KEY

let config: CHEFConfig = {
    tools: {
        tool_get_user_name: async () => "Theophilus"
    },
    apiKeys: {
        geminiApiKey: geminiApiKey ?? ""
    }
}

const chef = createCHEF(config)

// Register onIntent for real-time streaming visibility
chef.onIntent((name, value) => {
    console.log(`\n🔔 [${name}]`, JSON.stringify(value, null, 2));
});

if (!geminiApiKey) {
    console.log(chef.generatePrompt())
} else {
    let session = await chef.run("tell me a short story based on my name")
    console.log("\n=== Session Turns ===")
    console.log(JSON.stringify(session.turns, null, 2))
    console.log("\n=== System Prompt ===")
    console.log(session.systemPrompt)
}
