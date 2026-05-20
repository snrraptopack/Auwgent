//! # quew-runtime
//!
//! **Single responsibility:** execute compiled `QuewGraphIR` into runtime values.
//!
//! This crate is the deterministic graph executor (Phase 1 & 2 of the runtime).
//! It evaluates pure computation graphs — no LLM, no host tools, no async.
//! Those layers are added in later plans.
//!
//! ## Architecture
//!
//! ```text
//! QuewGraphIR (immutable, from compiler)
//!        │
//!        ▼
//!   ┌─────────────┐
//!   │ Execution   │──► walks AgentGraph nodes
//!   └─────────────┘
//!        │
//!        ├──► eval_expr ──► Value
//!        │
//!        ├──► NativeRegistry ──► @@rust builtin dispatch
//!        │
//!        └──► node outputs map ──► final result
//! ```
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`value`] | Runtime value representation (`Value`, `ValueError`) |
//! | [`eval`] | Pure expression evaluator (`eval_expr`, `EvalError`) |
//! | [`execution`] | Graph walker (`Execution::run`, `ExecutionError`) |
//! | [`native`] | Native function registry (`NativeRegistry`, `NativeEntry`) |

pub mod eval;
pub mod execution;
pub mod native;
pub mod value;
