import { createTypeSystemTest} from "../test-autoreg.agent.types"
import data from "../test-autoreg.agent.json"
import { TempKey1 } from "./keys";


const agent = createTypeSystemTest({
    geminiApiKey: TempKey1
})

agent.load(data as any)


let result = await agent.stream({ message: "what the name of the student with id 10" },{sessionId:"10"})
    .onChunk((text) => {
        console.log(text)    
    })
    .onToolResult((name,result)=>{
        console.log("tool",name)
        console.log("result",result)
    })
    .onToolEnd((name)=>{
        console.log("end",name)
    })
    .onToolArgs((name,delta)=>{
        console.log("args for",name, "args:",delta)
    })
    .run()

console.log("final", result)

