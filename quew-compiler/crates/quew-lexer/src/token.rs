//! `TokenKind` — every terminal symbol in the quew grammar.
//!
//! This module owns the `logos` derive macro. All pattern annotations live here.
//! No logic beyond classification belongs in this file.
//!
//! ## Ordering rules (logos priority)
//!
//! 1. Longer DFA match always wins (`tools` beats `tool` when input is `tools`).
//! 2. `#[token]` beats `#[regex]` at the same length (keywords beat identifiers).
//! 3. Multi-char operators (`==`) are listed before single-char (`=`) — redundant
//!    with rule 1 but explicit for readability.

use logos::Logos;

use crate::annotation::AnnotationKind;

/// Callback: classifies `@annotation` slices into [`AnnotationKind`].
fn lex_annotation(lex: &mut logos::Lexer<TokenKind>) -> AnnotationKind {
    AnnotationKind::from_slice(lex.slice())
}

/// Callback: lex a triple-quoted string `"""..."""` by scanning forward manually.
///
/// Called when the lexer has already consumed the opening `"""`. Advances past
/// the body and closing `"""`. Returns `true` on success, `false` if the string
/// is unterminated (EOF before closing `"""`).
fn lex_triple_string(lex: &mut logos::Lexer<TokenKind>) -> bool {
    let remainder = lex.remainder();
    // Search for the closing """ in the remainder.
    let mut i = 0;
    let bytes = remainder.as_bytes();
    while i + 2 < bytes.len() {
        if bytes[i] == b'"' && bytes[i + 1] == b'"' && bytes[i + 2] == b'"' {
            // Consume up to and including the closing """.
            lex.bump(i + 3);
            return true;
        }
        i += 1;
    }
    // Unterminated — consume everything and signal error.
    lex.bump(remainder.len());
    false
}

/// Callback: lex a block comment `/* ... */` by scanning forward manually.
///
/// Called when the lexer has already consumed `/*`. Advances past the body and
/// closing `*/`. Returns `()` (skip) on success, errors on unterminated comment.
fn lex_block_comment(lex: &mut logos::Lexer<TokenKind>) -> logos::Filter<()> {
    let remainder = lex.remainder();
    let bytes = remainder.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'*' && bytes[i + 1] == b'/' {
            lex.bump(i + 2);
            return logos::Filter::Skip;
        }
        i += 1;
    }
    // Unterminated block comment — consume rest and emit error.
    lex.bump(remainder.len());
    logos::Filter::Emit(())
}

/// Every terminal symbol in the quew grammar.
///
/// Produced by [`crate::lex`]. Each variant corresponds to one token the
/// parser may observe. Spans are stored separately in [`crate::LexResult`].
#[derive(Logos, Debug, Clone, PartialEq, Eq, Hash)]
#[logos(skip r"[ \t\r]+")] // skip spaces, tabs, carriage returns — NOT newlines
#[logos(skip(r"//[^\n]*", allow_greedy = true))] // skip line comments
pub enum TokenKind {
    // ── Block comments (handled by callback so we can detect unterminated) ──
    #[token("/*", lex_block_comment)]
    BlockComment, // only emitted when unterminated (Filter::Emit case)

    // ── Top-level declaration keywords ───────────────────────────────────────
    #[token("agent")]    KwAgent,
    #[token("function")] KwFunction,
    /// `tool` — single host-backed tool declaration.
    #[token("tool")]     KwTool,
    /// `tools` — group of tools (shorthand or progressive disclosure).
    #[token("tools")]    KwTools,
    #[token("type")]     KwType,
    #[token("model")]    KwModel,
    #[token("let")]      KwLet,

    // ── Control flow / expression keywords ───────────────────────────────────
    #[token("if")]       KwIf,
    #[token("else")]     KwElse,
    #[token("return")]   KwReturn,
    #[token("reply")]    KwReply,
    #[token("with")]     KwWith,
    #[token("for")]      KwFor,
    #[token("in")]       KwIn,
    /// `turns` — reserved keyword. Only valid in `return expr with turns`.
    /// No variable or binding may be named `turns`.
    #[token("turns")]    KwTurns,
    #[token("is")]       KwIs,


    // ── Logical operators (English words, not symbols) ────────────────────────
    #[token("and")]      KwAnd,
    #[token("or")]       KwOr,
    #[token("not")]      KwNot,

