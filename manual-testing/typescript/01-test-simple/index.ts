import { auwgent, type AuwgentConfig } from "./generated/main.agent.types"
import { GROQ_API_KEY,GEMINI_API_KEY,OPEN_AI_API } from "@snrraptopack/auwgent-sdk/secrets"
import { create_todo, deactivate_user, get_weather_summary, read_todo, run_read_query, run_shell_command, schedule_meeting, search_web, send_email, write_file } from "tools"

const config: AuwgentConfig = {
  apiKeys: {
    groqApiKey: GROQ_API_KEY || ""
  },
  tools: {
    'create_todo': create_todo,
    'read_todo': read_todo,
    'schedule_meeting': schedule_meeting,
    'deactivate_user': deactivate_user,
    'send_email': send_email,
    'search_web': search_web,
    'get_weather_summary': get_weather_summary,
    'run_shell_command': run_shell_command,
    'write_file': write_file
  }
}

/**
 *   dns_resolve_a_record, run_shell_command, write_file, hash_string, and cache_set
 */
const agent = auwgent(config)

agent.onIntent((intent, value, name) => {
  if (intent === "response_text") {
    console.log("text", value)
  }
  if (intent === "tool_call") {
    console.log("tool_call", value)
  }
})

const session = await agent.run(`
 I need you to execute a system diagnostic workflow. First, use search_web to look up 'Latest TypeScript version'. Second, get the weather summary for 'San Francisco' in celsius. Third, run a shell command saying 'echo diagnostic complete' in '/tmp'. Fourth, write that exact shell command output to a file located at '/tmp/log.txt'. Fifth, create a high-priority to-do due on '2024-05-30' titled 'System Diagnostic Complete'. Finally, send an email to 'admin@example.com' with the subject 'Diagnostic Logs' and the body matching the completed status. make sure your steps are logical, dont call a tool when you know it not time to use it, dont come up with your own tool results,you can run more than one tool in single turn
`)
console.log(JSON.stringify(agent.getMetadata(), null, 2))
console.log(JSON.stringify(session.turns, null, 2))
