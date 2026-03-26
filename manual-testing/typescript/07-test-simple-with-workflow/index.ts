import { auwgent, AuwgentConfig } from "./generated/main.agent.types"
import { GEMINI_API_KEY, GROQ_API_KEY, KIMI_API_KEY } from "@snrraptopack/auwgent-sdk/secrets"
import { startRepl } from "../loop"
import { tools } from "./tools"

const config: AuwgentConfig = {
  tools,
  context: {
    is_vip: true,
    user_id: "user_1",
    session_id: "session_" + Date.now()
  },
  apiKeys: {
    //geminiApiKey:GEMINI_API_KEY,
    my_groq_apiApiKey: GROQ_API_KEY
  }
}

const agent = auwgent(config)

console.log(agent.generatePrompt())

agent.onIntentPartial((name, value,agentName) => {
  if (name === "response_text") {
    process.stdout.write(value.delta ?? "")
  }
})

agent.onIntent((name, value, agentName) => {

  console.log("This block has to fire")

  if (name === "response_schema") {
    console.log("response_schema", value,"responding",agentName)
  }

  if (name === "response_text") {
    console.log(value,"responding",agentName)
  }

  if (name === "workflow_call") {
    console.log("workflow_call", value)
  }

  if(name === "workflow_result") {
    console.log("workflow_result", value)
  }

  if (name === "SpeakLoud") {
    console.log("💭 Thinking:", value.explain)
  }

  if (name === "tool_call") {
    console.log("🔧 Tool call:", value.type, value.args)
  }

  if (name === "tool_result") {
    console.log("🔧 Tool result:", value)
  }

  if (name === "helper_call") {
    console.log("🤝 Helper call:", value)
  }

  if (name === "helper_result") {
    console.log("helper result", value)
  }


})


startRepl(agent)


