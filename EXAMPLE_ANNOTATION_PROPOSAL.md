# Proposal: @example Annotations for Few-Shot Prompt Generation

## Problem Statement

The current system prompt generation creates documentation for tools, workflows, and schemas, but lacks concrete usage examples. This makes it harder for LLMs to understand the correct usage patterns, especially for complex nested structures or domain-specific tools.

## Current System Prompt Structure

```
# Things to Know
1. Intents are actions you perform...
2. A `tool_call` intent invokes an external tool...

# Available Actions (Options)

// Call a registered tool. See exact arguments below.
tool_call(
  type = "< fetch_session | get_user >"
  args = {
    // PASS YOUR ARGUMENTS HERE USING `=`
  }
)

# Tools Available
fetch_session(session_id: string)
get_user(user_id: string, include_history?: boolean)
```

**Problem**: The model sees the signature but no concrete example of how to use it.

---

## Proposed Solution: @example Annotations

### DSL Syntax

Add an optional `@example` annotation to tools, workflows, schemas, and custom intents:

```auwgent
tool fetch_session(session_id: string) {
  @example {
    session_id = "sess_8f3a92c1"
  }
  
  // implementation...
}

workflow compile_context(session_id: string, apply_compression: boolean) {
  @example {
    session_id = "sess_123"
    apply_compression = false
  }
  
  // implementation...
}

output ContextCompilerOutput {
  session_id: string
  user: {
    id: string
    name: string
  }
  
  @example {
    session_id = "sess_123"
    user = {
      id = "usr_456"
      name = "Nana"
    }
  }
}

custom intent ask_user {
  question: string
  options: string[]
  
  @example {
    question = "Are you sure you want to proceed?"
    options = ["yes", "no", "cancel"]
  }
}
```

### Multiple Examples

Support multiple examples for different use cases:

```auwgent
tool search_logs(query: string, limit: number, filters?: object) {
  @example {
    query = "error"
    limit = 10
  }
  
  @example {
    query = "authentication failed"
    limit = 50
    filters = {
      severity = "high"
      date_range = {
        start = "2024-01-01"
        end = "2024-01-31"
      }
    }
  }
  
  // implementation...
}
```

---

## Generated System Prompt (New Format)

### With Examples

```
# Available Actions (Options)

// Call a registered tool. See exact arguments below.
tool_call(
  type = "< fetch_session | get_user | search_logs >"
  args = {
    // PASS YOUR ARGUMENTS HERE USING `=`
  }
)

# Tools Available

fetch_session(session_id: string)
Example:
@@tool
fetch_session(session_id = "sess_8f3a92c1")
@@end

get_user(user_id: string, include_history?: boolean)
Example:
@@tool
get_user(user_id = "usr_456", include_history = true)
@@end

search_logs(query: string, limit: number, filters?: object)
Example 1:
@@tool
search_logs(query = "error", limit = 10)
@@end

Example 2:
@@tool
search_logs(
  query = "authentication failed"
  limit = 50
  filters = {
    severity = "high"
    date_range = {
      start = "2024-01-01"
      end = "2024-01-31"
    }
  }
)
@@end
```

### For Workflows

```
# Workflows Available

compile_context(session_id: string, apply_compression: boolean)
Example:
@@workflow
compile_context(session_id = "sess_123", apply_compression = false)
@@end
```

### For Output Schemas

```
# Response Schemas Available

ContextCompilerOutput(session_id: string, user: object, memory: object)
Example:
@@out ContextCompilerOutput
{
  session_id: "sess_123",
  user: {
    id: "usr_456",
    name: "Nana",
    preferences: {
      language: "en",
      verbosity: "detailed",
      topics: []
    }
  },
  memory: {
    current_message: "hello",
    recent: [],
    summary: ""
  }
}
@@end
```

### For Custom Intents

```
# Custom Intents Available

ask_user(question: string, options: string[])
Example:
@@ask_user
confirm(question = "Are you sure?", options = ["yes", "no"])
@@end
```

