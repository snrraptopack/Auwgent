//! Top-level parsers: agent, helper, type decl, prompt, model def, import, model.

use auwgent_ast::*;
use auwgent_errors::Span;
use auwgent_lexer::TokenKind;
use chumsky::prelude::*;

use crate::config::{
    agent_config_parser, intent_body_parser, model_provider_parser, prompt_stmt_parser,
};
use crate::expr::named_args_parser;
use crate::primitives::*;
use crate::types::{type_config_decl_block_parser, type_config_decl_parser};

pub(crate) fn agent_parser() -> impl Parser<TokenKind, Agent, Error = Simple<TokenKind>> + Clone {
    let protocol_annotation = choice((
        tok(TokenKind::AtNative).to("native".to_string()),
        tok(TokenKind::AtBlock).to("block".to_string()),
    ))
    .or_not();

    protocol_annotation
        .then_ignore(tok(TokenKind::Agent))
        .then(ident())
        .then(
            agent_config_parser()
                .repeated()
                .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace)),
        )
        .map_with_span(|((protocol_mode, name), configs), span| Agent {
            name,
            configs,
            protocol_mode,
            span: s(span),
        })
}

pub(crate) fn helper_parser() -> impl Parser<TokenKind, Helper, Error = Simple<TokenKind>> + Clone {
    let example_args = tok(TokenKind::AtExample).ignore_then(
        named_args_parser().delimited_by(tok(TokenKind::LParen), tok(TokenKind::RParen)),
    );

    tok(TokenKind::Helper)
        .ignore_then(ident())
        .then(
            tok(TokenKind::Description)
                .ignore_then(tok(TokenKind::Colon))
                .ignore_then(string_lit())
                .then(agent_config_parser().repeated())
                .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace))
                .then(example_args.repeated()),
        )
        .map_with_span(|(name, ((description, configs), examples)), span| Helper {
            exported: false,
            name,
            description,
            configs,
            examples,
            span: s(span),
        })
}

pub(crate) fn component_decl_parser(
) -> impl Parser<TokenKind, ComponentDeclaration, Error = Simple<TokenKind>> + Clone {
    let action_target = ident()
        .then(
            type_config_decl_parser()
                .separated_by(tok(TokenKind::Comma))
                .allow_trailing()
                .delimited_by(tok(TokenKind::LParen), tok(TokenKind::RParen))
                .or_not()
                .map(|opt| opt.unwrap_or_default()),
        )
        .map_with_span(|(name, params), span| ComponentActionTarget {
            name,
            params,
            span: s(span),
        });

    let action_target_union = action_target
        .clone()
        .then(tok(TokenKind::Pipe).ignore_then(action_target).repeated())
        .map(|(first, rest)| {
            let mut targets = vec![first];
            targets.extend(rest);
            targets
        });

    let action_binding = ident()
        .then_ignore(tok(TokenKind::Colon))
        .then(action_target_union)
        .then_ignore(tok(TokenKind::Comma).or_not())
        .map_with_span(|(name, targets), span| ComponentActionBinding {
            name,
            targets,
            span: s(span),
        });

    let action_field = ident()
        .try_map(|name, span| {
            if name.value == "action" {
                Ok(name)
            } else {
                Err(Simple::custom(span, "expected 'action'"))
            }
        })
        .ignore_then(tok(TokenKind::Colon))
        .ignore_then(
            action_binding
                .repeated()
                .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace)),
        )
        .map_with_span(|bindings, span| {
            ComponentField::Action(ComponentAction {
                bindings,
                span: s(span),
            })
        });

    let children_constraint = choice((
        tok(TokenKind::All).to(ComponentChildrenConstraint::All),
        ident()
            .then(tok(TokenKind::Pipe).ignore_then(ident()).repeated())
            .map(|(first, rest)| {
                let mut allowed = vec![first];
                allowed.extend(rest);
                ComponentChildrenConstraint::Only(allowed)
            }),
    ));

    let children_field = ident()
        .try_map(|name, span| {
            if name.value == "children" {
                Ok(name)
            } else {
                Err(Simple::custom(span, "expected 'children'"))
            }
        })
        .ignore_then(tok(TokenKind::Colon))
        .ignore_then(children_constraint)
        .map_with_span(|allowed, span| {
            ComponentField::Children(ComponentChildren {
                allowed,
                span: s(span),
            })
        });

    let prop_field = type_config_decl_parser().map(ComponentField::Prop);

    let component_field = choice((action_field, children_field, prop_field))
        .then_ignore(tok(TokenKind::Comma).or_not());

    tok(TokenKind::Component)
        .ignore_then(ident())
        .then(
            component_field
                .repeated()
                .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace)),
        )
        .map_with_span(|(name, fields), span| ComponentDeclaration {
            exported: false,
            name,
            fields,
            span: s(span),
        })
}

