import { createManger, MangerConfig, Student } from "./main.agent.types";

const geminiApiKey = Bun.env.GEMINI_API_KEY

let student: Student = {
    name: "Amihere Theophilus",
    id: "300",
    location: "Ghana",
    grades: ["A", "B", "C"]
}

let config: MangerConfig = {
    tools: {
        get_student_details: async ({ id }) => student,
        edit_student_details: async ({ id }) => student
    },
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


