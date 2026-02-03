# Auwgent Feature Proposals: Prompt Composition & Durable Execution

After analyzing the current DSL, BAML's approach, and Inngest's durable execution model, here are two exciting paths forward:

---

## ✅ IMPLEMENTED: Enhanced Template Syntax

### What's Now Supported in `"""..."""` Multiline Strings

#### 1. Variable Interpolation (already worked)
```auwgent
"""
Hello {{user.name}}!
Your location: {{user.location}}
"""
```

#### 2. NEW: Inline Conditionals with Explicit Comparisons
```auwgent
"""
Welcome {{user.name}}!

{{#if user.role == "admin"}}
You have full administrative access.
{{/if}}

{{#if user.subscription == "premium"}}
Premium features unlocked!
{{else}}
Consider upgrading for more features.
{{/if}}

{{#if user.age >= 18}}
Adult content available.
{{/if}}
"""
```

**Supported operators:** `==`, `!=`, `>`, `<`, `>=`, `<=`

**Important:** We require **explicit comparisons** (not truthy/falsy) for cross-language compatibility:
```auwgent
// ❌ BAD - Won't work (truthy is JS-specific)
{{#if user.premium}}

// ✅ GOOD - Explicit comparison
{{#if user.premium == true}}
```

#### 3. NEW: Schema Injection with `{{@schema()}}`
```auwgent
"""
Generate a response matching this schema:

{{@schema(output)}}

For the user type:
{{@schema(types.User)}}
"""
```

**Supported paths:**

| Path | Description |
|------|-------------|
| `{{@schema(output)}}` | Current agent/helper's output schema |
| `{{@schema(output.property)}}` | Specific output property |
| `{{@schema(input)}}` | Current agent/helper's input schema |
| `{{@schema(context)}}` | Current context schema |
| `{{@schema(types.TypeName)}}` | Named type definition |

**Scoping:** The schema is resolved relative to where the prompt is defined:
- In an agent prompt → uses agent's output/input/types
- In a helper prompt → uses helper's output/input/types

---

## Example: Full Prompt with New Features

```auwgent
prompt AssistantPrompt(user, mode) {
    """
    You are helping {{user.name}} (ID: {{user.id}}).
    
    {{#if mode == "formal"}}
    Use formal language. Address them as Mr./Ms. {{user.name}}.
    {{else}}
    Be casual and friendly!
    {{/if}}
    
    {{#if user.role == "admin"}}
    ADMIN MODE: You can access all system functions.
    {{/if}}
    
    User details:
    - Location: {{user.location}}
    - Age: {{user.age}}
    
    Your response must match this schema:
    {{@schema(output)}}
    """
}

agent CustomerService {
    output {
        response: string @desc "The response to the user"
        sentiment: "positive" | "negative" | "neutral"
        escalate?: boolean @desc "Whether to escalate to human"
    }
    
    default config {
        model: gemini("gemini-2.5-flash")
        prompt: AssistantPrompt(ctx.user, ctx.mode)
    }
}
```

---

## What About Few-Shot Examples?

Current syntax (unchanged):
```auwgent
prompt ClassifierPrompt() {
    "Classify sentiment."
    
    example {
        user: "I love this!"
        assistant: "positive"
    }
    
    example {
        user: "This is terrible."
        assistant: "negative"
    }
}
```

We kept `example { }` instead of `@example { }` because:
- `@` prefix is for annotations (`@desc`)
- `example` is a block, like `tools { }` or `helpers { }`
- Consistent with existing grammar patterns

---

## Option B: Durable Execution & Event-Driven Agents 🏗️

### Current State

The `WorkflowRunner` runs workflows synchronously:
```typescript
async runWorkflow(name: string, args: any[]): Promise<any> {
    // Runs to completion, no checkpointing
    // Failures lose all progress
    // No event subscriptions
}
```

### Proposed: Inngest-Style Durable Steps

```auwgent
agent OrderProcessor {
    tools {
        charge_payment(amount: number): PaymentResult
        send_confirmation(order_id: string): boolean
        update_inventory(items: Item[]): boolean
    }
    
    // NEW: Durable workflow with steps
    durable workflow process_order(order: Order) {
        description: "Process an order with retries and checkpoints"
        
        // Each step is a checkpoint - retries independently
        step "validate" {
            let validation = validate_order(order)
            if (!validation.valid) {
                fail "Invalid order: " + validation.error
            }
        }
        
        // Parallel steps with automatic fan-out
        parallel step "prepare" {
            let inventory = update_inventory(order.items)
            let payment = charge_payment(order.total)
        }
        
        // Wait for external event (webhook, user action, etc.)
        wait for event "payment.confirmed" {
            timeout: "5m"
            match: event.order_id == order.id
        }
        
        // Sleep/delay built-in
        sleep "2s"
        
        step "notify" {
            send_confirmation(order.id)
        }
        
        return { status: "completed", order_id: order.id }
    }
}
```

### Key Primitives