    // ── Primitive type keywords ───────────────────────────────────────────────
    #[token("string")]   TyString,
    #[token("number")]   TyNumber,
    #[token("float")]    TyFloat,
    #[token("bool")]     TyBool,
    #[token("void")]     TyVoid,

    // ── Built-in provider keywords ────────────────────────────────────────────
    // Hardcoded for v2 first milestone. Future: `extend model` syntax will let
    // users register additional providers without touching the compiler.
    #[token("gemini")]   KwGemini,
    #[token("openai")]   KwOpenAi,
    #[token("groq")]     KwGroq,

    // ── Bool / null literals (before Ident so they take keyword priority) ─────
    #[token("true")]     True,
    #[token("false")]    False,
    #[token("null")]     NullLiteral,

    // ── Annotations ──────────────────────────────────────────────────────────
    /// `@tool`, `@desc`, `@middleware`, etc. — all `@name` patterns.
    /// Unknown annotations produce `AnnotationKind::Unknown`, not an error.
    #[regex(r"@[a-zA-Z][a-zA-Z0-9_]*", lex_annotation)]
    Annotation(AnnotationKind),

    // ── Numeric literals ─────────────────────────────────────────────────────
    /// Float must be matched before Int — logos picks the longest match, but
    /// `3.14` matches FloatLiteral (4 chars) before IntLiteral would match `3`.
    #[regex(r"[0-9]+\.[0-9]+")]
    FloatLiteral,
    #[regex(r"[0-9]+")]
    IntLiteral,

    // ── String literals ───────────────────────────────────────────────────────
    /// Triple-quoted string `"""..."""`. Callback handles multi-line content and
    /// returns `false` (→ error token) if the string is unterminated.
    #[token("\"\"\"", lex_triple_string)]
    TripleString,
    /// Regular double-quoted string. `\\` and `\"` are the only escape sequences
    /// the lexer validates; further validation is the checker's job.
    #[regex(r#""([^"\\]|\\.)*""#)]
    StringLiteral,

    // ── Identifiers ───────────────────────────────────────────────────────────
    /// Any name that is not a reserved keyword. The caller interns the slice.
    /// Uses Unicode XID properties so identifiers like `héllo` are valid.
    #[regex(r"[\p{XID_Start}_][\p{XID_Continue}_]*")]
    Ident,

    // ── Delimiters ────────────────────────────────────────────────────────────
    #[token("{")]  LBrace,
    #[token("}")]  RBrace,
    #[token("(")]  LParen,
    #[token(")")]  RParen,
    #[token("[")]  LBracket,
    #[token("]")]  RBracket,
    #[token("<")]  LAngle,
    #[token(">")]  RAngle,

    // ── Structure punctuation ─────────────────────────────────────────────────
    #[token(":")]  Colon,
    #[token(",")]  Comma,
    #[token(".")]  Dot,
    /// `?` — optional parameter marker: `id?: string`.
    #[token("?")]  Question,
    /// `|` — union type separator: `string | number`.
    #[token("|")]  Pipe,

    // ── Operators (multi-char before single-char) ─────────────────────────────
    #[token("==")] EqEq,
    #[token("!=")] BangEq,
    #[token("=")]  Eq,
    #[token("+")]  Plus,
    #[token("-")]  Minus,
    #[token("*")]  Star,
    #[token("/")]  Slash,
    #[token("%")]  Percent,

    // ── Newlines (statement boundaries) ──────────────────────────────────────
    /// Emitted for every `\n`. The parser decides whether to treat it as a
    /// statement terminator or ignore it (e.g., inside `( )` or `[ ]`).
    #[token("\n")]
    Newline,

    // ── Error fallback ────────────────────────────────────────────────────────
    /// Any character the DFA cannot classify. The lexer continues rather than
    /// aborting — callers should treat this as a non-fatal lex error.
    Error,
}

