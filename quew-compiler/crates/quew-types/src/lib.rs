//! # quew-types
//!
//! The canonical in-memory representation of every quew type.
//! No syntax, no inference, no diagnostics — just the type algebra.

use indexmap::IndexMap;
use quew_interner::InternedStr;

// ── Primitive types ───────────────────────────────────────────────────────────

/// Every primitive type in the quew DSL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimTy {
    String,
    Number,
    Float,
    Bool,
    Void,
    Null,
    /// Dynamic value: accepts and produces any runtime value. Used by
    /// JSON builtins and other untyped boundaries.
    Any,
}

impl std::fmt::Display for PrimTy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrimTy::String => write!(f, "string"),
            PrimTy::Number => write!(f, "number"),
            PrimTy::Float => write!(f, "float"),
            PrimTy::Bool => write!(f, "bool"),
            PrimTy::Void => write!(f, "void"),
            PrimTy::Null => write!(f, "null"),
            PrimTy::Any => write!(f, "any"),
        }
    }
}

// ── Provider kind ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderKind {
    Gemini,
    OpenAi,
    Groq,
}

// ── Type variable ─────────────────────────────────────────────────────────────

/// Opaque type variable index — produced during inference, resolved by `quew-unify`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TyVar(pub u32);

// ── Callable subtypes ─────────────────────────────────────────────────────────

/// Plain DSL function: `function foo(a: string, b: number): bool`
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionTy {
    /// Generic parameters declared by the function, such as `<T, E>`.
    pub type_params: Vec<InternedStr>,
    /// Positional params: (name, type). All are caller-provided.
    pub params: Vec<(InternedStr, Ty)>,
    /// Return type. `Ty::Primitive(PrimTy::Void)` when omitted.
    pub return_ty: Box<Ty>,
}

/// Agent entry point: `agent Name(input: T): R`
/// Always exactly one input param. Not directly callable from DSL expressions.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentTy {
    pub input_name: InternedStr,
    pub input_ty: Box<Ty>,
    /// Return type. `Ty::Primitive(PrimTy::Void)` when omitted.
    pub return_ty: Box<Ty>,
}

/// Host-backed tool callable from the DSL.
///
/// `bound_params` — injected by the host from `@tool(name: Type)` context.  
/// `model_params` — supplied by the model / DSL caller.
///
/// Rule: every `@name: Type` param (`ParamBinding::BoundRef`) in the function
/// signature must have a matching entry in `bound_params` with the same type.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolTy {
    /// From `@tool(name: Type, ...)` — host-injected, NOT caller-provided.
    pub bound_params: Vec<(InternedStr, Ty)>,
    /// Regular params the model / DSL caller must supply.
    pub model_params: Vec<(InternedStr, Ty)>,
    pub return_ty: Box<Ty>,
}

// ── The main Ty enum ──────────────────────────────────────────────────────────

/// The in-memory representation of every quew type.
#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    // ── Primitives ────────────────────────────────────────────────────────────
    Primitive(PrimTy),

    // ── Composite ─────────────────────────────────────────────────────────────
    /// A named type reference that has not been resolved by the checker yet.
    Named(InternedStr),

    /// A generic type application that has not been resolved by the checker yet.
    GenericInstance {
        name: InternedStr,
        args: Vec<Ty>,
    },

    /// Named record: `{ name: string, age: number }`. Fields are ordered.
    Record(IndexMap<InternedStr, Ty>),

    /// Homogeneous array: `T[]`.
    Array(Box<Ty>),

    /// A generic parameter declared by a generic type or method, such as `T`.
    GenericParam(InternedStr),

    /// Union of two or more types: `string | number | bool`.
    Union(Vec<Ty>),

    /// Nullable: `T?` — kept as its own variant for cleaner error messages.
    /// Structurally equivalent to `T | null` but displayed as `T?`.
    Optional(Box<Ty>),

    // ── Callables ─────────────────────────────────────────────────────────────
    Function(FunctionTy),
    Agent(AgentTy),
    Tool(ToolTy),

    // ── Provider ──────────────────────────────────────────────────────────────
    Provider(ProviderKind),

    // ── Inference ─────────────────────────────────────────────────────────────
    /// A fresh type variable — resolved by `quew-unify`.
    Var(TyVar),

    /// Error sentinel. Any operation on `Ty::Error` propagates `Ty::Error`.
    /// Prevents cascading diagnostics from a single root cause.
    Error,
}

