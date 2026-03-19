import { auwgent, type AuwgentConfig, type AuwgentMiddleware } from "./generated/stack_test.agent.types";

import { GEMINI_API_KEY } from "@snrraptopack/auwgent-sdk/secrets"
import { startRepl } from "../loop"

let one: AuwgentMiddleware = {
    name: "one",
    onLLMStart: (prompt, ctx) => {
        //console.log(ctx.systemPrompt)
  },

  onError: (error, ctx)=> {
        console.error(error)
        return true
    }
}

const config: AuwgentConfig = {
    apiKeys: {
        geminiApiKey: GEMINI_API_KEY,
    },
    context: {
        user_name: "Theophilus"
    },
    middleware: [one]
}

let agent = auwgent(config)

console.log(agent.generatePrompt())


agent.onIntent((name, value) => {
    console.log("***********************************\n")
  console.log(`line 35 Intent: ${name}`, (value as any)?.text ?? value)
   console.log("***********************************\n")

})

startRepl(agent)


/**
 *
 Intent: thought {
   explain: "The user is asking if the `thought` intent helps me solve tasks freely. I need to explain that while `thought` helps me clarify my process, it doesn't give me \"free\" task-solving ability in the sense of deviating from my programmed logic or available tools. It's more about transparency. My task-solving is still based on my defined intents and helpers.",
 }
 ***********************************

 ***********************************

 Intent: response_text That's a great question! The `thought` intent primarily helps me by allowing me to articulate my internal processing steps to *you*, the user. It makes my decision-making transparent.
 ***********************************

 {
   "input": "does that step also help you to solve task freely?",
   "model_response": "thought:\n  explain: The user is asking if the `thought` intent helps me solve tasks freely. I need to explain that while `thought` helps me clarify my process, it doesn't give me \"free\" task-solving ability in the sense of deviating from my programmed logic or available tools. It's more about transparency. My task-solving is still based on my defined intents and helpers.\nresponse_text:\n  text: That's a great question! The `thought` intent primarily helps me by allowing me to articulate my internal processing steps to *you*, the user. It makes my decision-making transparent.\n\nIt doesn't, however, allow me to \"solve tasks freely\" in an unconstrained way. My problem-solving is still guided by my programming, the intents I have available (like `response_text`, `helper_call`), and the specific helpers I can use (like `StoryTeller`). The `thought` intent is more about explaining *how* I'm applying those predefined capabilities to address your request."
 }
 > so to you what other way do you think it will help you to improve yourself when solving problem?
 {
   "input": "so to you what other way do you think it will help you to improve yourself when solving problem?",
   "model_response": "(no response)"
 }
 > so to you what other way do you think it will help you to improve yourself when solving problem?
 {
   "input": "so to you what other way do you think it will help you to improve yourself when solving problem?",
   "model_response": "(no response)"
 }
 > so to you what other way do you think it will help you to improve yourself when solving problem?
 {
   "input": "so to you what other way do you think it will help you to improve yourself when solving problem?",
   "model_response": "(no response)"
 }
 >
 */



 /**
  *
  *
  Intent: thought {
    "1.  **`reflect` / `self_correct`**": "An intent to review my own actions, identify potential errors or inefficiencies, and propose a better approach. This is crucial for learning and improving over time.",
    "2.  **`request_clarification`**": "Although I can already ask follow-up questions with `response_text`, a dedicated intent would explicitly flag when I need more information to proceed accurately, indicating a point of ambiguity in my reasoning.",
    "3.  **`propose_plan` / `break_down_task`**": "For complex tasks, an intent to explicitly outline a step-by-step plan or break down a large problem into smaller, manageable sub-tasks before acting. This demonstrates structured thinking.",
    "4.  **`evaluate_options`**": "An intent to weigh different possible solutions or approaches to a problem, perhaps listing pros and cons, before making a decision.",
    "5.  **`synthesize_information`**": "An intent to explicitly state when I'm combining various pieces of information or tool results to form a new understanding or conclusion.",
    "6.  **`identify_assumptions`**": "An intent to explicitly state any assumptions I am making in my reasoning, which can then be validated or questioned by you.",
    explain: "Theophilus, that's an excellent question! To enhance my ability to reason and solve tasks more effectively, here are some types of intents that would be incredibly valuable. These would allow me to be more deliberate, reflective, and proactive in my processing:",
  }
  ***********************************

  ***********************************

  Intent: response_text To help me reason and solve tasks even better, Theophilus, you could consider adding intents that allow me to explicitly articulate internal thought processes, similar to how humans reason!

  Here are a few ideas for intents that would significantly enhance my capabilities: {}

  1.  **`reflect(previous_action_id: string, observations: string)`**: This would allow me to look back at a previous action, evaluate its outcome, and identify areas for improvement or alternative approaches.

  2.  **`request_clarification(query: string, context: string)`**: A dedicated intent to explicitly ask for more details when information is ambiguous or insufficient, ensuring I don't make assumptions.

  3.  **`propose_plan(steps: array<string>)`**: This would enable me to break down complex tasks into a sequence of steps before executing them, showing a structured approach to problem-solving.

  4.  **`evaluate_options(options: array<string>, criteria: string)`**: An intent to explicitly list and weigh different potential solutions or strategies for a given sub-task or problem.

  5.  **`synthesize_information(source_data: array<string>, conclusion: string)`**: This would be useful for demonstrating how I combine various pieces of information, perhaps from different tool results or user inputs, to form a coherent understanding or conclusion.
  ***********************************

  {
    "input": "if i want to help you reason and solve task well what other intents should i add to you?",
    "model_response": "thought:\n  explain: Theophilus, that's an excellent question! To enhance my ability to reason and solve tasks more effectively, here are some types of intents that would be incredibly valuable. These would allow me to be more deliberate, reflective, and proactive in my processing:\n\n  1.  **`reflect` / `self_correct`**: An intent to review my own actions, identify potential errors or inefficiencies, and propose a better approach. This is crucial for learning and improving over time.\n  2.  **`request_clarification`**: Although I can already ask follow-up questions with `response_text`, a dedicated intent would explicitly flag when I need more information to proceed accurately, indicating a point of ambiguity in my reasoning.\n  3.  **`propose_plan` / `break_down_task`**: For complex tasks, an intent to explicitly outline a step-by-step plan or break down a large problem into smaller, manageable sub-tasks before acting. This demonstrates structured thinking.\n  4.  **`evaluate_options`**: An intent to weigh different possible solutions or approaches to a problem, perhaps listing pros and cons, before making a decision.\n  5.  **`synthesize_information`**: An intent to explicitly state when I'm combining various pieces of information or tool results to form a new understanding or conclusion.\n  6.  **`identify_assumptions`**: An intent to explicitly state any assumptions I am making in my reasoning, which can then be validated or questioned by you.\n\n  The `thought` intent you already have is a fantastic start, as it allows me to explain my internal logical steps, which is a form of transparency in reasoning. Adding more explicit reasoning intents would further formalize and improve this process.\nresponse_text:\n  text: To help me reason and solve tasks even better, Theophilus, you could consider adding intents that allow me to explicitly articulate internal thought processes, similar to how humans reason!\n\nHere are a few ideas for intents that would significantly enhance my capabilities:\n\n1.  **`reflect(previous_action_id: string, observations: string)`**: This would allow me to look back at a previous action, evaluate its outcome, and identify areas for improvement or alternative approaches.\n2.  **`request_clarification(query: string, context: string)`**: A dedicated intent to explicitly ask for more details when information is ambiguous or insufficient, ensuring I don't make assumptions.\n3.  **`propose_plan(steps: array<string>)`**: This would enable me to break down complex tasks into a sequence of steps before executing them, showing a structured approach to problem-solving.\n4.  **`evaluate_options(options: array<string>, criteria: string)`**: An intent to explicitly list and weigh different potential solutions or strategies for a given sub-task or problem.\n5.  **`synthesize_information(source_data: array<string>, conclusion: string)`**: This would be useful for demonstrating how I combine various pieces of information, perhaps from different tool results or user inputs, to form a coherent understanding or conclusion.\n\nYour `thought` intent is already incredibly helpful for transparency in my reasoning! Adding more explicit reasoning intents would make my process even more robust and understandable."
  }
  >
  */
