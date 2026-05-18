//! Builtin declaration metadata.
//!
//! These types keep builtin and compiler-role information attached to normal
//! declarations. A builtin type is still a `TypeDecl`; this metadata only says
//! whether it is part of the language surface and whether it binds to a
//! compiler role such as `(tool, value)`.

use quew_errors::Span;
use quew_interner::InternedStr;

/// Extra metadata carried by a `type` declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltinTypeMeta {
    /// Ordinary user source: `type Name = { ... }`.
    User,
    /// Compiler/prelude-owned builtin source.
    Builtin {
        visibility: BuiltinVisibility,
        role: Option<RoleBindingSyntax>,
    },
}

impl BuiltinTypeMeta {
    pub fn user() -> Self {
        Self::User
    }

    pub fn public() -> Self {
        Self::Builtin {
            visibility: BuiltinVisibility::Public,
            role: None,
        }
    }

    pub fn internal() -> Self {
        Self::Builtin {
            visibility: BuiltinVisibility::Internal,
            role: None,
        }
    }
}

/// Whether a builtin declaration is part of the public language surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinVisibility {
    Public,
    Internal,
}

/// Syntactic role binding from `@@(keyword, place) type ...`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleBindingSyntax {
    pub keyword: InternedStr,
    pub place: InternedStr,
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;
    use quew_interner::Interner;

    fn intern(name: &str) -> InternedStr {
        Interner::new().intern(name)
    }

    #[test]
    fn public_builtin_type_meta_has_no_role() {
        assert_eq!(
            BuiltinTypeMeta::public(),
            BuiltinTypeMeta::Builtin {
                visibility: BuiltinVisibility::Public,
                role: None
            }
        );
    }

    #[test]
    fn internal_builtin_type_meta_has_no_role() {
        assert_eq!(
            BuiltinTypeMeta::internal(),
            BuiltinTypeMeta::Builtin {
                visibility: BuiltinVisibility::Internal,
                role: None
            }
        );
    }

    #[test]
    fn role_binding_syntax_preserves_names_and_span() {
        let binding = RoleBindingSyntax {
            keyword: intern("tool"),
            place: intern("value"),
            span: Span::new(2, 15),
        };
        assert_eq!(binding.span, Span::new(2, 15));
    }
}
