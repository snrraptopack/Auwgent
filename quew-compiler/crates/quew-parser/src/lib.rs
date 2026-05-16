//! # quew-parser
//!
//! **Single responsibility:** consume a [`LexResult`] and produce a [`ParseResult`]
//! containing the full [`Module`] AST and any parse-error diagnostics.
//!
//! ## Design
//!
//! - Built on **chumsky 0.13** with the `pratt` feature for operator precedence.
//! - Input is `&[(TokenKind, Span)]` from `quew-lexer`. Newlines are preserved
//!   in the stream and consumed explicitly by combinators that need them as
//!   statement separators.
//! - The parser never aborts. Errors are recovered via `skip_then_any_output`
//!   at the statement level and `via_parser` at the item level.
//! - Identifier and literal values are extracted from the source string by span
//!   in `map_with` — `TokenKind` itself carries no payload.
//!
//! ## Modules
//!
//! | Module | Responsibility |
//! |--------|---------------|
//! | `common` | Type aliases, leaf combinators, `make_stream()` |
//! | `parse_type` | `TypeExpr` combinators |
//! | `parse_annot` | Annotation combinators |
//! | `parse_param` | Parameter list combinators |
//! | `parse_expr` | Expression parser with `.pratt()` |
//! | `parse_stmt` | Statement combinators |
//! | `parse_item` | Top-level item combinators, `module()` |

mod common;
mod parse_annot;
mod parse_expr;
mod parse_item;
mod parse_param;
mod parse_stmt;
mod parse_type;

use std::sync::Arc;

use chumsky::prelude::*;
use quew_ast::Module;
use quew_errors::{Diagnostic, Span};
use quew_interner::Interner;
use quew_lexer::LexResult;

use crate::common::make_stream;
use crate::parse_item::module;

// ── Public API ────────────────────────────────────────────────────────────────

/// The result of parsing a single source file.
#[derive(Debug)]
pub struct ParseResult {
    /// The parsed module. Even when there are errors, the module is as complete
    /// as error recovery allows.
    pub module: Module,
    /// Parse errors. Empty on a clean parse.
    pub errors: Vec<Diagnostic>,
}

/// Parse a token stream into a [`Module`].
///
/// # Arguments
///
/// * `result` — the [`LexResult`] from `quew_lexer::lex()`
/// * `source` — the original source string (span-based value extraction)
/// * `interner` — shared string interner for zero-cost name deduplication
pub fn parse(result: &LexResult, source: &str, interner: &Arc<Interner>) -> ParseResult {
    let stream = make_stream(&result.tokens, source.len());
    let interner = Arc::clone(interner);

    let (module, errs) = module(source, interner).parse(stream).into_output_errors();

    // Convert chumsky Rich errors into Diagnostic.
    let errors: Vec<Diagnostic> = errs
        .into_iter()
        .map(|err| {
            let span = Span::new(err.span().start, err.span().end);
            Diagnostic::error(format!("{}", err.reason()), span)
        })
        .collect();

    // If the module is None (total failure), return an empty module.
    let module = module.unwrap_or_else(|| Module {
        items: vec![],
        span: Span::new(0, source.len()),
    });

    ParseResult { module, errors }
}
