# Block Protocol System Prompt Generation Proposal

## Current State

The current `generate_intents()` function in `ir-runtime/src/intents.rs` generates prompts using the OLD function-style syntax:

```
tool_call(
  type = "fetch_session"
  args = {
    // PASS YOUR ARGUMENTS HERE
  }
)
```

## Target State

We need to migrate to the NEW block protocol format:

```
@@tool
fetch_session(session_id = "sess_123")
@@end
```

---

## Proposed System Prompt Template

### Core Structure

```
You are an execution engine. You communicate EXCLUSIVELY using the `@@` block protocol.

# AVAILABLE BLOCKS

You may ONLY use the following block types:

@@chat
Use this to speak to the user, explain your actions, or think out loud.
@@end

{{if has_tools}}
@@tool
Use this to execute parallel tools.
Available tools and their exact arguments:
- fetch_session(session_id: string)
- get_user(user_id: string, include_history?: boolean)
- search_logs(query: string, limit: number)

Example:
@@tool
fetch_session(session_id = "sess_123")
get_user(user_id = "usr_456", include_history = true)
@@end
{{/if}}

{{if has_workflows}}
@@workflow
Use this to execute a single, sequential backend workflow.
Available workflows:
- compile_context(session_id: string, apply_compression: boolean)
- reset_environment(force: boolean)

Example:
@@workflow
compile_context(session_id = "sess_123", apply_compression = false)
@@end
{{/if}}

{{if has_output_schemas}}
@@out [SchemaName]
Use this to return structured data to the system.
Available schemas and their exact shapes:

1. ContextCompilerOutput
{
  session_id: string;
  user: {
    id: string;
    name: string;
    preferences: {
      language: string;
      verbosity: "concise" | "detailed";
      topics: string[]
    };
  };
  memory: {
    current_message: string;
    recent: string[];
    summary: string
  };
}

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
{{/if}}

{{if has_helpers}}
@@helper
Use this to delegate to a specialized sub-agent.
Available helpers:
- StoryTeller(city: string, days: number)
- DataAnalyzer(dataset: string, query: string)

Example:
@@helper
StoryTeller(city = "Accra", days = 3)
@@end
{{/if}}

{{if has_custom_intents}}
@@ask_user
Custom intent for asking the user questions.

Example:
@@ask_user
confirm(question = "Are you sure?", options = ["yes", "no"])
@@end
{{/if}}

# CRITICAL CONSTRAINTS

- NEVER invent tools, workflows, or schemas. Only use what is listed above.
- Your `@@tool` arguments and `@@out` JSON must STRICTLY match the types and shapes provided.
- Do not add properties to `@@out` objects that are not defined in the schema shape.
- You can use multiple blocks in one response (e.g., @@chat then @@tool then @@chat).
- Blocks auto-close when you start a new block, but using @@end is recommended for clarity.

# MULTI-BLOCK RESPONSE PATTERN

You can combine multiple blocks in one response:

@@chat
Let me fetch that data for you.
@@end

@@tool
fetch_session(session_id = "sess_123")
get_user(user_id = "usr_456")
@@end

@@chat
Here's what I found.
@@end
```

---

## Implementation Strategy

### Option 1: Template-Based Generation (Recommended)

Create a template engine that processes the structure above with conditionals:

