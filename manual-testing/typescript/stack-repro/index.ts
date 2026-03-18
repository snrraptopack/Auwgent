import { auwgent, AuwgentConfig } from "./generated/stack_test.agent.types";

import { GEMINI_API_KEY } from "@snrraptopack/auwgent-sdk/secrets"
import { startRepl } from "../loop"

const config: AuwgentConfig = {
    apiKeys: {
        geminiApiKey: GEMINI_API_KEY
    }
}


let agent = await auwgent(config)



console.log(agent.generatePrompt())

startRepl(agent)