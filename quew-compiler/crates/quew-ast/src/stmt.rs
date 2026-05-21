//! Statements — every control-flow and binding construct in quew.

use quew_errors::Span;
use quew_interner::InternedStr;

use crate::expr::Expr;
use crate::ty::TypeExpr;

/// A single executable statement.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let(LetStmt),
    If(IfStmt),
    Return(ReturnStmt),
    Reply(ReplyStmt),
    For(ForStmt),
    While(WhileStmt),
    /// An expression used as a statement (e.g. a bare function call).
    Expr(ExprStmt),
    /// `break` — exit the enclosing loop.
    Break(Span),
    /// `continue` — skip to the next iteration of the enclosing loop.
    Continue(Span),
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Self::Let(s) => s.span,
            Self::If(s) => s.span,
            Self::Return(s) => s.span,
            Self::Reply(s) => s.span,
            Self::For(s) => s.span,
            Self::While(s) => s.span,
            Self::Expr(s) => s.span,
            Self::Break(span) => *span,
            Self::Continue(span) => *span,
        }
    }
}

// ── Let ───────────────────────────────────────────────────────────────────────

/// `let name: Type = expr` or `let name = expr`.
#[derive(Debug, Clone, PartialEq)]
pub struct LetStmt {
    pub name: InternedStr,
    /// Explicit type annotation — `None` means the type is inferred.
    pub ty: Option<TypeExpr>,
    pub init: Expr,
    pub span: Span,
}

// ── If ────────────────────────────────────────────────────────────────────────

/// `if condition { body } else { body }` or `if condition { body }`.
#[derive(Debug, Clone, PartialEq)]
pub struct IfStmt {
    pub condition: Expr,
    pub then_body: Vec<Stmt>,
    pub else_clause: ElseClause,
    pub span: Span,
}

/// What (if anything) follows an `if` body.
#[derive(Debug, Clone, PartialEq)]
pub enum ElseClause {
    /// No else branch.
    None,
    /// `else { ... }`.
    Else(Vec<Stmt>, Span),
    /// `else if condition { ... }` — chained.
    ElseIf(Box<IfStmt>),
}

/// Controls how a delegating `return` merges the child agent's context into the parent.
///
/// This is a graph-level directive, not a type-level one. The checker validates
/// the value's type regardless of mode. The IR lowerer uses `mode` to decide
/// whether to emit `AgentCallMode::BlackBox` or `AgentCallMode::WithTurns`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnMode {
    /// Default (`return expr`). Parent journal records only the child's final output.
    /// Child's internal tool calls and model turns are invisible to the parent context.
    Normal,
    /// `return expr with turns`. Child nodes are inlined into the parent graph.
    /// The parent journal carries the child's full turn trace with a cursor marking the start.
    WithTurns,
}

/// `return expr`, `return expr with turns`, or bare `return`.
#[derive(Debug, Clone, PartialEq)]
pub struct ReturnStmt {
    pub value: Option<Expr>,
    /// Whether the child's turn trace is merged into the parent context.
    /// `Normal` for plain `return`; `WithTurns` for `return … with turns`.
    pub mode: ReturnMode,
    pub span: Span,
}

// ── Reply ─────────────────────────────────────────────────────────────────────

/// `reply(expr) with { ... }`.
#[derive(Debug, Clone, PartialEq)]
pub struct ReplyStmt {
    pub input: Expr,
    pub with_block: WithBlock,
    pub span: Span,
}

/// The `with { key: value, ... }` configuration block of a `reply`.
#[derive(Debug, Clone, PartialEq)]
pub struct WithBlock {
    pub fields: Vec<WithField>,
    pub span: Span,
}

/// A single `key: value` entry inside a `with` block.
#[derive(Debug, Clone, PartialEq)]
pub struct WithField {
    pub key: InternedStr,
    pub value: Expr,
    pub span: Span,
}

// ── For ───────────────────────────────────────────────────────────────────────

/// `for idx, value in iterable { body }` or `for value in iterable { body }`.
#[derive(Debug, Clone, PartialEq)]
pub struct ForStmt {
    /// Optional index binding (`idx` in `for idx, value in`).
    pub index: Option<InternedStr>,
    pub value: InternedStr,
    pub iterable: Expr,
    pub body: Vec<Stmt>,
    pub span: Span,
}