/**
 * 
 * </response_textHello! How can I assist you today?This block has to fire
{
  text: "Hello! How can I assist you today?",
} responding Main
This log is from the repl loop file
[
  {
    "input": "hello",
    "model_response": "<response_text>\nHello! How can I assist you today?\n</response_text>"
  }
]
> add a person called ama with an email ama@gmail.com and give me a report
[tool_call: db_create_userThis block has to fire
🔧 Tool call: db_create_user {
  email: "ama@gmail.com",
  name: "ama",
}
This block has to fire
🔧 Tool result: {
  name: "db_create_user",
  args: {
    email: "ama@gmail.com",
    name: "ama",
  },
  result: "{\"success\":true,\"user\":{\"id\":\"user_4\",\"name\":\"ama\",\"email\":\"ama@gmail.com\",\"created_at\":\"2026-03-26\"}}",
}
[helper_call: ReportGeneratorThis block has to fire
🤝 Helper call: {
  type: "ReportGenerator",
  args: {
    parameters: "{}",
    report_type: "users",
  },
}
<To generate a user report, I will first query the database for all users and then format the data into a table.
[/response_text]

[tool_call: dbTo generate a user report, I will first query the database for all users and then format the data into a table.
[/response_text][customTo generate a user report, I will first query the database for all users and then format the data into a table.
[/response_text][custom: SpeakLTo generate a user report, I will first query the database for all users and then format the data into a table.
[/response_text][toolTo generate a user report, I will first query the database for all users and then format the data into a table.
[/response_text][tool_call: formatTo generate a user report, I will first query the database for all users and then format the data into a table.
[/response_text]"
[/toolTo generate a user report, I will first query the database for all users and then format the data into a table.
[/response_text]"


<To generate a user report, I will first query the database for all users and then format the data into a table.
[/response_text]"NowTo generate a user report, I will first query the database for all users and then format the data into a table.
[/response_text]"Now,To generate a user report, I will first query the database for all users and then format the data into a table.
[/response_text]"Now, let's generateTo generate a user report, I will first query the database for all users and then format the data into a table.
[/response_text]"Now, let's generate aTo generate a user report, I will first query the database for all users and then format the data into a table.
[/response_text]"Now, let's generate a chartTo generate a user report, I will first query the database for all users and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user dataTo generate a user report, I will first query the database for all users and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]To generate a user report, I will first query the database for all users 
and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]

[tool_call: generateTo generate a user report, I will first query the database for all users and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]To generate a user report, I will first query the database for all users 
and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]To generate a user report, I will first query the database for all users 
and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]To generate a user report, I will first query the database for all users 
and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]To generate a user report, I will first query the database for all users 
and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]To generate a user report, I will first query the database for all users 
and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text][sTo generate a user report, I will first query the database for all users and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]To generate a user report, I will first query the database for all users 
and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]To generate a user report, I will first query the database for all users 
and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]To generate a user report, I will first query the database for all users 
and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]To generate a user report, I will first query the database for all users 
and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]To generate a user report, I will first query the database for all users 
and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]To generate a user report, I will first query the database for all users 
and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]To generate a user report, I will first query the database for all users 
and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]To generate a user report, I will first query the database for all users 
and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]To generate a user report, I will first query the database for all users 
and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]To generate a user report, I will first query the database for all users 
and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]To generate a user report, I will first query the database for all users 
and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]To generate a user report, I will first query the database for all users 
and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]To generate a user report, I will first query the database for all users 
and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]To generate a user report, I will first query the database for all users 
and then format the data into a table.
[/response_text]"Now, let's generate a chart to visualize the user data.
[/response_text]This block has to fire
{
  text: "To generate a user report, I will first query the database for all users and then format the data into a table.\n[/response_text]",
} responding ReportGenerator
This block has to fire
🔧 Tool call: db_query_users {
  filter: "all",
}
This block has to fire
🔧 Tool result: {
  name: "db_query_users",
  args: {
    filter: "all",
  },
  result: "[\n  {\n    \"id\": \"user_1\",\n    \"name\": \"Alice Johnson\",\n    \"email\": \"alice@example.com\",\n    \"created_at\": \"2024-01-15\"\n  },\n  {\n    \"id\": \"user_2\",\n    \"name\": \"Bob Smith\",\n    \"email\": \"bob@example.com\",\n    \"created_at\": \"2024-02-20\"\n  },\n  {\n    \"id\": \"user_3\",\n    \"name\": \"Carol White\",\n    \"email\": \"carol@example.com\",\n    \"created_at\": \"2024-03-10\"\n  },\n  {\n    \"id\": \"user_4\",\n    \"name\": \"ama\",\n    \"email\": \"ama@gmail.com\",\n    
\"created_at\": \"2026-03-26\"\n  }\n]",
}
This block has to fire
💭 Thinking: I'm formatting the query results into a table for better readability...     
This block has to fire
🔧 Tool call: format_table {
  data: "}",
}
This block has to fire
🔧 Tool result: {
  name: "format_table",
  args: {
    data: "}",
  },
  result: "Error formatting table: }",
}
This block has to fire
{
  text: "\"",
} responding ReportGenerator
This block has to fire
{
  text: "Now, let's generate a chart to visualize the user data.\n[/response_text]",     
} responding ReportGenerator
This block has to fire
🔧 Tool call: generate_chart_description {
  chart_type: "bar",
  data: "10,20,30,40",
}
This block has to fire
🔧 Tool result: {
  name: "generate_chart_description",
  args: {
    chart_type: "bar",
    data: "10,20,30,40",
  },
  result: "Chart description for bar: 10,20,30,40",
}
This block has to fire
response_schema {
  type: "Output",
  response: {
    report_title: "User Report",
    sections: [ "Overview", "User Data", "Chart" ],
    generated_at: "2026-03-26",
    summary: "User report generated successfully",
  },
} responding ReportGenerator
This block has to fire
helper result {
  name: "ReportGenerator",
  args: {
    parameters: "{}",
    report_type: "users",
  },
  result: {
    __handoff_stop: true,
  },
}
This log is from the repl loop file
[
  {
    "input": "hello",
    "model_response": "<response_text>\nHello! How can I assist you today?\n</response_text>"
  },
  {
    "input": "add a person called ama with an email ama@gmail.com and give me a report", 
    "model_response": "[tool_call: db_create_user]\nemail: \"ama@gmail.com\"\nname: \"ama\"\n[/tool]"
  },
  {
    "input": "[result]\nname: db_create_user\nargs:\n  email: ama@gmail.com\n  name: ama\nresult: '{\"success\":true,\"user\":{\"id\":\"user_4\",\"name\":\"ama\",\"email\":\"ama@gmail.com\",\"created_at\":\"2026-03-26\"}}'\n[/result]",
    "model_response": "[helper_call: ReportGenerator]\nparameters: \"{}\"\nreport_type: \"users\"\n[/helper]"
  }
]
>
 */