---

## Implementation Plan

### Phase 1: AST & Parser Changes

**File**: `auwgent-compiler/crates/auwgent-ast/src/lib.rs`

Add `examples` field to relevant structs:

```rust
pub struct ToolFunction {
    pub name: Spanned<String>,
    pub params: Vec<TypeConfigDecl>,
    pub returns: Option<TypeExpr>,
    pub body: Option<ToolBody>,
    pub examples: Vec<ExampleBlock>, // NEW
}

pub struct WorkflowConfig {
    pub name: Spanned<String>,
    pub params: Vec<TypeConfigDecl>,
    pub returns: Option<TypeExpr>,
    pub body: Vec<Statement>,
    pub examples: Vec<ExampleBlock>, // NEW
}

pub struct OutputDecl {
    pub type_expr: TypeExpr,
    pub examples: Vec<ExampleBlock>, // NEW
}

pub struct CustomIntentDecl {
    pub name: Spanned<String>,
    pub fields: Vec<TypeConfigDecl>,
    pub examples: Vec<ExampleBlock>, // NEW
}

#[derive(Debug, Clone)]
pub struct ExampleBlock {
    pub fields: HashMap<String, ASTValue>, // Parsed example data
}
```

**File**: `auwgent-compiler/crates/auwgent-parser/src/*.rs`

Add parsing logic for `@example` blocks:
- Recognize `@example` keyword
- Parse the following `{ ... }` block as a TS-style object
- Store in AST node's `examples` field
- Support multiple `@example` blocks per declaration

### Phase 2: IR Generation Changes

**File**: `auwgent-compiler/crates/auwgent-ir/src/lib.rs`

Update IR types to include examples:

```rust
pub struct Tool {
    pub name: String,
    pub description: Option<String>,
    pub params: Value,
    pub returns: Value,
    pub examples: Vec<Value>, // NEW - array of example objects
}

pub struct Workflow {
    pub name: String,
    pub params: Value,
    pub returns: Value,
    pub description: Option<String>,
    pub body: Vec<Expression>,
    pub tools: Vec<Tool>,
    pub examples: Vec<Value>, // NEW
}

pub struct CustomIntentDef {
    pub name: String,
    pub description: Option<String>,
    pub fields: Value,
    pub examples: Vec<Value>, // NEW
}
```

Convert AST examples to JSON during IR generation.

### Phase 3: Runtime Prompt Generation Changes

**File**: `ir-runtime/src/intents.rs`

Update `generate_intents()` function to include examples:

```rust
// Tools section (modified)
if !ir.tools.is_empty() {
    let mut tool_lines = Vec::new();
    for tool in &ir.tools {
        let mut params = Vec::new();
        if let Some(obj) = tool.params.as_object() {
            for (name, def) in obj {
                let field_type = def["type"].as_str().unwrap_or("any");
                params.push(format!("{}: {}", name, field_type));
            }
        }
        let mut sig = format!("{}({})", tool.name, params.join(", "));
        if let Some(desc) = &tool.description {
            sig.push_str(" // ");
            sig.push_str(desc);
        }
        
        // NEW: Add examples if available
        if let Some(examples) = &tool.examples {
            if !examples.is_empty() {
                sig.push_str("\n");
                for (idx, example) in examples.iter().enumerate() {
                    let example_label = if examples.len() > 1 {
                        format!("Example {}:", idx + 1)
                    } else {
                        "Example:".to_string()
                    };
                    
                    sig.push_str(&format!("\n{}\n@@tool\n", example_label));
                    sig.push_str(&format_example_call(&tool.name, example));
                    sig.push_str("\n@@end");
                }
            }
        }
        
        tool_lines.push(sig);
    }
    expanded_tools_str = format!(
        "# Tools Available\n{}", 
        tool_lines.join("\n\n")
    );
}

fn format_example_call(name: &str, example: &Value) -> String {
    if let Some(obj) = example.as_object() {
        let mut args = Vec::new();
        for (key, val) in obj {
            args.push(format!("{} = {}", key, format_value_for_example(val)));
        }
        format!("{}({})", name, args.join(", "))
    } else {
        format!("{}()", name)
    }
}

fn format_value_for_example(val: &Value) -> String {
    match val {
        Value::String(s) => format!("\"{}\"", s),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(format_value_for_example).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(obj) => {
            let mut fields = Vec::new();
            for (k, v) in obj {
                fields.push(format!("{} = {}", k, format_value_for_example(v)));
            }
            format!("{{\n    {}\n  }}", fields.join(",\n    "))
        }
    }
}
```