pub(crate) fn type_decl_parser(
) -> impl Parser<TokenKind, TypeDeclaration, Error = Simple<TokenKind>> + Clone {
    let example_args = tok(TokenKind::AtExample).ignore_then(
        named_args_parser().delimited_by(tok(TokenKind::LParen), tok(TokenKind::RParen)),
    );

    tok(TokenKind::Type)
        .ignore_then(ident())
        .then(
            type_config_decl_block_parser()
                .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace))
                .then(example_args.repeated()),
        )
        .map_with_span(|(name, (fields, examples)), span| TypeDeclaration {
            exported: false,
            is_output: false,
            name,
            fields,
            examples,
            span: s(span),
        })
}

pub(crate) fn named_prompt_parser(
) -> impl Parser<TokenKind, NamedPrompt, Error = Simple<TokenKind>> + Clone {
    tok(TokenKind::Prompt)
        .ignore_then(ident())
        .then(
            type_config_decl_parser()
                .separated_by(tok(TokenKind::Comma))
                .allow_trailing()
                .delimited_by(tok(TokenKind::LParen), tok(TokenKind::RParen))
                .or_not()
                .map(|opt| opt.unwrap_or_default()),
        )
        .then(
            prompt_stmt_parser()
                .repeated()
                .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace)),
        )
        .map_with_span(|((name, params), body), span| NamedPrompt {
            exported: false,
            name,
            params,
            body,
            span: s(span),
        })
}

pub(crate) fn model_def_parser(
) -> impl Parser<TokenKind, ModelDefinition, Error = Simple<TokenKind>> + Clone {
    let provider_block = tok(TokenKind::Provider)
        .ignore_then(tok(TokenKind::Colon))
        .ignore_then(model_provider_parser())
        .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace));

    tok(TokenKind::Model)
        .ignore_then(ident())
        .then(
            tok(TokenKind::Eq)
                .ignore_then(model_provider_parser())
                .or(provider_block),
        )
        .map_with_span(|(name, provider), span| ModelDefinition {
            exported: false,
            name,
            provider,
            span: s(span),
        })
}

pub(crate) fn intent_decl_parser(
) -> impl Parser<TokenKind, IntentDeclaration, Error = Simple<TokenKind>> + Clone {
    tok(TokenKind::Intent)
        .ignore_then(intent_body_parser())
        .map(|mut decl| {
            // exported flag will be set by the outer export wrapper
            decl.exported = false;
            decl
        })
}

pub(crate) fn import_parser(
) -> impl Parser<TokenKind, FileImport, Error = Simple<TokenKind>> + Clone {
    let import_spec = ident().map_with_span(|name, span| ImportSpecifier {
        kind: None,
        name,
        alias: None,
        span: s(span),
    });

    let named_imports = import_spec
        .separated_by(tok(TokenKind::Comma))
        .allow_trailing()
        .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace))
        .map(ImportShape::Named);

    let wildcard = tok(TokenKind::Star)
        .ignore_then(tok(TokenKind::As))
        .ignore_then(ident())
        .map(|ns| ImportShape::Wildcard { namespace: ns });

    tok(TokenKind::Import)
        .ignore_then(named_imports.or(wildcard))
        .then_ignore(tok(TokenKind::From))
        .then(string_lit())
        .map_with_span(|(kind, path), span| FileImport {
            kind,
            path,
            span: s(span),
        })
}

// ── Entry Point ──────────────────────────────────────────────────────────

pub(crate) fn model_parser() -> impl Parser<TokenKind, Model, Error = Simple<TokenKind>> + 'static {
    let element = choice((
        tok(TokenKind::Export).ignore_then(choice((
            helper_parser().map(|mut h| {
                h.exported = true;
                Element::Helper(h)
            }),
            component_decl_parser().map(|mut c| {
                c.exported = true;
                Element::ComponentDecl(c)
            }),
            type_decl_parser().map(|mut td| {
                td.exported = true;
                Element::TypeDecl(td)
            }),
            named_prompt_parser().map(|mut p| {
                p.exported = true;
                Element::NamedPrompt(p)
            }),
            model_def_parser().map(|mut m| {
                m.exported = true;
                Element::ModelDef(m)
            }),
            intent_decl_parser().map(|mut id| {
                id.exported = true;
                Element::IntentDecl(id)
            }),
        ))),
        agent_parser().map(Element::Agent),
        helper_parser().map(Element::Helper),
        component_decl_parser().map(Element::ComponentDecl),
        type_decl_parser().map(Element::TypeDecl),
        named_prompt_parser().map(Element::NamedPrompt),
        model_def_parser().map(Element::ModelDef),
        intent_decl_parser().map(Element::IntentDecl),
    ))
    .recover_with(nested_delimiters(
        TokenKind::LBrace,
        TokenKind::RBrace,
        [(TokenKind::LParen, TokenKind::RParen)],
        |span: std::ops::Range<usize>| {
            let recovery_span = Span::new(span.start, span.end.max(span.start + 1));
            Element::TypeDecl(TypeDeclaration {
                exported: false,
                is_output: false,
                name: Spanned::new("__error__".to_string(), recovery_span),
                fields: vec![],
                examples: vec![],
                span: recovery_span,
            })
        },
    ))
    .boxed();

    import_parser()
        .repeated()
        .then(element.repeated())
        .then_ignore(end())
        .map(|(imports, elements)| Model { imports, elements })
        .boxed()
}
