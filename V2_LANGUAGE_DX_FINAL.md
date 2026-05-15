# Auwgent v2 Language DX

Status: design reference  
Source notes: `not.txt`, `not_standard.txt`, `not_graph.txt`

Auwgent v2 moves the DSL from a mostly declarative agent description into a small language for writing resumable agent programs.

The central DX goal is simple:

```text
Write agent logic like normal program logic.
Compile it into a resumable execution graph.
Keep host-language glue optional, not required for every behavior.
```

## 1. Core Shape

An agent is a typed callable unit.

```ts
agent Hello(input: Text): Text {
    reply(input) with {
        prompt = "You are helpful."
        model = gemini("gemini-pro")
    }
}
```

The `reply(...) with { ... }` expression is the explicit model boundary. The value passed to `reply` becomes the model-facing user turn. The `with` block configures how that model turn runs.

Conceptually:

```json
{
  "systemPrompt": "You are helpful.",
  "turns": [
    {
      "user": "hello",
      "model": "..."
    }
  ]
}
```

If the agent transforms input before calling `reply`, the transformed value is what the model receives.

## 2. Functions

Functions are ordinary deterministic or runtime-executed logic.

```ts
function sanitizeInput(input: Text): Text {
    let cleaned = input.remove("hash word")
    return "cleaned now it is: {cleaned}"
}

agent Hello(input: Text): Text {
    let userInput = if input.contains("hash word")
        then sanitizeInput(input)
        else input

    reply(userInput) with {
        prompt = "Answer the user."
        model = gemini("gemini-pro")
    }
}
```

The first compiler pass should make these building blocks real syntax, not just concepts.

### 2.1 Let Bindings

```ts
agent Hello(input: Text): Text {
    let cleaned = sanitizeInput(input)
    let label = "user message"

    reply({ label, cleaned }) with {
        prompt = "Answer using the cleaned input."
        model = Gemini
    }
}
```

### 2.2 Reassignment

Reassignment should be allowed for local variables that were already declared.

```ts
agent Hello(input: Text): Text {
    let userInput = input

    if input.contains("hash word") {
        userInput = sanitizeInput(input)
    }

    reply(userInput) with {
        prompt = "Answer the user."
        model = Gemini
    }
}
```

Open decision: whether reassignment is allowed everywhere or only for local variables in the current function/agent body.

### 2.3 If Expressions

Use `if ... then ... else ...` when choosing a value.

```ts
agent Hello(input: Text): Text {
    let selectedModel = if isVip then Gemini else Groq
    let userInput = if input.contains("hash word")
        then sanitizeInput(input)
        else input

    reply(userInput) with {
        prompt = "Answer the user."
        model = selectedModel
    }
}
```

### 2.4 If Blocks

Use block form when the branch performs multiple statements or exits early.

```ts
agent Hello(input: Text): Text {
    if input.contains("weather") {
        let weather = getWeather(input)

        reply({ input, weather }) with {
            prompt = "Use the fetched weather data."
            model = Gemini
        }
    }

    reply(input) with {
        prompt = "Answer normally."
        model = Gemini
    }
}
```

### 2.5 Function Calls

```ts
function systemPrompt(userName: string): Text {
    return """
    You are helping {userName}.
    Keep answers concise.
    """
}

@context(Context)
agent Hello(input: Text): Text {
    reply(input) with {
        prompt = systemPrompt(user_name)
        model = Gemini
    }
}
```

### 2.6 Agent Calls

```ts
agent Analyze(input: Text): Text {
    reply(input) with {
        prompt = "Return high or low based on request complexity."
        model = Gemini
    }
}

agent Main(input: Text): Text {
    let analysis = Analyze(input)

    if analysis.data.includes("high") {
        return Expert(input)
    }

    return General(input)
}
```

### 2.7 Object Literals

Object literals are useful when preparing richer model input.

```ts
agent Hello(input: Text): Text {
    let account = getAccount(user_id)

    reply({
        message: input
        account: account.data
        userName: user_name
    }) with {
        prompt = "Use the provided account context."
        model = Gemini
    }
}
```

### 2.8 Arrays

Arrays are used for tool lists, model-callable agents, and normal values.

```ts
@context(Context)
agent Hello(input: Text): Text {
    let baseTools = [getWeather, userTools]
    let adminTools = [getWeather, userTools, delete_person]
    let availableTools = if isAdmin then adminTools else baseTools

    reply(input) with {
        prompt = "Help the user."
        model = Gemini
        tools = availableTools
    }
}
```

### 2.9 Field Access

```ts
type User = {
    name: string
    location: string
}

agent Hello(input: User): Text {
    let message = "Hello {input.name} in {input.location}"

    reply(message) with {
        prompt = "Greet the user."
        model = Gemini
    }
}
```

### 2.10 Collection Loops

Loops should support collection processing without forcing host-language code.

```ts
type Score = {
    name: string
    value: number
}

function summarizeScores(scores: Score[]): Text {
    let lines = []

    for idx, score in scores {
        lines.add("{idx + 1}. {score.name}: {score.value}")
    }

    return lines.join("\n")
}
```