// ── While ─────────────────────────────────────────────────────────────────────

/// `while condition { body }`.
#[derive(Debug, Clone, PartialEq)]
pub struct WhileStmt {
    pub condition: Expr,
    pub body: Vec<Stmt>,
    pub span: Span,
}

// ── Expr statement ────────────────────────────────────────────────────────────

/// An expression used as a statement.
#[derive(Debug, Clone, PartialEq)]
pub struct ExprStmt {
    pub expr: Expr,
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lit::Lit;
    use quew_interner::Interner;
    use std::sync::Arc;

    fn intern(s: &str) -> InternedStr {
        Arc::new(Interner::new()).intern(s)
    }

    fn sp() -> Span {
        Span::new(0, 5)
    }
    fn bool_expr(v: bool) -> Expr {
        Expr::Lit(Lit::Bool(v, sp()))
    }
    fn int_expr() -> Expr {
        Expr::Lit(Lit::Int(0, sp()))
    }

    #[test]
    fn let_stmt_span() {
        let s = Stmt::Let(LetStmt {
            name: intern("x"),
            ty: None,
            init: int_expr(),
            span: sp(),
        });
        assert_eq!(s.span(), sp());
    }

    #[test]
    fn if_stmt_no_else() {
        let s = Stmt::If(IfStmt {
            condition: bool_expr(true),
            then_body: vec![],
            else_clause: ElseClause::None,
            span: sp(),
        });
        assert_eq!(s.span(), sp());
    }

    #[test]
    fn if_stmt_with_else() {
        let s = IfStmt {
            condition: bool_expr(true),
            then_body: vec![],
            else_clause: ElseClause::Else(vec![], Span::new(5, 10)),
            span: sp(),
        };
        assert!(matches!(s.else_clause, ElseClause::Else(_, _)));
    }

    #[test]
    fn if_stmt_chained_else_if() {
        let inner = IfStmt {
            condition: bool_expr(false),
            then_body: vec![],
            else_clause: ElseClause::None,
            span: Span::new(10, 20),
        };
        let outer = IfStmt {
            condition: bool_expr(true),
            then_body: vec![],
            else_clause: ElseClause::ElseIf(Box::new(inner)),
            span: sp(),
        };
        assert!(matches!(outer.else_clause, ElseClause::ElseIf(_)));
    }

    #[test]
    fn return_stmt_with_value() {
        let s = Stmt::Return(ReturnStmt {
            value: Some(int_expr()),
            mode: ReturnMode::Normal,
            span: sp(),
        });
        assert_eq!(s.span(), sp());
    }

    #[test]
    fn return_stmt_no_value() {
        let s = Stmt::Return(ReturnStmt {
            value: None,
            mode: ReturnMode::Normal,
            span: sp(),
        });
        assert!(matches!(
            s,
            Stmt::Return(ReturnStmt {
                value: None,
                mode: ReturnMode::Normal,
                ..
            })
        ));
    }

    #[test]
    fn reply_stmt_span() {
        let with = WithBlock {
            fields: vec![],
            span: Span::new(3, 5),
        };
        let s = Stmt::Reply(ReplyStmt {
            input: int_expr(),
            with_block: with,
            span: sp(),
        });
        assert_eq!(s.span(), sp());
    }

    #[test]
    fn for_stmt_with_index() {
        let s = ForStmt {
            index: Some(intern("idx")),
            value: intern("item"),
            iterable: int_expr(),
            body: vec![],
            span: sp(),
        };
        assert!(s.index.is_some());
    }

    #[test]
    fn for_stmt_no_index() {
        let s = ForStmt {
            index: None,
            value: intern("item"),
            iterable: int_expr(),
            body: vec![],
            span: sp(),
        };
        assert!(s.index.is_none());
    }

    #[test]
    fn expr_stmt_span() {
        let s = Stmt::Expr(ExprStmt {
            expr: int_expr(),
            span: sp(),
        });
        assert_eq!(s.span(), sp());
    }
}
