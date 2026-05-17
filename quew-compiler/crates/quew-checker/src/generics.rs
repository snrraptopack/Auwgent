use indexmap::IndexMap;
use quew_errors::{Diagnostic, Severity, Span};
use quew_interner::InternedStr;
use quew_types::{FunctionTy, Ty};

/// Instantiate a function call after the caller has inferred all argument types.
///
/// This handles both ordinary functions and generic functions. Generic
/// parameters are inferred from positional arguments using structural matching:
/// `T` binds directly, while records/unions/options recurse through their parts.
pub(crate) fn instantiate_function_call(
    function: &FunctionTy,
    arg_tys: &[Ty],
    call_span: Span,
    diags: &mut Vec<Diagnostic>,
) -> Ty {
    if function.params.len() != arg_tys.len() {
        diags.push(error(
            call_span,
            format!(
                "function expects {} argument(s), found {}",
                function.params.len(),
                arg_tys.len()
            ),
            "wrong number of arguments",
        ));
        return Ty::Error;
    }

    let mut bindings = IndexMap::new();
    for ((_, param_ty), arg_ty) in function.params.iter().zip(arg_tys) {
        bind_generics(param_ty, arg_ty, &mut bindings, call_span, diags);
    }

    for generic in &function.type_params {
        if !bindings.contains_key(generic) {
            diags.push(error(
                call_span,
                format!("could not infer generic parameter `{generic:?}`"),
                "add an argument that determines this generic parameter",
            ));
            return Ty::Error;
        }
    }

    for ((_, param_ty), arg_ty) in function.params.iter().zip(arg_tys) {
        let expected = param_ty.substitute(&bindings);
        if !arg_ty.is_assignable_to(&expected) {
            diags.push(error(
                call_span,
                format!("argument type mismatch: expected `{expected}`, found `{arg_ty}`"),
                "argument is not assignable to the instantiated parameter type",
            ));
            return Ty::Error;
        }
    }

    function.return_ty.substitute(&bindings)
}

fn bind_generics(
    pattern: &Ty,
    actual: &Ty,
    bindings: &mut IndexMap<InternedStr, Ty>,
    span: Span,
    diags: &mut Vec<Diagnostic>,
) {
    match (pattern, actual) {
        (Ty::GenericParam(name), ty) => bind_param(*name, ty, bindings, span, diags),
        (Ty::Optional(p), Ty::Optional(a)) => bind_generics(p, a, bindings, span, diags),
        (Ty::Optional(p), a) => bind_generics(p, a, bindings, span, diags),
        (Ty::Union(p_arms), Ty::Union(a_arms)) if p_arms.len() == a_arms.len() => {
            for (p, a) in p_arms.iter().zip(a_arms) {
                bind_generics(p, a, bindings, span, diags);
            }
        }
        (Ty::Record(p_fields), Ty::Record(a_fields)) => {
            for (name, p_ty) in p_fields {
                if let Some(a_ty) = a_fields.get(name) {
                    bind_generics(p_ty, a_ty, bindings, span, diags);
                }
            }
        }
        (
            Ty::GenericInstance {
                name: p_name,
                args: p_args,
            },
            Ty::GenericInstance {
                name: a_name,
                args: a_args,
            },
        ) if p_name == a_name && p_args.len() == a_args.len() => {
            for (p, a) in p_args.iter().zip(a_args) {
                bind_generics(p, a, bindings, span, diags);
            }
        }
        _ => {}
    }
}

fn bind_param(
    name: InternedStr,
    actual: &Ty,
    bindings: &mut IndexMap<InternedStr, Ty>,
    span: Span,
    diags: &mut Vec<Diagnostic>,
) {
    if let Some(existing) = bindings.get(&name) {
        if existing != actual {
            diags.push(error(
                span,
                format!(
                    "conflicting inference for generic parameter `{name:?}`: `{existing}` and `{actual}`"
                ),
                "generic parameter inferred as two incompatible types",
            ));
        }
        return;
    }

    bindings.insert(name, actual.clone());
}

fn error(span: Span, message: String, label: &str) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        message,
        primary_span: span,
        primary_label: Some(label.into()),
        secondary: vec![],
        help: None,
        code: None,
    }
}