The loop syntax above is the proposed DX. The compiler can lower it into iterator/evaluator operations first, then optimize later.

## 3. Tools

There are two tool categories.

### 3.1 Host Tools

Host tools are implemented by the SDK host language and registered with the runtime.

```ts
tool getWeather(city: string): string
    @desc "Use this to get the weather for a city"
```

An agent exposes the tool to the model:

```ts
agent Weather(input: Text): Text {
    reply(input) with {
        prompt = "Help the user with weather questions."
        model = gemini("gemini-pro")
        tools = [getWeather]
    }
}
```

The host still provides the implementation.

### 3.2 DSL Tool Functions

A function can be exposed as a model-callable tool.

```ts
tool delete_user(id: string): bool
    @desc "Delete a user by id"

@tool
@desc "Use this to delete a user"
function delete_person(id: string, isAdmin: bool): string {
    if not isAdmin {
        return "user is not an admin"
    }

    let result = delete_user(id)

    if result.error.isEmpty() {
        return "user deleted successfully"
    }

    return result.error
}
```

This lets the developer put deterministic policy in the DSL before a dangerous host tool is reached.

## 4. Tool Gating

Tool gating is one of the most important DX improvements.

Instead of exposing a raw host tool directly:

```ts
tool delete_user(id: string): bool
```

wrap it:

```ts
@tool
@desc "Use this to delete a user after authorization is checked"
function delete_person(id: string): string {
    if not isAdmin {
        return "user is not an admin"
    }

    let deleted = delete_user(id)

    if deleted.error.isEmpty() {
        return "user deleted successfully"
    }

    return deleted.error
}
```

Then expose only the wrapper:

```ts
@context(Context)
agent Admin(input: Text): Text {
    reply(input) with {
        prompt = "Help with admin tasks."
        model = Gemini
        tools = [delete_person]
    }
}
```

This makes the model see a safe callable surface while the runtime still has access to the raw host capability.

## 5. Tool Groups And Progressive Disclosure

Tool groups allow many tools to be summarized as one exposed capability.

```ts
tools userTools {
    getUsername(): string
        @desc "Use this to get the user's name"

    getUserLocation(): string
        @desc "Use this to get the user's location"
} @desc "User profile and location tools"
```

Usage:

```ts
agent Hello(input: Text): Text {
    reply(input) with {
        prompt = "Help the user."
        model = Gemini
        tools = [userTools]
    }
}
```

Intended behavior:

- The model first sees the group name and group description.
- Inner tools are not all loaded into the prompt/tool context immediately.
- If the model needs details, the runtime exposes an internal discovery action.
- This reduces prompt/tool bloat for large tool sets.

## 6. Built-In Provider Tools

Some models expose provider-native built-in tools. The DSL should let the agent opt into them.

```ts
agent Hello(input: Text): Text {
    reply(input) with {
        prompt = "Answer with current information when needed."
        model = Gemini
        tools = [getWeather]
        builtin = [web_search]
    }
}
```

Open question: exact names and validation for provider built-ins should be provider-aware.

## 7. Context

Context binds runtime data into the agent.

```ts
type Context = {
    isAdmin: bool
    user_id: string
    user_name: string
}

@context(Context)
agent Hello(input: Text): Text {
    reply(input) with {
        prompt = systemPrompt(user_name)
        model = Gemini
    }
}
```

Current proposal allows context fields to be directly available in scope. This is convenient, but it creates collision risk.

Example collision:

```ts
@context(Context)
agent Hello(input: Text): Text {
    let isAdmin = false // conflicts with context field
    reply(input) with { model = Gemini }
}
```

Recommended compiler rule if direct context fields remain:

```text
No local binding may collide with a context field at any scope.
```

Alternative under consideration:

```ts
ctx.isAdmin
ctx.user_id
ctx.user_name
```

## 8. Dynamic Tools

Tools can be selected dynamically from context.

```ts
@context(Context)
agent Hello(input: Text): Text {
    let availableTools = if isAdmin
        then [getWeather, delete_person]
        else [getWeather]

    reply(input) with {
        prompt = "Help the user."
        model = Gemini
        tools = availableTools
    }
}
```

This lets authorization and routing logic remain in the DSL.

## 9. Preprocessing Before Reply

Agents can do work before the model turn.

```ts
@context(Context)
agent Hello(input: Text): Text {
    if input.contains("weather") {
        let fetchedData = getWeather(input)

        reply({ input, fetchedData }) with {
            prompt = "Use the fetched data when answering."
            model = Gemini
        }
    }

    reply(input) with {
        prompt = "Answer normally."
        model = Gemini
    }
}
```

In graph IR, `getWeather` becomes its own resumable node before the `reply` node.

## 10. Agent Composition

Agents can call other agents.

