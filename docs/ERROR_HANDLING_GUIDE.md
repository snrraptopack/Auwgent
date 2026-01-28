# Error Handling Guide

**Date**: 2026-01-27  
**Status**: Production Ready ✅

---

## Overview

The Auwgent runtime now has comprehensive error handling across all components. All errors are caught, classified, and wrapped in specialized error classes that provide:

- Clear, user-friendly error messages
- Error type classification
- Retryability information
- Original error preservation for debugging
- Context about where the error occurred

---

## Error Types

### DriverError

Wraps all driver-level errors (network, API, authentication).

```typescript
import { DriverError } from './loader';

try {
    await agent.run(input);
} catch (error) {
    if (error instanceof DriverError) {
        console.log(error.type);              // 'AUTH_ERROR', 'RATE_LIMIT', etc.
        console.log(error.retryable);         // true/false
        console.log(error.getUserMessage());  // User-friendly message
        console.log(error.statusCode);        // HTTP status code (if available)
        console.log(error.originalError);     // Original SDK error
    }
}
```

**Error Types**:
- `AUTH_ERROR` - Invalid API key (not retryable)
- `RATE_LIMIT` - 429 Too Many Requests (retryable)
- `NETWORK_ERROR` - Connection failed (retryable)
- `INVALID_REQUEST` - 400 Bad Request (not retryable)
- `CONTENT_POLICY` - Content filtered (not retryable)
- `TOKEN_LIMIT` - Context window exceeded (not retryable)
- `MODEL_NOT_FOUND` - Invalid model name (not retryable)
- `UNKNOWN_ERROR` - Unclassified (not retryable)

### LifecycleError

Wraps lifecycle hook failures.

```typescript
import { LifecycleError } from './loader';

try {
    await agent.run(input);
} catch (error) {
    if (error instanceof LifecycleError) {
        console.log(error.hook);           // 'prune', 'load', or 'save'
        console.log(error.originalError);  // Original error from hook
    }
}
```

### WorkflowError

Wraps workflow execution failures.

```typescript
import { WorkflowError } from './loader';

try {
    await agent.run(input);
} catch (error) {
    if (error instanceof WorkflowError) {
        console.log(error.workflowName);   // Name of failed workflow
        console.log(error.stepName);       // Name of failed step (if available)
        console.log(error.originalError);  // Original error
    }
}
```

### StreamError

Wraps streaming failures.

```typescript
import { StreamError } from './loader';

try {
    await agent.stream(input).run();
} catch (error) {
    if (error instanceof StreamError) {
        console.log(error.phase);          // 'initialization', 'streaming', 'handler'
        console.log(error.originalError);  // Original error
    }
}
```

### ConfigurationError

Thrown for invalid agent configuration.

```typescript
import { ConfigurationError } from './loader';

try {
    const agent = createAgent({ /* invalid config */ });
} catch (error) {
    if (error instanceof ConfigurationError) {
        console.log(error.message);  // Clear message about what's wrong
    }
}
```

### SchemaValidationError

Thrown when model output doesn't match expected schema (future feature).

```typescript
import { SchemaValidationError } from './loader';

try {
    await agent.run(input);
} catch (error) {
    if (error instanceof SchemaValidationError) {
        console.log(error.output);            // Actual output from model
        console.log(error.expectedSchema);    // Expected schema
        console.log(error.validationErrors);  // List of validation errors
    }
}
```

---

## Usage Patterns

### Basic Error Handling

Always wrap agent calls in try-catch:

```typescript
try {
    const result = await agent.run(input);
    console.log('Success:', result);
} catch (error) {
    console.error('Error:', error.message);
}
```

### Retry Logic for Retryable Errors

```typescript
async function runWithRetry(agent, input, maxRetries = 3) {
    for (let i = 0; i < maxRetries; i++) {
        try {
            return await agent.run(input);
        } catch (error) {
            if (error instanceof DriverError && error.retryable) {
                if (i === maxRetries - 1) throw error;
                
                // Exponential backoff
                const delay = 1000 * Math.pow(2, i);
                console.log(`Retrying in ${delay}ms...`);
                await sleep(delay);
            } else {
                // Not retryable, throw immediately
                throw error;
            }
        }
    }
}
```

### Handle Specific Error Types

```typescript
try {
    await agent.run(input);
} catch (error) {
    if (error instanceof DriverError) {
        switch (error.type) {
            case 'AUTH_ERROR':
                console.error('Invalid API key. Please check your configuration.');
                process.exit(1);
                break;
            
            case 'RATE_LIMIT':
                console.log('Rate limited. Waiting 60 seconds...');
                await sleep(60000);
                return retry();
            
            case 'NETWORK_ERROR':
                console.log('Network error. Retrying...');
                return retry();
            
            case 'TOKEN_LIMIT':
                console.error('Context too long. Please reduce input size.');
                break;
            
            default:
                console.error('Unknown error:', error.getUserMessage());
        }
    } else if (error instanceof LifecycleError) {
        console.error(`Lifecycle ${error.hook} failed:`, error.originalError.message);
    } else if (error instanceof WorkflowError) {
        console.error(`Workflow ${error.workflowName} failed:`, error.message);
        if (error.stepName) {
            console.error(`  at step: ${error.stepName}`);
        }
    }
}
```

### Streaming with Error Recovery

Handler errors don't crash the stream:

```typescript
try {
    const result = await agent.stream(input)
        .onText(delta => {
            // Even if this throws, stream continues
            process.stdout.write(delta);
        })
        .onToolResult((name, result) => {
            // Handler errors are logged but don't crash stream
            console.log(`Tool ${name}:`, result);
        })
        .run();
    
    console.log('Final result:', result);
} catch (error) {
    // Only stream-level errors reach here
    console.error('Stream failed:', error.message);
}
```

