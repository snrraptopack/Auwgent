use thiserror::Error;

/// Typed error hierarchy for the Auwgent runtime.
/// This enables FFI consumers to pattern-match on specific error categories
/// instead of dealing with opaque `Box<dyn Error>`.
#[derive(Debug, Error)]
pub enum AuwgentError {
    // ── IR / Config Errors ──────────────────────────────────────────────
    #[error("IR parse error: {0}")]
    IrParse(String),

    #[error("Missing configuration: {0}")]
    MissingConfig(String),

    // ── Evaluation Errors ───────────────────────────────────────────────
    #[error("Evaluation error: {0}")]
    Evaluation(String),

    #[error("Variable not found: {0}")]
    VariableNotFound(String),

    #[error(
        "Property '{property}' not found on {context}. If this is an agent context field, provide it in config.context or set_context before running."
    )]
    PropertyNotFound { property: String, context: String },

    #[error("Unsupported operator: {0}")]
    UnsupportedOperator(String),

    #[error("Unknown function or tool: {0}")]
    UnknownFunction(String),

    #[error("Unknown helper: {0}")]
    UnknownHelper(String),

    // ── Driver / LLM Errors ─────────────────────────────────────────────
    #[error("Driver error: {0}")]
    Driver(String),

    #[error("LLM stream error: {0}")]
    StreamError(String),

    // ── Tool Execution Errors ───────────────────────────────────────────
    #[error("Tool execution error [{tool_name}]: {message}")]
    ToolExecution { tool_name: String, message: String },

    #[error("Tool not registered: {0}")]
    ToolNotFound(String),

    // ── Engine Loop Errors ──────────────────────────────────────────────
    #[error("Max agentic loop iterations ({0}) exceeded")]
    MaxLoopsExceeded(usize),

    #[error("No driver configured for engine")]
    NoDriver,

    // ── Serialization ───────────────────────────────────────────────────
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Convenience type alias used throughout the codebase.
pub type AuwgentResult<T> = Result<T, AuwgentError>;
