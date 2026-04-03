; Keywords
[
  "agent"
  "helper"
  "tool"
  "tools"
  "workflow"
  "type"
  "import"
  "export"
  "from"
  "as"
  "prompt"
  "model"
  "embedding"
  "config"
  "default"
  "input"
  "output"
  "context"
  "helpers"
  "let"
  "return"
  "if"
  "else"
  "true"
  "false"
  "transfer"
  "to"
  "then"
  "continue"
  "parallel"
  "example"
  "user"
  "assistant"
  "test"
  "error"
  "with"
  "all"
  "handoff"
  "use"
  "lifecycle"
  "provider"
  "gemini"
  "openai"
  "custom"
  "intent"
  "component"
  "fields"
  "action"
  "children"
] @keyword

; Built-in types
[
  "string"
  "number"
  "boolean"
  "Text"
] @type.builtin

; Operators
[
  "="
  "=="
  "!="
  ">="
  "<="
  ">"
  "<"
  "&&"
  "||"
  "+"
  "-"
  "*"
  "/"
] @operator

; Punctuation
[
  "{"
  "}"
  "("
  ")"
  "["
  "]"
  ":"
  ","
  "."
  "?"
  "|"
  "@desc"
] @punctuation

; Roles in examples
[
  "user"
  "assistant"
] @keyword.role

; Literals
(string_literal) @string
(number_literal) @number
(boolean_literal) @boolean

; Comments
(comment) @comment

; Functions and Agents
(agent_declaration (identifier) @type)
(helper_declaration (identifier) @type)
(type_declaration (identifier) @type)
(component_declaration (identifier) @type)
(named_prompt_declaration (identifier) @function)
(workflow_config (identifier) @function)
(tool_function (identifier) @function)
(component_action_binding (identifier) @function.method)

; Variables and Properties
(let_statement (identifier) @variable)
(parameter (identifier) @variable.parameter)
(property_name) @property
(identifier) @variable

; Specific built-ins
"ctx" @variable.builtin
"hlp" @variable.builtin
