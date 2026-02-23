# Multi-Turn Helper Routing: The "Stateful Hand-off" Proposal

## The Problem
Currently, in frameworks like Auwgent (as well as AutoGen, CrewAI, LangGraph), when a Router agent delegates to a Helper (e.g., `FoodWizard`) with a `handoff: user` mode, the Helper may ask a follow-up question ("Sweet or savory?"). 
The session ends, waiting for user input. When the user responds ("Savory"), the next `.run("Savory")` call is routed back to the **Router** agent by default. The Router lacks the immediate context of the Helper's active dialog and gets confused, breaking the flow.

## The Goal
To build an "Operating System" style multi-agent architecture where the `SessionState` acts as an **Agent Call Stack**. 
Helpers will have the ability to hold state across multiple turns without the parent agent needing to re-evaluate every single input, while still guaranteeing the parent can regain control when the Helper's task is complete or out-of-scope.

## Proposed Architecture: Strict Hierarchy with Control Intents

We maintain a strict hierarchical model (Helpers cannot spawn other Helpers), but we give Helpers specialized primitive intents that the Main Agent does not have.

### 1. Session State Enhancement
Update the `SessionState` to track the active agent context.
```json
{
  "active_helper": "FoodWizard", // Stores the name of the currently active helper
  "turns": [ ... ]
}
```

### 2. Automatic Engine Routing
When the Node/Svelte app calls `agent.run(userInput)`:
1. The Auwgent Engine inspects the `SessionState`.
2. If `active_helper` is set (e.g., `"FoodWizard"`), the Engine bypasses the `Router` prompt completely.
3. The Engine boots up the `FoodWizard`'s IR nested engine directly and feeds it the user's input.

### 3. Helper-Specific Primitive Intents
The compiler will automatically inject new built-in options into every Helper's `Instructions`/`Options` block, giving them explicit ways to yield control:

* **`response_text`** (Existing behavior):
  The Helper talks to the user. It remains the `active_helper`. The session is saved, and the next user input goes directly to this Helper.

* **`finish_step`** (New primitive):
  The Helper determines its specific task is complete.
  ```yaml
  finish_step:
    result: "I recommend the Lemon Chicken Piccata!" 
  ```
  _Engine Action:_ The Engine clears the `active_helper` state. It immediately wakes up the parent `Router`, feeding it the `result` as a `helper_result`. The Router takes over the rest of the turn.

* **`return_to_parent` / `ask_user`** (New primitive):
  The Helper determines the user's input is out-of-scope or requires re-routing (e.g., the user says "Actually, tell me a story instead" to the FoodWizard).
  ```yaml
  return_to_parent:
    reason: "The user requested a story, which is outside my food recommendation expertise."
  ```
  _Engine Action:_ The Engine clears the `active_helper` state. It passes the `reason` and the original user input back to the parent `Router` so it can handle the pivot and route to the correct agent (e.g., the `Story` helper).

## Developer Experience Benefits
* **Zero Manual Routing:** Developers simply call `router.run(userInput)` on their backend. They do not need to write `switch(session.active_agent)` logic. Auwgent handles the call stack natively.
* **Token Efficiency:** The parent `Router` is not invoked on every single conversation turn, saving significant tokens and latency when the user is deep in a Helper's specific workflow.
* **Absolute State Control:** Because Helpers must explicitly use `finish_step` or `return_to_parent` to yield, there is zero ambiguity about who is in charge of the conversation at any given time.