---

## Benefits

1. **Few-Shot Learning**: Models see concrete examples of correct usage
2. **Complex Structures**: Examples show how to construct nested objects and arrays
3. **Domain Context**: Examples can include domain-specific values that guide the model
4. **Error Reduction**: Reduces hallucination and incorrect argument passing
5. **Backward Compatible**: Examples are optional - existing code works without them

---

## Example Use Cases

### 1. Complex Nested Arguments

```auwgent
tool create_user(
  name: string,
  email: string,
  preferences: {
    notifications: boolean,
    theme: "light" | "dark",
    tags: string[]
  }
) {
  @example {
    name = "Alice"
    email = "alice@example.com"
    preferences = {
      notifications = true
      theme = "dark"
      tags = ["developer", "premium"]
    }
  }
}
```

Generated prompt shows:
```
@@tool
create_user(
  name = "Alice",
  email = "alice@example.com",
  preferences = {
    notifications = true,
    theme = "dark",
    tags = ["developer", "premium"]
  }
)
@@end
```

### 2. Domain-Specific Values

```auwgent
tool query_database(sql: string, params: object) {
  @example {
    sql = "SELECT * FROM users WHERE created_at > $1 LIMIT $2"
    params = {
      values = ["2024-01-01", 10]
    }
  }
}
```

### 3. Multiple Usage Patterns

```auwgent
tool send_notification(
  user_id: string,
  message: string,
  channel?: "email" | "sms" | "push"
) {
  @example {
    user_id = "usr_123"
    message = "Your order has shipped!"
    channel = "email"
  }
  
  @example {
    user_id = "usr_456"
    message = "Payment received"
    // channel is optional, omitted here
  }
}
```

---

## Alternative: Inline Examples in Comments

Instead of `@example` blocks, use structured comments:

```auwgent
tool fetch_session(session_id: string) {
  // Example: fetch_session(session_id = "sess_8f3a92c1")
  
  // implementation...
}
```

**Pros**: Simpler to parse, no new syntax
**Cons**: Less structured, harder to extract programmatically, no multi-line support

---

## Recommendation

Implement the `@example` block syntax because:
1. Structured and parseable
2. Supports complex multi-line examples
3. Can have multiple examples per declaration
4. Clear separation from implementation code
5. Aligns with existing `@` annotation pattern in Auwgent

---

## Migration Path

1. Add `examples: Vec<Value>` to IR types (Tool, Workflow, CustomIntentDef)
2. Update compiler to parse `@example` blocks and include in IR
3. Update `generate_intents()` to format and inject examples
4. Examples are optional - existing agents work without modification
5. Gradually add examples to commonly-used tools/workflows

---

## Open Questions

1. **Example validation**: Should examples be validated against the param schema at compile time?
2. **Example format**: Should examples use the function call syntax or just the args object?
3. **Example placement**: Should examples appear inline with each tool or in a separate "Examples" section?
4. **Block protocol**: Should examples show `@@tool` blocks or just the function call?

## Recommended Answers

1. **Yes** - Validate at compile time to catch errors early
2. **Full function call** - Shows complete usage including tool name
3. **Inline with each tool** - Keeps related information together
4. **Show @@blocks** - Teaches the model the complete block protocol syntax
