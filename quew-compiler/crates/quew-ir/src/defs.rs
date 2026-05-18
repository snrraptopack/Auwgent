//! Definition-section structs.
//!
//! Definitions are the **static** part of the IR. They describe what exists
//! (types, models, tools, functions, agents) but say nothing about execution
//! order. The `graphs` section of `QuewGraphIR` says what runs and when.
//!
//! Analogy: definitions are the cast list. Graphs are the script.

use indexmap::IndexMap;
use quew_interner::InternedStr;

use crate::types::{IrField, IrType};

// ── Definitions (the registry) ────────────────────────────────────────────────

/// The complete static declaration registry for one compiled program.
#[derive(Debug, Clone, Default)]
pub struct Definitions {
    /// Named record/type aliases declared with `type T = { … }`.
    pub types: IndexMap<InternedStr, TypeDef>,

    /// Named and anonymous model declarations.
    /// Anonymous inline calls like `gemini("gemini-pro")` are interned here
    /// under a generated key like `"__anon_0"`.
    pub models: IndexMap<InternedStr, ModelDef>,

    /// All tool declarations: host-backed (`tool`), DSL-defined (`@tool function`),
    /// and group (`tools name { … }`).
    pub tools: IndexMap<InternedStr, ToolDef>,

    /// Non-tool function declarations (`function f() { … }`).
    /// Tool functions appear in `tools` with `ToolKind::Dsl`.
    pub functions: IndexMap<InternedStr, FunctionDef>,

    /// One entry per `agent` declaration.
    pub agents: IndexMap<InternedStr, AgentDef>,

    /// Compiler role bindings declared by builtin types.
    pub roles: IndexMap<IrRoleKey, IrRoleBinding>,
}

// ── Type definitions ──────────────────────────────────────────────────────────

/// A `type T = { … }` declaration lowered to its field map.
#[derive(Debug, Clone)]
pub struct TypeDef {
    /// Generic parameters declared by the type, such as `<T, E>`.
    pub type_params: Vec<InternedStr>,
    /// All fields in declaration order.
    pub fields: IndexMap<InternedStr, IrField>,
    /// Whether this type is ordinary user source or builtin language surface.
    pub visibility: IrTypeVisibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrTypeVisibility {
    User,
    PublicBuiltin,
    InternalBuiltin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IrRoleKey {
    pub keyword: InternedStr,
    pub place: InternedStr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrRoleBinding {
    pub type_name: InternedStr,
}

// ── Model definitions ─────────────────────────────────────────────────────────

/// A model declaration — either named (`model M { … }`) or anonymous
/// (an inline `gemini("model-name")` call).
#[derive(Debug, Clone)]
pub struct ModelDef {
    pub provider: ProviderKind,
    /// The provider-specific model name (e.g. `"gemini-2.0-flash"`).
    pub model_name: InternedStr,
    /// Optional static config fields (`temperature`, `maxTokens`, …).
    /// Values are stored as strings for now; the runtime parses them.
    pub config: IndexMap<InternedStr, String>,
}

/// Which LLM provider backs this model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Gemini,
    OpenAi,
    Groq,
}

// ── Tool definitions ──────────────────────────────────────────────────────────

/// A tool declaration — three distinct kinds.
#[derive(Debug, Clone)]
pub struct ToolDef {
    pub kind: ToolKind,
    /// Human-readable description for the LLM system prompt.
    pub description: Option<InternedStr>,
}

#[derive(Debug, Clone)]
pub enum ToolKind {
    /// `tool name(params): ReturnType` — implementation is in the host.
    /// The runtime calls out via FFI when the model invokes this tool.
    Host {
        /// Parameters the *model* receives and must supply.
        params: IndexMap<InternedStr, ToolParam>,
        returns: IrType,
    },

    /// `@tool function name(params): ReturnType { … }` — implemented in the DSL.
    /// The runtime executes the function body internally; the host never sees it.
    Dsl {
        /// Parameters the *model* receives (non-bound params from the function).
        model_params: IndexMap<InternedStr, ToolParam>,
        /// Parameters pre-bound by the agent call site (`delete_person(ctx.isAdmin)`).
        /// The model never sees these; they are injected at call time.
        host_params: IndexMap<InternedStr, ToolParam>,
        returns: IrType,
        /// Key into `QuewGraphIR::graphs` for this function's body.
        graph_ref: String,
    },

    /// `tools name { … }` — a progressive-disclosure group.
    /// The model receives the group's description first; individual tools
    /// are disclosed lazily when the model requests them.
    Group {
        /// Names of the member tools (must exist in `definitions.tools`).
        members: Vec<InternedStr>,
        /// How the group discloses its members to the model.
        disclosure: DisclosureMode,
    },
}

/// A single parameter in a tool signature.
#[derive(Debug, Clone)]
pub struct ToolParam {
    pub ty: IrType,
    pub optional: bool,
    pub description: Option<InternedStr>,
}

/// Controls how a tool group reveals its members to the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisclosureMode {
    /// All members are visible in the system prompt upfront.
    Eager,
    /// The group description is shown; members are revealed via an internal
    /// `list_group` call the model can make.
    Lazy,
}

// ── Function definitions ──────────────────────────────────────────────────────

/// A non-tool function declaration.
#[derive(Debug, Clone)]
pub struct FunctionDef {
    /// Generic parameters declared by the function, such as `<T>`.
    pub type_params: Vec<InternedStr>,
    pub params: IndexMap<InternedStr, IrType>,
    pub returns: IrType,
    /// Native implementation id for trusted builtin leaves.
    ///
    /// Plan 11 only preserves this id. Runtime dispatch can later use it without
    /// reparsing Quew prelude source.
    pub native: Option<InternedStr>,
    /// Key into `QuewGraphIR::graphs` for this function's body.
    pub graph_ref: String,
}

// ── Agent definitions ─────────────────────────────────────────────────────────

/// An agent declaration — metadata only. The body lives in `QuewGraphIR::graphs`.
#[derive(Debug, Clone)]
pub struct AgentDef {
    /// Type of the agent's input parameter.
    pub input: Option<IrType>,
    /// Declared return type. `None` means the agent returns plain text (default).
    pub output: Option<IrType>,
    /// The `@context(T)` type, if the agent carries a context object.
    pub context: Option<InternedStr>,
    /// Which provider interaction protocol this agent should use.
    pub protocol: ProtocolMode,
    /// Key into `QuewGraphIR::graphs` for this agent's body.
    pub graph_ref: String,
}

/// Provider interaction mode for an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolMode {
    /// Use the text block protocol.
    Block,
    /// Use provider-native tool/function calling.
    Native,
}
