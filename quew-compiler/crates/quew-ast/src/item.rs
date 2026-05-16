//! Top-level item declarations — agents, functions, tools, types, models, lets.

use quew_errors::Span;
use quew_interner::InternedStr;
use quew_lexer::AnnotationKind;

use crate::expr::{ConfigField, Expr, ProviderCall};
use crate::lit::StringLit;
use crate::stmt::Stmt;
use crate::ty::TypeExpr;

/// A full source file — the root of the AST.
#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub items: Vec<Item>,
    pub span: Span,
}

/// A single top-level declaration.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Agent(AgentDecl),
    Function(FunctionDecl),
    Tool(ToolDecl),
    Tools(ToolsDecl),
    Type(TypeDecl),
    Model(ModelDecl),
    /// Top-level `let` binding (rare but valid).
    Let(LetDecl),
}

impl Item {
    pub fn span(&self) -> Span {
        match self {
            Self::Agent(d) => d.span,
            Self::Function(d) => d.span,
            Self::Tool(d) => d.span,
            Self::Tools(d) => d.span,
            Self::Type(d) => d.span,
            Self::Model(d) => d.span,
            Self::Let(d) => d.span,
        }
    }
}

// ── Annotations ───────────────────────────────────────────────────────────────

/// An `@annotation` that precedes a declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub kind: AnnotationKind,
    pub args: AnnotationArgs,
    pub span: Span,
}

/// The argument form of an annotation.
#[derive(Debug, Clone, PartialEq)]
pub enum AnnotationArgs {
    /// `@native`, `@block` — no arguments.
    None,
    /// `@tool(id: string, ...)` — parameter list.
    Params(Vec<Param>),
    /// `@context(Context)` — a single type name.
    Type(TypeExpr),
    /// `@desc "..."` — a string literal.
    String(StringLit),
}

// ── Parameters ────────────────────────────────────────────────────────────────

/// A function/tool parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub binding: ParamBinding,
    pub name: InternedStr,
    pub ty: TypeExpr,
    /// `name?: Type` — the model may omit this argument.
    pub optional: bool,
    pub span: Span,
}

/// How the parameter relates to enclosing `@tool` args.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamBinding {
    /// Normal parameter: `name: Type`.
    Normal,
    /// Binding reference: `@name: Type` — binds to the @tool arg of the same name.
    /// The value flows from the `@tool(args)` annotation, not from the caller.
    BoundRef,
}

// ── Agent ─────────────────────────────────────────────────────────────────────

/// `@context(...) agent Name(input: Type): ReturnType { body }`.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentDecl {
    pub annotations: Vec<Annotation>,
    /// An agent always has exactly one input parameter.
    pub param: Param,
    pub name: InternedStr,
    /// `None` means the agent returns `Text` (the default).
    pub return_ty: Option<TypeExpr>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

// ── Function ──────────────────────────────────────────────────────────────────

/// `@tool @desc "..." function Name(params): ReturnType { body }`.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub annotations: Vec<Annotation>,
    pub name: InternedStr,
    pub params: Vec<Param>,
    /// `None` means the function's return type is inferred (checked later).
    pub return_ty: Option<TypeExpr>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

// ── Tool ──────────────────────────────────────────────────────────────────────

/// `tool name(params): ReturnType @desc "..."` — a host-backed tool.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDecl {
    pub name: InternedStr,
    pub params: Vec<Param>,
    pub return_ty: TypeExpr,
    /// `@desc` is optional on a single tool.
    pub desc: Option<StringLit>,
    pub span: Span,
}

/// `tools { ... }` or `tools Name { ... } @desc "..."`.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolsDecl {
    /// `None` = shorthand group; `Some` = named progressive disclosure group.
    pub name: Option<InternedStr>,
    pub entries: Vec<ToolEntry>,
    /// `@desc` is required when `name` is `Some` (progressive disclosure).
    pub desc: Option<StringLit>,
    pub span: Span,
}

/// One entry inside a `tools { }` block.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolEntry {
    pub name: InternedStr,
    pub params: Vec<Param>,
    pub return_ty: TypeExpr,
    pub desc: Option<StringLit>,
    pub span: Span,
}

// ── Type ──────────────────────────────────────────────────────────────────────

/// `type Name = { field: Type, ... }`.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeDecl {
    pub name: InternedStr,
    pub fields: Vec<FieldDef>,
    pub span: Span,
}

/// One field in a type definition.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    pub name: InternedStr,
    pub ty: TypeExpr,
    pub optional: bool,
    pub span: Span,
}

// ── Model ─────────────────────────────────────────────────────────────────────

/// `model Name = { model: gemini("..."), config: { ... } }`.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelDecl {
    pub name: InternedStr,
    pub provider: ProviderCall,
    pub config: Vec<ConfigField>,
    pub span: Span,
}

// ── Top-level let ─────────────────────────────────────────────────────────────

/// `let name = expr` at the top level.
#[derive(Debug, Clone, PartialEq)]
pub struct LetDecl {
    pub name: InternedStr,
    pub ty: Option<TypeExpr>,
    pub init: Expr,
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::{Provider, ProviderCall};
    use crate::lit::{Lit, StringKind, StringLit};
    use quew_interner::Interner;
    use std::sync::Arc;

