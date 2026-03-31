import { auwgent, AuwgentConfig, AuwgentMiddleware } from "./generated/main.agent.types"
import { GROQ_API_KEY } from "@snrraptopack/auwgent-sdk/secrets"

let one: AuwgentMiddleware = {
  name: "one",
  target: "one",

}

const config: AuwgentConfig = {
  apiKeys: {
    my_groq_apiApiKey: GROQ_API_KEY
  },

}

const agent = auwgent(config)

agent.onIntent((name, value, agentname) => {
  console.log(JSON.stringify(value, null, 2))
})

console.log(agent.generatePrompt())

await agent.run(`Acme Corp is a software company based at 42 Innovation Drive, San Francisco, 94105, United States.
You can reach them at contact@acmecorp.io or +1-415-555-0199.

They are currently on the Pro pricing tier which costs $149 per month or $1,490 annually and
supports up to 50 seats. Their subscription id is sub_991 and it became active on 2024-01-15,
renewing on 2025-01-15. The subscription is currently active.

As part of their plan they have three features. The first is Analytics, id feat_01, which is
enabled. It allows up to 10000 units of type "events" and charges $0.002 per unit over the limit.
The second is Export, id feat_02, also enabled, capped at 500 units of type "exports" with an
overage rate of $0.05. The third is API Access, id feat_03, which is currently disabled, with a
limit of 1000 units of type "requests" and an overage rate of $0.001.

Acme Corp has 38 members and is tagged as enterprise, b2b and saas. Their organization id is
org_acme_01.`)

console.log(agent.getMetadata().aggregate)
