# Error Handling Analysis

**Date**: 2026-01-27  
**Scope**: Complete error boundary analysis across all runtime components

---

## Executive Summary

The Auwgent runtime has **minimal explicit error handling** across most components. Errors propagate naturally through the call stack, relying on:
1. SDK-level error handling (Google AI SDK, OpenAI SDK)
2. JavaScript's native error propagation
3. Limited try-catch blocks in critical paths

**Key Finding**: Most errors will surface as **unhandled promise rejections** to the user unless they wrap agent calls in try-catch blocks.

---

## Component-by-Component Analysis

### 1. **Drivers** (GoogleDriver, OpenAIDriver)

**Location**: `javascript/loader/drivers/`

**Current State**: ❌ **NO ERROR HANDLING**

Both drivers have **zero try-catch blocks**. They rely entirely on SDK error handling:

```typescript
// GoogleDriver.ts - No error boundaries
async execute(request: SyntheticRequest): Promise<DriverResult> {
    const model = this.genAI.getGenerativeModel({
        model: request.config.modelName || "gemini-2.0-flash-exp",
        // ...
    });
    
    const result = await model.generateContent(contents);
    // If SDK throws, error propagates directly to caller
}
```

**Error Scenarios NOT Handled**:
- Network failures (timeout, connection refused)
- API authentication errors (invalid API key)
- Rate limiting (429 errors)
- Invalid model names
- Malformed requests
- Token limit exceeded
- Content policy violations

**User Experience**: Raw SDK errors bubble up with technical stack traces.

---

### 2. **IrInterpreter** (Agent Class)

**Location**: `javascript/loader/IrInterpreter.ts`

**Current State**: ⚠️ **PARTIAL ERROR HANDLING**


**Error Handling Present**:

1. **Tool Execution Errors** (Lines 260-275):
```typescript
try {
    // Execute tool/workflow/helper
    toolResult = await safeTools[name](args);
} catch (e: any) {
    console.error(`[Agent] Execution Error:`, e);
    currentMessages.push({
        role: 'user',
        content: `Tool Error: ${e.message}`
    });
    toolsStillAvailable = false;
}
```

**What This Handles**:
- Tool execution failures
- Workflow errors
- Helper (sub-agent) errors
- Converts errors to messages for model to see

**What This DOESN'T Handle**:
- Driver errors (network, API failures)
- JSON parsing errors in final output
- Lifecycle hook errors
- Validation errors

2. **JSON Parsing Errors** (Lines 290-310):
```typescript
try {
    output = JSON.parse(result.text ?? "{}") as TOutput;
} catch (e) {
    // Fallback: wrap text in schema format
    if (toolsStillAvailable && request.responseSchema.properties) {
        const firstProp = Object.keys(request.responseSchema.properties)[0];
        output = { [firstProp]: result.text } as TOutput;
    } else {
        console.error("Failed to parse JSON response:", result.text);
        throw new Error("Model failed to return valid JSON");
    }
}
```

**What This Handles**:
- Invalid JSON from model
- Attempts graceful fallback

**What This DOESN'T Handle**:
- Schema validation (structure mismatch)
- Type coercion errors

3. **Validation Errors** (Lines 50-60):
```typescript
if (this.ir.lifecycle?.enabled && !lifecycle) {
    throw new Error(
        `Agent "${this.ir.name}" has lifecycle enabled. ` +
        `You must provide lifecycle hooks: { prune, load, save }`
    );
}
```

**What This Handles**:
- Missing lifecycle hooks when required
- Missing drivers for required providers

**What This DOESN'T Handle**:
- Invalid lifecycle hook implementations
- Lifecycle hook runtime errors

---

### 3. **WorkflowRunner**

**Location**: `javascript/loader/WorkflowRunner.ts`

**Current State**: ❌ **NO ERROR HANDLING**

```typescript
async run(flowName: string, args: Record<string, any>): Promise<any> {
    const workflow = this.ir.workflows?.find(w => w.flowName === flowName);
    if (!workflow) {
        throw new Error(`Workflow not found: ${flowName}`);
    }
    // No try-catch around step execution
    return this.evaluator.evaluate(workflow.steps, args);
}
```

**Error Scenarios NOT Handled**:
- Step execution failures
- Expression evaluation errors
- Tool call failures within workflows
- Helper delegation errors

**User Experience**: Errors propagate to IrInterpreter's tool execution try-catch.

---

### 4. **ExpressionEvaluator**

**Location**: `javascript/loader/ExpressionEvaluator.ts`

**Current State**: ✅ **GRACEFUL ERROR HANDLING**

This is the **ONLY component** with comprehensive error handling:

