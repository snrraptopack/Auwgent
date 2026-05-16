//! # quew-ast
//!
//! AST node definitions for the quew DSL.
//!
//! ## Design rules
//!
//! 1. Every node struct/enum carries a `span: Span` field — no exceptions.
//! 2. All string-like fields (names, identifiers) use `InternedStr`, not `String`.
//! 3. No business logic here — only data. Validation belongs in `quew-checker`.
//!
//! ## Status: stub
//!
//! Node definitions will be added as the grammar is specified in `quew-parser`.

// TODO: define AST node structs as grammar productions are finalized.