// ── Core operations ───────────────────────────────────────────────────────────

impl Ty {
    // ── Constructors ──────────────────────────────────────────────────────────

    pub fn string() -> Self {
        Ty::Primitive(PrimTy::String)
    }
    pub fn number() -> Self {
        Ty::Primitive(PrimTy::Number)
    }
    pub fn float() -> Self {
        Ty::Primitive(PrimTy::Float)
    }
    pub fn bool_ty() -> Self {
        Ty::Primitive(PrimTy::Bool)
    }
    pub fn void() -> Self {
        Ty::Primitive(PrimTy::Void)
    }
    pub fn null() -> Self {
        Ty::Primitive(PrimTy::Null)
    }
    pub fn any() -> Self {
        Ty::Primitive(PrimTy::Any)
    }

    /// Wrap `self` in `Optional`. Idempotent: `T??` stays `T?`.
    pub fn optional(self) -> Self {
        match self {
            Ty::Optional(_) => self,
            other => Ty::Optional(Box::new(other)),
        }
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /// True unless this is `Ty::Error`. Use to short-circuit on errors.
    pub fn is_ok(&self) -> bool {
        !matches!(self, Ty::Error)
    }

    /// True if this type is or contains `Ty::Error`.
    pub fn has_error(&self) -> bool {
        match self {
            Ty::Error => true,
            Ty::Named(_) => false,
            Ty::GenericInstance { args, .. } => args.iter().any(|t| t.has_error()),
            Ty::Optional(inner) => inner.has_error(),
            Ty::Union(arms) => arms.iter().any(|a| a.has_error()),
            Ty::Record(fields) => fields.values().any(|t| t.has_error()),
            Ty::Array(elem) => elem.has_error(),
            Ty::GenericParam(_) => false,
            Ty::Function(f) => {
                f.return_ty.has_error() || f.params.iter().any(|(_, t)| t.has_error())
            }
            Ty::Agent(a) => a.input_ty.has_error() || a.return_ty.has_error(),
            Ty::Tool(t) => {
                t.return_ty.has_error()
                    || t.bound_params.iter().any(|(_, t)| t.has_error())
                    || t.model_params.iter().any(|(_, t)| t.has_error())
            }
            _ => false,
        }
    }

    /// Substitute generic parameters according to `subst`.
    pub fn substitute(&self, subst: &IndexMap<InternedStr, Ty>) -> Ty {
        match self {
            Ty::Named(_) => self.clone(),
            Ty::GenericInstance { name, args } => Ty::GenericInstance {
                name: *name,
                args: args.iter().map(|ty| ty.substitute(subst)).collect(),
            },
            Ty::GenericParam(name) => subst.get(name).cloned().unwrap_or_else(|| self.clone()),
            Ty::Record(fields) => {
                let mut out = IndexMap::new();
                for (name, ty) in fields {
                    out.insert(*name, ty.substitute(subst));
                }
                Ty::Record(out)
            }
            Ty::Array(elem) => Ty::Array(Box::new(elem.substitute(subst))),
            Ty::Union(arms) => {
                Ty::Union(arms.iter().map(|ty| ty.substitute(subst)).collect()).flatten_union()
            }
            Ty::Optional(inner) => Ty::Optional(Box::new(inner.substitute(subst))),
            Ty::Function(f) => {
                let mut scoped_subst = subst.clone();
                for param in &f.type_params {
                    scoped_subst.shift_remove(param);
                }
                Ty::Function(FunctionTy {
                    type_params: f.type_params.clone(),
                    params: f
                        .params
                        .iter()
                        .map(|(name, ty)| (*name, ty.substitute(&scoped_subst)))
                        .collect(),
                    return_ty: Box::new(f.return_ty.substitute(&scoped_subst)),
                })
            }
            Ty::Agent(a) => Ty::Agent(AgentTy {
                input_name: a.input_name,
                input_ty: Box::new(a.input_ty.substitute(subst)),
                return_ty: Box::new(a.return_ty.substitute(subst)),
            }),
            Ty::Tool(t) => Ty::Tool(ToolTy {
                bound_params: t
                    .bound_params
                    .iter()
                    .map(|(name, ty)| (*name, ty.substitute(subst)))
                    .collect(),
                model_params: t
                    .model_params
                    .iter()
                    .map(|(name, ty)| (*name, ty.substitute(subst)))
                    .collect(),
                return_ty: Box::new(t.return_ty.substitute(subst)),
            }),
            _ => self.clone(),
        }
    }

    /// Unwrap `Optional`: `T? → Some(T)`, anything else → `None`.
    pub fn inner_optional(&self) -> Option<&Ty> {
        match self {
            Ty::Optional(inner) => Some(inner),
            _ => None,
        }
    }

    /// Flatten nested unions: `(A | (B | C)) → [A, B, C]`.
    pub fn flatten_union(self) -> Ty {
        match self {
            Ty::Union(arms) => {
                let mut flat: Vec<Ty> = Vec::new();
                for arm in arms {
                    match arm.flatten_union() {
                        Ty::Union(inner) => flat.extend(inner),
                        other => flat.push(other),
                    }
                }
                if flat.len() == 1 {
                    flat.remove(0)
                } else {
                    Ty::Union(flat)
                }
            }
            other => other,
        }
    }

    /// Expand `Optional(T)` to `Union([T, Null])` for assignability checks.
    /// This normalises the two nullable representations into one.
    fn expand_optional(self) -> Ty {
        match self {
            Ty::Optional(inner) => Ty::Union(vec![*inner, Ty::null()]).flatten_union(),
            other => other,
        }
    }

    // ── Assignability ─────────────────────────────────────────────────────────

    /// Returns `true` if `self` is structurally assignable to `target`.
    ///
    /// Rules (in priority order):
    /// 1. `Ty::Error` → always assignable (prevents cascading errors)
    /// 2. Identical types are assignable
    /// 3. Any type is assignable to `void`
    /// 4. `null` is assignable to `Optional(T)` or `Union([..., null, ...])`
    /// 5. `T` is assignable to `T?` (T is assignable to Optional(T))
    /// 6. A union `A | B` is assignable to `target` if every arm is assignable
    /// 7. `T` is assignable to a union target if any arm accepts it
    /// 8. Record subtyping: every field in `target` must be in `self` with compatible type
    pub fn is_assignable_to(&self, target: &Ty) -> bool {
        // Error propagation — suppress cascading diagnostics
        if matches!(self, Ty::Error) || matches!(target, Ty::Error) {
            return true;
        }

        // Type variables are handled by quew-unify, not here
        if matches!(self, Ty::Var(_)) || matches!(target, Ty::Var(_)) {
            return true;
        }

        if matches!(self, Ty::GenericParam(_)) || matches!(target, Ty::GenericParam(_)) {
            return true;
        }

        if matches!(self, Ty::Named(_) | Ty::GenericInstance { .. })
            || matches!(target, Ty::Named(_) | Ty::GenericInstance { .. })
        {
            return self == target;
        }

        // Void accepts everything (used as "don't care" return position)
        if matches!(target, Ty::Primitive(PrimTy::Void)) {
            return true;
        }

        // `any` accepts everything and everything accepts `any`.
        if matches!(target, Ty::Primitive(PrimTy::Any))
            || matches!(self, Ty::Primitive(PrimTy::Any))
        {
            return true;
        }

        // Identical structural match
        if self == target {
            return true;
        }

        match (self, target) {
            // null → Optional(T) or union containing null
            (Ty::Primitive(PrimTy::Null), Ty::Optional(_)) => true,
            (Ty::Primitive(PrimTy::Null), Ty::Union(arms)) => arms
                .iter()
                .any(|a| matches!(a, Ty::Primitive(PrimTy::Null))),

            // T → T?  (non-null value into optional slot)
            (src, Ty::Optional(inner)) => src.is_assignable_to(inner),

            // Optional(T) → target: expand and check
            (Ty::Optional(_), _) => self.clone().expand_optional().is_assignable_to(target),

            // Union source: every arm must be assignable to target
            (Ty::Union(arms), _) => arms.iter().all(|arm| arm.is_assignable_to(target)),

            // T → Union target: at least one arm accepts T
            (src, Ty::Union(arms)) => arms.iter().any(|arm| src.is_assignable_to(arm)),

            // Record subtyping: target fields must all be present with compatible
            // types — except optional (`T?`) target fields, which may be omitted.
            (Ty::Record(src_fields), Ty::Record(tgt_fields)) => {
                tgt_fields.iter().all(|(name, tgt_ty)| {
                    match src_fields.get(name) {
                        Some(src_ty) => src_ty.is_assignable_to(tgt_ty),
                        // Absent field: fine when the target marks it optional
                        // or nullable.
                        None => matches!(tgt_ty, Ty::Optional(_))
                            || matches!(tgt_ty, Ty::Primitive(PrimTy::Null)),
                    }
                })
            }

            // Array covariance: element type must be assignable
            (Ty::Array(src), Ty::Array(tgt)) => src.is_assignable_to(tgt),

            // Primitive identity already handled above by the `self == target` arm.
            // All other combinations are incompatible.
            _ => false,
        }
    }
}

// ── Display ───────────────────────────────────────────────────────────────────

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Primitive(p) => write!(f, "{p}"),
            Ty::Named(name) => write!(f, "{name:?}"),
            Ty::GenericInstance { name, args } => {
                let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
                write!(f, "{name:?}<{}>", args.join(", "))
            }
            Ty::Optional(t) => write!(f, "{t}?"),
            Ty::Union(arms) => {
                let s: Vec<String> = arms.iter().map(|a| a.to_string()).collect();
                write!(f, "{}", s.join(" | "))
            }
            Ty::Record(fields) => {
                write!(f, "{{ ")?;
                for (i, (name, ty)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    // InternedStr has no Display — show debug repr (opaque u32 key).
                    // For human-readable output use a TyPrinter with an Interner.
                    write!(f, "{name:?}: {ty}")?;
                }
                write!(f, " }}")
            }
            Ty::Array(elem) => write!(f, "{elem}[]"),
            Ty::GenericParam(name) => write!(f, "{name:?}"),
            Ty::Function(ft) => {
                let generics = if ft.type_params.is_empty() {
                    String::new()
                } else {
                    let params: Vec<String> =
                        ft.type_params.iter().map(|p| format!("{p:?}")).collect();
                    format!("<{}>", params.join(", "))
                };
                let params: Vec<String> = ft
                    .params
                    .iter()
                    .map(|(n, t)| format!("{n:?}: {t}"))
                    .collect();
                write!(f, "{generics}({}) -> {}", params.join(", "), ft.return_ty)
            }
            Ty::Agent(a) => write!(
                f,
                "agent({:?}: {}) -> {}",
                a.input_name, a.input_ty, a.return_ty
            ),
            Ty::Tool(t) => {
                let bound: Vec<String> = t
                    .bound_params
                    .iter()
                    .map(|(n, ty)| format!("@{n:?}: {ty}"))
                    .collect();
                let model: Vec<String> = t
                    .model_params
                    .iter()
                    .map(|(n, ty)| format!("{n:?}: {ty}"))
                    .collect();
                let all = [bound, model].concat();
                write!(f, "tool({}) -> {}", all.join(", "), t.return_ty)
            }
            Ty::Provider(p) => match p {
                ProviderKind::Gemini => write!(f, "gemini"),
                ProviderKind::OpenAi => write!(f, "openai"),
                ProviderKind::Groq => write!(f, "groq"),
            },
            Ty::Var(v) => write!(f, "?{}", v.0),
            Ty::Error => write!(f, "<error>"),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use quew_interner::Interner;
    use std::sync::Arc;

