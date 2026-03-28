# Auwgent Architecture: Strategy for Native & Multi-Language Support

## Overview
Current SDK implementations (TypeScript and Python) follow a **"Thin Wrapper"** pattern, where the core logic is in Rust (`ir-runtime`) and exposed via FFI (N-API for Node.js, PyO3 for Python). However, a significant amount of "high-level" logic is currently duplicated in each language target, which creates maintenance overhead and complicates the addition of new languages like Java or Go.

## Current State: Logic Duplication
The following features are implemented separately in each language SDK:

### 1. Middleware Pipelines
*   **TypeScript**: Implements the `Middleware` interface and execution pipeline in `auwgent.ts`.
*   **Python**: Implements similar logic in `auwgent_sdk.py`.
*   **Problem**: Adding a new middleware hook (e.g., `onPromptModified`) requires updating every single SDK individually.

### 2. Session & Stack Management
*   **TypeScript**: Manages `agentStack` and `helperSessions` in memory, handling the serialization/deserialization during the `run()` loop.
*   **Python**: Duplicates this stack-aware resumption logic.
*   **Problem**: Core agentic behavior (like how a conversation is resumed or branched) should be consistent across all languages but is currently "re-implemented" on the surface.

### 3. Prompt Management
*   **Current**: The `EngineBridge` provides `generate_prompt`, but the "injection" logic (adding context, system prompt overrides) often happens in the language-specific wrappers.
*   **Problem**: This leads to subtle differences in how prompts are constructed between TS and Python.

---

## Proposed Solution: The "Shared Core" Architecture

To support **Native Rust** and make adding **Java/Go/C++** easier, we must move the "High-Level" logic into the `ir-runtime` crate.

### 1. Centralized Middleware Engine (Rust)
*   Define a `Middleware` trait in the Rust core.
*   The `AuwgentEngine` will manage a collection of these middlewares.
*   **Native & Hybrid Hooks**:
    *   **Native Middleware**: Logic written in Rust that runs directly.
    *   **FFI Middleware**: Logic that "calls back" into the host language (TS/Python/Java) via the Bridge.
*   **Benefit**: The orchestration flow is defined **once** in Rust. Language SDKs only provide the specific callback implementations.

### 2. Unified Session Orchestrator
*   Move the `agentStack` and "Execution Tunneling" logic entirely into the Rust `AuwgentEngine`.
*   The `EngineBridge` will handle session persistence as a first-class citizen inside the shared runtime.

### 3. Native Language Support Path
| Language | Implementation Difficulty | Primary Task |
| :--- | :--- | :--- |
| **Rust (Native)** | **Low** | Expose `ir-runtime` as a public crate. |
| **Java** | **Medium** | Create JNI bindings for the `EngineBridge`. |
| **Go** | **Medium** | Create C-ABI bindings for the `EngineBridge`. |

---

## Implementation Roadmap
1.  **Refactor `ir-runtime`**: Integrate middleware and session management into the core engine.
2.  **Simplify `EngineBridge`**: Reduce the complexity of the bridge by handling shared logic in the core.
3.  **Update SDKs**: Refactor TypeScript and Python SDKs to be "true" thin wrappers that simply forward calls to the new bridge.
4.  **Java/Go POC**: Prototype a JNI or C-ABI binding to verify the architecture.
