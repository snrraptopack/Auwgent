//! Compiler role binding registry.
//!
//! Builtin declarations can bind types to compiler roles such as
//! `(tool, value)` or `(with, body)`. This module owns the registry shape and
//! validation so `lib.rs` can stay focused on symbol-table construction.

use indexmap::IndexMap;
use quew_errors::{Diagnostic, Severity, Span};
use quew_interner::{InternedStr, Interner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinVisibility {
    User,
    PublicBuiltin,
    InternalBuiltin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoleKey {
    pub keyword: InternedStr,
    pub place: InternedStr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleBinding {
    pub type_name: InternedStr,
    pub span: Span,
}

#[derive(Debug, Default)]
pub struct RoleRegistry {
    pub bindings: IndexMap<RoleKey, RoleBinding>,
}

impl RoleRegistry {
    pub fn register(
        &mut self,
        key: RoleKey,
        binding: RoleBinding,
        interner: &Interner,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !validate_role_key(key, interner, binding.span, diagnostics) {
            return;
        }

        if let Some(prev) = self.bindings.get(&key) {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message: format!(
                    "duplicate role binding for `({}, {})`",
                    interner.resolve(key.keyword),
                    interner.resolve(key.place)
                ),
                primary_span: binding.span,
                primary_label: Some("role already bound here".into()),
                secondary: vec![],
                help: Some(format!(
                    "first binding was for type `{:?}` at {:?}",
                    prev.type_name, prev.span
                )),
                code: None,
            });
            return;
        }

        self.bindings.insert(key, binding);
    }
}

fn validate_role_key(
    key: RoleKey,
    interner: &Interner,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let keyword = interner.resolve(key.keyword);
    let place = interner.resolve(key.place);
    let keyword_ok = matches!(keyword, "tool" | "with" | "middleware");
    let place_ok = matches!(place, "value" | "args" | "body");

    if !keyword_ok {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            message: format!("unknown role keyword `{keyword}`"),
            primary_span: span,
            primary_label: Some("unknown compiler role keyword".into()),
            secondary: vec![],
            help: Some("supported role keywords are `tool`, `with`, and `middleware`".into()),
            code: None,
        });
    }

    if !place_ok {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            message: format!("unknown role place `{place}`"),
            primary_span: span,
            primary_label: Some("unknown compiler role place".into()),
            secondary: vec![],
            help: Some("supported role places are `value`, `args`, and `body`".into()),
            code: None,
        });
    }

    keyword_ok && place_ok
}
