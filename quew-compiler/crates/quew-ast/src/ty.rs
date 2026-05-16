//! Type expressions — the syntactic representation of types in quew.
//!
//! This is purely structural: `TypeExpr::Named("string", span)` says "the source
//! text has the word `string` here". Whether that name resolves to a valid type
//! is the checker's job.

use quew_errors::Span;
use quew_interner::InternedStr;

/// A type as written in source code.
///
/// Every variant carries a `Span` covering the full extent of the type expression.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    /// A simple named type: `string`, `bool`, `MyType`.
    Named(InternedStr, Span),

    /// A union: `A | B | C`.
    /// Always has at least two members.
    Union(Vec<TypeExpr>, Span),

    /// An optional type written with `?`: `string?`.
    /// Note: optional *parameters* use `name?: Type` — that is recorded on
    /// [`crate::Param`], not here. This variant is for type-level optionals.
    Optional(Box<TypeExpr>, Span),

    /// A generic application: `Fetch<string>`.
    /// Reserved for future use; the checker will reject unknown generics.
    Generic(InternedStr, Vec<TypeExpr>, Span),
}

impl TypeExpr {
    /// The span covering this entire type expression.
    pub fn span(&self) -> Span {
        match self {
            Self::Named(_, s)       => *s,
            Self::Union(_, s)       => *s,
            Self::Optional(_, s)    => *s,
            Self::Generic(_, _, s)  => *s,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use quew_interner::Interner;

    fn intern(s: &str) -> InternedStr {
        let i = Arc::new(Interner::new());
        i.intern(s)
    }

    fn sp() -> Span { Span::new(0, 4) }

    #[test]
    fn named_type_span() {
        let t = TypeExpr::Named(intern("string"), sp());
        assert_eq!(t.span(), sp());
    }

    #[test]
    fn union_type_span() {
        let a = TypeExpr::Named(intern("string"), Span::new(0, 6));
        let b = TypeExpr::Named(intern("number"), Span::new(9, 15));
        let u = TypeExpr::Union(vec![a, b], Span::new(0, 15));
        assert_eq!(u.span(), Span::new(0, 15));
    }

    #[test]
    fn optional_type_span() {
        let inner = TypeExpr::Named(intern("string"), sp());
        let opt = TypeExpr::Optional(Box::new(inner), Span::new(0, 7));
        assert_eq!(opt.span(), Span::new(0, 7));
    }

    #[test]
    fn generic_type_span() {
        let arg = TypeExpr::Named(intern("string"), Span::new(6, 12));
        let g = TypeExpr::Generic(intern("Fetch"), vec![arg], Span::new(0, 13));
        assert_eq!(g.span(), Span::new(0, 13));
    }
}
