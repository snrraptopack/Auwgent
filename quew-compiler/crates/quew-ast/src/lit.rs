//! Literals — the syntactic representation of constant values.

use quew_errors::Span;
use quew_interner::InternedStr;

/// A literal value as it appears in source.
#[derive(Debug, Clone, PartialEq)]
pub enum Lit {
    Int(i64, Span),
    Float(f64, Span),
    String(StringLit),
    Bool(bool, Span),
    Null(Span),
}

impl Lit {
    pub fn span(&self) -> Span {
        match self {
            Self::Int(_, s)    => *s,
            Self::Float(_, s)  => *s,
            Self::String(s)    => s.span,
            Self::Bool(_, s)   => *s,
            Self::Null(s)      => *s,
        }
    }
}

/// A double-quoted or triple-quoted string literal.
///
/// `value` is the interned content **without** surrounding quotes.
/// String interpolation (`{var}`) is preserved as-is in the content —
/// the evaluator handles it at runtime.
#[derive(Debug, Clone, PartialEq)]
pub struct StringLit {
    /// Interned raw content (no surrounding quotes, escape sequences left intact).
    pub value: InternedStr,
    pub kind: StringKind,
    pub span: Span,
}

/// Whether the string was written with `"..."` or `"""..."""`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringKind {
    Regular,
    Triple,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use quew_interner::Interner;

    fn intern(s: &str) -> InternedStr {
        Arc::new(Interner::new()).intern(s)
    }

    fn sp() -> Span { Span::new(0, 5) }

    #[test]
    fn int_lit_span() {
        assert_eq!(Lit::Int(42, sp()).span(), sp());
    }

    #[test]
    fn float_lit_span() {
        assert_eq!(Lit::Float(3.14, sp()).span(), sp());
    }

    #[test]
    fn bool_lit_span() {
        assert_eq!(Lit::Bool(true, sp()).span(), sp());
        assert_eq!(Lit::Bool(false, sp()).span(), sp());
    }

    #[test]
    fn null_lit_span() {
        assert_eq!(Lit::Null(sp()).span(), sp());
    }

    #[test]
    fn string_lit_regular() {
        let s = StringLit { value: intern("hello"), kind: StringKind::Regular, span: sp() };
        assert_eq!(s.kind, StringKind::Regular);
        assert_eq!(Lit::String(s).span(), sp());
    }

    #[test]
    fn string_lit_triple() {
        let s = StringLit { value: intern("multi\nline"), kind: StringKind::Triple, span: sp() };
        assert_eq!(s.kind, StringKind::Triple);
    }
}
