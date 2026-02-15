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

chef.onIntentPartial((name, value) => {
    if (name === "response_text") {
        console.log("partial", value)
    }
})

if (!geminiApiKey) {
    console.log(chef.generatePrompt())
} else {
    let session = await chef.run("tell me a short story based on my name")
}
