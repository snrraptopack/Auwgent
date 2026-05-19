//! Resolved expression sidecar — bridges checker type information to the IR lowerer.
//!
//! The checker infers types and resolves calls (functions, tools, agents, extension
//! methods), but `CheckResult` historically discarded all per-expression resolution.
//! This module provides a `ResolvedExpressionMap` that the checker populates and
//! the lowerer consumes, eliminating the need for the lowerer to re-do lookup logic
//! without type context.

use std::collections::HashMap;

use quew_errors::Span;
use quew_interner::InternedStr;
use quew_types::Ty;

/// Maps expression spans to their checker-resolved meanings.
///
/// Every AST expression carries a unique `Span` (byte offsets into source text).
/// The checker uses these spans as stable keys so the lowerer can look up what
/// a given expression was resolved to during type inference.
#[derive(Debug, Clone, Default)]
pub struct ResolvedExpressionMap {
    /// Resolved call targets keyed by the call expression's span.
    pub calls: HashMap<Span, ResolvedCall>,
}

impl ResolvedExpressionMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that a call expression at `span` resolves to `call`.
    pub fn record_call(&mut self, span: Span, call: ResolvedCall) {
        self.calls.insert(span, call);
    }

    /// Look up the resolved call for a call expression at `span`.
    pub fn get_call(&self, span: Span) -> Option<&ResolvedCall> {
        self.calls.get(&span)
    }
}

/// What the checker determined a call expression refers to.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedCall {
    /// The kind of callable that was resolved.
    pub kind: CallKind,
    /// The resolved target name.
    ///
    /// - `Function`: name of the function in `definitions.functions`
    /// - `Tool`: name of the tool in `definitions.tools`
    /// - `Agent`: name of the agent in `definitions.agents`
    /// - `ExtensionMethod`: method name (disambiguated by `receiver_ty`)
    pub target: InternedStr,
    /// For extension methods: the inferred receiver type used to select the
    /// correct overload. `None` for non-extension calls.
    pub receiver_ty: Option<Ty>,
}

impl ResolvedCall {
    pub fn new(kind: CallKind, target: InternedStr) -> Self {
        Self {
            kind,
            target,
            receiver_ty: None,
        }
    }

    pub fn function(target: InternedStr) -> Self {
        Self::new(CallKind::Function, target)
    }

    pub fn tool(target: InternedStr) -> Self {
        Self::new(CallKind::Tool, target)
    }

    pub fn agent(target: InternedStr) -> Self {
        Self::new(CallKind::Agent, target)
    }

    pub fn extension_method(target: InternedStr, receiver_ty: Ty) -> Self {
        Self {
            kind: CallKind::ExtensionMethod,
            target,
            receiver_ty: Some(receiver_ty),
        }
    }
}

/// Kinds of callable the checker can resolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallKind {
    /// A plain `function foo()` call.
    Function,
    /// An extension method call: `value.isEmpty()`.
    ExtensionMethod,
    /// A host-backed or DSL tool call.
    Tool,
    /// A child-agent call.
    Agent,
}