    #[allow(dead_code)]
    fn intern(s: &str) -> InternedStr {
        let i = Arc::new(Interner::new());
        i.intern(s)
    }

    // ── Constructors ──────────────────────────────────────────────────────────

    #[test]
    fn prim_string() {
        assert_eq!(Ty::string(), Ty::Primitive(PrimTy::String));
    }
    #[test]
    fn prim_number() {
        assert_eq!(Ty::number(), Ty::Primitive(PrimTy::Number));
    }
    #[test]
    fn prim_float() {
        assert_eq!(Ty::float(), Ty::Primitive(PrimTy::Float));
    }
    #[test]
    fn prim_bool() {
        assert_eq!(Ty::bool_ty(), Ty::Primitive(PrimTy::Bool));
    }
    #[test]
    fn prim_void() {
        assert_eq!(Ty::void(), Ty::Primitive(PrimTy::Void));
    }
    #[test]
    fn prim_null() {
        assert_eq!(Ty::null(), Ty::Primitive(PrimTy::Null));
    }

    #[test]
    fn optional_wraps_once() {
        let t = Ty::string().optional();
        assert!(matches!(t, Ty::Optional(_)));
    }

    #[test]
    fn optional_idempotent() {
        // T?? should stay T?
        let t = Ty::string().optional().optional();
        assert!(
            matches!(t, Ty::Optional(inner) if matches!(*inner, Ty::Primitive(PrimTy::String)))
        );
    }

