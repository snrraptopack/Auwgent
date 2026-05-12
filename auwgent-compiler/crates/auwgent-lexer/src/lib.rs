//! # auwgent-lexer
//!
//! Tokenizer for the Auwgent DSL using `logos` for fast lexing.
//! Produces a stream of `Token`s with `Span` locations for the parser.

use auwgent_errors::Span;
use logos::Logos;
use std::fmt;
use std::hash::{Hash, Hasher};

/// All token types in the Auwgent DSL.
/// Maps to the terminals in `auwgent.langium`.
///
/// NOTE: PartialEq/Eq/Hash are implemented manually so that payload variants
/// (Ident, Number, strings) compare equal regardless of content. This is needed
/// for chumsky's error reporting to show "expected identifier" instead of
/// "expected identifier 'someSpecificName'".
#[derive(Logos, Debug, Clone)]
#[logos(skip r"[ \t\r\n\f]+")] // WS
#[logos(skip r"//[^\n\r]*")] // SL_COMMENT
#[logos(skip r"/\*([^*]|\*[^/])*\*/")] // ML_COMMENT
pub enum TokenKind {
    // ── Keywords ─────────────────────────────────────────────────────
    #[token("agent")]
    Agent,
    #[token("helper")]
    Helper,
    #[token("component")]
    Component,
    #[token("tool")]
    Tool,
    #[token("tools")]
    Tools,
    #[token("workflow")]
    Workflow,
    #[token("type")]
    Type,
    #[token("import")]
    Import,
    #[token("export")]
    Export,
    #[token("from")]
    From,
    #[token("as")]
    As,
    #[token("prompt")]
    Prompt,
    #[token("model")]
    Model,
    #[token("embedding")]
    Embedding,
    #[token("config")]
    Config,
    #[token("default")]
    Default,
    #[token("input")]
    Input,
    #[token("output")]
    Output,
    #[token("context")]
    Context,
    #[token("helpers")]
    Helpers,
    #[token("let")]
    Let,
    #[token("return")]
    Return,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("transfer")]
    Transfer,
    #[token("to")]
    To,
    #[token("then")]
    Then,
    #[token("continue")]
    Continue,
    #[token("parallel")]
    Parallel,
    #[token("example")]
    #[token("Example")]
    Example,
    #[token("user")]
    #[token("User")]
    User,
    #[token("assistant")]
    #[token("Assistant")]
    Assistant,
    #[token("test")]
    Test,
    #[token("expect")]
    Expect,
    #[token("error")]
    Error,
    #[token("returns")]
    Returns,
    #[token("with")]
    With,
    #[token("all")]
    All,
    #[token("handoff")]
    Handoff,
    #[token("use")]
    Use,
    #[token("lifecycle")]
    Lifecycle,
    #[token("provider")]
    Provider,
    #[token("gemini")]
    Gemini,
    #[token("openai")]
    Openai,
    #[token("groq")]
    Groq,
    #[token("custom")]
    Custom,
    #[token("maxTokens")]
    MaxTokens,
    #[token("maxMessages")]
    MaxMessages,
    #[token("description")]
    Description,
    #[token("intent")]
    Intent,
    #[token("fields")]
    Fields,

    ErrorToken,

    // ── Type Keywords ────────────────────────────────────────────────
    #[token("string")]
    StringType,
    #[token("number")]
    NumberType,
    #[token("boolean")]
    BooleanType,
    #[token("Text")]
    TextType,
    #[token("Image")]
    ImageType,
    #[token("File")]
    FileType,
    #[token("Audio")]
    AudioType,
    #[token("Video")]
    VideoType,

    // ── Operators ────────────────────────────────────────────────────
    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token(">=")]
    GtEq,
    #[token("<=")]
    LtEq,
    #[token("&&")]
    And,
    #[token("||")]
    Or,
    #[token(">")]
    Gt,
    #[token("<")]
    Lt,
    #[token("=")]
    Eq,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,

    // ── Punctuation ──────────────────────────────────────────────────
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token(".")]
    Dot,
    #[token("?")]
    Question,
    #[token("|")]
    Pipe,
    #[token("@desc")]
    AtDesc,
    #[token("@example")]
    AtExample,
    #[token("@native")]
    AtNative,
    #[token("@block")]
    AtBlock,
    #[token("hlp")]
    Hlp,
    #[token("ctx")]
    Ctx,

    // ── Literals ─────────────────────────────────────────────────────
    // Match 3 quotes, then any characters (lazily), and optionally up to 3 quotes.
    // We validate in the closure if it actually ended with 3 quotes.
    #[regex(r#""""(?:[^"]|"[^"]|""[^"])*("{0,3})"#, |lex| lex.slice().to_string())]
    MultilineString(String),

    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        s[1..s.len()-1].to_string()
    })]
    DoubleString(String),

    #[regex(r#"'([^'\\]|\\.)*'"#, |lex| {
        let s = lex.slice();
        s[1..s.len()-1].to_string()
    })]
    SingleString(String),

    #[regex(r"[0-9]+(\.[0-9]+)?", |lex| lex.slice().to_string())]
    Number(String),

    #[regex(r"[_a-zA-Z][_a-zA-Z0-9]*", |lex| lex.slice().to_string(), priority = 1)]
    Ident(String),
}