    fn intern(s: &str) -> InternedStr {
        Arc::new(Interner::new()).intern(s)
    }

    fn sp() -> Span {
        Span::new(0, 10)
    }
    fn ty_string() -> TypeExpr {
        TypeExpr::Named(intern("string"), sp())
    }
    fn str_lit(v: &str) -> StringLit {
        StringLit {
            value: intern(v),
            kind: StringKind::Regular,
            span: sp(),
        }
    }
    fn int_expr() -> Expr {
        Expr::Lit(Lit::Int(0, sp()))
    }

    fn normal_param(name: &str) -> Param {
        Param {
            binding: ParamBinding::Normal,
            name: intern(name),
            ty: ty_string(),
            optional: false,
            span: sp(),
        }
    }

    fn provider_call() -> ProviderCall {
        ProviderCall {
            provider: Provider::Gemini,
            model_name: str_lit("gemini-pro"),
            config: vec![],
            span: sp(),
        }
    }

    // ── Item span coverage ────────────────────────────────────────────────────

    #[test]
    fn agent_decl_item_span() {
        let a = AgentDecl {
            annotations: vec![],
            param: normal_param("input"),
            name: intern("Hello"),
            return_ty: None,
            body: vec![],
            span: sp(),
        };
        assert_eq!(Item::Agent(a).span(), sp());
    }

    #[test]
    fn function_decl_item_span() {
        let f = FunctionDecl {
            annotations: vec![],
            name: intern("greet"),
            params: vec![normal_param("name")],
            return_ty: Some(ty_string()),
            body: vec![],
            span: sp(),
        };
        assert_eq!(Item::Function(f).span(), sp());
    }

    #[test]
    fn tool_decl_item_span() {
        let t = ToolDecl {
            name: intern("getWeather"),
            params: vec![],
            return_ty: ty_string(),
            desc: Some(str_lit("get the weather")),
            span: sp(),
        };
        assert_eq!(Item::Tool(t).span(), sp());
    }

    #[test]
    fn tools_decl_shorthand() {
        let d = ToolsDecl {
            name: None,
            entries: vec![],
            desc: None,
            span: sp(),
        };
        assert!(d.name.is_none(), "shorthand tools group has no name");
    }

    #[test]
    fn tools_decl_named_progressive() {
        let d = ToolsDecl {
            name: Some(intern("userTools")),
            entries: vec![],
            desc: Some(str_lit("User management tools")),
            span: sp(),
        };
        assert!(d.name.is_some());
        assert!(d.desc.is_some());
    }

    #[test]
    fn type_decl_with_fields() {
        let t = TypeDecl {
            name: intern("Response"),
            fields: vec![
                FieldDef {
                    name: intern("userName"),
                    ty: ty_string(),
                    optional: false,
                    span: sp(),
                },
                FieldDef {
                    name: intern("age"),
                    ty: TypeExpr::Named(intern("number"), sp()),
                    optional: true,
                    span: sp(),
                },
            ],
            span: sp(),
        };
        assert_eq!(t.fields.len(), 2);
        assert!(t.fields[1].optional);
    }

    #[test]
    fn model_decl_item_span() {
        let m = ModelDecl {
            name: intern("Gemini"),
            provider: provider_call(),
            config: vec![],
            span: sp(),
        };
        assert_eq!(Item::Model(m).span(), sp());
    }

    #[test]
    fn let_decl_item_span() {
        let d = LetDecl {
            name: intern("x"),
            ty: None,
            init: int_expr(),
            span: sp(),
        };
        assert_eq!(Item::Let(d).span(), sp());
    }

    // ── Annotations ───────────────────────────────────────────────────────────

    #[test]
    fn annotation_none_args() {
        let a = Annotation {
            kind: AnnotationKind::Native,
            args: AnnotationArgs::None,
            span: sp(),
        };
        assert!(matches!(a.args, AnnotationArgs::None));
    }

    #[test]
    fn annotation_string_args() {
        let a = Annotation {
            kind: AnnotationKind::Desc,
            args: AnnotationArgs::String(str_lit("A description")),
            span: sp(),
        };
        assert!(matches!(a.args, AnnotationArgs::String(_)));
    }

    #[test]
    fn annotation_params_args() {
        let a = Annotation {
            kind: AnnotationKind::Tool,
            args: AnnotationArgs::Params(vec![normal_param("id")]),
            span: sp(),
        };
        if let AnnotationArgs::Params(params) = &a.args {
            assert_eq!(params.len(), 1);
        } else {
            panic!("expected Params");
        }
    }

    // ── ParamBinding ──────────────────────────────────────────────────────────

    #[test]
    fn bound_ref_param() {
        let p = Param {
            binding: ParamBinding::BoundRef,
            name: intern("id"),
            ty: ty_string(),
            optional: false,
            span: sp(),
        };
        assert_eq!(p.binding, ParamBinding::BoundRef);
    }
}
