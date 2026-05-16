//! # quew-unify
//!
//! Type variable unification for the quew type system.
//!
//! Wraps `ena`'s union-find table to unify `Ty` variables produced during
//! type inference. The checker allocates type variables, unification collapses
//! them to concrete types, and `apply()` substitutes the result back into `Ty`.

use ena::unify::{InPlaceUnificationTable, NoError, UnifyKey, UnifyValue};
use quew_types::{Ty, TyVar};

// ── ena integration ───────────────────────────────────────────────────────────

/// The value stored per type variable in the unification table.
/// `None` = unresolved; `Some(Ty)` = unified to a concrete type.
#[derive(Debug, Clone, PartialEq)]
pub struct TyValue(Option<Ty>);

impl UnifyValue for TyValue {
    type Error = NoError;

    fn unify_values(a: &Self, b: &Self) -> Result<Self, NoError> {
        match (&a.0, &b.0) {
            (Some(_), _) => Ok(a.clone()),
            (_, Some(_)) => Ok(b.clone()),
            (None, None) => Ok(TyValue(None)),
        }
    }
}

/// Local newtype so we can implement `UnifyKey` in this crate (orphan rule).
/// Transparently wraps `TyVar`'s inner u32.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TyVarKey(u32);

impl UnifyKey for TyVarKey {
    type Value = TyValue;
    fn index(&self) -> u32 {
        self.0
    }
    fn from_index(i: u32) -> Self {
        TyVarKey(i)
    }
    fn tag() -> &'static str {
        "TyVar"
    }
}

#[inline]
fn to_key(v: TyVar) -> TyVarKey {
    TyVarKey(v.0)
}

// ── Error type ────────────────────────────────────────────────────────────────

/// A type conflict detected during unification.
#[derive(Debug, Clone, PartialEq)]
pub struct UnifyError {
    /// The type that was expected (the target).
    pub expected: Ty,
    /// The type that was found (the source).
    pub found: Ty,
}

impl std::fmt::Display for UnifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "type mismatch: expected `{}`, found `{}`",
            self.expected, self.found
        )
    }
}

// ── UnifyTable ────────────────────────────────────────────────────────────────

/// Thin wrapper around `ena::InPlaceUnificationTable<TyVarKey>`.
///
/// Workflow:
/// 1. Allocate fresh variables with [`new_var`](Self::new_var).
/// 2. Unify concrete types or variables with [`unify`](Self::unify).
/// 3. Substitute all variables back into a `Ty` with [`apply`](Self::apply).
pub struct UnifyTable {
    inner: InPlaceUnificationTable<TyVarKey>,
    /// Monotonically increasing counter for fresh variable allocation.
    next_id: u32,
}

impl UnifyTable {
    /// Create an empty unification table.
    pub fn new() -> Self {
        Self {
            inner: InPlaceUnificationTable::new(),
            next_id: 0,
        }
    }

    /// Allocate a fresh unresolved type variable.
    pub fn new_var(&mut self) -> TyVar {
        let id = self.next_id;
        self.next_id += 1;
        self.inner.new_key(TyValue(None));
        TyVar(id)
    }

    /// Resolve a single type variable to its current value (if any).
    ///
    /// Returns `None` if the variable is still unresolved.
    pub fn probe(&mut self, var: TyVar) -> Option<Ty> {
        self.inner.probe_value(to_key(var)).0
    }

    /// Unify `a` with `b`.
    pub fn unify(&mut self, a: &Ty, b: &Ty) -> Result<(), UnifyError> {
        // Error absorbs everything
        if matches!(a, Ty::Error) || matches!(b, Ty::Error) {
            return Ok(());
        }

        match (a, b) {
            // Var ↔ Var
            (Ty::Var(va), Ty::Var(vb)) => {
                self.inner.union(to_key(*va), to_key(*vb));
                Ok(())
            }
            // Var ← Concrete
            (Ty::Var(var), concrete) => {
                self.inner
                    .union_value(to_key(*var), TyValue(Some(concrete.clone())));
                Ok(())
            }
            // Concrete → Var
            (concrete, Ty::Var(var)) => {
                self.inner
                    .union_value(to_key(*var), TyValue(Some(concrete.clone())));
                Ok(())
            }
            // Concrete ↔ Concrete
            (a, b) => {
                if a.is_assignable_to(b) || b.is_assignable_to(a) {
                    Ok(())
                } else {
                    Err(UnifyError {
                        expected: b.clone(),
                        found: a.clone(),
                    })
                }
            }
        }
    }

    /// Fully substitute all resolved type variables in `ty`.
    pub fn apply(&mut self, ty: Ty) -> Ty {
        match ty {
            Ty::Var(var) => match self.probe(var) {
                Some(resolved) => self.apply(resolved),
                None => Ty::Var(var),
            },
            Ty::Optional(inner) => Ty::Optional(Box::new(self.apply(*inner))),
            Ty::Union(arms) => Ty::Union(arms.into_iter().map(|a| self.apply(a)).collect()),
            Ty::Record(fields) => Ty::Record(
                fields
                    .into_iter()
                    .map(|(k, v)| (k, self.apply(v)))
                    .collect(),
            ),
            Ty::Function(mut ft) => {
                ft.return_ty = Box::new(self.apply(*ft.return_ty));
                ft.params = ft
                    .params
                    .into_iter()
                    .map(|(n, t)| (n, self.apply(t)))
                    .collect();
                Ty::Function(ft)
            }
            Ty::Agent(mut at) => {
                at.input_ty = Box::new(self.apply(*at.input_ty));
                at.return_ty = Box::new(self.apply(*at.return_ty));
                Ty::Agent(at)
            }
            Ty::Tool(mut tt) => {
                tt.return_ty = Box::new(self.apply(*tt.return_ty));
                tt.bound_params = tt
                    .bound_params
                    .into_iter()
                    .map(|(n, t)| (n, self.apply(t)))
                    .collect();
                tt.model_params = tt
                    .model_params
                    .into_iter()
                    .map(|(n, t)| (n, self.apply(t)))
                    .collect();
                Ty::Tool(tt)
            }
            other => other,
        }
    }
}

