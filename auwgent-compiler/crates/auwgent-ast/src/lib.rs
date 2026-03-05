//! # auwgent-ast
//!
//! AST node definitions for the Auwgent DSL.
//! These map 1:1 to the Langium grammar rules in `auwgent.langium`.
//! Every variant carries a `Span` so we can report precise error locations.

use auwgent_errors::Span;

// ── Top-Level ────────────────────────────────────────────────────────────

/// Root of a parsed `.agent` file.
#[derive(Debug, Clone)]
pub struct Model {
    pub imports: Vec<FileImport>,
    pub elements: Vec<Element>,
}

#[derive(Debug, Clone)]
pub struct FileImport {
    pub kind: ImportShape,
    pub path: Spanned<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ImportShape {
    Named(Vec<ImportSpecifier>),
    Wildcard { namespace: Spanned<String> },
}

#[derive(Debug, Clone)]
pub struct ImportSpecifier {
    pub kind: Option<ImportKind>,
    pub name: Spanned<String>,
    pub alias: Option<Spanned<String>>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportKind {
    Helper,
    Type,
    Prompt,
    Model,
}

#[derive(Debug, Clone)]
pub enum Element {
    Agent(Agent),
    Helper(Helper),
    TypeDecl(TypeDeclaration),
    NamedPrompt(NamedPrompt),
    ModelDef(ModelDefinition),
}

// ── Spanned Helper ───────────────────────────────────────────────────────

/// A value with its source span.
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub value: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(value: T, span: Span) -> Self {
        Self { value, span }
    }
}

// ── Agent / Helper ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Agent {
    pub name: Spanned<String>,
    pub configs: Vec<AgentConfig>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Helper {
    pub exported: bool,
    pub name: Spanned<String>,
    pub description: Spanned<String>,
    pub configs: Vec<AgentConfig>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum AgentConfig {
    Input(InputConfig),
    Output(OutputConfig),
    Context(ContextConfig),
    Tool(ToolFunction),
    Tools(Vec<ToolFunction>),
    Model(AgentModelConfig),
    Workflow(WorkflowConfig),
    Helpers(HelpersConfig),
    Lifecycle(LifecycleConfig),
    Test(TestConfig),
}

// ── Input / Output / Context ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct InputConfig {
    pub properties: Vec<TypeConfigDecl>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct OutputConfig {
    pub shape: OutputShape,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum OutputShape {
    /// `output { name: string, age: number }`
    Properties(Vec<OutputProperty>),
    /// `output: TypeA | TypeB`
    Union(Vec<Spanned<String>>),
    /// `output: string @desc "..."`
    Direct {
        ty: TypeExpr,
        desc: Option<Spanned<String>>,
    },
}

#[derive(Debug, Clone)]
pub struct OutputProperty {
    pub decl: TypeConfigDecl,
    pub description: Option<Spanned<String>>,
}

#[derive(Debug, Clone)]
pub struct ContextConfig {
    pub properties: Vec<TypeConfigDecl>,
    pub span: Span,
}

// ── Tools ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ToolFunction {
    pub name: Spanned<String>,
    pub params: Vec<TypeConfigDecl>,
    pub returns: TypeExpr,
    pub description: Vec<Spanned<String>>,
    pub span: Span,
}

// ── Workflow ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct WorkflowConfig {
    pub name: Spanned<String>,
    pub params: Vec<TypeConfigDecl>,
    pub return_type: TypeExpr,
    pub description: Spanned<String>,
    pub tool_configs: Vec<ToolFunction>,
    pub body: Vec<Statement>,
    pub span: Span,
}

// ── Helpers Config ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HelpersConfig {
    pub helpers: Vec<HelperRef>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HelperRef {
    pub name: Spanned<String>,
    pub with_all_tools: bool,
    pub granted_tools: Vec<Spanned<String>>,
    pub handoff_user: bool,
    pub handoff_then_continue: bool,
    pub span: Span,
}

// ── Model Config ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AgentModelConfig {
    pub default_config: ModelConfig,
    pub named_configs: Vec<NamedModelConfig>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct NamedModelConfig {
    pub name: Spanned<String>,
    pub config: ModelConfig,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub model: ModelProviderRef,
    pub prompt_block: Vec<PromptStatement>,
    pub prompt_expr: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ModelProviderRef {
    Inline(ModelProvider),
    Ref(Spanned<String>),
}

#[derive(Debug, Clone)]
pub enum ModelProvider {
    Gemini {
        model_name: Spanned<String>,
        config: Option<ObjectLiteral>,
        span: Span,
    },
    OpenAI {
        model_name: Spanned<String>,
        config: Option<ObjectLiteral>,
        span: Span,
    },
    Custom {
        url: Spanned<String>,
        model_name: Spanned<String>,
        config: Option<ObjectLiteral>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct ModelDefinition {
    pub exported: bool,
    pub name: Spanned<String>,
    pub provider: ModelProvider,
    pub span: Span,
}

// ── Lifecycle ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LifecycleConfig {
    pub max_tokens: Option<Spanned<i64>>,
    pub max_messages: Option<Spanned<i64>>,
    pub span: Span,
}

// ── Prompts ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct NamedPrompt {
    pub exported: bool,
    pub name: Spanned<String>,
    pub params: Vec<TypeConfigDecl>,
    pub body: Vec<PromptStatement>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum PromptStatement {
    Expr(Expr),
    Example(ExampleBlock),
    If(IfStatement),
}

#[derive(Debug, Clone)]
pub struct ExampleBlock {
    pub messages: Vec<ExampleMessage>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ExampleMessage {
    pub role: Spanned<String>, // "user" | "assistant"
    pub text: Spanned<String>,
    pub span: Span,
}

// ── Type Declarations ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TypeDeclaration {
    pub exported: bool,
    pub is_output: bool,
    pub name: Spanned<String>,
    pub fields: Vec<TypeConfigDecl>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeConfigDecl {
    pub name: Spanned<String>,
    pub optional: bool,
    pub ty: TypeExpr,
    pub description: Option<Spanned<String>>,
    pub span: Span,
}

// ── Type Expressions ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum TypeExpr {
    String(Span),
    Number(Span),
    Boolean(Span),
    Array {
        element: Box<TypeExpr>,
        span: Span,
    },
    Object {
        properties: Vec<PropertyType>,
        span: Span,
    },
    TypeRef(Spanned<String>),
    Union {
        options: Vec<Spanned<String>>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct PropertyType {
    pub name: Spanned<String>,
    pub optional: bool,
    pub ty: TypeExpr,
    pub description: Option<Spanned<String>>,
    pub span: Span,
}

// ── Statements ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Statement {
    Let(LetStatement),
    Assign(AssignStatement),
    Return(ReturnStatement),
    If(IfStatement),
    Transfer(TransferStatement),
    Parallel(ParallelStatement),
}

#[derive(Debug, Clone)]
pub struct LetStatement {
    pub name: Spanned<String>,
    pub ty: Option<TypeExpr>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AssignStatement {
    pub variable: Spanned<String>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ReturnStatement {
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct IfStatement {
    pub condition: Condition,
    pub then_block: Vec<Statement>,
    pub else_block: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TransferStatement {
    pub call: HelperCall,
    pub then_continue: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ParallelStatement {
    pub body: Vec<Statement>,
    pub span: Span,
}

// ── Conditions ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Condition {
    Comparison {
        left: Expr,
        op: ComparisonOp,
        right: Expr,
        span: Span,
    },
    Logical {
        left: Box<Condition>,
        op: LogicalOp,
        right: Box<Condition>,
        span: Span,
    },
    Boolean {
        value: Expr,
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOp {
    Eq,  // ==
    Neq, // !=
    Gt,  // >
    Lt,  // <
    Gte, // >=
    Lte, // <=
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOp {
    And, // &&
    Or,  // ||
}

// ── Expressions ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Expr {
    StringLit(Spanned<String>),
    MultilineStringLit(Spanned<String>),
    NumberLit(Spanned<f64>),
    BooleanLit(Spanned<bool>),
    Array(ArrayLiteral),
    Object(ObjectLiteral),
    VarRef(Spanned<String>),
    MemberAccess(MemberAccess),
    IndexAccess(IndexAccess),
    BinaryOp(BinaryOp),
    FunctionCall(FunctionCall),
    HelperCall(HelperCall),
    PromptCall(PromptCall),
    ContextRef(ContextRef),
    InlinePrompt(InlinePromptBlock),
    Grouped(Box<Expr>, Span),
}

#[derive(Debug, Clone)]
pub struct ArrayLiteral {
    pub elements: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ObjectLiteral {
    pub properties: Vec<PropertyValue>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct PropertyValue {
    pub name: Spanned<String>,
    pub value: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MemberAccess {
    pub object: Spanned<String>,
    pub property: Spanned<String>,
    /// Additional chained properties: `a.b.c.d` → chain = [c, d]
    pub chain: Vec<Spanned<String>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct IndexAccess {
    pub object: Spanned<String>,
    pub index: Box<Expr>,
    pub property: Option<Spanned<String>>,
    pub chain: Vec<Spanned<String>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct BinaryOp {
    pub left: Box<Expr>,
    pub op: BinOperator,
    pub right: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOperator {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone)]
pub struct FunctionCall {
    pub name: Spanned<String>,
    pub args: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HelperCall {
    pub helper: Spanned<String>,
    pub args: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct PromptCall {
    pub prompt: Spanned<String>,
    pub args: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ContextRef {
    pub property: Spanned<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct InlinePromptBlock {
    pub parts: Vec<PromptStatement>,
    pub span: Span,
}

// ── Test Config ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TestConfig {
    pub name: Spanned<String>,
    pub config_name: Option<Spanned<String>>,
    pub input: Option<ObjectLiteral>,
    pub tool_stubs: Vec<ToolStub>,
    pub expectations: Vec<TestExpectation>,
    pub model: Option<TestModel>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ToolStub {
    pub name: Spanned<String>,
    pub value: ToolStubValue,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ToolStubValue {
    Returns(Expr),
    Error(Spanned<String>),
}

#[derive(Debug, Clone)]
pub enum TestExpectation {
    Output {
        path: Vec<Spanned<String>>,
        value: Expr,
        span: Span,
    },
    ToolError {
        error: Spanned<String>,
        span: Span,
    },
    PromptContains {
        contains: Spanned<String>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct TestModel {
    pub tool_calls: Vec<TestToolCall>,
    pub final_text: Option<Spanned<String>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TestToolCall {
    pub name: Spanned<String>,
    pub args: Vec<PropertyValue>,
    pub span: Span,
}
