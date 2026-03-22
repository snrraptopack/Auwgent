//! # auwgent-parser
//!
//! Chumsky-based parser for the Auwgent DSL.
//! Consumes a token stream from `auwgent-lexer` and produces an AST from `auwgent-ast`.
//! Chumsky provides built-in error recovery and human-readable error messages.

mod config;
mod expr;
mod primitives;
mod stmt;
mod toplevel;
mod types;

use auwgent_ast::Model;
use auwgent_errors::{Diagnostic, Span};
use auwgent_lexer::{Token, TokenKind};
use chumsky::prelude::*;

/// Parse a token stream into a Model AST.
/// Returns (model, diagnostics) — model is best-effort even with errors.
pub fn parse(tokens: &[Token]) -> (Model, Vec<Diagnostic>) {
    let eoi = tokens.last().map(|t| t.span.end).unwrap_or(0);

    let stream = chumsky::Stream::from_iter(
        eoi..eoi,
        tokens
            .iter()
            .map(|t| (t.kind.clone(), t.span.start..t.span.end)),
    );

    let (model, errors) = toplevel::model_parser().parse_recovery(stream);
    let diagnostics = errors
        .into_iter()
        .map(|e| chumsky_to_diagnostic(e))
        .collect();
    (
        model.unwrap_or(Model {
            imports: vec![],
            elements: vec![],
        }),
        diagnostics,
    )
}

/// Convert a chumsky `Simple` error into an ariadne-friendly `Diagnostic`.
fn chumsky_to_diagnostic(err: Simple<TokenKind>) -> Diagnostic {
    let span_range = err.span();
    let span = Span::new(span_range.start, span_range.end);

    // Build "expected" list
    let expected: Vec<String> = err
        .expected()
        .filter_map(|e| e.as_ref().map(|tok| format!("{}", tok)))
        .collect();

    // What was found?
    let found = err
        .found()
        .map(|tok| format!("{}", tok))
        .unwrap_or_else(|| "end of file".to_string());

    // Build message
    let message = if expected.is_empty() {
        format!("Unexpected {}", found)
    } else if expected.len() == 1 {
        format!("Expected {} but found {}", expected[0], found)
    } else {
        let last = expected.last().unwrap().clone();
        let rest = expected[..expected.len() - 1].join(", ");
        format!("Expected {} or {} but found {}", rest, last, found)
    };

    // Add contextual help
    let help = get_help(&expected, &found);

    let mut diag = Diagnostic::error(&message, span).with_label(span, &message);

    if let Some(h) = help {
        diag = diag.with_help(h);
    }

    diag
}

/// Provide contextual help based on what was expected/found.
fn get_help(expected: &[String], found: &str) -> Option<String> {
    let joined = expected.join(" ");
    
    // Special case: @example parsing errors
    if joined.contains("'='") && (found.contains("string") || found.contains("number") || found.contains("boolean")) {
        return Some("@example requires named arguments using '=' syntax, e.g., @example(param = \"value\", count = 42). Don't use positional arguments like @example(\"value\").".into());
    }
    
    if joined.contains("':'") {
        Some("Properties need a colon between name and type, e.g. `name: string`. Tools need a return type: `tool name(params): ReturnType`".into())
    } else if joined.contains("','") {
        Some("This usually means a separator issue. In field blocks like `type`, `input`, `context`, and `output`, try adding a comma or starting the next field on a new entry after the previous type/description is complete.".into())
    } else if joined.contains("'{'") {
        Some("Every agent, helper, and config block must be wrapped in { }".into())
    } else if joined.contains("'|'") {
        Some("This often means the previous field used a string-literal type and the next field started immediately after it. In `type` and object type blocks, finish the field first, then start the next one on a new entry or add a comma for clarity.".into())
    } else if joined.contains("identifier") {
        Some("Names must start with a letter or underscore".into())
    } else {
        None
    }
}
