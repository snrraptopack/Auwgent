---
title: The Agent
description: Learn how to declare and configure an agent in Auwgent.
---

The `agent` block is the top-level declaration in Auwgent. Everything your AI agent is permitted to do — the model it runs on, the prompts it uses, the tools it can call, the workflows it follows — is defined inside it.

---

## Declaring an agent

```ts
agent Main {
    // configuration goes here
}
```

The name after the `agent` keyword is the agent's identifier. It is used by the compiler to generate the corresponding TypeScript types and exported functions in the output file.

---

## Configuration

Every agent must have at least one configuration block — the `default config`. This is the baseline the agent runs on unless told otherwise.

```ts
agent Main {
    default config {
        model: gemini("gemini-2.5-flash")
        prompt: "You are a helpful assistant. Be polite."
    }
}
```

Inside a config block, two fields are mandatory: `model` and `prompt`.

---

## The model field

The `model` field tells the agent which language model to use and how to reach it. Auwgent currently supports three providers.

### Inline definition

The simplest way to define a model is directly inside the config block:

```ts
default config {
    model: gemini("gemini-2.5-flash")
    prompt: "You are a helpful assistant."
}
```

### Standalone model block

For cleaner organisation — or when you want to reuse the same model definition across multiple configs or agents — you can define the model as a standalone block outside the agent and reference it by name:

```ts
model MyGemini {
    provider: gemini("gemini-2.5-flash")
}

agent Main {
    default config {
        model: MyGemini
        prompt: "You are a helpful assistant."
    }
}
```

You can define as many standalone model blocks as you need in the same file.

---

## Providers

### `gemini`

Uses Google's Gemini API.

```ts
model MyGemini {
    provider: gemini("gemini-2.5-flash")
}
```

The first argument is the full model name as recognised by the Gemini API.

### `openai`

Uses the OpenAI API.

```ts
model MyGPT {
    provider: openai("gpt-4o")
}
```

The first argument is the full model name as recognised by the OpenAI API.

### `custom`

For any OpenAI-compatible API endpoint — self-hosted models, third-party providers, or internal inference servers.

```ts
model MyCustomModel {
    provider: custom("my-provider", "https://api.example.com/v1", "model-name")
}
```

The three positional arguments are:

| Argument | Description |
|---|---|
| `id` | A unique identifier for this provider. Used to generate the corresponding API key field in the config |
| `base_url` | The base URL of the OpenAI-compatible endpoint |
| `model` | The model name to pass in the request |

---

## Provider configuration options

All three providers accept an optional second argument — an object for fine-grained model parameters. The fields must match the provider's HTTP API equivalents exactly.

```ts
model MyGemini {
    provider: gemini("gemini-2.5-flash", {
        temperature: 0.7
        top_k: 40
        top_p: 0.95
        max_output_tokens: 1024
    })
}
```

Any parameter the provider's API accepts can be set here.

---

## Named configs

Beyond `default config`, an agent can have as many named configs as you need. A named config follows the same rules as `default config` — it requires a `model` and a `prompt`.

```ts
model MyGemini {
    provider: gemini("gemini-2.5-flash")
}

agent Main {
    default config {
        model: MyGemini
        prompt: "You are a helpful assistant. Be polite."
    }

    config Strict {
        model: MyGemini
        prompt: "You are a strict validator. Return only structured data."
    }

    config Creative {
        model: MyGemini
        prompt: "You are a creative writer. Be expressive and imaginative."
    }
}
```

Named configs let a single agent behave differently depending on context — different instructions, different models, different parameters — without duplicating the agent itself. How you switch between configs at runtime is covered in the SDK reference.

---

## The prompt field

The `prompt` field defines the system prompt passed to the model. Because prompts are a significant part of how you shape agent behaviour in Auwgent — with support for dynamic context, intent shaping, and more — they have a dedicated page.

→ See [Prompts and Context](/core-concepts/prompt-context) for the full reference.

---

## Next steps

With your agent and its configuration defined, the next thing to explore is giving your agent capabilities it can act on.

→ See [Tools](/core-concepts/tools) to learn how to declare tools the model can call.