// ── Custom PartialEq/Eq/Hash ─────────────────────────────────────────
// Payload variants compare by discriminant only so chumsky error messages
// say "expected identifier" rather than "expected identifier 'Manager'".

impl PartialEq for TokenKind {
    fn eq(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

impl Eq for TokenKind {}

impl Hash for TokenKind {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
    }
}

// ── Display (for chumsky error messages) ─────────────────────────────

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Agent => write!(f, "'agent'"),
            Self::Helper => write!(f, "'helper'"),
            Self::Component => write!(f, "'component'"),
            Self::Tool => write!(f, "'tool'"),
            Self::Tools => write!(f, "'tools'"),
            Self::Workflow => write!(f, "'workflow'"),
            Self::Type => write!(f, "'type'"),
            Self::Import => write!(f, "'import'"),
            Self::Export => write!(f, "'export'"),
            Self::From => write!(f, "'from'"),
            Self::As => write!(f, "'as'"),
            Self::Prompt => write!(f, "'prompt'"),
            Self::Model => write!(f, "'model'"),
            Self::Embedding => write!(f, "'embedding'"),
            Self::Config => write!(f, "'config'"),
            Self::Default => write!(f, "'default'"),
            Self::Input => write!(f, "'input'"),
            Self::Output => write!(f, "'output'"),
            Self::Context => write!(f, "'context'"),
            Self::Helpers => write!(f, "'helpers'"),
            Self::Description => write!(f, "'description'"),
            Self::Intent => write!(f, "'intent'"),
            Self::Fields => write!(f, "'fields'"),
            Self::Let => write!(f, "'let'"),
            Self::Return => write!(f, "'return'"),
            Self::If => write!(f, "'if'"),
            Self::Else => write!(f, "'else'"),
            Self::True => write!(f, "'true'"),
            Self::False => write!(f, "'false'"),
            Self::Transfer => write!(f, "'transfer'"),
            Self::To => write!(f, "'to'"),
            Self::Then => write!(f, "'then'"),
            Self::Continue => write!(f, "'continue'"),
            Self::Parallel => write!(f, "'parallel'"),
            Self::Example => write!(f, "'example'"),
            Self::User => write!(f, "'user'"),
            Self::Assistant => write!(f, "'assistant'"),
            Self::Test => write!(f, "'test'"),
            Self::Expect => write!(f, "'expect'"),
            Self::Error => write!(f, "'error'"),
            Self::Returns => write!(f, "'returns'"),
            Self::With => write!(f, "'with'"),
            Self::All => write!(f, "'all'"),
            Self::Handoff => write!(f, "'handoff'"),
            Self::Use => write!(f, "'use'"),
            Self::Lifecycle => write!(f, "'lifecycle'"),
            Self::Provider => write!(f, "'provider'"),
            Self::Gemini => write!(f, "'gemini'"),
            Self::Openai => write!(f, "'openai'"),
            Self::Groq => write!(f, "'groq'"),
            Self::Custom => write!(f, "'custom'"),
            Self::MaxTokens => write!(f, "'maxTokens'"),
            Self::MaxMessages => write!(f, "'maxMessages'"),
            Self::StringType => write!(f, "'string'"),
            Self::NumberType => write!(f, "'number'"),
            Self::BooleanType => write!(f, "'boolean'"),
            Self::TextType => write!(f, "'Text'"),
            Self::ImageType => write!(f, "'Image'"),
            Self::FileType => write!(f, "'File'"),
            Self::AudioType => write!(f, "'Audio'"),
            Self::VideoType => write!(f, "'Video'"),
            Self::EqEq => write!(f, "'=='"),
            Self::NotEq => write!(f, "'!='"),
            Self::GtEq => write!(f, "'>='"),
            Self::LtEq => write!(f, "'<='"),
            Self::And => write!(f, "'&&'"),
            Self::Or => write!(f, "'||'"),
            Self::Gt => write!(f, "'>'"),
            Self::Lt => write!(f, "'<'"),
            Self::Eq => write!(f, "'='"),
            Self::Plus => write!(f, "'+'"),
            Self::Minus => write!(f, "'-'"),
            Self::Star => write!(f, "'*'"),
            Self::Slash => write!(f, "'/'"),
            Self::LBrace => write!(f, "'{{'"),
            Self::RBrace => write!(f, "'}}'"),
            Self::LParen => write!(f, "'('"),
            Self::RParen => write!(f, "')'"),
            Self::LBracket => write!(f, "'['"),
            Self::RBracket => write!(f, "']'"),
            Self::Colon => write!(f, "':'"),
            Self::Comma => write!(f, "','"),
            Self::Dot => write!(f, "'.'"),
            Self::Question => write!(f, "'?'"),
            Self::Pipe => write!(f, "'|'"),
            Self::AtDesc => write!(f, "'@desc'"),
            Self::AtExample => write!(f, "'@example'"),
            Self::AtNative => write!(f, "'@native'"),
            Self::AtBlock => write!(f, "'@block'"),
            Self::Hlp => write!(f, "'hlp'"),
            Self::Ctx => write!(f, "'ctx'"),
            Self::Ident(_) => write!(f, "identifier"),
            Self::Number(_) => write!(f, "number"),
            Self::DoubleString(_) | Self::SingleString(_) => write!(f, "string"),
            Self::MultilineString(_) => write!(f, "multiline string"),
            Self::ErrorToken => write!(f, "error token"),
        }
    }
}

