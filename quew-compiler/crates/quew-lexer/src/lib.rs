//! # quew-lexer
//!
//! **Single responsibility:** transforms raw quew source text into a flat token stream.
//!
//! ## Public API
//!
//! ```rust,ignore
//! use quew_lexer::{lex, TokenKind, AnnotationKind, LexResult};
//!
//! let result = lex(source, source_id, &interner);
//! // result.tokens  — Vec<(TokenKind, Span)>
//! // result.errors  — Vec<Diagnostic>
//! // result.ident_table — Vec<Option<InternedStr>>
//! ```
//!
//! ## Design rules
//!
//! - The lexer **never panics** and **never aborts** on bad input.
//! - Unknown characters produce `TokenKind::Error` + a `Diagnostic`; the stream is complete.
//! - Whitespace, line comments (`//`), and block comments (`/* */`) are silently skipped.
//! - Newlines are emitted as `TokenKind::Newline` — the parser decides their significance.
//! - All identifiers are interned via `quew_interner::Interner`; `token.rs` carries no Strings.

pub mod annotation;
pub mod lex;
pub mod token;

pub use annotation::AnnotationKind;
pub use lex::{LexResult, lex};
pub use token::TokenKind;
