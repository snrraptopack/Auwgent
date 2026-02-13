use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Token types emitted by the tokenizer
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenType {
    Key,     // Identifier before colon
    Colon,   // :
    Dash,    // - (sequence item)
    Scalar,  // Unquoted value
    Quoted,  // "value" or 'value'
    Indent,  // Indentation increase
    Dedent,  // Indentation decrease
    Newline, // Line terminator
    Comment, // # comment (usually stripped)
    Eof,     // End of input
}

/// A single token from the tokenizer
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenType,
    pub value: String,
    pub line: usize,
    pub column: usize,
    /// Indent level (number of 2-space units)
    pub indent: usize,
}

/// Position tracking for error messages
#[derive(Debug, Clone, Serialize)]
pub struct Position {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

// ═══════════════════════════════════════════════════════════════════════════
// AST TYPES (Parser Output)
// ═══════════════════════════════════════════════════════════════════════════

/// Base AST node with position info
pub trait ASTNodeBase {
    fn line(&self) -> usize;
    fn column(&self) -> usize;
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ASTNode {
    Scalar(ScalarNode),
    Mapping(MappingNode),
    Sequence(SequenceNode),
    Ref(RefNode),
    Empty(EmptyNode),
}

impl ASTNodeBase for ASTNode {
    fn line(&self) -> usize {
        match self {
            ASTNode::Scalar(n) => n.line,
            ASTNode::Mapping(n) => n.line,
            ASTNode::Sequence(n) => n.line,
            ASTNode::Ref(n) => n.line,
            ASTNode::Empty(n) => n.line,
        }
    }
    fn column(&self) -> usize {
        match self {
            ASTNode::Scalar(n) => n.column,
            ASTNode::Mapping(n) => n.column,
            ASTNode::Sequence(n) => n.column,
            ASTNode::Ref(n) => n.column,
            ASTNode::Empty(n) => n.column,
        }
    }
}

/// Scalar value node (all values are strings at AST level)
#[derive(Debug, Clone, Serialize)]
pub struct ScalarNode {
    pub kind: String, // "scalar"
    pub value: String,
    /// Whether the value was quoted in source
    pub quoted: bool,
    pub line: usize,
    pub column: usize,
}

/// Mapping (object) node
#[derive(Debug, Clone, Serialize)]
pub struct MappingNode {
    pub kind: String, // "mapping"
    pub entries: Vec<MappingEntry>,
    pub line: usize,
    pub column: usize,
}

/// A single key-value pair in a mapping
#[derive(Debug, Clone, Serialize)]
pub struct MappingEntry {
    pub key: String,
    pub value: ASTNode,
    pub line: usize,
    pub column: usize,
}

/// Sequence (array) node
#[derive(Debug, Clone, Serialize)]
pub struct SequenceNode {
    pub kind: String, // "sequence"
    pub items: Vec<ASTNode>,
    pub line: usize,
    pub column: usize,
}

/// Reference node (ref: some_id)
#[derive(Debug, Clone, Serialize)]
pub struct RefNode {
    pub kind: String, // "ref"
    pub target: String,
    pub line: usize,
    pub column: usize,
}

/// Empty block node (auto-initialized)
#[derive(Debug, Clone, Serialize)]
pub struct EmptyNode {
    pub kind: String, // "empty"
    /// Hint: 'mapping' or 'sequence' based on context
    pub hint: Option<String>,
    pub line: usize,
    pub column: usize,
}

/// Parse result containing the AST and any errors
#[derive(Debug, Clone)]
pub struct ParseResult {
    pub ast: Option<ASTNode>,
    pub errors: Vec<ParseError>,
    /// Whether the document was complete (vs partial)
    pub complete: bool,
}

// ═══════════════════════════════════════════════════════════════════════════
// IR TYPES (After Type Coercion)
// ═══════════════════════════════════════════════════════════════════════════

/// JSON-compatible output types
#[derive(Debug, Clone)]
pub enum IRValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
    Object(HashMap<String, IRValue>),
    Array(Vec<IRValue>),
    Ref(IRRef),
}

impl IRValue {
    pub fn into_json(self) -> Value {
        match self {
            IRValue::String(s) => Value::String(s),
            IRValue::Number(n) => {
                if let Some(num) = serde_json::Number::from_f64(n) {
                    Value::Number(num)
                } else {
                    Value::Null
                }
            }
            IRValue::Boolean(b) => Value::Bool(b),
            IRValue::Null => Value::Null,
            IRValue::Object(map) => {
                let mut obj = serde_json::Map::new();
                for (k, v) in map {
                    obj.insert(k, v.into_json());
                }
                Value::Object(obj)
            }
            IRValue::Array(items) => {
                Value::Array(items.into_iter().map(|v| v.into_json()).collect())
            }
            IRValue::Ref(r) => {
                let mut obj = serde_json::Map::new();
                obj.insert("ref".to_string(), Value::String(r.reference));
                Value::Object(obj)
            }
        }
    }
}

/// Reference placeholder in IR (resolved later or kept as-is)
#[derive(Debug, Clone)]
pub struct IRRef {
    pub reference: String, // renamed from $ref because reserved syntax
}

/// IR build result
#[derive(Debug, Clone)]
pub struct IRResult {
    pub value: IRValue,
    /// All nodes with id fields, keyed by id
    pub registry: HashMap<String, IRValue>,
    /// Unresolved references
    pub unresolved_refs: Vec<String>,
    pub errors: Vec<IRError>,
}

// ═══════════════════════════════════════════════════════════════════════════
// ERROR TYPES
// ═══════════════════════════════════════════════════════════════════════════

/// Severity levels for errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorSeverity {
    Error,
    Warning,
    Info,
}

