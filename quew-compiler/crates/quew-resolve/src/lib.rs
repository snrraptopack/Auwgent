//! # quew-resolve
//!
//! **Single responsibility:** resolves cross-file imports by building and querying
//! a module dependency graph, including cycle detection.
//!
//! ## What belongs here
//!
//! - [`ModuleGraph`] — a directed graph of `SourceId → [imported SourceId]`.
//! - [`ImportResolver`] — walks each file's AST import declarations, locates the
//!   imported file on disk via `SourceMap`, and populates the `ModuleGraph`.
//! - Cycle detection — reports an import cycle as a `Diagnostic` with all involved
//!   files listed in the error chain.
//!
//! ## What does NOT belong here
//!
//! - Single-file symbol binding (→ `quew-scope`)
//! - Type checking of imported names (→ `quew-checker`)
//! - File reading or source registration (→ `quew-source`)
//!
//! ## Design rules
//!
//! 1. `rayon` is used for parallel file resolution — each file's imports can be
//!    resolved independently before the graph is merged.
//! 2. Cycle detection uses DFS with a "gray set" (currently being visited) and
//!    "black set" (fully visited). This is the standard approach from CLRS.
//! 3. All errors are returned as `Vec<Diagnostic>` — never panics on bad input.
//!
//! ## Status: stub
//!
//! Will be implemented once `quew-ast` defines import declaration nodes.

// TODO: implement ModuleGraph, ImportResolver, and cycle detection after quew-ast.