```rust
pub fn generate_block_protocol_prompt(ir: &AgentIR) -> String {
    let mut sections = Vec::new();
    
    // Header
    sections.push(
        "You are an execution engine. You communicate EXCLUSIVELY using the `@@` block protocol.\n\n\
         # AVAILABLE BLOCKS\n\n\
         You may ONLY use the following block types:\n\n\
         @@chat\n\
         Use this to speak to the user, explain your actions, or think out loud.\n\
         @@end".to_string()
    );
    
    // Tools section
    if !ir.tools.is_empty() {
        let mut tool_section = String::from("\n\n@@tool\nUse this to execute parallel tools.\nAvailable tools and their exact arguments:\n");
        
        for tool in &ir.tools {
            let params = format_tool_params(&tool.params);
            tool_section.push_str(&format!("- {}({})", tool.name, params));
            if let Some(desc) = &tool.description {
                tool_section.push_str(&format!(" // {}", desc));
            }
            tool_section.push('\n');
        }
        
        // Add examples
        if let Some(examples) = collect_tool_examples(&ir.tools) {
            tool_section.push_str("\nExample:\n@@tool\n");
            tool_section.push_str(&examples);
            tool_section.push_str("\n@@end");
        }
        
        sections.push(tool_section);
    }
    
    // Workflows section
    if !ir.workflows.is_empty() {
        let mut wf_section = String::from("\n\n@@workflow\nUse this to execute a single, sequential backend workflow.\nAvailable workflows:\n");
        
        for wf in &ir.workflows {
            let params = format_workflow_params(&wf.params);
            wf_section.push_str(&format!("- {}({})", wf.name, params));
            if let Some(desc) = &wf.description {
                wf_section.push_str(&format!(" // {}", desc));
            }
            wf_section.push('\n');
        }
        
        // Add examples
        if let Some(examples) = collect_workflow_examples(&ir.workflows) {
            wf_section.push_str("\nExample:\n@@workflow\n");
            wf_section.push_str(&examples);
            wf_section.push_str("\n@@end");
        }
        
        sections.push(wf_section);
    }
    
    // Output schemas section
    if let Some(output) = &ir.output {
        if let Some(obj) = output.as_object() {
            if !obj.is_empty() {
                let mut schema_section = String::from("\n\n@@out [SchemaName]\nUse this to return structured data to the system.\nAvailable schemas and their exact shapes:\n\n");
                
                // Format schema structure
                schema_section.push_str(&format_output_schema(output, ir));
                
                // Add examples
                if let Some(examples) = extract_output_examples(output) {
                    schema_section.push_str("\n\nExample:\n");
                    schema_section.push_str(&examples);
                }
                
                sections.push(schema_section);
            }
        }
    }
    
    // Helpers section
    if !ir.helpers.is_empty() {
        let mut helper_section = String::from("\n\n@@helper\nUse this to delegate to a specialized sub-agent.\nAvailable helpers:\n");
        
        for helper in &ir.helpers {
            let params = format_helper_params(&helper.input);
            helper_section.push_str(&format!("- {}({})", helper.name, params));
            if let Some(desc) = &helper.description {
                helper_section.push_str(&format!(" // {}", desc));
            }
            helper_section.push('\n');
        }
        
        // Add examples
        if let Some(examples) = collect_helper_examples(&ir.helpers) {
            helper_section.push_str("\nExample:\n@@helper\n");
            helper_section.push_str(&examples);
            helper_section.push_str("\n@@end");
        }
        
        sections.push(helper_section);
    }
    
    // Custom intents section
    if let Some(custom) = &ir.custom_intents {
        for ci in custom {
            let mut custom_section = format!("\n\n@@{}\n", ci.name);
            if let Some(desc) = &ci.description {
                custom_section.push_str(&format!("{}\n", desc));
            }
            
            // Add examples
            if let Some(examples) = &ci.examples {
                if !examples.is_empty() {
                    custom_section.push_str("\nExample:\n");
                    custom_section.push_str(&format!("@@{}\n", ci.name));
                    custom_section.push_str(&format_custom_intent_example(&examples[0]));
                    custom_section.push_str("\n@@end");
                }
            }
            
            sections.push(custom_section);
        }
    }
    
    // Constraints
    sections.push(
        "\n\n# CRITICAL CONSTRAINTS\n\n\
         - NEVER invent tools, workflows, or schemas. Only use what is listed above.\n\
         - Your `@@tool` arguments and `@@out` JSON must STRICTLY match the types and shapes provided.\n\
         - Do not add properties to `@@out` objects that are not defined in the schema shape.\n\
         - You can use multiple blocks in one response (e.g., @@chat then @@tool then @@chat).\n\
         - Blocks auto-close when you start a new block, but using @@end is recommended for clarity.".to_string()
    );
    
    sections.join("")
}
```