    #[test]
    fn inner_optional_some() {
        let t = Ty::number().optional();
        assert_eq!(t.inner_optional(), Some(&Ty::number()));
    }

    #[test]
    fn inner_optional_none_on_plain() {
        assert_eq!(Ty::string().inner_optional(), None);
    }

    // ── is_ok / has_error ─────────────────────────────────────────────────────

    #[test]
    fn error_not_ok() {
        assert!(!Ty::Error.is_ok());
    }
    #[test]
    fn string_is_ok() {
        assert!(Ty::string().is_ok());
    }
    #[test]
    fn error_has_error() {
        assert!(Ty::Error.has_error());
    }
    #[test]
    fn string_no_error() {
        assert!(!Ty::string().has_error());
    }

    #[test]
    fn optional_error_has_error() {
        let t = Ty::Optional(Box::new(Ty::Error));
        assert!(t.has_error());
    }

    #[test]
    fn union_with_error_has_error() {
        let t = Ty::Union(vec![Ty::string(), Ty::Error]);
        assert!(t.has_error());
    }

    // ── flatten_union ─────────────────────────────────────────────────────────

    #[test]
    fn flatten_union_nested() {
        let t = Ty::Union(vec![
            Ty::string(),
            Ty::Union(vec![Ty::number(), Ty::bool_ty()]),
        ])
        .flatten_union();
        assert_eq!(
            t,
            Ty::Union(vec![Ty::string(), Ty::number(), Ty::bool_ty()])
        );
    }

