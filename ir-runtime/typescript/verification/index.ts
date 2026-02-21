import { createManger, MangerConfig, Student, type MangerMiddleware } from "./main.agent.types";

const geminiApiKey = Bun.env.GEMINI_API_KEY

let student: Student = {
    name: "Amihere Theophilus",
    id: "300",
    location: "Ghana",
    grades: ["A", "B", "C"]
}

const LoggingMiddleware: MangerMiddleware = {
    name: "Logger",
    onRunStart: async (session, ctx) => {
        ctx.startTime = Date.now();
        console.log(`[Middleware] Run starting with ${session.turns.length} past turns.`);
        return session;
    },
    onIntent: (name, value, ctx) => {
        console.log(`[Middleware Intent] ${name}`);
    },
    onRunComplete: async (session, ctx) => {
        console.log(`[Middleware] Run finished in ${Date.now() - ctx.startTime}ms`);
    }
};

let config: MangerConfig = {
    tools: {
        get_student_details: async ({ id }) => student,
        edit_student_details: async ({ id }) => student
    },
    middleware: [LoggingMiddleware],
    apiKeys: {
        geminiApiKey: geminiApiKey ?? ""
    },
    context: {
        user_name: "Theophilus",
        id: "300"
    }
}

const chef = createManger(config)


chef.onIntent((name, value) => {
    if (name === "workflow_result") {
        console.log(value)
    }
})



if (!geminiApiKey) {
    console.log(chef.generatePrompt())
} else {
    let session = await chef.run("what is my grade")
    console.log(JSON.stringify(session.turns, null, 2))
}
