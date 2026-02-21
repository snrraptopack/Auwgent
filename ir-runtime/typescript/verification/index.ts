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
    if (name === "response_text") {
        console.log(value.text)
    }

    if (name === "tool_call") {
        console.log(value)
    }

    if (name === "tool_result" && value.name === "get_student_details") {
        console.log(value.result.name)
    }
})


if (!geminiApiKey) {
    console.log(chef.generatePrompt())
} else {
    let session = await chef.run("hello what is my name,can you get my full details?")
    console.log(JSON.stringify(session.turns, null, 2))
}



// issues to solve....


/**
 * {
  "300, Location": {},
  "Ghana, Grades": "A, B, C.",
  "Hello Amihere Theophilus! Your full details are": {},
  ID: {},
  text: {},
}
[
  {
    "input": "hello what is my name,can you get my full details?",
    "model_response": "```yaml\ntool_call:\n  type: get_student_details\n  args:\n    id: \"300\"\n```"     
  },
  {
    "input": "tool_result:\n  name: get_student_details\n  result: {\"grades\":[\"A\",\"B\",\"C\"],\"id\":\"300\",\"location\":\"Ghana\",\"name\":\"Amihere Theophilus\"}",
    "model_response": "```yaml\nresponse_text:\n  text: Hello Amihere Theophilus! Your full details are: ID: 300, Location: Ghana, Grades: A, B, C.\n```"
  }
]
PS C:\Users\babyface\Desktop\auwgent\Auwgent\i
 */