### Option 2: Hybrid Approach (Simpler)

Keep the current `generate_intents()` structure but add a new function `generate_block_examples()` that creates an "Examples" section:

```rust
pub fn generate_intents(ir: &AgentIR) -> String {
    let mut sections = Vec::new();
    
    // ... existing code ...
    
    // NEW: Add examples section at the end
    let examples = generate_block_examples(ir);
    if !examples.is_empty() {
        sections.push(format!("\n\n# Usage Examples\n\n{}", examples));
    }
    
    sections.join("\n\n")
}

fn generate_block_examples(ir: &AgentIR) -> String {
    let mut examples = Vec::new();
    
    // Tool examples
    for tool in &ir.tools {
        if let Some(tool_examples) = &tool.examples {
            for (idx, ex) in tool_examples.iter().enumerate() {
                let label = if tool_examples.len() > 1 {
                    format!("{}. {} (example {})", examples.len() + 1, tool.name, idx + 1)
                } else {
                    format!("{}. {}", examples.len() + 1, tool.name)
                };
                
                examples.push(format!(
                    "{}\n@@tool\n{}\n@@end",
                    label,
                    format_function_call(&tool.name, ex)
                ));
            }
        }
    }
    
    // Workflow examples
    for wf in &ir.workflows {
        if let Some(wf_examples) = &wf.examples {
            for ex in wf_examples {
                examples.push(format!(
                    "{}. {}\n@@workflow\n{}\n@@end",
                    examples.len() + 1,
                    wf.name,
                    format_function_call(&wf.name, ex)
                ));
            }
        }
    }
    
    // Output schema examples
    if let Some(output) = &ir.output {
        if let Some(output_examples) = output.get("@examples").and_then(|v| v.as_array()) {
            for ex in output_examples {
                let schema_name = ex.get("__schema_name").and_then(|v| v.as_str()).unwrap_or("Output");
                examples.push(format!(
                    "{}. {} output\n@@out {}\n{}\n@@end",
                    examples.len() + 1,
                    schema_name,
                    schema_name,
                    format_ts_object(ex)
                ));
            }
        }
    }
    
    examples.join("\n\n")
}
```

---

## DSL Syntax for @example

### In .auwgent Files

```auwgent
tool fetch_session(session_id: string) {
  @example {
    session_id = "sess_8f3a92c1"
  }
  
  // implementation
}

workflow compile_context(
  session_id: string,
  apply_compression: boolean
) {
  @example {
    session_id = "sess_123"
    apply_compression = false
  }
  
  @example {
    session_id = "sess_456"
    apply_compression = true
  }
  
  // implementation
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
    question = "Are you sure?"
    options = ["yes", "no"]
  }
}
```

---

## IR Structure Changes

### Add examples field to IR types

```rust
// ir-runtime/src/types.rs

pub struct Tool {
    pub name: String,
    pub description: Option<String>,
    pub params: Value,
    pub returns: Value,
    pub examples: Option<Vec<Value>>, // NEW
}

pub struct Workflow {
    pub name: String,
    pub params: Value,
    pub returns: Value,
    pub description: Option<String>,
    pub body: Vec<Expression>,
    pub tools: Vec<Tool>,
    pub examples: Option<Vec<Value>>, // NEW
}

pub struct CustomIntentDef {
    pub name: String,
    pub description: Option<String>,
    pub fields: Value,
    pub examples: Option<Vec<Value>>, // NEW
}

// For output schemas, examples are stored in the output Value itself
// as a special key: "@examples": [...]
```

---

## Prompt Generation Implementation

### New Function: generate_block_protocol_prompt()

