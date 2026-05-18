//! Top-level item parser — agents, functions, tools, types, models, lets.

use std::sync::Arc;

use chumsky::prelude::*;
use quew_ast::expr::{Provider, ProviderCall};
use quew_ast::lit::{StringKind, StringLit};
use quew_ast::{
    AgentDecl, BuiltinTypeMeta, BuiltinVisibility, ConfigField, FieldDef, FunctionDecl, Item,
    LetDecl, ModelDecl, Module, RoleBindingSyntax, ToolDecl, ToolEntry, ToolsDecl, TypeDecl,
};
use quew_interner::Interner;
use quew_lexer::{AnnotationKind, TokenKind};

use crate::common::{Input, ParseError, field_name, ident, string_literal, to_span};
use crate::parse_annot::annotations;
use crate::parse_expr::expr;
use crate::parse_param::{param, param_list};
use crate::parse_stmt::block;
use crate::parse_type::type_expr;

/// Parse the entire module (the root of the AST).
pub fn module<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, Module, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    item(source, interner)
        .separated_by(just(TokenKind::Newline).repeated().at_least(1))
        .allow_leading()
        .allow_trailing()
        .collect::<Vec<_>>()
        .map_with(|items, extra| Module {
            items,
            span: to_span(extra.span()),
        })
        .then_ignore(end())
}

/// Parse a single top-level item, with error recovery at the item boundary.
fn item<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, Item, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    choice((
        agent(source, interner.clone()),
        function(source, interner.clone()),
        tools_group(source, interner.clone()), // `tools` before `tool`
        tool_decl(source, interner.clone()),
        type_decl(source, interner.clone()),
        model_decl(source, interner.clone()),
        let_decl(source, interner.clone()),
    ))
    .recover_with(via_parser(
        // On unrecognised top-level token, skip to the next keyword that starts
        // a known declaration.
        any()
            .repeated()
            .at_least(1)
            .to(Item::Let(quew_ast::LetDecl {
                name: interner.intern("<error>"),
                ty: None,
                init: quew_ast::Expr::Error(quew_errors::Span::new(0, 0)),
                span: quew_errors::Span::new(0, 0),
            })),
    ))
}

// ── Field separator helper ────────────────────────────────────────────────────

/// Accepts one-or-more commas OR newlines between block entries.
/// This lets both inline `{ a: T, b: T }` and multiline `{\n  a: T\n  b: T\n}`
/// styles work everywhere.
fn field_sep<'tok, I>() -> impl Parser<'tok, I, (), ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    just(TokenKind::Comma)
        .or(just(TokenKind::Newline))
        .repeated()
        .at_least(1)
        .ignored()
}

fn type_params<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, Vec<quew_interner::InternedStr>, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    ident(source, interner)
        .separated_by(just(TokenKind::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(TokenKind::LAngle), just(TokenKind::RAngle))
        .or_not()
        .map(|params| params.unwrap_or_default())
}

// ── Agent ─────────────────────────────────────────────────────────────────────

fn agent<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, Item, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    annotations(source, interner.clone())
        .then_ignore(just(TokenKind::KwAgent))
        .then(ident(source, interner.clone()))
        .then(
            param(source, interner.clone())
                .delimited_by(just(TokenKind::LParen), just(TokenKind::RParen)),
        )
        .then(
            just(TokenKind::Colon)
                .ignore_then(type_expr(source, interner.clone()))
                .or_not(),
        )
        .then(block(source, interner.clone()))
        .map_with(|((((annotations, name), param), return_ty), body), extra| {
            Item::Agent(AgentDecl {
                annotations,
                name,
                param,
                return_ty,
                body,
                span: to_span(extra.span()),
            })
        })
}

// ── Function ──────────────────────────────────────────────────────────────────

fn function<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, Item, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    annotations(source, interner.clone())
        .then_ignore(just(TokenKind::KwFunction))
        .then(ident(source, interner.clone()))
        .then(type_params(source, interner.clone()))
        .then(param_list(source, interner.clone()))
        .then(
            just(TokenKind::Colon)
                .ignore_then(type_expr(source, interner.clone()))
                .or_not(),
        )
        .then(block(source, interner.clone()))
        .map_with(
            |(((((annotations, name), type_params), params), return_ty), body), extra| {
                Item::Function(FunctionDecl {
                    annotations,
                    name,
                    type_params,
                    params,
                    return_ty,
                    body,
                    span: to_span(extra.span()),
                })
            },
        )
}

