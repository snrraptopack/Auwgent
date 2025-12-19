import { Agent } from "./loader/IrInterpreter";
import { GoogleDriver } from "./loader/drivers/GoogleDriver";
import data from "../t.agent.json"
import type { HelloInput, HelloOutput, HelloTools } from "../t.agent.types"


const TempKey = "Your api key"

const driver = new GoogleDriver(TempKey);
const agent = new Agent<HelloInput, HelloOutput, HelloTools>(driver);
agent.load(data as any);


const tools: HelloTools = {
    getLoction: async () => {
        console.log(">>> Tool Called: getLocation");
        return "Tarkwa, Ghana";
    },
    getWeather: async () => {
        console.log(">>> Tool Called: getWeather");
        return "Sunny, 32C";
    }
};

const result = await agent.run({
    text: "what is my current weather ?"
}, tools);
console.log("final", result);