```rust
// ir-runtime/src/intents.rs

pub fn generate_block_protocol_prompt(ir: &AgentIR) -> String {
    let mut sections = Vec::new();
    
    // ═══ HEADER ═══
    sections.push(
        "You are an execution engine. You communicate EXCLUSIVELY using the `@@` block protocol.\n\n\
         # AVAILABLE BLOCKS\n\n\
         You may ONLY use the following block types:".to_string()
    );
    
    // ═══ @@chat BLOCK ═══
    sections.push(
        "\n@@chat\n\
         Use this to speak to the user, explain your actions, or think out loud.\n\
         @@end".to_string()
    );
    
    // ═══ @@tool BLOCK ═══
    if !ir.tools.is_empty() {
        let mut tool_section = String::from("\n@@tool\nUse this to execute parallel tools.\nAvailable tools and their exact arguments:\n");
        
        for tool in &ir.tools {
            let params = format_params_signature(&tool.params);
            tool_section.push_str(&format!("- {}({})", tool.name, params));
            if let Some(desc) = &tool.description {
                tool_section.push_str(&format!(" // {}", desc));
            }
            tool_section.push('\n');
        }
        
        // Collect examples from tools that have them
        let example_calls = collect_tool_example_calls(&ir.tools);
        if !example_calls.is_empty() {
            tool_section.push_str("\nExample:\n@@tool\n");
            tool_section.push_str(&example_calls.join("\n"));
            tool_section.push_str("\n@@end");
        }
        
        sections.push(tool_section);
    }
    
    // ═══ @@workflow BLOCK ═══
    if !ir.workflows.is_empty() {
        let mut wf_section = String::from("\n@@workflow\nUse this to execute a single, sequential backend workflow.\nAvailable workflows:\n");
        
        for wf in &ir.workflows {
            let params = format_params_signature(&wf.params);
            wf_section.push_str(&format!("- {}({})", wf.name, params));
            if let Some(desc) = &wf.description {
                wf_section.push_str(&format!(" // {}", desc));
            }
            wf_section.push('\n');
        }
        
        // Collect examples
        let example_calls = collect_workflow_example_calls(&ir.workflows);
        if !example_calls.is_empty() {
            wf_section.push_str("\nExample:\n@@workflow\n");
            wf_section.push_str(&example_calls[0]); // Only one workflow per block
            wf_section.push_str("\n@@end");
        }
        
        sections.push(wf_section);
    }
    
    // ═══ @@out BLOCK ═══
    if let Some(output) = &ir.output {
        if let Some(obj) = output.as_object() {t
            if !obj.is_empty() {
                let mut schema_section = String::from("\n@@out [SchemaName]\nUse this to return structured data to the system.\nAvailable schemas and their exact shapes:\n\n");
                
                // Format schema structure (TypeScript-style)
                schema_section.push_str(&format_output_schema_ts_style(output, ir));
                
                // Add examples
                if let Some(examples) = output.get("@examples").and_then(|v| v.as_array()) {
                    if !examples.is_empty() {
                        let ex = &examples[0];
                        let schema_name = ex.get("__schema_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Output");
                        
                        schema_section.push_str(&format!("\nExample:\n@@out {}\n", schema_name));
                        schema_section.push_str(&format_ts_object_for_example(ex));
                        schema_section.push_str("\n@@end");
                    }
                }
                
                sections.push(schema_section);
            }
        }
    }
    
    // ═══ @@helper BLOCK ═══
    if !ir.helpers.is_empty() {
        let mut helper_section = String::from("\n@@helper\nUse this to delegate to a specialized sub-agent.\nAvailable helpers:\n");
        
        for helper in &ir.helpers {
            let params = format_helper_params(&helper.input);
            helper_section.push_str(&format!("- {}({})", helper.name, params));
            if let Some(desc) = &helper.description {
                helper_section.push_str(&format!(" // {}", desc));
            }
            helper_section.push('\n');
        }
        
        // Add examples
        let example_calls = collect_helper_example_calls(&ir.helpers);
        if !example_calls.is_empty() {
            helper_section.push_str("\nExample:\n@@helper\n");
            helper_section.push_str(&example_calls[0]); // Only one helper per block
            helper_section.push_str("\n@@end");
        }
        
        sections.push(helper_section);
    }
    
    // ═══ CUSTOM INTENTS ═══
    if let Some(custom) = &ir.custom_intents {
        for ci in custom {
            let mut custom_section = format!("\n@@{}\n", ci.name);
            if let Some(desc) = &ci.description {
                custom_section.push_str(&format!("{}\n", desc));
            }
            
            // Add examples
            if let Some(examples) = &ci.examples {
                if !examples.is_empty() {
                    custom_section.push_str(&format!("\nExample:\n@@{}\n", ci.name));
                    custom_section.push_str(&format_custom_intent_example(&ci.name, &examples[0]));
                    custom_section.push_str("\n@@end");
                }
            }
            
            sections.push(custom_section);
        }
    }
    
    // ═══ CONSTRAINTS ═══
    sections.push(
        "\n# CRITICAL CONSTRAINTS\n\n\
         - NEVER invent tools, workflows, or schemas. Only use what is listed above.\n\
         - Your `@@tool` arguments and `@@out` JSON must STRICTLY match the types and shapes provided.\n\
         - Do not add properties to `@@out` objects that are not defined in the schema shape.\n\
         - You can use multiple blocks in one response (e.g., @@chat then @@tool then @@chat).\n\
         - Blocks auto-close when you start a new block, but using @@end is recommended for clarity.".to_string()
    );
    
    sections.join("")
}

// ═══ HELPER FUNCTIONS ═══

fn format_params_signature(params: &Value) -> String {
    if let Some(obj) = params.as_object() {
        let mut parts = Vec::new();
        for (name, def) in obj {
            let type_str = def.get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("any");
            let optional = def.get("optional")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            
            if optional {
                parts.push(format!("{}?: {}", name, type_str));
            } else {
                parts.push(format!("{}: {}", name, type_str));
            }
        }
        parts.join(", ")
    } else {
        String::new()
    }
}

fn collect_tool_example_calls(tools: &[Tool]) -> Vec<String> {
    let mut calls = Vec::new();
    
    for tool in tools {
        if let Some(examples) = &tool.examples {
            if let Some(first_example) = examples.first() {
                calls.push(format_function_call(&tool.name, first_example));
                break; // Only use first tool's first example for the block example
            }
        }
    }
    
    // If no examples found, create a synthetic one from the first tool
    if calls.is_empty() && !tools.is_empty() {
        let tool = &tools[0];
        calls.push(format!("{}(/* your args */)", tool.name));
    }
    
    calls
}

fn format_function_call(name: &str, example: &Value) -> String {
    if let Some(obj) = example.as_object() {
        let mut args = Vec::new();
        for (key, val) in obj {
            args.push(format!("{} = {}", key, format_value_inline(val)));
        }
        
        if args.is_empty() {
            format!("{}()", name)
        } else if args.len() == 1 {
            format!("{}({})", name, args[0])
        } else {
            // Multi-line format for multiple args
            format!("{}(\n  {}\n)", name, args.join(",\n  "))
        }
    } else {
        format!("{}()", name)
    }
}

fn format_value_inline(val: &Value) -> String {
    match val {
        Value::String(s) => format!("\"{}\"", s),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(format_value_inline).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(obj) => {
            let mut fields = Vec::new();
            for (k, v) in obj {
                fields.push(format!("{} = {}", k, format_value_inline(v)));
            }
            format!("{{ {} }}", fields.join(", "))
        }
    }
}

fn format_ts_object_for_example(val: &Value) -> String {
    if let Some(obj) = val.as_object() {
        let mut lines = Vec::new();
        lines.push("{".to_string());
        
        for (key, value) in obj {
            if key.starts_with("__") {
                continue; // Skip metadata fields
            }
            lines.push(format!("  {}: {},", key, format_value_multiline(value, 2)));
        }
        
        lines.push("}".to_string());
        lines.join("\n")
    } else {
        "{}".to_string()
    }
}

fn format_value_multiline(val: &Value, indent: usize) -> String {
    let indent_str = "  ".repeat(indent);
    
    match val {
        Value::String(s) => format!("\"{}\"", s),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(arr) => {
            if arr.is_empty() {
                "[]".to_string()
            } else {
                let items: Vec<String> = arr.iter()
                    .map(|v| format_value_multiline(v, indent + 1))
                    .collect();
                format!("[\n{}{}\n{}]", indent_str, items.join(&format!(",\n{}", indent_str)), "  ".repeat(indent - 1))
            }
        }
        Value::Object(obj) => {
            if obj.is_empty() {
                "{}".to_string()
            } else {
                let mut fields = Vec::new();
                for (k, v) in obj {
                    fields.push(format!("{}{}: {}", indent_str, k, format_value_multiline(v, indent + 1)));
                }
                format!("{{\n{}\n{}}}", fields.join(",\n"), "  ".repeat(indent - 1))
            }
        }
    }
}
```

