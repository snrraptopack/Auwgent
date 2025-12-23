
import { GoogleDriver } from "./loader/drivers/GoogleDriver";
import data from "../output/t.agent.json"
import { createOrderProcessor } from "../output/t.agent.types"
import { OpenAIDriver } from "./loader/drivers/OpenAIDriver";
import { kimi, TempKey } from "./keys";
import { DriverRegistry } from "./loader/DriverRegistry";
import { tools } from "./tools";



const driver = new GoogleDriver(TempKey);
const driver1 = new OpenAIDriver(kimi, "https://api.moonshot.ai/v1")

const registry = new DriverRegistry()
registry.registerProvider("kimi", driver1)
registry.registerProvider("google", driver)

const agent = createOrderProcessor(registry)
agent.load(data as any);


const result = await agent.run({
    request: "what model are you?"
}, tools, { id: 10, isAdmin: false });

console.log("final", result);







