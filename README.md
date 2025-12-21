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

## Roadmap

- [ ] Context block for runtime metadata
- [ ] Multi-agent orchestration
- [ ] Streaming support
- [ ] More drivers (Anthropic, Cohere)
- [ ] Agent testing framework

## License

MIT
