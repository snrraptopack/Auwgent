//! # quew-scope
//!
//! **Single responsibility:** builds a symbol table by walking the AST of a single
//! quew source file.
//!
//! ## What belongs here
//!
//! - [`Scope`] — a stack of bindings representing the current lexical scope.
//! - [`SymbolTable`] — the final output for a single file: all declared names
//!   mapped to their `Ty` and the `Span` where they were defined.
//! - The scope-building pass that walks the AST and populates the table.
//!
//! ## What does NOT belong here
//!
//! - Cross-file resolution (→ `quew-resolve`)
//! - Type inference or unification (→ `quew-unify`)
//! - Diagnostic rendering (→ `quew-errors`)
//!
//! ## Design rules
//!
//! 1. All names are stored as `InternedStr`, never as `String`.
//! 2. `IndexMap` is used for all name → info maps so that insertion order
//!    is preserved and iteration is deterministic.
//! 3. The scope builder emits `Vec<Diagnostic>` for errors (undefined name,
//!    duplicate definition) and continues — it does not panic or abort.
//!
//! ## Status: stub
//!
//! The `Scope` and `SymbolTable` types will be defined once `quew-ast` and
//! `quew-types::Ty` are finalized.

// TODO: implement Scope, SymbolTable, and scope builder pass after quew-ast is defined.