// ── Tool ──────────────────────────────────────────────────────────────────────

/// Single host-backed tool: `tool name(params): ReturnType @desc "..."`
fn tool_decl<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, Item, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    // `@desc "..."` — Annotation token carrying AnnotationKind::Desc, then a string.
    let opt_desc = select! { TokenKind::Annotation(AnnotationKind::Desc) => () }
        .ignore_then(string_literal(source, interner.clone()))
        .map(|(val, s)| StringLit {
            value: val,
            kind: StringKind::Regular,
            span: to_span(s),
        })
        .or_not();

    just(TokenKind::KwTool)
        .ignore_then(ident(source, interner.clone()))
        .then(param_list(source, interner.clone()))
        .then_ignore(just(TokenKind::Colon))
        .then(type_expr(source, interner.clone()))
        .then(opt_desc)
        .map_with(|(((name, params), return_ty), desc), extra| {
            Item::Tool(ToolDecl {
                name,
                params,
                return_ty,
                desc,
                span: to_span(extra.span()),
            })
        })
}

/// Tool group: `tools { ... }` or `tools Name { ... }`
fn tools_group<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, Item, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    // `@desc "..."` — same annotation pattern as tool_decl entries.
    let opt_entry_desc = select! { TokenKind::Annotation(AnnotationKind::Desc) => () }
        .ignore_then(string_literal(source, interner.clone()))
        .map(|(val, s)| StringLit {
            value: val,
            kind: StringKind::Regular,
            span: to_span(s),
        })
        .or_not();

    let entry = ident(source, interner.clone())
        .then(param_list(source, interner.clone()))
        .then_ignore(just(TokenKind::Colon))
        .then(type_expr(source, interner.clone()))
        .then(opt_entry_desc)
        .map_with(|(((name, params), return_ty), desc), extra| ToolEntry {
            name,
            params,
            return_ty,
            desc,
            span: to_span(extra.span()),
        });

    let entries = entry
        .separated_by(field_sep())
        .allow_leading()
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(TokenKind::LBrace), just(TokenKind::RBrace));

    // Named tools group also supports an `@desc` on the group name.
    let opt_group_desc = select! { TokenKind::Annotation(AnnotationKind::Desc) => () }
        .ignore_then(string_literal(source, interner.clone()))
        .map(|(val, s)| StringLit {
            value: val,
            kind: StringKind::Regular,
            span: to_span(s),
        })
        .or_not();

    just(TokenKind::KwTools)
        .ignore_then(ident(source, interner.clone()).or_not())
        .then(entries)
        .then(opt_group_desc)
        .map_with(|((name, entries), desc), extra| {
            Item::Tools(ToolsDecl {
                name,
                entries,
                desc,
                span: to_span(extra.span()),
            })
        })
}

// ── Type ──────────────────────────────────────────────────────────────────────

fn type_decl<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, Item, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    let role_prefix = just(TokenKind::AtAt)
        .ignore_then(
            field_name(source, interner.clone())
                .then_ignore(just(TokenKind::Comma))
                .then(field_name(source, interner.clone()))
                .map_with(|(keyword, place), extra| RoleBindingSyntax {
                    keyword,
                    place,
                    span: to_span(extra.span()),
                })
                .delimited_by(just(TokenKind::LParen), just(TokenKind::RParen)),
        )
        .map(|role| BuiltinTypeMeta::Builtin {
            visibility: BuiltinVisibility::Public,
            role: Some(role),
        })
        .then_ignore(just(TokenKind::Newline).repeated().ignored());

    let builtin_prefix = choice((
        role_prefix,
        just(TokenKind::BangAtAt)
            .to(BuiltinTypeMeta::internal())
            .then_ignore(just(TokenKind::Newline).repeated().ignored()),
        just(TokenKind::AtAt)
            .to(BuiltinTypeMeta::public())
            .then_ignore(just(TokenKind::Newline).repeated().ignored()),
        empty().to(BuiltinTypeMeta::User),
    ));

    // Keywords like `agent`, `model`, `for` are valid field names inside a type.
    let field = field_name(source, interner.clone())
        .then(just(TokenKind::Question).or_not())
        .then_ignore(just(TokenKind::Colon))
        .then(type_expr(source, interner.clone()))
        .map_with(|((name, q), ty), extra| FieldDef {
            name,
            ty,
            optional: q.is_some(),
            span: to_span(extra.span()),
        });

    let fields = field
        .separated_by(field_sep()) // comma OR newline
        .allow_leading()
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(TokenKind::LBrace), just(TokenKind::RBrace));

    builtin_prefix
        .then_ignore(just(TokenKind::KwType))
        .then(ident(source, interner.clone()))
        .then(type_params(source, interner.clone()))
        .then_ignore(just(TokenKind::Eq))
        .then(fields)
        .map_with(|(((builtin, name), type_params), fields), extra| {
            Item::Type(TypeDecl {
                name,
                type_params,
                fields,
                builtin,
                span: to_span(extra.span()),
            })
        })
}

