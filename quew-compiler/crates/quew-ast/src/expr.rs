//! Expressions — every value-producing construct in quew.

use quew_errors::Span;
use quew_interner::InternedStr;

use crate::lit::Lit;
use crate::ty::TypeExpr;

/// A value-producing expression.
///
/// Every variant carries or can produce a `Span`. Use [`Expr::span()`] to get it.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A constant literal: `42`, `3.14`, `"hello"`, `true`, `null`.
    Lit(Lit),

    /// A bare name reference: `myVar`, `delete_user`.
    Ident(IdentExpr),

    /// Binary operation: `a + b`, `a == b`, `a and b`, `x = 1`.
    Binary(BinaryExpr),

    /// Unary operation: `not expr`.
    Unary(UnaryExpr),

    /// A function or agent call: `getWeather()`, `One(input)`.
    Call(CallExpr),

    /// A built-in provider call: `gemini("gemini-pro")`, `groq("llama-3")`.
    Provider(ProviderCall),

    /// Field access: `result.error`, `event.on`.
    Member(MemberExpr),

    /// Array literal: `[getWeather, userTools]`.
    Array(ArrayExpr),

    /// Postfix conditional: `value if condition else other`.
    PostfixIf(PostfixIfExpr),

    /// Type discrimination: `response is MyType`.
    Is(IsExpr),

    /// Sentinel emitted by the parser when it cannot parse an expression.
    /// Lets parsing continue after a bad expression without losing subsequent items.
    Error(Span),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Self::Lit(l) => l.span(),
            Self::Ident(e) => e.span,
            Self::Binary(e) => e.span,
            Self::Unary(e) => e.span,
            Self::Call(e) => e.span,
            Self::Provider(e) => e.span,
            Self::Member(e) => e.span,
            Self::Array(e) => e.span,
            Self::PostfixIf(e) => e.span,
            Self::Is(e) => e.span,
            Self::Error(s) => *s,
        }
    }
}

// ── Sub-nodes ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct IdentExpr {
    pub name: InternedStr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BinaryExpr {
    pub left: Box<Expr>,
    pub op: BinaryOp,
    pub right: Box<Expr>,
    pub span: Span,
}

/// Binary operators available in the quew grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    // Equality
    Eq,
    NotEq,
    // Logical (English keywords — no && or ||)
    And,
    Or,
    // Assignment
    Assign,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnaryExpr {
    pub op: UnaryOp,
    pub operand: Box<Expr>,
    pub span: Span,
}

/// Unary operators available in the quew grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// `not expr` — logical negation.
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CallExpr {
    pub callee: Box<Expr>,
    pub args: Vec<Expr>,
    pub span: Span,
}

/// A built-in provider call: `gemini("model")`, `openai("model")`, `groq("model")`.
///
/// Hardcoded for v2 first milestone. Future: `extend model` syntax.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderCall {
    pub provider: Provider,
    pub model_name: crate::lit::StringLit,
    /// Optional second argument — pass-through config `{ topK: 40, ... }`.
    pub config: Vec<ConfigField>,
    pub span: Span,
}

/// The three built-in providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Gemini,
    OpenAi,
    Groq,
}

