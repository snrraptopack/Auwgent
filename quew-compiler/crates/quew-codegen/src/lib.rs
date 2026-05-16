//! # quew-codegen
//!
//! Thin type-stub generators for the quew DSL.
//!
//! In v2, codegen emits **type-only** wrappers — no runtime SDK imports, no
//! JSON IR references. The host SDK is independent of the generated stubs.
//!
//! ## Planned targets
//!
//! - TypeScript (`.d.ts` + `.ts` type definitions)
//! - Python (`.pyi` type stubs)
//! - Dart (`abstract class` declarations)
//! - Rust (pure trait + struct definitions)
//!
//! ## Status: stub

// TODO: implement per-target code generation after quew-ir is stable.