```ts
agent Analyze(input: Text): Text {
    reply(input) with {
        prompt = "Classify the request as high or low complexity."
        model = Gemini
    }
}

agent One(input: Text): Text {
    reply(input) with {
        prompt = "Handle high complexity requests."
        model = Gemini
    }
}

agent Two(input: Text): Text {
    reply(input) with {
        prompt = "Handle normal requests."
        model = Gemini
    }
}

agent Main(input: Text): Text {
    let inputType = Analyze(input)

    if inputType.error.isEmpty() and inputType.data.includes("high") {
        return One(input)
    }

    return Two(input)
}
```

Default behavior:

- `Main` records the original input and final output.
- The child agent's internal turns are hidden from the parent session view.
- The child still has its own execution state for resumability.

Transparent behavior:

```ts
agent Main(input: Text): Text {
    let inputType = Analyze(input)

    if inputType.data.includes("high") {
        return One(input) with turns
    }

    return Two(input) with turns
}
```

`with turns` means the parent can expose the child execution trace.

## 11. Agent Return Types

Text output:

```ts
agent Hello(input: Text): Text {
    reply(input) with {
        prompt = "Answer the user."
        model = Gemini
    }
}
```

Structured output:

```ts
type UserProfile = {
    userName: string
    age: number
    location: string
}

agent Extract(input: Text): UserProfile {
    reply(input) with {
        prompt = "Extract the user's profile."
        model = Gemini
    }
}
```

Union output:

```ts
type Profile = {
    userName: string
    age: number
}

type Risk = {
    dizzy: bool
    confidence: number
}

agent Check(input: Text): Profile | Risk | Text {
    reply(input) with {
        prompt = "Return the best matching output."
        model = Gemini
    }
}
```

Consumers can narrow with `is`:

```ts
agent FollowUp(input: Text): Text {
    let response = Check(input)

    if response.data is Risk {
        reply(response.data) with {
            prompt = "Explain the risk."
            model = Gemini
        }
    }

    if response.data is Profile {
        reply(response.data) with {
            prompt = "Summarize the profile."
            model = Gemini
        }
    }

    reply(response.data) with {
        prompt = "Answer normally."
        model = Gemini
    }
}
```

## 12. Model Definitions

Named models:

```ts
model Gemini = {
    provider = "gemini"
    modelName = "gemini-pro"
    config = {
        temperature = 0.4
    }
}

model Groq = {
    provider = "groq"
    modelName = "llama-3.3-70b-versatile"
}
```

Inline model:

```ts
agent Hello(input: Text): Text {
    reply(input) with {
        prompt = "Answer the user."
        model = gemini("gemini-pro", {
            temperature = 0.4
        })
    }
}
```

Dynamic routing:

```ts
@context(Context)
agent Hello(input: Text): Text {
    let selectedModel = if isVip then Gemini else Groq

    reply(input) with {
        prompt = systemPrompt(user_name)
        model = selectedModel
    }
}
```

## 13. Reply Config

The `with` block is special. It configures model execution.

```ts
reply(input) with {
    prompt = systemPrompt(user_name)
    model = Gemini
    fallback = Groq
    retry = 3
    maxTurn = 3
    tools = [getWeather]
    agents = { Planner, Researcher }
    builtin = [web_search]
}
```

Initial config keys:

- `prompt`
- `model`
- `tools`
- `agents`
- `builtin`
- `fallback`
- `retry`
- `maxTurn`

## 14. Inputs And Media

Basic text input:

```ts
agent Hello(input: Text): Text {
    reply(input) with { model = Gemini }
}
```

Media input:

```ts
agent Describe(input: Text | Image): Text {
    reply(input) with {
        prompt = "Describe the image or answer the text."
        model = Gemini
    }
}
```

Custom input shape:

```ts
type ImageRequest = {
    data: string
    location: string
    image: Image
}

agent Describe(input: ImageRequest): Text | Image {
    reply(input) with {
        prompt = "Use all provided data."
        model = Gemini
    }
}
```

Open question: whether `Image` output implicitly includes `Text`. The safer compiler rule is:

```text
omitted output -> Text
: Image -> Image only
: Text | Image -> either Text or Image
```

## 15. Decided Direction

- Agents become typed executable units.
- `reply(...) with { ... }` is the explicit model turn boundary.
- Tools can be host tools or DSL tool functions.
- Agent calls are first-class.
- Context is available inside agents.
- Dynamic model/tool selection is part of the language.
- Functions and workflows collapse into normal callable logic.
- Old helper behavior becomes agent composition plus handoff/trace policy.
- The compiler targets graph IR for execution.

## 16. Still Deciding

- Whether context should be direct-scope or only `ctx.field`.
- Exact syntax for `agents = { One, Two }` versus `agents = [One, Two]`.
- Exact tool-function partial binding syntax, such as `delete_person(user_id, isAdmin)`.
- Whether output media types implicitly include `Text`.
- Exact built-in provider tool names and validation.
- Whether `reply(...)` should be allowed without an explicit `with` block.
- Whether reassignment should be broad or limited.
- How much of this syntax lands in the first compiler milestone.