---

## Error Messages

### User-Friendly Messages

All error classes provide `getUserMessage()` for user-friendly output:

```typescript
catch (error) {
    if (error instanceof DriverError) {
        console.error(error.getUserMessage());
        // "Authentication failed. Please check your API key."
        // "Rate limit exceeded. Please try again later."
        // "Network error. Please check your connection and try again."
    }
}
```

### Debug Information

Original errors are preserved for debugging:

```typescript
catch (error) {
    if (error instanceof DriverError) {
        console.error('User message:', error.getUserMessage());
        console.error('Debug info:', error.originalError);
        console.error('Status code:', error.statusCode);
    }
}
```

---

## Testing

### Test Script

Run the error handling test suite:

```bash
bun run test-error-handling.ts
```

**Tests**:
1. Invalid API key → `DriverError` with `AUTH_ERROR`
2. Missing lifecycle hooks → `ConfigurationError`
3. Stream handler error recovery
4. Error type classification

### Manual Testing

#### Test Auth Error
```typescript
const agent = createAgent({
    apiKeys: { geminiApiKey: 'invalid_key' },
    ir
});
await agent.run({ message: "Hello" });
// Throws: DriverError { type: 'AUTH_ERROR', retryable: false }
```

#### Test Network Error
```typescript
// Disconnect network
await agent.run({ message: "Hello" });
// Throws: DriverError { type: 'NETWORK_ERROR', retryable: true }
```

#### Test Configuration Error
```typescript
const agent = createAgent({
    apiKeys: { geminiApiKey: 'key' },
    ir: { ...ir, lifecycle: { enabled: true } }
    // Missing lifecycle hooks!
});
await agent.run({ message: "Hello" });
// Throws: ConfigurationError
```

---

## Best Practices

### 1. Always Use Try-Catch

```typescript
// ❌ BAD
const result = await agent.run(input);

// ✅ GOOD
try {
    const result = await agent.run(input);
} catch (error) {
    console.error('Error:', error.message);
}
```

### 2. Check Retryability

```typescript
// ✅ GOOD
catch (error) {
    if (error instanceof DriverError && error.retryable) {
        return retry();
    }
    throw error;  // Don't retry non-retryable errors
}
```

### 3. Validate Configuration Early

```typescript
// ✅ GOOD - Fails fast at creation
const agent = createAgent({
    apiKeys: { geminiApiKey: process.env.GEMINI_API_KEY },
    ir,
    lifecycle: ir.lifecycle?.enabled ? lifecycleHooks : undefined
});
```

### 4. Log Original Errors for Debugging

```typescript
// ✅ GOOD
catch (error) {
    console.error('User message:', error.getUserMessage?.() || error.message);
    console.debug('Original error:', error.originalError || error);
}
```

### 5. Handle Streaming Errors Gracefully

```typescript
// ✅ GOOD
const result = await agent.stream(input)
    .onText(delta => {
        try {
            // Your handler logic
        } catch (e) {
            console.error('Handler error:', e);
            // Don't throw - let stream continue
        }
    })
    .run();
```

---

## Migration from Old Code

### Before (No Error Handling)

```typescript
const agent = createAgent({ geminiApiKey: '...' });
agent.load(ir);
const result = await agent.run(input, tools, context);
// Crashes on any error
```

### After (With Error Handling)

```typescript
try {
    const agent = createAgent({
        apiKeys: { geminiApiKey: '...' },
        ir,
        tools,
        context
    });
    const result = await agent.run(input);
} catch (error) {
    if (error instanceof DriverError) {
        console.error(error.getUserMessage());
        if (error.retryable) {
            // Retry logic
        }
    } else {
        console.error('Unexpected error:', error.message);
    }
}
```

---

## Error Handling Coverage

| Component | Status | Notes |
|-----------|--------|-------|
| GoogleDriver | ✅ Complete | All errors caught and classified |
| OpenAIDriver | ✅ Complete | All errors caught and classified |
| IrInterpreter | ✅ Complete | Lifecycle errors wrapped |
| WorkflowRunner | ✅ Complete | Workflow errors with context |
| StreamBuilder | ✅ Complete | Handler errors don't crash stream |
| ExpressionEvaluator | ✅ Complete | Errors returned as data |
| Synthesizer | ⚠️ Future | Schema generation errors |

---

## Future Enhancements

### Schema Validation

Add optional schema validation:

```typescript
const agent = createAgent({
    apiKeys: { geminiApiKey: '...' },
    ir,
    validateSchema: true  // Enable schema validation
});

try {
    await agent.run(input);
} catch (error) {
    if (error instanceof SchemaValidationError) {
        console.error('Schema mismatch:', error.validationErrors);
    }
}
```

### Retry Middleware

Built-in retry logic:

```typescript
const agent = createAgent({
    apiKeys: { geminiApiKey: '...' },
    ir,
    retry: {
        maxAttempts: 3,
        backoff: 'exponential'
    }
});
```

### Error Telemetry

Track errors for monitoring:

```typescript
const agent = createAgent({
    apiKeys: { geminiApiKey: '...' },
    ir,
    onError: (error) => {
        // Send to monitoring service
        telemetry.trackError(error);
    }
});
```

---

## Summary

The Auwgent runtime now has production-ready error handling:

✅ All errors caught and classified  
✅ User-friendly error messages  
✅ Retryability information  
✅ Original errors preserved  
✅ Context about error location  
✅ Stream handler error recovery  
✅ Configuration validation  
✅ Comprehensive test coverage  

No more unhandled promise rejections!
