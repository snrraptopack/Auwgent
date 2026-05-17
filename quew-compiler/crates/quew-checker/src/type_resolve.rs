use indexmap::IndexMap;
use quew_ast::TypeExpr;
use quew_errors::{Diagnostic, Severity, Span};
use quew_interner::InternedStr;
use quew_scope::{SymbolKind, SymbolTable};
use quew_types::{AgentTy, FunctionTy, ToolTy, Ty};

use crate::keys::PrimKeys;

/// Resolve a syntactic type expression into the semantic type algebra.
pub(crate) fn resolve_type(
    expr: &TypeExpr,
    table: &SymbolTable,
    prim: &PrimKeys,
    diags: &mut Vec<Diagnostic>,
) -> Ty {
    resolve_type_with_params(expr, &[], table, prim, diags)
}

pub(crate) fn resolve_type_with_params(
    expr: &TypeExpr,
    type_params: &[InternedStr],
    table: &SymbolTable,
    prim: &PrimKeys,
    diags: &mut Vec<Diagnostic>,
) -> Ty {
    match expr {
        TypeExpr::Named(name, span) => {
            if type_params.contains(name) {
                Ty::GenericParam(*name)
            } else {
                resolve_named_type(*name, table, prim, diags, *span)
            }
        }
        TypeExpr::Optional(inner, _) => {
            resolve_type_with_params(inner, type_params, table, prim, diags).optional()
        }
        TypeExpr::Union(arms, _) => {
            let lowered: Vec<Ty> = arms
                .iter()
                .map(|a| resolve_type_with_params(a, type_params, table, prim, diags))
                .collect();
            Ty::Union(lowered).flatten_union()
        }
        TypeExpr::Generic(name, args, span) => {
            let args: Vec<Ty> = args
                .iter()
                .map(|arg| resolve_type_with_params(arg, type_params, table, prim, diags))
                .collect();
            instantiate_generic_type(*name, &args, table, prim, diags, *span)
        }
    }
}

/// Resolve already-lowered semantic types that may still contain named or
/// generic references from the scope layer.
pub(crate) fn resolve_semantic_ty(
    ty: &Ty,
    table: &SymbolTable,
    prim: &PrimKeys,
    diags: &mut Vec<Diagnostic>,
    span: Span,
) -> Ty {
    match ty {
        Ty::Named(name) => resolve_named_type(*name, table, prim, diags, span),
        Ty::GenericInstance { name, args } => {
            let args: Vec<Ty> = args
                .iter()
                .map(|arg| resolve_semantic_ty(arg, table, prim, diags, span))
                .collect();
            instantiate_generic_type(*name, &args, table, prim, diags, span)
        }
        Ty::Record(fields) => {
            let mut out = IndexMap::new();
            for (name, field_ty) in fields {
                out.insert(*name, resolve_semantic_ty(field_ty, table, prim, diags, span));
            }
            Ty::Record(out)
        }
        Ty::Union(arms) => Ty::Union(
            arms.iter()
                .map(|arm| resolve_semantic_ty(arm, table, prim, diags, span))
                .collect(),
        )
        .flatten_union(),
        Ty::Optional(inner) => {
            Ty::Optional(Box::new(resolve_semantic_ty(inner, table, prim, diags, span)))
        }
        Ty::Function(f) => Ty::Function(FunctionTy {
            type_params: f.type_params.clone(),
            params: f
                .params
                .iter()
                .map(|(name, ty)| (*name, resolve_semantic_ty(ty, table, prim, diags, span)))
                .collect(),
            return_ty: Box::new(resolve_semantic_ty(&f.return_ty, table, prim, diags, span)),
        }),
        Ty::Agent(a) => Ty::Agent(AgentTy {
            input_name: a.input_name,
            input_ty: Box::new(resolve_semantic_ty(&a.input_ty, table, prim, diags, span)),
            return_ty: Box::new(resolve_semantic_ty(&a.return_ty, table, prim, diags, span)),
        }),
        Ty::Tool(t) => Ty::Tool(ToolTy {
            bound_params: t
                .bound_params
                .iter()
                .map(|(name, ty)| (*name, resolve_semantic_ty(ty, table, prim, diags, span)))
                .collect(),
            model_params: t
                .model_params
                .iter()
                .map(|(name, ty)| (*name, resolve_semantic_ty(ty, table, prim, diags, span)))
                .collect(),
            return_ty: Box::new(resolve_semantic_ty(&t.return_ty, table, prim, diags, span)),
        }),
        _ => ty.clone(),
    }
}

fn resolve_named_type(
    name: InternedStr,
    table: &SymbolTable,
    prim: &PrimKeys,
    diags: &mut Vec<Diagnostic>,
    span: Span,
) -> Ty {
    if let Some(ty) = prim.resolve(name) {
        return ty;
    }

    let Some(sym) = table.globals.get(&name) else {
        diags.push(type_error(span, format!("unknown type `{:?}`", name)));
        return Ty::Error;
    };

    match sym.kind {
        SymbolKind::Type => {
            if !sym.type_params.is_empty() {
                diags.push(type_error(
                    span,
                    format!(
                        "generic type `{:?}` expects {} type argument(s)",
                        name,
                        sym.type_params.len()
                    ),
                ));
                Ty::Error
            } else {
                resolve_semantic_ty(&sym.ty, table, prim, diags, span)
            }
        }
        SymbolKind::Agent => resolve_semantic_ty(&sym.ty, table, prim, diags, span),
        _ => {
            diags.push(type_error(span, format!("`{:?}` is not a type", name)));
            Ty::Error
        }
    }
}

fn instantiate_generic_type(
    name: InternedStr,
    args: &[Ty],
    table: &SymbolTable,
    prim: &PrimKeys,
    diags: &mut Vec<Diagnostic>,
    span: Span,
) -> Ty {
    let Some(sym) = table.globals.get(&name) else {
        diags.push(type_error(span, format!("unknown generic type `{:?}`", name)));
        return Ty::Error;
    };

    if !matches!(sym.kind, SymbolKind::Type) {
        diags.push(type_error(span, format!("`{:?}` is not a type", name)));
        return Ty::Error;
    }

    if sym.type_params.len() != args.len() {
        diags.push(type_error(
            span,
            format!(
                "generic type `{:?}` expects {} type argument(s), found {}",
                name,
                sym.type_params.len(),
                args.len()
            ),
        ));
        return Ty::Error;
    }

    let mut subst = IndexMap::new();
    for (param, arg) in sym.type_params.iter().zip(args) {
        subst.insert(*param, arg.clone());
    }

    resolve_semantic_ty(&sym.ty.substitute(&subst), table, prim, diags, span)
}

fn type_error(span: Span, msg: String) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        message: msg,
        primary_span: span,
        primary_label: None,
        secondary: vec![],
        help: None,
        code: None,
    }
}