---

## Recommendation

**Use Option 1 (Template-Based)** because:

1. **Complete rewrite** - We're migrating from function-style to block protocol anyway
2. **Cleaner structure** - Block-first organization matches the new protocol
3. **Better examples** - Examples are shown in context with each block type
4. **Easier to maintain** - Template structure is more readable
5. **Consistent format** - All blocks follow the same pattern

The hybrid approach (Option 2) would keep the old structure and bolt on examples, which doesn't fully embrace the block protocol.

---

## Implementation Steps

1. **Add `examples` field to IR types** (Tool, Workflow, CustomIntentDef)
2. **Update compiler** to parse `@example` blocks and include in IR JSON
3. **Create `generate_block_protocol_prompt()`** function in intents.rs
4. **Update `generate_main_prompt()`** in engine.rs to call the new function
5. **Add helper functions** for formatting examples (format_function_call, format_ts_object_for_example, etc.)
6. **Test with real agents** to verify examples improve model behavior

---

## Example Output

For an agent with tools and output schema:

```
You are an execution engine. You communicate EXCLUSIVELY using the `@@` block protocol.

# AVAILABLE BLOCKS

You may ONLY use the following block types:

@@chat
Use this to speak to the user, explain your actions, or think out loud.
@@end

@@tool
Use this to execute parallel tools.
Available tools and their exact arguments:
- fetch_session(session_id: string) // Fetches session data
- get_user(user_id: string, include_history?: boolean) // Gets user profile

Example:
@@tool
fetch_session(session_id = "sess_8f3a92c1")
get_user(user_id = "usr_456", include_history = true)
@@end

@@out [SchemaName]
Use this to return structured data to the system.
Available schemas and their exact shapes:

ContextCompilerOutput {
  session_id: string;
  user: {
    id: string;
    name: string;
  };
}

Example:
@@out ContextCompilerOutput
{
  session_id: "sess_123",
  user: {
    id: "usr_456",
    name: "Nana"
  }
}
@@end

# CRITICAL CONSTRAINTS

- NEVER invent tools, workflows, or schemas. Only use what is listed above.
- Your `@@tool` arguments and `@@out` JSON must STRICTLY match the types and shapes provided.
- Do not add properties to `@@out` objects that are not defined in the schema shape.
- You can use multiple blocks in one response (e.g., @@chat then @@tool then @@chat).
- Blocks auto-close when you start a new block, but using @@end is recommended for clarity.
```

---

## Key Design Decisions

### 1. Example Placement

**Decision**: Inline with each block type section

**Rationale**: Keeps examples close to the documentation they illustrate

### 2. Example Format

**Decision**: Show complete `@@block ... @@end` syntax

**Rationale**: Teaches the model the full protocol, not just the function call

### 3. Multiple Examples

**Decision**: Show one representative example per block type in the main section

**Rationale**: Keeps prompt concise while still providing guidance. Individual tool/workflow examples can be added in their detailed sections if needed.

### 4. Example Selection

**Decision**: Use the first example from the first tool/workflow that has one

**Rationale**: Simple, predictable, and sufficient for teaching the pattern

---

## Future Enhancements

1. **Smart example selection**: Choose the most representative example based on complexity
2. **Example validation**: Validate examples against schemas at compile time
3. **Example composition**: Show multi-block examples (chat + tool + chat)
4. **Conditional examples**: Show different examples based on context
5. **Example descriptions**: Add optional descriptions to examples for clarity