| Primitive | Description |
|-----------|-------------|
| `step "name" { }` | Checkpoint - state persisted, retries independently |
| `parallel step { }` | Fan-out execution, all run concurrently |
| `wait for event "name"` | Pause until external event received |
| `sleep "duration"` | Durable sleep (doesn't consume compute) |
| `fail "message"` | Explicit failure with message |
| `retry { attempts: 3, backoff: "exponential" }` | Per-step retry config |

### Architecture Changes

```
┌─────────────────────────────────────────────────────────┐
│                    Auwgent Runtime                       │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────┐    ┌──────────────┐    ┌───────────┐  │
│  │  Event Bus   │───▶│ Step Executor│───▶│  State    │  │
│  │  (EventEmitter/   │  (Durable)   │    │  Store    │  │
│  │   Redis/etc)      └──────────────┘    │ (Memory/  │  │
│  └──────────────┘           │            │  Redis/   │  │
│        ▲                    │            │  Postgres)│  │
│        │                    ▼            └───────────┘  │
│  ┌─────┴────────────────────────┐                       │
│  │     Workflow Checkpoint      │                       │
│  │  {                           │                       │
│  │    runId: "abc-123",         │                       │
│  │    currentStep: "notify",    │                       │
│  │    completedSteps: [...],    │                       │
│  │    stepResults: {...},       │                       │
│  │    waitingFor: null          │                       │
│  │  }                           │                       │
│  └──────────────────────────────┘                       │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

### New Types

```typescript
// In types/ir.ts
interface DurableWorkflow {
    flowName: string;
    params: Record<string, ParamInfo>;
    returns: TypeInfo;
    steps: WorkflowStep[];
    isDurable: true;
}

interface WorkflowStep {
    id: string;
    type: 'step' | 'parallel' | 'wait' | 'sleep';
    body?: Statement[];
    timeout?: string;
    retryPolicy?: RetryPolicy;
    eventMatch?: EventMatch;
}

// In types/protocol.ts
interface WorkflowCheckpoint {
    runId: string;
    workflowName: string;
    currentStep: string;
    completedSteps: string[];
    stepResults: Record<string, any>;
    waitingFor?: {
        type: 'event' | 'sleep';
        event?: string;
        until?: Date;
    };
    status: 'running' | 'waiting' | 'completed' | 'failed';
}

interface DurableWorkflowRunner {
    start(name: string, args: any[]): Promise<string>; // Returns runId
    resume(runId: string, event?: any): Promise<any>;
    getStatus(runId: string): Promise<WorkflowCheckpoint>;
    cancel(runId: string): Promise<void>;
}
```

### Grammar Additions

```langium
WorkFlowConfig:
    durable?="durable"? "workflow" name=ID 
    "(" (params+=TypeConfigDeclaration)* ")" 
    ":" return=Types "{"
        "description" ":" desc=STRING
        (body+=DurableStatement | body+=Statement)*
    "}";

DurableStatement:
    StepStatement | ParallelStepStatement | WaitStatement | SleepStatement | FailStatement;

StepStatement:
    "step" name=STRING ("retry" retryConfig=RetryConfig)? "{" (body+=Statement)* "}";

ParallelStepStatement:
    "parallel" "step" name=STRING "{" (body+=Statement)* "}";

WaitStatement:
    "wait" "for" "event" eventName=STRING "{"
        ("timeout" ":" timeout=STRING)?
        ("match" ":" matchExpr=Expression)?
    "}";

SleepStatement:
    "sleep" duration=STRING;

FailStatement:
    "fail" message=Expression;

RetryConfig:
    "{" ("attempts" ":" attempts=INT)? ("backoff" ":" backoff=STRING)? "}";
```

---

## Comparison Matrix

| Aspect | Prompt Freedom (A) | Durable Execution (B) |
|--------|-------------------|----------------------|
| **Complexity** | Medium - Template parsing | High - State machine, persistence |
| **User Impact** | Better DX for prompt engineers | Better reliability for production |
| **Token Efficiency** | Helps (cleaner prompts) | Neutral |
| **Unique Selling Point** | BAML-like expressiveness | Temporal/Inngest in DSL |
| **Implementation Time** | ~1-2 weeks | ~3-4 weeks |
| **Dependencies** | None (pure parsing) | Optional: Redis, Postgres |

---

## My Recommendation

**Start with Option A (Prompt Freedom)** because:

1. **Immediate value** - Every agent benefits from cleaner prompts
2. **Lower risk** - No runtime architecture changes
3. **Differentiator** - Few agent frameworks have first-class prompt DSL
4. **Foundation for B** - Better prompts help durable workflows later

Then **add Option B incrementally**:
1. First: `step` blocks with in-memory checkpointing
2. Then: `wait for event` with EventEmitter
3. Finally: Pluggable persistence (Redis, Postgres, etc.)

---

## Quick Win: Enhanced Multiline Strings

Before full Jinja, we can enhance the existing `"""..."""` syntax:

```auwgent
prompt AssistantPrompt(user) {
    """
    You are helping {{user.name}}.
    
    Their details:
    - Age: {{user.age}}
    - Location: {{user.location}}
    
    {{#if user.premium}}
    They are a premium user, prioritize their requests.
    {{/if}}
    """
}
```

This only requires changes to `processMultilineString()` in the generator!

---

## Next Steps

Which direction excites you more? I can:

1. **Option A**: Implement the `{{ }}` interpolation enhancements first
2. **Option B**: Design the `DurableWorkflowRunner` architecture
3. **Both**: Start with the "Quick Win" multiline enhancement while designing durable execution

Let me know your preference! 🚀