```typescript
async evaluate(steps: StepIR[], context: Record<string, any>): Promise<any> {
    for (const step of steps) {
        try {
            // Execute step
            result = await this.executeStep(step, localContext);
        } catch (error: any) {
            // Graceful error handling - return error as data
            result = {
                __toolError: true,
                message: error.message,
                step: step.name
            };
        }
        localContext[step.name] = result;
    }
}
```

**What This Handles**:
- Tool execution errors
- Expression evaluation errors
- Returns errors as data (`__toolError` pattern)
- Allows workflow to continue

**Philosophy**: Errors are **data**, not exceptions. The model can see and react to errors.

---

### 5. **Synthesizer**

**Location**: `javascript/loader/Synthesizer.ts`

**Current State**: ❌ **NO ERROR HANDLING**

```typescript
async synthesize(input: any, context?: any, configName?: string): Promise<SyntheticRequest> {
    // No try-catch around schema building, message construction, etc.
    const responseSchema = this.buildOutputSchema();
    const messages = this.buildMessages(input, context);
    return { messages, responseSchema, config, tools };
}
```

**Error Scenarios NOT Handled**:
- Invalid IR structure
- Type resolution failures
- Schema generation errors
- Template rendering errors

**User Experience**: Errors throw immediately, preventing agent execution.

---

### 6. **StreamBuilder**

**Location**: `javascript/loader/StreamBuilder.ts`

**Current State**: ❌ **NO ERROR HANDLING**

```typescript
async run(): Promise<TOutput> {
    const stream = this.streamGenerator();
    let result: TOutput;
    
    while (true) {
        const { value, done } = await stream.next();
        // No try-catch around stream iteration or handler dispatch
        if (done) {
            result = value;
            break;
        }
        this.dispatch(value);
    }
    return result;
}
```

**Error Scenarios NOT Handled**:
- Stream iteration errors
- Handler callback errors
- Chunk dispatch errors

**User Experience**: Any error in a handler callback will crash the entire stream.

---

## Error Propagation Flow

```
User Code
    ↓
Agent.run() / Agent.stream()
    ↓
IrInterpreter (partial try-catch for tools)
    ↓
Driver.execute() (NO error handling)
    ↓
SDK (Google AI / OpenAI)
    ↓
Network / API
```

**Critical Gap**: Driver layer has **zero error boundaries**, so all SDK/network errors propagate directly to user code.

---

## User-Facing Error Experience

### Scenario 1: Network Failure

```typescript
const agent = createAgent({ apiKeys: { geminiApiKey: 'key' }, ir });
await agent.run({ message: "Hello" });
// ❌ Unhandled Promise Rejection: FetchError: request to https://... failed
```

### Scenario 2: Invalid API Key

```typescript
await agent.run({ message: "Hello" });
// ❌ Unhandled Promise Rejection: GoogleGenerativeAIError: [400] API key not valid
```

### Scenario 3: Tool Execution Error

```typescript
const tools = {
    searchWeb: async () => { throw new Error("API down"); }
};
await agent.run({ message: "Search for cats" }, { tools });
// ✅ Handled gracefully - model sees "Tool Error: API down"
```

### Scenario 4: Invalid JSON Response

```typescript
await agent.run({ message: "Hello" });
// ⚠️ Partial handling - attempts fallback, may throw "Model failed to return valid JSON"
```

---

## Recommendations

### Priority 1: Driver Error Boundaries (CRITICAL)

**Problem**: All network/API errors crash the application.

**Solution**: Wrap driver execution in try-catch with error classification:

```typescript
async execute(request: SyntheticRequest): Promise<DriverResult> {
    try {
        const result = await model.generateContent(contents);
        return this.parseResult(result);
    } catch (error: any) {
        // Classify and wrap error
        throw new DriverError({
            type: this.classifyError(error),
            message: error.message,
            originalError: error,
            retryable: this.isRetryable(error)
        });
    }
}

private classifyError(error: any): ErrorType {
    if (error.status === 401) return 'AUTH_ERROR';
    if (error.status === 429) return 'RATE_LIMIT';
    if (error.status === 400) return 'INVALID_REQUEST';
    if (error.code === 'ECONNREFUSED') return 'NETWORK_ERROR';
    return 'UNKNOWN_ERROR';
}
```

### Priority 2: Streaming Error Recovery

**Problem**: Handler errors crash the entire stream.

**Solution**: Wrap handler dispatch in try-catch:

```typescript
private dispatch(chunk: StreamChunk): void {
    try {
        this.handlers.onChunk?.(chunk);
        // ... dispatch to specific handlers
    } catch (error: any) {
        console.error('[StreamBuilder] Handler error:', error);
        // Continue streaming despite handler failure
    }
}
```

### Priority 3: Lifecycle Hook Error Handling

**Problem**: Lifecycle hook errors are not caught.