    #[test]
    fn flatten_union_single_collapses() {
        let t = Ty::Union(vec![Ty::string()]).flatten_union();
        assert_eq!(t, Ty::string());
    }

    #[test]
    fn flatten_union_no_change_when_flat() {
        let t = Ty::Union(vec![Ty::string(), Ty::number()]).flatten_union();
        assert_eq!(t, Ty::Union(vec![Ty::string(), Ty::number()]));
    }

    // ── is_assignable_to — primitives ─────────────────────────────────────────

    #[test]
    fn string_to_string() {
        assert!(Ty::string().is_assignable_to(&Ty::string()));
    }
    #[test]
    fn number_to_number() {
        assert!(Ty::number().is_assignable_to(&Ty::number()));
    }
    #[test]
    fn string_not_to_number() {
        assert!(!Ty::string().is_assignable_to(&Ty::number()));
    }
    #[test]
    fn anything_to_void() {
        assert!(Ty::string().is_assignable_to(&Ty::void()));
    }
    #[test]
    fn bool_to_void() {
        assert!(Ty::bool_ty().is_assignable_to(&Ty::void()));
    }

    // ── is_assignable_to — error propagation ──────────────────────────────────

    #[test]
    fn error_to_any() {
        assert!(Ty::Error.is_assignable_to(&Ty::string()));
    }
    #[test]
    fn any_to_error() {
        assert!(Ty::string().is_assignable_to(&Ty::Error));
    }

    // ── is_assignable_to — optional ───────────────────────────────────────────

    #[test]
    fn null_to_optional() {
        assert!(Ty::null().is_assignable_to(&Ty::string().optional()));
    }

    #[test]
    fn string_to_optional_string() {
        assert!(Ty::string().is_assignable_to(&Ty::string().optional()));
    }

    #[test]
    fn number_not_to_optional_string() {
        assert!(!Ty::number().is_assignable_to(&Ty::string().optional()));
    }

