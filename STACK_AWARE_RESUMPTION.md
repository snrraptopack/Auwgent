# Stack-Aware Resumption (Execution-Tunneling)

## The Concept

**Stack-Aware Resumption** is a deterministic routing pattern for multi-agent systems. Unlike traditional "Stateful Memory" (which relies on the LLM to remember where it was), Execution-Tunneling uses the agent's absolute **Call Stack** to re-enter a deeply nested execution tree precisely where it left off.

This treats the Agent Stack not just as breadcrumbs (output), but as a **Target Path** (input).

---

## The Problem: The "Drunken Parent" Drift

In most multi-turn agent frameworks (LangGraph, AutoGen, PydanticAI), when a nested sub-agent asks a user a question, the user's response must flow back through the **Parent** first.

1. **TopAgent** calls **AccountHelper**.
2. **AccountHelper** calls **ValidatorHelper**.
3. **ValidatorHelper** asks: *"What is your ZIP code?"* (Process Pauses).
4. **User** replies: *"90210"*.
5. **TopAgent** receives "90210".

**The Failure Mode:** If the `TopAgent` is not perfectly prompted, it might misinterpret "90210" as a random number or a new request, failing to route it back down to the `ValidatorHelper`. This is "Drunken Parent" drift—the higher-level agents forget the context of their delegates.

---

## The Solution: Execution-Tunneling

With Stack-Aware Resumption, the host application (via Middleware) persists the `stack` alongside the `session`. When the user provides input, the engine **tunnels** straight to the active agent.

### Scenario 1: Human-in-the-Loop Validation
**Stack:** `["Main", "FinancialAdvisor", "Broker", "RiskValidator"]`

1. **RiskValidator** pauses for human approval.
2. The user saves the stack: `["Main", "FinancialAdvisor", "Broker", "RiskValidator"]`.
3. When the human approves, the host resumes with that stack.
4. The Engine **skips LLM calls** for "Main", "FinancialAdvisor", and "Broker". It knows they already made the decision to call the next guy.
5. It "teleports" straight back into the **RiskValidator** logic.
6. When **RiskValidator** finishes, it naturally **pops** back to the **Broker**, then **FinancialAdvisor**, then **Main**.

### Scenario 2: Stateless Scaling (Serverless)
In a serverless environment (Lambda/Edge), the agent object is destroyed after every turn.

*   By saving the `stack` in a database (e.g., Redis), you can recreate the exact "Focus" of the agent on a completely different server.
*   The `SessionState` gives the model the **Memory** of what was said.
*   The `Stack` gives the engine the **Direction** of who should talk.

---

## Implementation via Middleware

This design keeps the core Agent logic "dumb" and puts the "smart" routing in the Middleware layer.

```typescript
const focusMiddleware: Middleware = {
  name: "FocusManager",

  // Save focus when a turn ends
  onRunComplete: (session, ctx) => {
    myDb.saveStack(session.id, ctx.stack);
  },

  // Inject focus when a turn starts
  onRunStart: async (session, ctx) => {
    const savedStack = await myDb.getStack(session.id);
    if (savedStack) {
      // TELEPORT: Force the engine to follow this path
      ctx.stack = savedStack; 
    }
    return session;
  }
}
```

---

## Comparison with Other Frameworks

| Feature | LangGraph / AutoGen | Auwgent (Stack-Aware) |
| :--- | :--- | :--- |
| **Resumption Type** | Data-Checkpointing | **Execution-Tunneling** |
| **Routing** | Probabilistic (LLM decides) | **Deterministic (Path enforced)** |
| **Nesting Support** | Manual Sub-graph wiring | **Automatic / Natural** |
| **Drift Risk** | High (Parent can get lost) | **Zero (Tunneling bypasses parent)** |
| **Complexity** | High (State Reducers) | **Low (Middleware + Array)** |

---

## Roadmap

This feature is planned for the **Auwgent 0.1.0-beta** release. 
The implementation will require:
1.  **NATIVE**: Updating the Rust `Engine.run()` to accept an optional `initial_stack`.
2.  **SDK**: Exposing `ctx.stack` as a mutable property in the `onRunStart` middleware hook.
3.  **LIFECYCLE**: Implementing "Fast-Forward" logic in the execution loop to skip LLM calls for agents present in the initial stack.
