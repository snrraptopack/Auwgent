//! Native function registry.
//!
//! [`NativeRegistry`] maps stable `@@rust("id")` strings to Rust function
//! implementations. This is the bridge between compiled Quew code and the
//! host runtime.
//!
//! **Single responsibility:** This module provides *only* the registration
//! container and dispatch mechanism. Actual builtin implementations live in
//! a separate `quew-stdlib` crate (or host-provided crates) and register
//! themselves at link time (e.g. via the `inventory` crate). The runtime
//! never hardcodes a list of builtins — it only holds what was injected.
//!
//! # Example
//!
//! ```
//! use quew_runtime::native::{NativeRegistry, NativeHandler};
//! use quew_runtime::value::Value;
//!
//! let mut registry = NativeRegistry::new();
//! registry.register(
//!     "std.string.is_empty",
//!     NativeHandler::Sync(|args| {
//!         let s = args[0].as_str().ok_or("expected string")?;
//!         Ok(Value::Bool(s.is_empty()))
//!     }),
//! );
//! ```

use std::collections::HashMap;

use crate::value::{Value, ValueError};

/// A registry of native functions keyed by their stable `@@rust("id")` string.
#[derive(Debug, Default)]
pub struct NativeRegistry {
    entries: HashMap<String, NativeEntry>,
}

/// A native function implementation.
#[derive(Clone, Copy)]
pub struct NativeEntry {
    /// Stable dispatch id (e.g. `"std.string.len"`).
    pub id: &'static str,
    /// The handler implementation.
    pub handler: NativeHandler,
}

impl std::fmt::Debug for NativeEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "NativeEntry {{ id: {:?}, handler: {:?} }}",
            self.id, self.handler
        )
    }
}

/// The callable body of a native function.
#[derive(Clone, Copy)]
pub enum NativeHandler {
    /// A synchronous pure function.
    Sync(fn(&[Value]) -> Result<Value, NativeError>),
}

impl std::fmt::Debug for NativeHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NativeHandler::Sync(_) => write!(f, "NativeHandler::Sync(<fn>)"),
        }
    }
}

/// An error produced by a native function.
#[derive(Debug, Clone, PartialEq)]
pub struct NativeError {
    pub message: String,
}

impl NativeError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
        }
    }
}

impl From<ValueError> for NativeError {
    fn from(e: ValueError) -> Self {
        Self::new(e.to_string())
    }
}

impl From<&str> for NativeError {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl std::fmt::Display for NativeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for NativeError {}

inventory::collect!(NativeEntry);

impl NativeRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Collect all link-time registered builtins into a registry.
    ///
    /// Call this once at runtime startup when `quew-stdlib` (or other
    /// builtin-providing crates) are linked into the binary.
    pub fn collect() -> Self {
        let mut reg = Self::new();
        for entry in inventory::iter::<NativeEntry> {
            reg.entries.insert(entry.id.to_string(), entry.clone());
        }
        reg
    }

    /// Register a native function under the given id.
    pub fn register(&mut self, id: impl Into<String>, handler: NativeHandler) {
        let id = id.into();
        self.entries.insert(
            id.clone(),
            NativeEntry {
                id: Box::leak(id.into_boxed_str()),
                handler,
            },
        );
    }

    /// Look up a native function by id.
    pub fn get(&self, id: &str) -> Option<&NativeEntry> {
        self.entries.get(id)
    }

    /// Returns true if the registry contains the given id.
    pub fn contains(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_lookup() {
        let mut reg = NativeRegistry::new();
        reg.register(
            "test.identity",
            NativeHandler::Sync(|args| Ok(args[0].clone())),
        );

        assert!(reg.contains("test.identity"));
        assert!(!reg.contains("test.missing"));
    }

    #[test]
    fn dispatch_sync_function() {
        let mut reg = NativeRegistry::new();
        reg.register(
            "test.double",
            NativeHandler::Sync(|args| {
                let n = args[0].as_number().ok_or("expected number")?;
                Ok(Value::Number(n * 2))
            }),
        );

        let entry = reg.get("test.double").unwrap();
        let result = match &entry.handler {
            NativeHandler::Sync(f) => f(&[Value::Number(21)]).unwrap(),
        };
        assert_eq!(result, Value::Number(42));
    }
}