    #[test]
    fn optional_string_to_optional_string() {
        let opt = Ty::string().optional();
        assert!(opt.is_assignable_to(&Ty::string().optional()));
    }

    // ── is_assignable_to — union source ───────────────────────────────────────

    #[test]
    fn union_source_all_arms_assignable() {
        let src = Ty::Union(vec![Ty::string(), Ty::number()]);
        // target must accept both arms
        let tgt = Ty::Union(vec![Ty::string(), Ty::number(), Ty::bool_ty()]);
        assert!(src.is_assignable_to(&tgt));
    }

    #[test]
    fn union_source_one_arm_not_assignable() {
        let src = Ty::Union(vec![Ty::string(), Ty::bool_ty()]);
        let tgt = Ty::string(); // bool is not assignable to string
        assert!(!src.is_assignable_to(&tgt));
    }

    // ── is_assignable_to — union target ───────────────────────────────────────

    #[test]
    fn value_to_union_target_matches_one_arm() {
        let tgt = Ty::Union(vec![Ty::string(), Ty::number()]);
        assert!(Ty::string().is_assignable_to(&tgt));
        assert!(Ty::number().is_assignable_to(&tgt));
    }

    #[test]
    fn value_to_union_target_no_match() {
        let tgt = Ty::Union(vec![Ty::string(), Ty::number()]);
        assert!(!Ty::bool_ty().is_assignable_to(&tgt));
    }

    // ── is_assignable_to — null in union ──────────────────────────────────────

    #[test]
    fn null_to_union_containing_null() {
        let tgt = Ty::Union(vec![Ty::string(), Ty::null()]);
        assert!(Ty::null().is_assignable_to(&tgt));
    }

    #[test]
    fn null_not_to_union_without_null() {
        let tgt = Ty::Union(vec![Ty::string(), Ty::number()]);
        assert!(!Ty::null().is_assignable_to(&tgt));
    }

    // ── is_assignable_to — record subtyping ───────────────────────────────────

    #[test]
    fn record_exact_match() {
        let interner = Arc::new(Interner::new());
        let name = interner.intern("name");
        let age = interner.intern("age");

        let mut src = IndexMap::new();
        src.insert(name, Ty::string());
        src.insert(age, Ty::number());

        let mut tgt = IndexMap::new();
        tgt.insert(name, Ty::string());
        tgt.insert(age, Ty::number());

        assert!(Ty::Record(src).is_assignable_to(&Ty::Record(tgt)));
    }

    #[test]
    fn record_src_has_extra_fields_ok() {
        // src has name + age + extra — target only requires name
        let interner = Arc::new(Interner::new());
        let name = interner.intern("name");
        let age = interner.intern("age");
        let extra = interner.intern("extra");

        let mut src = IndexMap::new();
        src.insert(name, Ty::string());
        src.insert(age, Ty::number());
        src.insert(extra, Ty::bool_ty());

        let mut tgt = IndexMap::new();
        tgt.insert(name, Ty::string());

        assert!(Ty::Record(src).is_assignable_to(&Ty::Record(tgt)));
    }

    #[test]
    fn record_missing_required_field_fails() {
        let interner = Arc::new(Interner::new());
        let name = interner.intern("name");
        let age = interner.intern("age");

        let mut src = IndexMap::new();
        src.insert(name, Ty::string());

        let mut tgt = IndexMap::new();
        tgt.insert(name, Ty::string());
        tgt.insert(age, Ty::number()); // required but missing in src

        assert!(!Ty::Record(src).is_assignable_to(&Ty::Record(tgt)));
    }

    #[test]
    fn record_field_type_mismatch_fails() {
        let interner = Arc::new(Interner::new());
        let name = interner.intern("name");

        let mut src = IndexMap::new();
        src.insert(name, Ty::number()); // number where string expected

        let mut tgt = IndexMap::new();
        tgt.insert(name, Ty::string());

        assert!(!Ty::Record(src).is_assignable_to(&Ty::Record(tgt)));
    }

    // ── Callable types ────────────────────────────────────────────────────────

    #[test]
    fn function_ty_display() {
        let interner = Arc::new(Interner::new());
        let a = interner.intern("a");
        let ft = FunctionTy {
            type_params: vec![],
            params: vec![(a, Ty::string())],
            return_ty: Box::new(Ty::bool_ty()),
        };
        let d = format!("{}", Ty::Function(ft));
        // Display shows InternedStr as {:?} (opaque key) + type
        assert!(d.contains("-> bool"), "display: {d}");
    }