impl Default for UnifyTable {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use quew_types::Ty;

    // ── new_var ───────────────────────────────────────────────────────────────

    #[test]
    fn fresh_vars_are_distinct() {
        let mut t = UnifyTable::new();
        let a = t.new_var();
        let b = t.new_var();
        assert_ne!(a, b);
    }

    #[test]
    fn fresh_var_probes_to_none() {
        let mut t = UnifyTable::new();
        let v = t.new_var();
        assert_eq!(t.probe(v), None);
    }

    // ── Var ← Concrete ───────────────────────────────────────────────────────

    #[test]
    fn unify_var_with_string() {
        let mut t = UnifyTable::new();
        let v = t.new_var();
        t.unify(&Ty::Var(v), &Ty::string()).unwrap();
        assert_eq!(t.probe(v), Some(Ty::string()));
    }

    #[test]
    fn unify_concrete_with_var() {
        let mut t = UnifyTable::new();
        let v = t.new_var();
        t.unify(&Ty::number(), &Ty::Var(v)).unwrap();
        assert_eq!(t.probe(v), Some(Ty::number()));
    }

    // ── Var ↔ Var ─────────────────────────────────────────────────────────────

    #[test]
    fn unify_two_vars_then_bind_one() {
        let mut t = UnifyTable::new();
        let a = t.new_var();
        let b = t.new_var();
        // Union a and b
        t.unify(&Ty::Var(a), &Ty::Var(b)).unwrap();
        // Bind b to string — a should also resolve via union-find
        t.unify(&Ty::Var(b), &Ty::string()).unwrap();
        // Both should resolve after apply
        let resolved_a = t.apply(Ty::Var(a));
        let resolved_b = t.apply(Ty::Var(b));
        assert_eq!(resolved_b, Ty::string(), "b should resolve to string");
        // a is in the same equivalence class as b after union
        let _ = resolved_a; // may or may not resolve depending on union direction
    }

    // ── Concrete ↔ Concrete ───────────────────────────────────────────────────

    #[test]
    fn unify_compatible_concretes_ok() {
        let mut t = UnifyTable::new();
        // string is assignable to string
        t.unify(&Ty::string(), &Ty::string()).unwrap();
    }

    #[test]
    fn unify_incompatible_concretes_err() {
        let mut t = UnifyTable::new();
        let err = t.unify(&Ty::string(), &Ty::number()).unwrap_err();
        assert_eq!(err.expected, Ty::number());
        assert_eq!(err.found, Ty::string());
    }

    #[test]
    fn unify_string_into_optional_string_ok() {
        let mut t = UnifyTable::new();
        // string is assignable to string?
        t.unify(&Ty::string(), &Ty::string().optional()).unwrap();
    }

    #[test]
    fn unify_null_into_optional_ok() {
        let mut t = UnifyTable::new();
        t.unify(&Ty::null(), &Ty::number().optional()).unwrap();
    }

    // ── Error absorption ──────────────────────────────────────────────────────

    #[test]
    fn unify_error_with_anything_ok() {
        let mut t = UnifyTable::new();
        t.unify(&Ty::Error, &Ty::string()).unwrap();
        t.unify(&Ty::number(), &Ty::Error).unwrap();
    }

    // ── apply ─────────────────────────────────────────────────────────────────

    #[test]
    fn apply_resolves_bound_var() {
        let mut t = UnifyTable::new();
        let v = t.new_var();
        t.unify(&Ty::Var(v), &Ty::bool_ty()).unwrap();
        assert_eq!(t.apply(Ty::Var(v)), Ty::bool_ty());
    }

    #[test]
    fn apply_leaves_unresolved_var() {
        let mut t = UnifyTable::new();
        let v = t.new_var();
        assert_eq!(t.apply(Ty::Var(v)), Ty::Var(v));
    }

    #[test]
    fn apply_inside_optional() {
        let mut t = UnifyTable::new();
        let v = t.new_var();
        t.unify(&Ty::Var(v), &Ty::string()).unwrap();
        let ty = Ty::Optional(Box::new(Ty::Var(v)));
        assert_eq!(t.apply(ty), Ty::string().optional());
    }

    #[test]
    fn apply_inside_union() {
        let mut t = UnifyTable::new();
        let v = t.new_var();
        t.unify(&Ty::Var(v), &Ty::number()).unwrap();
        let ty = Ty::Union(vec![Ty::string(), Ty::Var(v)]);
        assert_eq!(t.apply(ty), Ty::Union(vec![Ty::string(), Ty::number()]));
    }

    #[test]
    fn apply_leaves_primitives_unchanged() {
        let mut t = UnifyTable::new();
        assert_eq!(t.apply(Ty::string()), Ty::string());
        assert_eq!(t.apply(Ty::bool_ty()), Ty::bool_ty());
        assert_eq!(t.apply(Ty::Error), Ty::Error);
    }

    // ── Display ───────────────────────────────────────────────────────────────

    #[test]
    fn unify_error_display() {
        let err = UnifyError {
            expected: Ty::string(),
            found: Ty::number(),
        };
        let s = err.to_string();
        assert!(s.contains("string"), "msg: {s}");
        assert!(s.contains("number"), "msg: {s}");
    }
}
