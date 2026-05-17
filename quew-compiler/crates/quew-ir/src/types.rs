//! IR type representations.
//!
//! `IrType` is a simplified, lowered representation of the checker's `Ty` enum.
//! It strips away type-inference metadata and represents only what the runtime
//! needs to know: the shape of values flowing through graph nodes.

use indexmap::IndexMap;
use quew_interner::InternedStr;

// ── IrType ────────────────────────────────────────────────────────────────────

/// A lowered, runtime-facing type representation.
///
/// Unlike the checker's `Ty` (which carries spans, inference variables, and
/// error sentinels), `IrType` carries only the structural information needed
/// by the runtime and codegen. Every `IrType` is valid — the checker already
/// rejected errors before lowering begins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrType {
    // ── Primitives ────────────────────────────────────────────────────────────
    String,
    Number,
    Float,
    Bool,
    Null,
    Void,

    // ── Composite ─────────────────────────────────────────────────────────────
    /// An inline or named record: `{ field: Type, … }`.
    Object(IndexMap<InternedStr, IrField>),

    /// A homogeneous array: `Type[]`.
    Array(Box<IrType>),

    /// A type union: `A | B`.
    Union(Vec<IrType>),

    /// A named reference to a type declared in `definitions.types`.
    /// Resolved by name; the runtime looks it up in the definitions table.
    Named(InternedStr),

    /// A generic type application such as `Box<string>` or `Pair<A, B>`.
    GenericInstance {
        name: InternedStr,
        args: Vec<IrType>,
    },

    /// A generic parameter in a generic type or function declaration.
    GenericParam(InternedStr),

    // ── Special ───────────────────────────────────────────────────────────────
    /// The `Text` DSL alias — equivalent to `String` at the IR level.
    Text,
    /// An agent's output type — used when an agent call's result type is
    /// another agent's declared output.
    AgentOutput(InternedStr),
}

/// A single field in an `IrType::Object`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrField {
    pub ty: IrType,
    /// `true` for `field?: Type` (optional field).
    pub optional: bool,
}

impl IrField {
    pub fn required(ty: IrType) -> Self {
        Self {
            ty,
            optional: false,
        }
    }

    pub fn optional(ty: IrType) -> Self {
        Self { ty, optional: true }
    }
}