    #[test]
    fn generic_substitution_rewrites_record_fields() {
        let interner = Arc::new(Interner::new());
        let t = interner.intern("T");
        let value = interner.intern("value");

        let mut fields = IndexMap::new();
        fields.insert(value, Ty::GenericParam(t));

        let mut subst = IndexMap::new();
        subst.insert(t, Ty::string());

        match Ty::Record(fields).substitute(&subst) {
            Ty::Record(fields) => assert_eq!(fields[&value], Ty::string()),
            other => panic!("expected record, got {other:?}"),
        }
    }

    #[test]
    fn generic_substitution_respects_function_type_param_scope() {
        let interner = Arc::new(Interner::new());
        let t = interner.intern("T");
        let value = interner.intern("value");

        let function = Ty::Function(FunctionTy {
            type_params: vec![t],
            params: vec![(value, Ty::GenericParam(t))],
            return_ty: Box::new(Ty::GenericParam(t)),
        });

        let mut subst = IndexMap::new();
        subst.insert(t, Ty::number());

        match function.substitute(&subst) {
            Ty::Function(function) => {
                assert_eq!(function.params[0].1, Ty::GenericParam(t));
                assert_eq!(*function.return_ty, Ty::GenericParam(t));
            }
            other => panic!("expected function, got {other:?}"),
        }
    }

    #[test]
    fn agent_ty_display() {
        let interner = Arc::new(Interner::new());
        let input = interner.intern("input");
        let at = AgentTy {
            input_name: input,
            input_ty: Box::new(Ty::string()),
            return_ty: Box::new(Ty::void()),
        };
        let d = format!("{}", Ty::Agent(at));
        assert!(d.starts_with("agent("), "display: {d}");
    }

    #[test]
    fn tool_ty_bound_and_model_params() {
        let interner = Arc::new(Interner::new());
        let id = interner.intern("id");
        let is_admin = interner.intern("isAdmin");

        let tt = ToolTy {
            bound_params: vec![(id, Ty::string())],
            model_params: vec![(is_admin, Ty::bool_ty())],
            return_ty: Box::new(Ty::string()),
        };
        let d = format!("{}", Ty::Tool(tt));
        // Both bound and model params appear; exact name repr is {:?}
        assert!(d.contains("tool("), "display: {d}");
        assert!(d.contains("-> string"), "display: {d}");
    }

    // ── Display ───────────────────────────────────────────────────────────────

    #[test]
    fn display_string() {
        assert_eq!(Ty::string().to_string(), "string");
    }
    #[test]
    fn display_number() {
        assert_eq!(Ty::number().to_string(), "number");
    }
    #[test]
    fn display_optional() {
        assert_eq!(Ty::string().optional().to_string(), "string?");
    }
    #[test]
    fn display_error() {
        assert_eq!(Ty::Error.to_string(), "<error>");
    }
    #[test]
    fn display_var() {
        assert_eq!(Ty::Var(TyVar(3)).to_string(), "?3");
    }

    #[test]
    fn display_union() {
        let t = Ty::Union(vec![Ty::string(), Ty::number()]);
        assert_eq!(t.to_string(), "string | number");
    }

    #[test]
    fn display_record() {
        let interner = Arc::new(Interner::new());
        let name = interner.intern("name");
        let mut fields = IndexMap::new();
        fields.insert(name, Ty::string());
        let t = Ty::Record(fields);
        // InternedStr displayed as {:?}; check the type part is visible
        assert!(t.to_string().contains("string"), "display: {}", t);
    }

    // ── Provider ──────────────────────────────────────────────────────────────

    #[test]
    fn provider_gemini() {
        assert_eq!(Ty::Provider(ProviderKind::Gemini).to_string(), "gemini");
    }
    #[test]
    fn provider_openai() {
        assert_eq!(Ty::Provider(ProviderKind::OpenAi).to_string(), "openai");
    }
    #[test]
    fn provider_groq() {
        assert_eq!(Ty::Provider(ProviderKind::Groq).to_string(), "groq");
    }
}
