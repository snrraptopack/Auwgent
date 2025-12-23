# Auwgent

A Domain-Specific Language (DSL) for building AI agents with declarative syntax, type safety, and provider-agnostic execution.

## The Problem with Traditional AI Development

Let's build a simple student lookup agent. Here's how it looks with popular frameworks:

### Vercel AI SDK

```typescript
import { generateText, tool } from 'ai';
import { openai } from '@ai-sdk/openai';
import { z } from 'zod';

const result = await generateText({
  model: openai('gpt-4o'),
  system: 'You are a helpful assistant with student lookup tools.',
  prompt: 'Get details for student 3',
  tools: {
    getStudentGrade: tool({
      description: 'Get student grade',
      parameters: z.object({ id: z.number() }),
      execute: async ({ id }) => db.getGrade(id),
    }),
    getStudentName: tool({
      description: 'Get student name', 
      parameters: z.object({ id: z.number() }),
      execute: async ({ id }) => db.getName(id),
    }),
    getStudentLocation: tool({
      description: 'Get student location',
      parameters: z.object({ id: z.number() }),
      execute: async ({ id }) => db.getLocation(id),
    }),
  },
  maxSteps: 5,
});
```

**Issues:** Provider locked to OpenAI. Tool definitions mixed with implementations. No way to define a "getStudentDetails" workflow that combines all three calls - you rely on the LLM to figure it out.

### LangChain

```typescript
import { ChatOpenAI } from "@langchain/openai";
import { DynamicStructuredTool } from "@langchain/core/tools";
import { AgentExecutor, createOpenAIFunctionsAgent } from "langchain/agents";
import { ChatPromptTemplate } from "@langchain/core/prompts";
import { z } from "zod";

const model = new ChatOpenAI({ model: "gpt-4o" });

const tools = [
  new DynamicStructuredTool({
    name: "getStudentGrade",
    description: "Get student grade",
    schema: z.object({ id: z.number() }),
    func: async ({ id }) => db.getGrade(id),
  }),
  new DynamicStructuredTool({
    name: "getStudentName",
    description: "Get student name",
    schema: z.object({ id: z.number() }),
    func: async ({ id }) => db.getName(id),
  }),
  new DynamicStructuredTool({
    name: "getStudentLocation", 
    description: "Get student location",
    schema: z.object({ id: z.number() }),
    func: async ({ id }) => db.getLocation(id),
  }),
];

const prompt = ChatPromptTemplate.fromMessages([
  ["system", "You are a helpful assistant with student lookup tools."],
  ["human", "{input}"],
  ["placeholder", "{agent_scratchpad}"],
]);

const agent = await createOpenAIFunctionsAgent({ llm: model, tools, prompt });
const executor = new AgentExecutor({ agent, tools });

const result = await executor.invoke({ input: "Get details for student 3" });
```

**Issues:** Verbose setup. Heavy abstraction layers. Still no deterministic workflows - the LLM decides which tools to call and in what order. Switching providers requires changing imports and agent creation.

### Common Problems

Both approaches share these issues:
- **No separation**: Tool schemas and implementations are coupled
- **No compile-time safety**: Errors only surface at runtime
- **No deterministic workflows**: Multi-step operations rely on LLM reasoning
- **Provider coupling**: Switching requires code changes throughout

## The Auwgent Solution

Separate **what** your agent does from **how** it runs:

**1. Define your agent (`student.agent`):**

```
agent StudentAssistant {
    default config {
        model: "gpt-4o"
        prompt: "You are a helpful assistant with student lookup tools."
    }

    input { request: string }
    output { result: string @desc "The response" }

    tool getStudentGrade(id: number): string {
        description: "Get student grade by ID"
    }

    tool getStudentName(id: number): string {
        description: "Get student name by ID"
    }

    tool getStudentLocation(id: number): string {
        description: "Get student location by ID"
    }

    workflow getStudentDetails(id: number): {name: string, location: string, grade: string} {
        description: "Get complete student details"
        let grade = getStudentGrade(id)
        let location = getStudentLocation(id)
        let name = getStudentName(id)
        return {name, location, grade}
    }
}
```

**2. Compile:** `npx auwgent generate student.agent ./output`

**3. Run with any provider:**

```typescript
import { Agent } from 'auwgent';
import { OpenAIDriver } from 'auwgent/drivers';
import agentIR from './output/student.agent.json';

const tools = {
    getStudentGrade: async ({ id }) => db.getGrade(id),
    getStudentName: async ({ id }) => db.getName(id),
    getStudentLocation: async ({ id }) => db.getLocation(id),
};

const agent = new Agent(new OpenAIDriver(process.env.OPENAI_API_KEY));
agent.load(agentIR);

const result = await agent.run({ request: "Get details for student 3" }, tools);
```

## Key Benefits

### Provider Agnostic
Same agent, different providers - just swap the driver:

```typescript
new Agent(new OpenAIDriver(key));           // OpenAI
new Agent(new GoogleDriver(key));           // Google Gemini
new Agent(new OpenAIDriver(key, groqUrl));  // Groq, Together, etc.
```

### Built-in Workflows
Workflows execute deterministically without LLM round-trips:

```
workflow getStudentDetails(id: number): {...} {
    let grade = getStudentGrade(id)    // Executes locally
    let name = getStudentName(id)      // No LLM calls
    return {name, grade}               // Faster, cheaper, reliable
}
```

### Dynamic Prompts
Conditionals and templates in prompts:

```
prompt AdminPrompt {
    "You are a helpful assistant."
    if (userRole == "admin") {
        "You have full access to all records."
    }
}
```

### Generated Types
The compiler outputs TypeScript types for your agent:

```typescript
// Auto-generated
export interface StudentAssistantInput { request: string }
export interface StudentAssistantOutput { result: string }
export interface StudentAssistantTools {
    getStudentGrade: (args: { id: number }) => Promise<string>;
    // ...
}
```

## Architecture

```
.agent file  ──▶  Compiler  ──▶  IR (JSON) + Types (.ts)
                                        │
Your App  ◀──▶  Auwgent Runtime  ◀──▶  LLM Driver
```

## Quick Start

```bash
npm install auwgent auwgent-cli
npx auwgent generate myagent.agent ./output
```

## DSL Reference

### Types
`string`, `number`, `boolean`, `string[]`, `{name: string}`, `"a" | "b"`, `field?: type`

### Blocks

```
agent Name {
    default config { model: "..." prompt: "..." }
    input { field: type }
    output { field: type @desc "description" }
    tool name(param: type): returnType { description: "..." }
    workflow name(param: type): returnType { description: "..." body... }
}
```

## Multi-Agent Orchestration

Auwgent supports **DSL-native multi-agent systems** with `helper` agents that can be dynamically or deterministically invoked.

### Defining Helpers

```
helper DeepThink {
    description: "A specialist for complex reasoning and analysis"
    
    default config {
        model: "gemini-2.5-flash"
        prompt: "You are a deep thinker. Analyze thoroughly."
    }
    
    input { question: string }
    output { analysis: string, conclusion: string }
    
    returns: back  // "back" = return to caller, "user" = return directly to user
}

helper CodeReviewer {
    description: "Reviews code for bugs and improvements"
    
    default config { model: "gpt-4o" prompt: "You are a senior code reviewer." }
    
    input { code: string, language: string }
    output { issues: string[], suggestions: string[] }
    
    returns: back
}
```

### Using Helpers in Agents

```
agent Manager {
    helpers { DeepThink, CodeReviewer }  // Declare available helpers
    
    default config {
        model: "kimi-k2-0905-preview"
        prompt {
            "You are a manager. Delegate complex tasks to helpers."
            "- DeepThink: For deep analysis"
            "- CodeReviewer: For code review"
        }
    }
    
    input { request: string }
    output { result: string }
    
    // Workflow that chains helpers deterministically
    workflow fullReview(code: string): string {
        description: "Analyze code then review it"
        let analysis = hlp.DeepThink({ question: "What does this code do?" })
        let review = hlp.CodeReviewer({ code: code, language: "typescript" })
        return review
    }
}
```

### How It Works

| Mode | Description |
|------|-------------|
| **Dynamic** | LLM decides when to call helpers (they appear as tools) |
| **Deterministic** | Workflows call helpers explicitly with `hlp.HelperName()` |
| **return: back** | Helper result goes back to calling agent for processing |
| **return: user** | Helper result bypasses caller, returns directly to user |

---

## Comparison with Other Frameworks

### CrewAI

```python
# CrewAI - Python only, verbose setup
researcher = Agent(role='Researcher', goal='...', backstory='...')
writer = Agent(role='Writer', goal='...', backstory='...')

task1 = Task(description='Research topic', agent=researcher)
task2 = Task(description='Write article', agent=writer)

crew = Crew(agents=[researcher, writer], tasks=[task1, task2])
result = crew.kickoff()
```

**Issues:** Sequential task execution only. Agents are defined in Python, not declarative. No type safety. Provider locked.

### LangGraph

```python
# LangGraph - Complex state machines
def research(state): ...
def write(state): ...

workflow = StateGraph(AgentState)
workflow.add_node("research", research)
workflow.add_node("write", write)
workflow.add_edge("research", "write")

app = workflow.compile()
```

**Issues:** Requires manual state management. Edges are runtime, not compile-time. No schema validation.

### AutoGen

```python
# AutoGen - Chat-based coordination
user = UserProxyAgent("user", human_input_mode="NEVER")
researcher = AssistantAgent("researcher", llm_config=...)
writer = AssistantAgent("writer", llm_config=...)

user.initiate_chat(researcher, message="Research AI", max_turns=3)
researcher.initiate_chat(writer, message="Write based on research")
```

**Issues:** Turn-based chat, not structured workflow. No deterministic paths. Heavy runtime overhead.

### Auwgent

```
helper Researcher {
    input { topic: string }
    output { research: string }
    returns: back
}

agent Coordinator {
    helpers { Researcher, Writer }
    
    workflow createArticle(topic: string): string {
        let research = hlp.Researcher({ topic: topic })
        let article = hlp.Writer({ content: research })
        return article
    }
}
```

**Advantages:**
- ✅ **Declarative DSL** - Define agents in `.agent` files, not code
- ✅ **Type-safe** - Compile-time validation of inputs/outputs
- ✅ **Provider agnostic** - Same agent, any LLM
- ✅ **Deterministic workflows** - `hlp.Helper()` executes without LLM guessing
- ✅ **Dynamic fallback** - LLM can still choose to call helpers dynamically
- ✅ **Return modes** - Control whether helper returns to caller or user

---

## Roadmap

- [x] Multi-agent orchestration (helpers)
- [x] Dynamic and deterministic helper calls
- [ ] Member access (`result.property`)
- [ ] Streaming support
- [ ] More drivers (Anthropic, Cohere)
- [ ] Agent testing framework
- [ ] Nested helper agents

## License

MIT
