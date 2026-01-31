import { TempKey1 } from "./keys";
import { createAuditMiddleware, createShortMemoryMiddleware } from "./loader";
import { createHello,type HelloConfig,type HelloTools,type User } from "./out/main.agent.types";

let user:User = {
    name:"Amihere",
    age:10,
    location:"Tarkwa",
    school: "Umat"
}

const middlewareState = {events:[], recent:[], maxMessages:20}

let tools:HelloTools = {
    get_user_details: async({id})=>{
        return user
    }
}

let config:HelloConfig = {
    apiKeys: {
        geminiApiKey:TempKey1
    },
    context:{
        id:10,
        user_name:"Amihere"
    },
    tools,
    middleware:[
        createAuditMiddleware({includeToolArgs:true,includeToolResults:true}),
        createShortMemoryMiddleware(20)
    ],
    middlewareState,
    runId:"run-123"
}

let helloAgent = createHello(config)

const run = async () => {
    await helloAgent
        .stream({text:"hello what do you know about me"})
        .onText((text)=>{
            console.log(text)
        })
        .onToolResult((name,result)=>{
            console.log("called ",name)
            console.log("with result",result)
        })
        .run()

    console.log(middlewareState.events)
    console.log("memory",middlewareState.recent)
}

run()
