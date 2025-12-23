
import { GoogleDriver } from "./loader/drivers/GoogleDriver";
import data from "../output/t.agent.json"
import { createUIMaker } from "../output/t.agent.types"
import { OpenAIDriver } from "./loader/drivers/OpenAIDriver";
import { kimi, TempKey } from "./keys";
import { DriverRegistry } from "./loader/DriverRegistry";
import { tools } from "./tools";



const driver = new GoogleDriver(TempKey);
const driver1 = new OpenAIDriver(kimi, "https://api.moonshot.ai/v1")

const registry = new DriverRegistry()
registry.registerProvider("kimi", driver1)
registry.registerProvider("google", driver)

const agent = createUIMaker(registry)
agent.load(data as any);


const result = await agent.run({
    request: "uild a dashboard with a sidebar navigation and a main content area showing user stats"
});

import fs from 'fs';

if ("code" in result) {
    // Option 2: Write to a file
    fs.writeFileSync('output.html', result.code);
}








