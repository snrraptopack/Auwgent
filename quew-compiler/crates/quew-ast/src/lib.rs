//! # quew-ast
//!
//! **Single responsibility:** own the AST data types for the quew DSL.
//!
//! This crate contains zero parsing logic. It can be depended on by the
//! checker, IR lowerer, and codegen without pulling in the parser.
//!
//! ## Rules
//!
//! - Every public struct and enum carries a `Span`.
//! - No semantic content — no resolved types, no symbol IDs, no inferred kinds.
//! - All names are `InternedStr` — zero heap allocations per identifier.
//! - Recursive nodes use `Box<T>` to keep enum sizes bounded.
//!
//! ## Modules
//!
//! | Module | Contains |
//! |--------|----------|
//! | `ty`   | `TypeExpr` — type expressions |
//! | `lit`  | `Lit`, `StringLit`, `StringKind` |
//! | `expr` | `Expr` and all sub-nodes |
//! | `stmt` | `Stmt` and all sub-nodes |
//! | `item` | `Module`, `Item`, top-level declarations, `Annotation`, `Param` |
//! | `builtin` | Builtin visibility and compiler role metadata |

pub mod builtin;
pub mod expr;
pub mod item;
pub mod lit;
pub mod stmt;
pub mod ty;

// Flatten the most commonly used types to the crate root for convenience.
pub use builtin::{
    BuiltinFunctionMeta, BuiltinTypeMeta, BuiltinVisibility, NativeBinding, RoleBindingSyntax,
};
pub use expr::{
    ArrayExpr, BinaryExpr, BinaryOp, CallExpr, ConfigField, Expr, IdentExpr, IsExpr, MemberExpr,
    PostfixIfExpr, Provider, ProviderCall, UnaryExpr, UnaryOp,
};
pub use item::{
    AgentDecl, Annotation, AnnotationArgs, FieldDef, FunctionDecl, Item, LetDecl, ModelDecl,
    Module, Param, ParamBinding, ToolDecl, ToolEntry, ToolsDecl, TypeDecl,
};
pub use lit::{Lit, StringKind, StringLit};
pub use stmt::{
    ElseClause, ExprStmt, ForStmt, IfStmt, LetStmt, ReplyStmt, ReturnMode, ReturnStmt, Stmt,
    WithBlock, WithField,
};
pub use ty::TypeExpr;
