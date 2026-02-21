# Auwgent

A Domain-Specific Language (DSL) for building AI agents with declarative syntax, type safety, and a lightning-fast Rust-powered runtime.

## The Problem with Traditional AI Development

Frameworks like Vercel AI SDK or LangChain often couple tool definitions with implementation, lock you into specific providers, and rely on LLM reasoning for multi-step workflows, which is slow and unpredictable.

## The Auwgent Solution

Auwgent separates **what** your agent does (the logic) from **how** it runs (the implementation).

**1. Define your agent (`main.agent`):**

```auwgent
agent StudentAssistant {
    default config {
        model: gemini("gemini-2.0-flash")
        prompt: "You are a helpful assistant with student lookup tools."
    }

    input { id: string }
    output { name: string, grades: string[] }

    tool get_student_details(id: string): { name: string, grades: string[] } {
        description: "Get complete student details by ID"
    }

    workflow get_summary(id: string): string {
        description: "Get a summary of a student"
        let student = get_student_details(id)
        return "Student " + student.name + " has grades: " + student.grades
    }
}
```

**2. Compile:** `npx auwgent-cli generate main.agent ./output`

**3. Run with full type safety:**

```typescript
import { createAuwgent, parseIR } from '@auwgent/runtime';
import irJson from './output/main.agent.json';

const ir = parseIR(JSON.stringify(irJson));

const agent = createAuwgent(ir, {
  apiKeys: { geminiApiKey: process.env.GEMINI_API_KEY },
  tools: {
    get_student_details: async ({ id }) => {
      return { name: "John Doe", grades: ["A", "B", "C"] };
    }
  }
});

// Observe intents in real-time
agent.onIntent((name, value) => {
  console.log(`[${name}]`, value);
});

const session = await agent.run('Tell me about student 123');
console.log(session.turns[0].model_response);
```

## Key Benefits

### 🚀 Rust-Powered Runtime
Auwgent's core is written in Rust, providing a high-performance orchestration engine that handles parallel execution and complex workflow logic with minimal overhead.

### 📜 Streaming YAML Architecture
Unlike traditional JSON function calling, Auwgent uses a custom YAML-based streaming protocol. This is more reliable for LLMs, enables partial parsing of results as they arrive, and allows for much richer structured outputs.

### 🛠️ Deterministic Workflows
Define complex multi-step processes in the DSL. Workflows execute locally in the engine, eliminating unnecessary LLM round-trips while maintaining full access to tools and helpers.

### 🔗 Multi-Agent Orchestration
Native support for `helper` agents. Move between specialized agents dynamically or deterministically with full context preservation.

## Why Auwgent?

| Feature | Vercel AI SDK | LangChain / CrewAI | Auwgent |
|---------|---------------|--------------------|---------|
| **Architecture** | Code-first (TS/JS) | Code-first (Py/TS) | **DSL-first (.agent)** |
| **Logic/Schema** | Coupled with impl | Coupled / Loose | **Separated by DSL** |
| **Workflows** | Non-deterministic | LLM-chained | **Deterministic (Engine-level)** |
| **Type Safety** | Runtime (Zod) | Varies | **Compile-time (Generated)** |
| **Protocol** | JSON/Tool Calls | Various | **Streaming YAML** |
| **Performance** | JS standard | High overhead | **Rust-powered core** |

### Auwgent vs. Vercel AI SDK
While the Vercel AI SDK is excellent for standard applications, it requires you to define tool schemas and implementations together in TypeScript. This leads to leaked implementation details and high coupling. Auwgent defines the "Interface" in a specialized language, allowing you to swap implementations or even languages (Rust/TS/Python) without changing the agent's logic.

### Auwgent vs. LangChain / CrewAI
LangChain and CrewAI rely heavily on the LLM to "figure out" the next step, often leading to unpredictability and high latency in complex tasks. Auwgent's **Engine-level Workflows** execute like standard code within the AI loop, ensuring that if you *know* the steps, the agent *follows* the steps—deterministically and fast.

## Architecture

```
.agent file  ──▶  Compiler (Langium)  ──▶  IR (JSON) + Types (.ts)
                                                │
Your Node App  ◀──  NAPI-RS FFI Bridge  ──▶  @auwgent/runtime (Rust Core)
```

## DSL Reference

### Types
`string`, `number`, `boolean`, `string[]`, `{ name: string }`, `"A" | "B"`, `field?: type`

### Blocks
- `agent`: Main agent definition.
- `helper`: Specialized sub-agent.
- `tool`: External function implementation.
- `workflow`: Deterministic logic block.
- `prompt`: Dynamic template with conditionals (`if`, `ctx`).

## Roadmap

- [x] Multi-agent orchestration (helpers)
- [x] Streaming support (`onIntentPartial`)
- [x] High-performance Rust engine
- [x] YAML-based streaming protocol
- [x] Intelligent Type Inference
- [ ] VS Code Extension (Beta)
- [ ] Python Runtime

## License

MIT
