import { createTestAutoReg, type TestAutoRegLifecycle, type TestAutoRegTools } from "../test-autoreg.agent.types"
import data from "../test-autoreg.agent.json"
import { TempKey1 } from "./keys";


const memoryStore = new Map<string, any[]>();

const lifecycle: TestAutoRegLifecycle = {
    prune: async ({ context, usage }) => {
        console.log(`Prune called - ${usage.currentMessages} messages`);
        return { messages: [] };  // No pruning for now
    },

    load: async ({ context }) => {
        const history = memoryStore.get(context.chatId) ?? [];
        console.log(`Load: ${history.length} messages for chat ${context.chatId}`);
        return { messages: history };
    },

    save: async ({ newMessages, context, output }) => {
        const existing = memoryStore.get(context.chatId) ?? [];
        memoryStore.set(context.chatId, [...existing, ...newMessages]);
        console.log(`Saved ${newMessages.length} new messages`);
    }
};

const tools: TestAutoRegTools = {
    getStudentName: async ({ id: string }) => {
        return "Amihere"
    }
}


const agent = createTestAutoReg({
    geminiApiKey: TempKey1
})

agent.load(data as any)


let result = await agent.stream({ message: "what the name of the student with id 10" }, tools, { chatId: "123" }, lifecycle)
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


console.log(memoryStore.get("123"))