// ── Model ─────────────────────────────────────────────────────────────────────

/// Syntax: `model Name = { model: gemini("..."), config: { key: val } }`
///
/// The `model:` field is required and holds the provider call.
/// The `config:` field is optional and holds pass-through config key-value pairs.
/// Both `model` and `config` are keywords — `field_name()` handles that.
fn model_decl<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, Item, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    // Provider: `gemini("model-name")`, `openai("gpt-4")`, `groq("llama-3")`
    let provider_kw = select! {
        TokenKind::KwGemini => Provider::Gemini,
        TokenKind::KwOpenAi => Provider::OpenAi,
        TokenKind::KwGroq   => Provider::Groq,
    };

    let int_prov = interner.clone();
    let provider_expr = provider_kw
        .then(
            string_literal(source, int_prov)
                .delimited_by(just(TokenKind::LParen), just(TokenKind::RParen)),
        )
        .map_with(
            |(provider, (model_str, model_span)), extra: &mut _| ProviderCall {
                provider,
                model_name: StringLit {
                    value: model_str,
                    kind: StringKind::Regular,
                    span: to_span(model_span),
                },
                config: vec![],
                span: to_span(extra.span()),
            },
        );

    // Config block: `{ key: expr, ... }` — keywords allowed as field names.
    let config_entry = field_name(source, interner.clone())
        .then_ignore(just(TokenKind::Colon))
        .then(expr(source, interner.clone()))
        .map_with(|(key, value), extra: &mut _| ConfigField {
            key,
            value: Box::new(value),
            span: to_span(extra.span()),
        });

    let config_block = config_entry
        .separated_by(field_sep())
        .allow_leading()
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(TokenKind::LBrace), just(TokenKind::RBrace));

    // `model: gemini(...)` — extracts the ProviderCall
    let model_field = field_name(source, interner.clone())
        .then_ignore(just(TokenKind::Colon))
        .then(provider_expr)
        .map(|(_, provider)| provider);

    // `config: { ... }` — extracts Vec<ConfigField>
    let config_field_entry = field_name(source, interner.clone())
        .then_ignore(just(TokenKind::Colon))
        .then(config_block)
        .map(|(_, fields)| fields);

    // Inner: required `model:`, optional `config:` (with separator between them).
    // Leading/trailing newlines inside `{ }` are consumed explicitly so that
    // both inline `{ model: gemini(...) }` and multiline styles work.
    let inner = just(TokenKind::Newline)
        .repeated()
        .ignored()
        .ignore_then(model_field)
        .then(field_sep().ignore_then(config_field_entry).or_not())
        .then_ignore(just(TokenKind::Newline).repeated().ignored());

    let model_block = inner.delimited_by(just(TokenKind::LBrace), just(TokenKind::RBrace));

    just(TokenKind::KwModel)
        .ignore_then(ident(source, interner.clone()))
        .then_ignore(just(TokenKind::Eq))
        .then(model_block)
        .map_with(|(name, (provider, config)), extra| {
            Item::Model(ModelDecl {
                name,
                provider,
                config: config.unwrap_or_default(),
                span: to_span(extra.span()),
            })
        })
}

// ── Top-level let ─────────────────────────────────────────────────────────────

fn let_decl<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, Item, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    just(TokenKind::KwLet)
        .ignore_then(ident(source, interner.clone()))
        .then(
            just(TokenKind::Colon)
                .ignore_then(type_expr(source, interner.clone()))
                .or_not(),
        )
        .then_ignore(just(TokenKind::Eq))
        .then(expr(source, interner.clone()))
        .map_with(|((name, ty), init), extra| {
            Item::Let(LetDecl {
                name,
                ty,
                init,
                span: to_span(extra.span()),
            })
        })
}
