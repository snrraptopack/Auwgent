//! Checker-side use of compiler role bindings.

use quew_errors::{Diagnostic, Span};
use quew_interner::InternedStr;
use quew_scope::{RoleKey, SymbolTable};
use quew_types::Ty;

use crate::keys::PrimKeys;
use crate::type_resolve::resolve_semantic_ty;

/// Wrap a value through a compiler role when that role is bound.
///
/// Missing roles intentionally fall back to the raw value type so low-level
/// tests can still exercise the checker without loading the prelude.
pub(crate) fn wrap_value_if_bound(
    keyword: InternedStr,
    place: InternedStr,
    value_ty: Ty,
    table: &SymbolTable,
    prim: &PrimKeys,
    diags: &mut Vec<Diagnostic>,
    span: Span,
) -> Ty {
    let key = RoleKey { keyword, place };
    let Some(binding) = table.roles.bindings.get(&key) else {
        return value_ty;
    };

    let wrapped = Ty::GenericInstance {
        name: binding.type_name,
        args: vec![value_ty],
    };
    resolve_semantic_ty(&wrapped, table, prim, diags, span)
}

pub(crate) fn resolve_role_type(
    keyword: InternedStr,
    place: InternedStr,
    table: &SymbolTable,
    prim: &PrimKeys,
    diags: &mut Vec<Diagnostic>,
    span: Span,
) -> Option<Ty> {
    let key = RoleKey { keyword, place };
    let binding = table.roles.bindings.get(&key)?;
    Some(resolve_semantic_ty(
        &Ty::Named(binding.type_name),
        table,
        prim,
        diags,
        span,
    ))
}
