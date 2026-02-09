use serde::{Deserialize, Serialize};
use serde_json::Value;

// --- Top Level ---

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentIR {
    pub name: String,
    pub model_config: Vec<ModelConfigEntry>,
    pub input: Option<Value>,
    pub output: Option<Value>,
    pub context: Option<Value>,
    pub tools: Vec<Tool>,
    pub workflows: Vec<Value>,
    pub helpers: Vec<Value>,
    #[serde(default)]
    pub tests: Vec<Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Tool {
    pub name: String,
    pub description: Option<String>,
    pub params: Value, // Object of field definitions
    pub returns: Value,
}

// --- Model Configuration ---

#[derive(Debug, Deserialize, Serialize)]
pub struct ModelConfigEntry {
    #[serde(rename = "defaultConfig")]
    pub default_config: Option<ModelConfig>,
    #[serde(rename = "namedConfig")]
    pub named_config: Option<Vec<NamedModelConfig>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NamedModelConfig {
    #[serde(rename = "configName")]
    pub config_name: String,
    #[serde(flatten)]
    pub config: ModelConfig,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ModelConfig {
    pub model: ModelProvider,
    pub prompt: Expression,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ModelProvider {
    Gemini {
        #[serde(rename = "modelName")]
        model_name: String,
        config: Option<Value>,
    },
    // We can add OpenAI or Custom later when they appear in your JSON
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

    Expression {
        value: Box<Expression>,
    },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Condition {
    Comparison(Comparison),
    Boolean { value: Box<Expression> },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Comparison {
    pub left: Box<Expression>,
    pub operator: String,
    pub right: Box<Expression>,
}