/// Parse-time error
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub severity: ErrorSeverity,
    pub line: usize,
    pub column: usize,
    /// Source context (the problematic line)
    pub context: Option<String>,
}

/// IR building error
#[derive(Debug, Clone)]
pub struct IRError {
    pub message: String,
    pub severity: ErrorSeverity,
    pub path: Vec<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// PARSER EVENTS (Middleware Hooks)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ParserEventType {
    Line,        // New line started
    Indent,      // Indentation increased
    Dedent,      // Indentation decreased
    Key,         // Key parsed
    Value,       // Value parsed
    BlockStart,  // Block (mapping/sequence) started
    BlockEnd,    // Block ended
    IntentReady, // Intent block is complete and executable
}

/// Parser event for middleware
#[derive(Debug, Clone)]
pub struct ParserEvent {
    pub event_type: ParserEventType,
    pub data: Value,
    pub position: Position,
}

/// Middleware function type
pub type ParserMiddleware = Arc<dyn Fn(ParserEvent) + Send + Sync>;

// ═══════════════════════════════════════════════════════════════════════════
// PARSER OPTIONS
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone)]
pub struct ParserOptions {
    /// Number of spaces per indent level (default: 2)
    pub indent_size: Option<usize>,

    /// Allow tabs (default: false)
    pub allow_tabs: Option<bool>,

    /// Emit comments as tokens (default: false)
    pub preserve_comments: Option<bool>,

    /// Enable strict mode (fail on any warning)
    pub strict: Option<bool>,

    /// Schema for intent validation
    pub intent_schema: Option<IntentSchema>,

    /// The key name(s) that trigger the `intent_ready` event.
    /// Can be a single string or array of strings.
    /// Default: "intent"
    pub intent_key: Option<Vec<String>>,

    /// Middleware functions
    pub middleware: Option<Vec<ParserMiddleware>>,
}

impl std::fmt::Debug for ParserOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParserOptions")
            .field("indent_size", &self.indent_size)
            .field("allow_tabs", &self.allow_tabs)
            .field("preserve_comments", &self.preserve_comments)
            .field("strict", &self.strict)
            .field("intent_schema", &self.intent_schema)
            .field("intent_key", &self.intent_key)
            .field(
                "middleware_count",
                &self.middleware.as_ref().map(|m| m.len()),
            )
            .finish()
    }
}

/// Schema for validating intent blocks
#[derive(Debug, Clone)]
pub struct IntentSchema {
    /// Required keys for the intent to be executable
    pub required_keys: Vec<String>,

    /// Known intent types
    pub known_types: Option<Vec<String>>,
}

// ═══════════════════════════════════════════════════════════════════════════
// STREAMING API TRAIT
// ═══════════════════════════════════════════════════════════════════════════

/// Streaming parser trait
pub trait StreamingParser<TIntent, TDoc, TPayload> {
    /// Write a chunk of input
    fn write(&mut self, chunk: &str);

    /// Signal end of input, returns final result
    fn end(&mut self) -> TDoc;

    /// Get current partial result (without ending)
    fn peek(&self) -> TDoc;

    /// Reset parser state
    fn reset(&mut self);

    // Callbacks would likely be implemented via struct members (Fn/FnMut traits) rather than
    // trait methods for registration, or using channels/events.
    // Simplifying here to just show intent.
}
