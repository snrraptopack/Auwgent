//! Centralized testing harness for Auwgent.
//!
//! This crate is intended to host deterministic and richer runtime tests that
//! exercise generated Rust fixtures without bloating the SDK target layer.

/// Returns the crate name for simple smoke assertions in tests.
pub fn crate_name() -> &'static str {
    "auwgent-testing"
}