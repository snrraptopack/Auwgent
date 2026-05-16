//! # quew-unify
//!
//! **Single responsibility:** maintains a unification table for type variables
//! during type inference in the quew checker.
//!
//! ## What unification is
//!
//! When the checker encounters an expression whose type is not yet known (e.g., the
//! return type of a helper call), it creates a fresh `TyVar`. As the checker collects
//! constraints (`TyVar must equal Ty::String`), it "unifies" the variable with the
//! concrete type. If two constraints conflict, unification fails and a diagnostic
//! is emitted.
//!
//! This is the same algorithm used by rustc, Haskell's GHC, and most typed functional
//! compilers. We use `ena` — rustc's own union-find implementation — as the underlying
//! data structure.
//!
//! ## Design rules
//!
//! 1. This crate owns no diagnostics — `UnifyError` is returned to `quew-checker`
//!    which turns it into a `Diagnostic`.
//! 2. `TyVar` is `Copy` and `u32`-sized. Store freely.
//! 3. The `UnifyTable` is not thread-safe by design — the checker is single-threaded
//!    per compilation unit.
//!
//! ## Status: stub
//!
//! The full `UnifyTable` and `TyVar` implementation will be added when `quew-types`
//! defines the `Ty` enum.

// TODO: implement UnifyTable and TyVar after quew-types::Ty is defined.
// Reference: ena's UnifyTable<ut: UnifyKey> API.
