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


let lastLength = 0;
let lastIntent = "";
chef.onIntentPartial((name, value) => {
    if (name !== lastIntent) {
        if (lastIntent === "response_text") {
            console.log(); // print newline to end the text stream
        }
        if (name !== "response_schema") {
            console.log(`\n[Agent is working on: ${name}]`);
        }
        lastLength = 0;
        lastIntent = name;
    }

    if (name === "response_schema") {
        console.log(value)
    }
})


if (!geminiApiKey) {
    console.log(chef.generatePrompt())
} else {
    let session = await chef.run("hello what is my name,can you get my full details and use it to write a story?")
    console.log(JSON.stringify(session.turns, null, 2))
}


