import { Agent } from "./loader/IrInterpreter";
import { GoogleDriver } from "./loader/drivers/GoogleDriver";
import data from "../t.agent.json"
import type { HelloInput, HelloOutput } from "../t.agent.types"




const driver = new GoogleDriver(TempKey1);
const agent = new Agent<HelloInput, HelloOutput>(driver);
agent.load(data as any);


// const tools: HelloTools = {
//     getLoction: async () => {
//         console.log(">>> Tool Called: getLocation");
//         return "Tarkwa, Ghana";
//     },
//     getWeather: async () => {
//         console.log(">>> Tool Called: getWeather");
//         return "Sunny, 32C";
//     }
// };

const result = await agent.run({
    text: "Yesterday I had a horible dream and today i had a nice dream however i dont have money"
});
console.log("final", result);