impl std::fmt::Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::KwAgent    => "`agent`",
            Self::KwFunction => "`function`",
            Self::KwTool     => "`tool`",
            Self::KwTools    => "`tools`",
            Self::KwType     => "`type`",
            Self::KwModel    => "`model`",
            Self::KwLet      => "`let`",
            Self::KwIf       => "`if`",
            Self::KwElse     => "`else`",
            Self::KwReturn   => "`return`",
            Self::KwReply    => "`reply`",
            Self::KwWith     => "`with`",
            Self::KwFor      => "`for`",
            Self::KwIn       => "`in`",
            Self::KwTurns    => "`turns`",
            Self::KwIs       => "`is`",
            Self::KwAnd      => "`and`",
            Self::KwOr       => "`or`",
            Self::KwNot      => "`not`",
            Self::TyString   => "`string`",
            Self::TyNumber   => "`number`",
            Self::TyFloat    => "`float`",
            Self::TyBool     => "`bool`",
            Self::TyVoid     => "`void`",
            Self::KwGemini   => "`gemini`",
            Self::KwOpenAi   => "`openai`",
            Self::KwGroq     => "`groq`",
            Self::True       => "`true`",
            Self::False      => "`false`",
            Self::NullLiteral => "`null`",
            Self::Annotation(k) => return write!(f, "`@{k:?}`"),
            Self::IntLiteral => "integer literal",
            Self::FloatLiteral => "float literal",
            Self::TripleString => "triple-quoted string",
            Self::StringLiteral => "string literal",
            Self::Ident      => "identifier",
            Self::LBrace     => "`{`",
            Self::RBrace     => "`}`",
            Self::LParen     => "`(`",
            Self::RParen     => "`)`",
            Self::LBracket   => "`[`",
            Self::RBracket   => "`]`",
            Self::LAngle     => "`<`",
            Self::RAngle     => "`>`",
            Self::Colon      => "`:`",
            Self::Comma      => "`,`",
            Self::Dot        => "`.`",
            Self::Question   => "`?`",
            Self::Pipe       => "`|`",
            Self::EqEq       => "`==`",
            Self::BangEq     => "`!=`",
            Self::Eq         => "`=`",
            Self::Plus       => "`+`",
            Self::Minus      => "`-`",
            Self::Star       => "`*`",
            Self::Slash      => "`/`",
            Self::Percent    => "`%`",
            Self::Newline    => "newline",
            Self::BlockComment => "block comment",
            Self::Error      => "unknown token",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use logos::Logos;

    fn tokens(src: &str) -> Vec<TokenKind> {
        TokenKind::lexer(src)
            .map(|r| r.unwrap_or(TokenKind::Error))
            .filter(|t| !matches!(t, TokenKind::Newline)) // strip newlines for brevity
            .collect()
    }

    // ── Declaration keywords ──────────────────────────────────────────────────

    #[test]
    fn kw_agent() { assert_eq!(tokens("agent"), vec![TokenKind::KwAgent]); }

    #[test]
    fn kw_function() { assert_eq!(tokens("function"), vec![TokenKind::KwFunction]); }

    #[test]
    fn kw_tool_and_tools_distinct() {
        assert_eq!(tokens("tool"), vec![TokenKind::KwTool]);
        assert_eq!(tokens("tools"), vec![TokenKind::KwTools]);
        // `tools` starts with `tool` — DFA must pick the longer match.
        assert_eq!(tokens("tools {"), vec![TokenKind::KwTools, TokenKind::LBrace]);
    }

    #[test]
    fn kw_type() { assert_eq!(tokens("type"), vec![TokenKind::KwType]); }
    #[test]
    fn kw_model() { assert_eq!(tokens("model"), vec![TokenKind::KwModel]); }
    #[test]
    fn kw_let() { assert_eq!(tokens("let"), vec![TokenKind::KwLet]); }

    // ── Control flow ──────────────────────────────────────────────────────────

    #[test]
    fn kw_if_else() {
        assert_eq!(tokens("if else"), vec![TokenKind::KwIf, TokenKind::KwElse]);
    }
    #[test]
    fn kw_return() { assert_eq!(tokens("return"), vec![TokenKind::KwReturn]); }
    #[test]
    fn kw_reply() { assert_eq!(tokens("reply"), vec![TokenKind::KwReply]); }
    #[test]
    fn kw_with() { assert_eq!(tokens("with"), vec![TokenKind::KwWith]); }
    #[test]
    fn kw_for_in() {
        assert_eq!(tokens("for in"), vec![TokenKind::KwFor, TokenKind::KwIn]);
    }
    #[test]
    fn kw_is() { assert_eq!(tokens("is"), vec![TokenKind::KwIs]); }

    // ── Logical operators ─────────────────────────────────────────────────────

    #[test]
    fn kw_and_or_not() {
        assert_eq!(
            tokens("and or not"),
            vec![TokenKind::KwAnd, TokenKind::KwOr, TokenKind::KwNot]
        );
    }

    #[test]
    fn logical_keywords_dont_bleed_into_identifiers() {
        // `android` starts with `and` — must lex as Ident, not KwAnd + Ident.
        assert_eq!(tokens("android"), vec![TokenKind::Ident]);
        assert_eq!(tokens("oracle"), vec![TokenKind::Ident]);
        assert_eq!(tokens("notable"), vec![TokenKind::Ident]);
    }

    // ── Types ─────────────────────────────────────────────────────────────────

    #[test]
    fn primitive_types() {
        assert_eq!(tokens("string"), vec![TokenKind::TyString]);
        assert_eq!(tokens("number"), vec![TokenKind::TyNumber]);
        assert_eq!(tokens("float"),  vec![TokenKind::TyFloat]);
        assert_eq!(tokens("bool"),   vec![TokenKind::TyBool]);
        assert_eq!(tokens("void"),   vec![TokenKind::TyVoid]);
    }

    // ── Provider keywords ─────────────────────────────────────────────────────

    #[test]
    fn provider_keywords_lex_as_dedicated_tokens() {
        assert_eq!(tokens("gemini"), vec![TokenKind::KwGemini]);
        assert_eq!(tokens("openai"), vec![TokenKind::KwOpenAi]);
        assert_eq!(tokens("groq"),   vec![TokenKind::KwGroq]);
    }

    #[test]
    fn provider_keywords_in_model_call_position() {
        // `model: gemini("gemini-pro")` — gemini must be KwGemini, not Ident.
        let toks = tokens(r#"gemini("gemini-pro")"#);
        assert_eq!(toks[0], TokenKind::KwGemini);
        assert_eq!(toks[1], TokenKind::LParen);
        assert_eq!(toks[2], TokenKind::StringLiteral);
        assert_eq!(toks[3], TokenKind::RParen);
    }

    // ── Literals ──────────────────────────────────────────────────────────────

    #[test]
    fn int_literal() { assert_eq!(tokens("42"), vec![TokenKind::IntLiteral]); }

    #[test]
    fn float_literal() { assert_eq!(tokens("3.14"), vec![TokenKind::FloatLiteral]); }

    #[test]
    fn float_beats_int_then_dot() {
        // `3.14` must be ONE FloatLiteral, not IntLiteral + Dot + IntLiteral.
        assert_eq!(tokens("3.14"), vec![TokenKind::FloatLiteral]);
    }

    #[test]
    fn bool_literals() {
        assert_eq!(tokens("true"),  vec![TokenKind::True]);
        assert_eq!(tokens("false"), vec![TokenKind::False]);
    }

    #[test]
    fn null_literal() { assert_eq!(tokens("null"), vec![TokenKind::NullLiteral]); }

    #[test]
    fn string_literal() {
        assert_eq!(tokens(r#""hello""#), vec![TokenKind::StringLiteral]);
    }

    #[test]
    fn string_literal_with_escape() {
        assert_eq!(tokens(r#""say \"hi\"""#), vec![TokenKind::StringLiteral]);
    }

    #[test]
    fn triple_string() {
        assert_eq!(tokens(r#""""hello world""""#), vec![TokenKind::TripleString]);
    }

    // ── Annotations ───────────────────────────────────────────────────────────

    #[test]
    fn annotation_tool() {
        assert_eq!(
            tokens("@tool"),
            vec![TokenKind::Annotation(AnnotationKind::Tool)]
        );
    }

    #[test]
    fn annotation_desc() {
        assert_eq!(
            tokens("@desc"),
            vec![TokenKind::Annotation(AnnotationKind::Desc)]
        );
    }

    #[test]
    fn annotation_middleware() {
        assert_eq!(
            tokens("@middleware"),
            vec![TokenKind::Annotation(AnnotationKind::Middleware)]
        );
    }

    #[test]
    fn annotation_middlewares() {
        assert_eq!(
            tokens("@middlewares"),
            vec![TokenKind::Annotation(AnnotationKind::Middlewares)]
        );
    }

    #[test]
    fn annotation_context() {
        assert_eq!(
            tokens("@context"),
            vec![TokenKind::Annotation(AnnotationKind::Context)]
        );
    }

    #[test]
    fn annotation_native() {
        assert_eq!(
            tokens("@native"),
            vec![TokenKind::Annotation(AnnotationKind::Native)]
        );
    }

    #[test]
    fn annotation_block() {
        assert_eq!(
            tokens("@block"),
            vec![TokenKind::Annotation(AnnotationKind::Block)]
        );
    }

    #[test]
    fn unknown_annotation_produces_unknown_kind() {
        // @toolbox is one token, AnnotationKind::Unknown — NOT split into @tool + Ident.
        assert_eq!(
            tokens("@toolbox"),
            vec![TokenKind::Annotation(AnnotationKind::Unknown)]
        );
    }

    // ── Identifiers ───────────────────────────────────────────────────────────

    #[test]
    fn ident_basic() { assert_eq!(tokens("hello"), vec![TokenKind::Ident]); }

    #[test]
    fn ident_with_underscore() { assert_eq!(tokens("my_var"), vec![TokenKind::Ident]); }

    #[test]
    fn ident_unicode() {
        // héllo contains a non-ASCII XID_Start char — must lex as a single Ident.
        let toks = tokens("héllo");
        assert_eq!(toks, vec![TokenKind::Ident]);
    }

    #[test]
    fn keyword_adjacent_to_punctuation_no_bleed() {
        // `tool{` must lex as KwTool + LBrace, not a single Ident.
        assert_eq!(tokens("tool{"), vec![TokenKind::KwTool, TokenKind::LBrace]);
    }

    // ── Delimiters ────────────────────────────────────────────────────────────

    #[test]
    fn all_delimiters() {
        let toks = tokens("{ } ( ) [ ] < >");
        assert_eq!(toks, vec![
            TokenKind::LBrace, TokenKind::RBrace,
            TokenKind::LParen, TokenKind::RParen,
            TokenKind::LBracket, TokenKind::RBracket,
            TokenKind::LAngle, TokenKind::RAngle,
        ]);
    }

    // ── Punctuation ───────────────────────────────────────────────────────────

    #[test]
    fn structure_punctuation() {
        let toks = tokens(": , . ? |");
        assert_eq!(toks, vec![
            TokenKind::Colon, TokenKind::Comma, TokenKind::Dot,
            TokenKind::Question, TokenKind::Pipe,
        ]);
    }

    // ── Operators ─────────────────────────────────────────────────────────────

    #[test]
    fn operators() {
        let toks = tokens("== != = + - * / %");
        assert_eq!(toks, vec![
            TokenKind::EqEq, TokenKind::BangEq, TokenKind::Eq,
            TokenKind::Plus, TokenKind::Minus, TokenKind::Star,
            TokenKind::Slash, TokenKind::Percent,
        ]);
    }

    #[test]
    fn eq_eq_beats_eq_plus_eq() {
        // `==` must be EqEq, not two Eq tokens.
        assert_eq!(tokens("=="), vec![TokenKind::EqEq]);
    }

    // ── Comments ──────────────────────────────────────────────────────────────

    #[test]
    fn line_comment_is_skipped() {
        assert_eq!(tokens("let // this is a comment"), vec![TokenKind::KwLet]);
    }

    #[test]
    fn block_comment_is_skipped() {
        assert_eq!(tokens("let /* ignored */ x"), vec![TokenKind::KwLet, TokenKind::Ident]);
    }

    // ── Error recovery ────────────────────────────────────────────────────────

    #[test]
    fn unknown_char_produces_error_not_panic() {
        let toks = tokens("let $ x");
        // Should have KwLet, Error (for $), Ident (x) — lexer does not abort.
        assert!(toks.contains(&TokenKind::Error));
        assert!(toks.contains(&TokenKind::KwLet));
        assert!(toks.contains(&TokenKind::Ident));
    }

    #[test]
    fn empty_input_produces_no_tokens() {
        assert_eq!(tokens(""), vec![]);
    }

    #[test]
    fn whitespace_only_produces_no_tokens() {
        assert_eq!(tokens("   \t  "), vec![]);
    }
}