/// A token with its kind and source span.
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub span: Span,
}

/// Tokenize a source string into a `Vec<Token>`.
pub fn tokenize(source: &str) -> (Vec<Token>, Vec<auwgent_errors::Diagnostic>) {
    let mut tokens = Vec::new();
    let mut errors = Vec::new();

    let mut lexer = TokenKind::lexer(source);

    while let Some(result) = lexer.next() {
        let span = Span::new(lexer.span().start, lexer.span().end);
        match result {
            Ok(TokenKind::MultilineString(_)) => {
                let slice = lexer.slice();
                if slice.ends_with("\"\"\"") && slice.len() >= 6 {
                    tokens.push(Token {
                        kind: TokenKind::MultilineString(slice[3..slice.len() - 3].to_string()),
                        text: slice.to_string(),
                        span,
                    });
                } else {
                    errors.push(
                        auwgent_errors::Diagnostic::error("Unclosed multiline string", span)
                            .with_help("Make sure the string ends with \"\"\""),
                    );
                    // Push anyway so parser can try to recover
                    tokens.push(Token {
                        kind: TokenKind::ErrorToken,
                        text: slice.to_string(),
                        span,
                    });
                }
            }
            Ok(kind) => {
                tokens.push(Token {
                    kind,
                    text: lexer.slice().to_string(),
                    span,
                });
            }
            Err(()) => {
                errors.push(auwgent_errors::Diagnostic::error(
                    format!("unexpected character: '{}'", lexer.slice()),
                    span,
                ));
                tokens.push(Token {
                    kind: TokenKind::ErrorToken,
                    text: lexer.slice().to_string(),
                    span,
                });
            }
        }
    }

    (tokens, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokens() {
        let source = r#"agent Manager { tool getStudentDetails(id: string): Student }"#;
        let (tokens, errors) = tokenize(source);
        assert!(errors.is_empty(), "unexpected lex errors: {:?}", errors);
        assert_eq!(tokens[0].kind, TokenKind::Agent);
        assert_eq!(tokens[1].text, "Manager");
    }

    #[test]
    fn test_string_literal() {
        let source = r#""hello world""#;
        let (tokens, errors) = tokenize(source);
        assert!(errors.is_empty());
        assert!(matches!(&tokens[0].kind, TokenKind::DoubleString(s) if s == "hello world"));
    }

    #[test]
    fn test_multiline_string() {
        let source = r#""""
            Hello {{name}}
        """"#;
        let (tokens, errors) = tokenize(source);
        assert!(errors.is_empty());
        assert!(matches!(&tokens[0].kind, TokenKind::MultilineString(_)));
    }

    #[test]
    fn test_native_block_annotations() {
        let source = "@native\n@block";
        let (tokens, errors) = tokenize(source);
        assert!(errors.is_empty(), "unexpected lex errors: {:?}", errors);
        assert_eq!(tokens[0].kind, TokenKind::AtNative);
        assert_eq!(tokens[1].kind, TokenKind::AtBlock);
    }
}