/// A key-value field inside a config block `{ key: value }`.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigField {
    pub key: InternedStr,
    pub value: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemberExpr {
    pub object: Box<Expr>,
    /// The field name after the `.`.
    ///
    /// ## Parser rule: keywords are valid field names
    ///
    /// After a `.` the parser must accept ANY token whose source text is
    /// identifier-like — including reserved keywords. Examples that must parse:
    ///
    /// ```text
    /// config.model      ← `model` is KwModel
    /// response.is       ← `is`    is KwIs
    /// obj.for           ← `for`   is KwFor
    /// result.not        ← `not`   is KwNot
    /// ```
    ///
    /// The parser implements a `parse_field_name()` helper that accepts
    /// `TokenKind::Ident` OR any keyword token, and interns the raw slice.
    /// This keeps the AST clean (always `InternedStr`) while the parser
    /// handles the contextual ambiguity.
    pub field: InternedStr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrayExpr {
    pub elements: Vec<Expr>,
    pub span: Span,
}

/// `value if condition else other_value` — postfix conditional.
#[derive(Debug, Clone, PartialEq)]
pub struct PostfixIfExpr {
    pub value: Box<Expr>,
    pub condition: Box<Expr>,
    pub else_value: Box<Expr>,
    pub span: Span,
}

/// `expr is Type` — runtime type discrimination.
#[derive(Debug, Clone, PartialEq)]
pub struct IsExpr {
    pub value: Box<Expr>,
    pub ty: TypeExpr,
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lit::{Lit, StringKind, StringLit};
    use crate::ty::TypeExpr;
    use quew_interner::Interner;
    use std::sync::Arc;

    fn intern(s: &str) -> InternedStr {
        Arc::new(Interner::new()).intern(s)
    }

    fn sp() -> Span {
        Span::new(0, 1)
    }

    fn int_expr() -> Expr {
        Expr::Lit(Lit::Int(1, sp()))
    }

    // ── Span coverage ────────────────────────────────────────────────────────

    #[test]
    fn lit_expr_span() {
        assert_eq!(int_expr().span(), sp());
    }

    #[test]
    fn ident_expr_span() {
        let e = Expr::Ident(IdentExpr {
            name: intern("x"),
            span: Span::new(0, 1),
        });
        assert_eq!(e.span(), Span::new(0, 1));
    }

    #[test]
    fn binary_expr_span() {
        let e = Expr::Binary(BinaryExpr {
            left: Box::new(int_expr()),
            op: BinaryOp::Add,
            right: Box::new(int_expr()),
            span: Span::new(0, 5),
        });
        assert_eq!(e.span(), Span::new(0, 5));
    }

    #[test]
    fn unary_expr_span() {
        let e = Expr::Unary(UnaryExpr {
            op: UnaryOp::Not,
            operand: Box::new(Expr::Lit(Lit::Bool(true, sp()))),
            span: Span::new(0, 8),
        });
        assert_eq!(e.span(), Span::new(0, 8));
    }

    #[test]
    fn call_expr_span() {
        let e = Expr::Call(CallExpr {
            callee: Box::new(Expr::Ident(IdentExpr {
                name: intern("f"),
                span: sp(),
            })),
            args: vec![],
            span: Span::new(0, 3),
        });
        assert_eq!(e.span(), Span::new(0, 3));
    }

    #[test]
    fn provider_call_span() {
        let lit = StringLit {
            value: intern("gemini-pro"),
            kind: StringKind::Regular,
            span: sp(),
        };
        let e = Expr::Provider(ProviderCall {
            provider: Provider::Gemini,
            model_name: lit,
            config: vec![],
            span: Span::new(0, 20),
        });
        assert_eq!(e.span(), Span::new(0, 20));
    }

    #[test]
    fn member_expr_span() {
        let e = Expr::Member(MemberExpr {
            object: Box::new(int_expr()),
            field: intern("error"),
            span: Span::new(0, 8),
        });
        assert_eq!(e.span(), Span::new(0, 8));
    }

    #[test]
    fn array_expr_span() {
        let e = Expr::Array(ArrayExpr {
            elements: vec![],
            span: Span::new(0, 2),
        });
        assert_eq!(e.span(), Span::new(0, 2));
    }

    #[test]
    fn postfix_if_span() {
        let e = Expr::PostfixIf(PostfixIfExpr {
            value: Box::new(int_expr()),
            condition: Box::new(Expr::Lit(Lit::Bool(true, sp()))),
            else_value: Box::new(int_expr()),
            span: Span::new(0, 20),
        });
        assert_eq!(e.span(), Span::new(0, 20));
    }

    #[test]
    fn is_expr_span() {
        let e = Expr::Is(IsExpr {
            value: Box::new(int_expr()),
            ty: TypeExpr::Named(intern("MyType"), Span::new(5, 11)),
            span: Span::new(0, 11),
        });
        assert_eq!(e.span(), Span::new(0, 11));
    }

    #[test]
    fn error_sentinel_span() {
        let e = Expr::Error(Span::new(3, 7));
        assert_eq!(e.span(), Span::new(3, 7));
    }

    // ── Operator enums ────────────────────────────────────────────────────────

    #[test]
    fn binary_ops_are_copy() {
        let op = BinaryOp::And;
        let _copy = op;
        assert_eq!(op, BinaryOp::And);
    }

    #[test]
    fn provider_variants_are_copy() {
        let p = Provider::Gemini;
        let _copy = p;
        assert_eq!(p, Provider::Gemini);
    }

    #[test]
    fn all_providers_distinct() {
        assert_ne!(Provider::Gemini, Provider::OpenAi);
        assert_ne!(Provider::OpenAi, Provider::Groq);
        assert_ne!(Provider::Gemini, Provider::Groq);
    }
}
