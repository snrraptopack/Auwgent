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
  "expect"
  "error"
  "returns"
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
  "fields"
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
(agent_declaration name: (identifier) @type)
(helper_declaration name: (identifier) @type)
(type_declaration name: (identifier) @type)
(named_prompt_declaration name: (identifier) @function)
(workflow_config name: (identifier) @function)
(tool_function name: (identifier) @function)

; Variables and Properties
(let_statement name: (identifier) @variable)
(parameter name: (identifier) @variable.parameter)
(property_name) @property
(identifier) @variable

; Specific built-ins
"ctx" @variable.builtin
"hlp" @variable.builtin