**Solution**: Wrap lifecycle calls in try-catch:

```typescript
if (lifecycle) {
    try {
        await lifecycle.prune({ context, agent, usage });
        const state = await lifecycle.load({ context });
        loadedMessages = state.messages;
    } catch (error: any) {
        throw new Error(`Lifecycle error: ${error.message}`);
    }
}
```

### Priority 4: Schema Validation

**Problem**: No validation that model output matches expected schema.

**Solution**: Add optional schema validation:

```typescript
if (request.responseSchema && config.validateSchema) {
    const valid = validateJsonSchema(output, request.responseSchema);
    if (!valid) {
        throw new SchemaValidationError(output, request.responseSchema);
    }
}
```

---

## Error Types to Define

```typescript
export class DriverError extends Error {
    constructor(
        public type: ErrorType,
        public originalError: Error,
        public retryable: boolean
    ) {
        super(`Driver error: ${type}`);
    }
}

export type ErrorType =
    | 'AUTH_ERROR'          // Invalid API key
    | 'RATE_LIMIT'          // 429 Too Many Requests
    | 'NETWORK_ERROR'       // Connection failed
    | 'INVALID_REQUEST'     // 400 Bad Request
    | 'CONTENT_POLICY'      // Content filtered
    | 'TOKEN_LIMIT'         // Context too long
    | 'UNKNOWN_ERROR';      // Unclassified

export class SchemaValidationError extends Error {
    constructor(
        public output: any,
        public expectedSchema: JsonSchema
    ) {
        super('Model output does not match expected schema');
    }
}

export class LifecycleError extends Error {
    constructor(
        public hook: 'prune' | 'load' | 'save',
        public originalError: Error
    ) {
        super(`Lifecycle hook "${hook}" failed: ${originalError.message}`);
    }
}
```

---

## Testing Error Scenarios

### Manual Test Cases

1. **Invalid API Key**:
   ```typescript
   const agent = createAgent({ apiKeys: { geminiApiKey: 'invalid' }, ir });
   await agent.run({ message: "Hello" });
   // Expected: DriverError with type='AUTH_ERROR'
   ```

2. **Network Timeout**:
   ```typescript
   // Disconnect network
   await agent.run({ message: "Hello" });
   // Expected: DriverError with type='NETWORK_ERROR', retryable=true
   ```

3. **Tool Error**:
   ```typescript
   const tools = { fail: async () => { throw new Error("Boom"); } };
   await agent.run({ message: "Call fail" }, { tools });
   // Expected: Model sees "Tool Error: Boom" and can respond
   ```

4. **Handler Error in Stream**:
   ```typescript
   await agent.stream({ message: "Hello" })
       .onText(() => { throw new Error("Handler crash"); })
       .run();
   // Expected: Error logged, stream continues
   ```

---

## Current Best Practices for Users

Until error handling is improved, users should:

1. **Always wrap agent calls in try-catch**:
   ```typescript
   try {
       const result = await agent.run(input);
   } catch (error) {
       console.error('Agent failed:', error);
       // Handle error
   }
   ```

2. **Validate API keys before creating agents**:
   ```typescript
   if (!process.env.GEMINI_API_KEY) {
       throw new Error('GEMINI_API_KEY not set');
   }
   ```

3. **Implement retry logic for network errors**:
   ```typescript
   async function runWithRetry(agent, input, maxRetries = 3) {
       for (let i = 0; i < maxRetries; i++) {
           try {
               return await agent.run(input);
           } catch (error) {
               if (i === maxRetries - 1) throw error;
               await sleep(1000 * Math.pow(2, i)); // Exponential backoff
           }
       }
   }
   ```

4. **Handle streaming errors gracefully**:
   ```typescript
   try {
       await agent.stream(input)
           .onText(delta => process.stdout.write(delta))
           .run();
   } catch (error) {
       console.error('Stream failed:', error);
   }
   ```

---

## Summary

| Component | Error Handling | User Impact |
|-----------|---------------|-------------|
| **GoogleDriver** | ❌ None | Raw SDK errors crash app |
| **OpenAIDriver** | ❌ None | Raw SDK errors crash app |
| **IrInterpreter** | ⚠️ Partial | Tool errors handled, driver errors crash |
| **WorkflowRunner** | ❌ None | Errors propagate to IrInterpreter |
| **ExpressionEvaluator** | ✅ Graceful | Errors become data, workflow continues |
| **Synthesizer** | ❌ None | Schema errors crash immediately |
| **StreamBuilder** | ❌ None | Handler errors crash stream |

**Overall Grade**: ⚠️ **C-** - Minimal error handling, relies on user-level try-catch.

**Recommended Priority**: Implement driver error boundaries first, as they affect all users and all error scenarios.
