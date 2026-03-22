use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

// --- Top Level ---

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CustomIntentDef {
    pub name: String,
    pub description: Option<String>,
    pub fields: Value, // Object of field definitions
    #[serde(default)]
    pub examples: Vec<HashMap<String, Value>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentIR {
    pub name: String,
    pub model_config: Vec<ModelConfigEntry>,
    pub input: Option<Value>,
    pub output: Option<Value>,
    pub context: Option<Value>,
    pub tools: Vec<Tool>,
    pub workflows: Vec<Workflow>,
    pub helpers: Vec<Helper>,
    #[serde(default)]
    pub types: Option<HashMap<String, TypeDefinition>>,
    #[serde(default)]
    pub helper_tool_grants: Option<HashMap<String, Value>>,
    #[serde(default)]
    pub helper_handoff: Option<HashMap<String, String>>,
    #[serde(default)]
    pub tests: Vec<Value>,
    /// Lifecycle configuration (maxTokens, maxMessages)
    #[serde(default)]
    pub lifecycle: Option<Value>,
    #[serde(default)]
    pub custom_intents: Option<Vec<CustomIntentDef>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Helper {
    pub name: String,
    pub description: Option<String>,
    pub model_config: Vec<ModelConfigEntry>,
    pub input: Option<Value>,
    pub output: Option<Value>,
    pub context: Option<Value>,
    pub tools: Vec<Tool>,
    pub workflows: Vec<Workflow>,
    #[serde(default)]
    pub custom_intents: Option<Vec<CustomIntentDef>>,
    #[serde(default)]
    pub examples: Vec<HashMap<String, Value>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Workflow {
    #[serde(rename = "flowName")]
    pub name: String,
    #[serde(rename = "flowParams")]
    pub params: Value, // Object of parameter definitions
    pub returns: Value,
    pub description: Option<String>,
    pub body: Vec<Expression>,
    #[serde(default)]
    pub tools: Vec<Tool>,
    #[serde(default)]
    pub examples: Vec<HashMap<String, Value>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Tool {
    pub name: String,
    pub description: Option<String>,
    pub params: Value, // Object of field definitions
    pub returns: Value,
    #[serde(default)]
    pub examples: Vec<HashMap<String, Value>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TypeDefinition {
    pub is_output: bool,
    pub properties: HashMap<String, TypeProperty>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TypeProperty {
    #[serde(rename = "type")]
    pub type_value: Value,
    pub optional: bool,
    pub description: Option<String>,
}

// --- Model Configuration ---

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModelConfigEntry {
    #[serde(rename = "defaultConfig")]
    pub default_config: Option<ModelConfig>,
    #[serde(rename = "namedConfig")]
    pub named_config: Option<Vec<NamedModelConfig>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NamedModelConfig {
    #[serde(rename = "configName")]
    pub config_name: String,
    #[serde(flatten)]
    pub config: ModelConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModelConfig {
    pub model: ModelProvider,
    pub embedding: Option<ModelProvider>,
    pub prompt: Expression,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ModelProvider {
    Gemini {
        #[serde(rename = "modelName")]
        model_name: String,
        config: Option<Box<Expression>>,
    },
    #[serde(rename = "openai")]
    OpenAI {
        #[serde(rename = "modelName")]
        model_name: String,
        config: Option<Box<Expression>>,
    },
    Custom {
        id:String,
        url: String,
        #[serde(rename = "modelName")]
        model_name: String,
        config: Option<Box<Expression>>,
    },
    ModelRef {
        name: String,
    },
}

// --- Expressions (The Core Logic) ---

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Expression {
    // Basic Literals
    Literal {
        value: Value, // string, number, boolean
    },

    // Variables & Access
    VarRef {
        value: String,
    },
    MemberAccess {
        object: Box<Expression>,
        properties: Vec<String>,
    },
    Parts {
        value: Vec<Expression>,
    },
    Template {
        value: Vec<Expression>,
    },
    Object {
        value: HashMap<String, Expression>,
    },

    // Conditional Logic
    InlineIf {
        condition: Comparison,
        then: Vec<Expression>,
        #[serde(rename = "else", default)]
        else_block: Vec<Expression>,
    },

    // Statements
    If {
        condition: Condition,
        then: Vec<Expression>,
        #[serde(rename = "else", default)]
        else_block: Vec<Expression>,
    },
    Return {
        value: Box<Expression>,
    },

    // References
    ContextRef {
        property: String,
    },
    SchemaDirective {
        path: String,
    },
    PromptRef {
        name: String,
        params: Vec<String>,
        args: Vec<Expression>,
        value: Vec<Expression>,
    },
    BinaryOp {
        left: Box<Expression>,
        op: String,
        right: Box<Expression>,
    },
    InlinePrompt {
        parts: Vec<Expression>,
    },
    PromptExamples {
        examples: Vec<ExamplePair>,
    },

    VariableDeclaration {
        name: String,
        value: Box<Expression>,
    },
    FunctionCall {
        value: String,
        args: Vec<Expression>,
    },
    HelperCall {
        value: String,
        args: Vec<Expression>,
    },
    Transfer {
        target: Box<Expression>,
        mode: String,
    },
    Parallel {
        body: Vec<Expression>,
    },

    Expression {
        value: Box<Expression>,
    },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Condition {
    Comparison(Comparison),
    Boolean { value: Box<Expression> },
    ContextRef { property: String },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Comparison {
    pub left: Box<Expression>,
    pub operator: String,
    pub right: Box<Expression>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ExamplePair {
    pub user: String,
    pub assistant: String,
}
