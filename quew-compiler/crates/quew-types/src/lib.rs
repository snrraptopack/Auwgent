//! # quew-types
//!
//! The structural type system for the quew DSL.
//!
//! This is the most critical crate in the compiler. It is the single source of truth
//! for what types exist, how they relate, and what operations are valid between them.
//!
//! ## Core responsibilities
//!
//! - **`Ty`**: the canonical in-memory representation of a quew type.
//! - **Subtyping**: is type `A` assignable to type `B`?
//! - **Union discrimination**: given `A | B | C`, which arm does a value belong to?
//! - **Shape checking**: does a record literal conform to a declared schema?
//! - **Unification** (via `ena`): type variable inference and unification table.
//!
//! ## Design rules
//!
//! 1. All string fields use `InternedStr` — no `String` or `&str` inside type data.
//! 2. Record field order is preserved via `IndexMap` (deterministic, ordered).
//! 3. `Ty` is `Clone` — the checker passes copies around freely.
//! 4. No diagnostic rendering here. The checker owns diagnostics; types only describe.
//!
//! ## Status: stub
//!
//! The `Ty` enum and associated operations will be defined as the grammar and checker
//! are developed together.

// TODO: define Ty enum, subtype relation, shape checking, and unification table.
