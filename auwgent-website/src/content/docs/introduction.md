---
title: Introduction
description: Auwgent is a compiler-first framework for building AI agents that introduces a clean architectural boundary between AI logic and application logic.
---

## What is Auwgent?

Auwgent is a compiler-first framework for building AI agents. It introduces a clean architectural boundary between what an AI agent can do and how your application does it — two concerns that, in most agent frameworks today, are collapsed into one.

---

## The problem with how agents are built today

Most agent frameworks treat agentic logic as something that lives inside your application. Your prompts, your tool definitions, your workflows — they sit alongside your business logic, written in the same language, running in the same process, coupled to the same runtime. The result is that your choice of runtime stops being an engineering decision. It becomes a constraint the framework imposes on you.

This matters more than it seems. If your application is best served by a performant systems language, a JVM-based stack, or a lightweight edge runtime, that decision gets overridden the moment you adopt a framework that only runs in one environment. The AI concern has leaked into an infrastructure decision where it was never supposed to be.

The coupling wasn't a deliberate design choice. It was an architectural accident — and it has quietly shaped the entire AI agent development ecosystem around it.

---

## Auwgent's answer: a defined boundary

Auwgent's core idea is that the AI's world and the application's world should have a contract between them, not a merger.

In Auwgent, you define your agent using a DSL. You declare a prompt, the tools the model has access to, and the workflows that govern how those tools are used. This definition is compiled and presented to the model — it tells the model exactly what it can do. That boundary is fixed. The model sits on the other side of a network boundary, and the compiled definition is what it sees.

On the application side, a tool declaration is not a function definition. When you declare that your agent has a `search` tool, you are telling the model that `search` exists and what its contract looks like. How `search` is implemented — what language it runs in, what it calls, what infrastructure it touches — is pure software development. Auwgent provides the binding point. You own everything behind it.

This means your prompts and helper tools never interfere with your business logic. Your business logic never bleeds into your model context. And your choice of runtime stays yours.
