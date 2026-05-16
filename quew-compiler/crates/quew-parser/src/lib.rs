//! # quew-parser
//!
//! chumsky 0.13 parser for the quew DSL.
//!
//! ## API contract (planned)
//!
//! ```ignore
//! pub fn parse(
//!     tokens: &[(TokenKind, Span)],
//!     interner: &Arc<Interner>,
//! ) -> (Option<ast::Module>, Vec<Diagnostic>)
//! ```
//!
//! Returns a partial AST even on error (best-effort error recovery). The
//! `Vec<Diagnostic>` may be non-empty even when `Option<ast::Module>` is `Some`.
//!
//! ## Key chumsky 0.13 concepts used
//!
//! - `Parser<'a, I, O, E>` — all parsers are generic over an `Input` token slice.
//! - `Rich<Token>` — the error type; provides labelled expected/found messages.
//! - `recursive()` — for left-recursive and mutually recursive productions.
//! - `pratt()` — for operator-precedence expression parsing.
//!
//! ## Status: stub

// TODO: implement the full quew grammar after TokenKind and AST are finalized.
