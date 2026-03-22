//! Agent config parsers: model config, tool, helpers, lifecycle, test,
//! workflow, prompt statements, and the overall agent_config combinator.

use auwgent_ast::*;

use auwgent_lexer::TokenKind;
use chumsky::prelude::*;

use crate::expr::{condition_parser, expr_parser, named_args_parser, object_literal_parser};
use crate::primitives::*;
use crate::stmt::statement_parser;
use crate::types::{type_config_decl_block_parser, type_config_decl_parser, type_expr_parser};

// ── Model Provider ───────────────────────────────────────────────────────

pub(crate) fn model_provider_parser(
) -> impl Parser<TokenKind, ModelProvider, Error = Simple<TokenKind>> + Clone {
    let obj_arg = object_literal_parser();

    let gemini = tok(TokenKind::Gemini)
        .ignore_then(
            string_lit()
                .then(tok(TokenKind::Comma).ignore_then(obj_arg.clone()).or_not())
                .delimited_by(tok(TokenKind::LParen), tok(TokenKind::RParen)),
        )
        .map_with_span(|(name, config), span| ModelProvider::Gemini {
            model_name: name,
            config,
            span: s(span),
        });

    let openai = tok(TokenKind::Openai)
        .ignore_then(
            string_lit()
                .then(tok(TokenKind::Comma).ignore_then(obj_arg.clone()).or_not())
                .delimited_by(tok(TokenKind::LParen), tok(TokenKind::RParen)),
        )
        .map_with_span(|(name, config), span| ModelProvider::OpenAI {
            model_name: name,
            config,
            span: s(span),
        });

    let custom = tok(TokenKind::Custom)
        .ignore_then(
            string_lit() // id
                .then_ignore(tok(TokenKind::Comma))
                .then(string_lit()) // url
                .then_ignore(tok(TokenKind::Comma))
                .then(string_lit()) // model_name
                .then(tok(TokenKind::Comma).ignore_then(obj_arg).or_not())
                .delimited_by(tok(TokenKind::LParen), tok(TokenKind::RParen)),
        )
        .map_with_span(
            |(((id, url), model_name), config), span| ModelProvider::Custom {
                id,
                url,
                model_name,
                config,
                span: s(span),
            },
        );

    choice((gemini, openai, custom))
}

// ── Prompt Statement ─────────────────────────────────────────────────────

pub(crate) fn prompt_stmt_parser(
) -> impl Parser<TokenKind, PromptStatement, Error = Simple<TokenKind>> + Clone + 'static {
    recursive(
        |pstmt: Recursive<'_, TokenKind, PromptStatement, Simple<TokenKind>>| {
            let expr = expr_parser();
            let statement = statement_parser().map(PromptStatement::Statement);

            // Example blocks
            let message = choice((
                tok(TokenKind::User).map_with_span(|_, span| sp("user".to_string(), span)),
                tok(TokenKind::Assistant)
                    .map_with_span(|_, span| sp("assistant".to_string(), span)),
            ))
            .then_ignore(tok(TokenKind::Colon))
            .then(any_string())
            .map_with_span(|(role, text), span| ExampleMessage {
                role,
                text,
                span: s(span),
            });

            let example = tok(TokenKind::Example)
                .ignore_then(
                    message
                        .repeated()
                        .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace)),
                )
                .map_with_span(|messages, span| {
                    PromptStatement::Example(ExampleBlock {
                        messages,
                        span: s(span),
                    })
                });

            // if/else in prompt
            let prompt_block = pstmt
                .clone()
                .repeated()
                .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace));

            let prompt_if = tok(TokenKind::If)
                .ignore_then(
                    condition_parser().delimited_by(tok(TokenKind::LParen), tok(TokenKind::RParen)),
                )
                .then(prompt_block.clone())
                .then(tok(TokenKind::Else).ignore_then(prompt_block).or_not())
                .map_with_span(|((condition, then_stmts), else_stmts), span| {
                    let block_span = s(span.clone());
                    let then_block = then_stmts
                        .into_iter()
                        .map(|ps| match ps {
                            PromptStatement::Statement(stmt) => stmt,
                            PromptStatement::Expr(e) => Statement::Return(ReturnStatement {
                                span: block_span,
                                value: e,
                            }),
                            PromptStatement::If(ifs) => Statement::If(ifs),
                            _ => Statement::Return(ReturnStatement {
                                span: block_span,
                                value: Expr::StringLit(sp(String::new(), span.clone())),
                            }),
                        })
                        .collect();
                    let else_block = else_stmts
                        .unwrap_or_default()
                        .into_iter()
                        .map(|ps| match ps {
                            PromptStatement::Statement(stmt) => stmt,
                            PromptStatement::Expr(e) => Statement::Return(ReturnStatement {
                                span: block_span,
                                value: e,
                            }),
                            PromptStatement::If(ifs) => Statement::If(ifs),
                            _ => Statement::Return(ReturnStatement {
                                span: block_span,
                                value: Expr::StringLit(sp(String::new(), span.clone())),
                            }),
                        })
                        .collect();
                    PromptStatement::If(IfStatement {
                        condition,
                        then_block,
                        else_block,
                        span: s(span),
                    })
                });

            choice((
                example,
                prompt_if,
                statement,
                expr.map(PromptStatement::Expr),
            ))
        },
    )
    .boxed()
}

// ── Model Config Block ───────────────────────────────────────────────────

enum ModelConfigField {
    Model(ModelProviderRef),
    Embedding(ModelProviderRef),
    Prompt((Vec<PromptStatement>, Option<Expr>)),
}

pub(crate) fn model_config_parser(
) -> impl Parser<TokenKind, ModelConfig, Error = Simple<TokenKind>> + Clone {
    let model_ref = choice((
        model_provider_parser().map(ModelProviderRef::Inline),
        ident().map(ModelProviderRef::Ref),
    ));

    let prompt_block = tok(TokenKind::Prompt).ignore_then(choice((
        // prompt { ... } (block form)
        prompt_stmt_parser()
            .repeated()
            .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace))
            .map(|parts| (parts, None)),
        // prompt: expr (expression form)
        tok(TokenKind::Colon)
            .ignore_then(expr_parser())
            .map(|e| (vec![], Some(e))),
    )));

    let field = choice((
        tok(TokenKind::Model)
            .ignore_then(tok(TokenKind::Colon))
            .ignore_then(model_ref.clone())
            .map(ModelConfigField::Model),
        tok(TokenKind::Embedding)
            .ignore_then(tok(TokenKind::Colon))
            .ignore_then(model_ref)
            .map(ModelConfigField::Embedding),
        prompt_block.map(ModelConfigField::Prompt),
    ));

    field.repeated().map_with_span(|fields, span| {
        let mut model = None;
        let mut embedding = None;
        let mut prompt_block = vec![];
        let mut prompt_expr = None;

        for f in fields {
            match f {
                ModelConfigField::Model(m) => model = Some(m),
                ModelConfigField::Embedding(e) => embedding = Some(e),
                ModelConfigField::Prompt((block, expr)) => {
                    prompt_block = block;
                    prompt_expr = expr;
                }
            }
        }

        ModelConfig {
            model: model.unwrap_or(ModelProviderRef::Inline(ModelProvider::Gemini {
                model_name: sp("gemini-2.0-flash".to_string(), span.clone()),
                config: None,
                span: s(span.clone()),
            })),
            embedding,
            prompt_block,
            prompt_expr,
            span: s(span),
        }
    })
}

pub(crate) fn agent_model_config_parser(
) -> impl Parser<TokenKind, AgentModelConfig, Error = Simple<TokenKind>> + Clone {
    let default_config = tok(TokenKind::Default)
        .ignore_then(tok(TokenKind::Config))
        .ignore_then(
            model_config_parser().delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace)),
        );

    let named_config = tok(TokenKind::Config)
        .ignore_then(ident())
        .then(model_config_parser().delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace)))
        .map_with_span(|(name, config), span| NamedModelConfig {
            name,
            config,
            span: s(span),
        });

    default_config.then(named_config.repeated()).map_with_span(
        |(default_config, named_configs), span| AgentModelConfig {
            default_config,
            named_configs,
            span: s(span),
        },
    )
}

// ── Tool Function ────────────────────────────────────────────────────────

pub(crate) fn tool_function_parser(
) -> impl Parser<TokenKind, ToolFunction, Error = Simple<TokenKind>> + Clone {
    let example_args = tok(TokenKind::AtExample)
        .ignore_then(
            named_args_parser()
                .delimited_by(tok(TokenKind::LParen), tok(TokenKind::RParen))
        );

    ident()
        .then(
            type_config_decl_parser()
                .separated_by(tok(TokenKind::Comma))
                .allow_trailing()
                .delimited_by(tok(TokenKind::LParen), tok(TokenKind::RParen)),
        )
        .then(
            tok(TokenKind::Colon)
                .ignore_then(type_expr_parser())
                .or_not(),
        )
        .then(tok(TokenKind::AtDesc).ignore_then(string_lit()).repeated())
        .then(example_args.repeated())
        .map_with_span(|((((name, params), returns), desc), examples), span| ToolFunction {
            name,
            params,
            returns,
            description: desc,
            examples,
            span: s(span),
        })
}

// ── Helpers Config ───────────────────────────────────────────────────────

pub(crate) fn helper_ref_parser(
) -> impl Parser<TokenKind, HelperRef, Error = Simple<TokenKind>> + Clone {
    let tool_grant = tok(TokenKind::With)
        .ignore_then(choice((
            tok(TokenKind::All)
                .ignore_then(tok(TokenKind::Tools))
                .to((true, vec![])),
            tok(TokenKind::Tools)
                .ignore_then(
                    ident()
                        .separated_by(tok(TokenKind::Comma))
                        .allow_trailing()
                        .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace)),
                )
                .map(|tools| (false, tools)),
        )))
        .or_not()
        .map(|opt| opt.unwrap_or((false, vec![])));

    let handoff = tok(TokenKind::Handoff)
        .ignore_then(tok(TokenKind::User))
        .or_not()
        .map(|opt| opt.is_some());

    let then_continue = tok(TokenKind::Then)
        .ignore_then(tok(TokenKind::Continue))
        .or_not()
        .map(|opt| opt.is_some());

    ident()
        .then(tool_grant)
        .then(handoff)
        .then(then_continue)
        .map_with_span(
            |(((name, (with_all, tools)), handoff_user), tc), span| HelperRef {
                name,
                with_all_tools: with_all,
                granted_tools: tools,
                handoff_user,
                handoff_then_continue: tc,
                span: s(span),
            },
        )
}

// ── Lifecycle Config ─────────────────────────────────────────────────────

pub(crate) fn lifecycle_parser(
) -> impl Parser<TokenKind, LifecycleConfig, Error = Simple<TokenKind>> + Clone {
    let setting = choice((
        tok(TokenKind::MaxTokens)
            .ignore_then(tok(TokenKind::Colon))
            .ignore_then(number_int())
            .map(LifecycleSetting::MaxTokens),
        tok(TokenKind::MaxMessages)
            .ignore_then(tok(TokenKind::Colon))
            .ignore_then(number_int())
            .map(LifecycleSetting::MaxMessages),
    ));

    tok(TokenKind::Use)
        .ignore_then(tok(TokenKind::Lifecycle))
        .ignore_then(
            setting
                .repeated()
                .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace)),
        )
        .map_with_span(|settings, span| {
            let mut max_tokens = None;
            let mut max_messages = None;
            for setting in settings {
                match setting {
                    LifecycleSetting::MaxTokens(n) => max_tokens = Some(n),
                    LifecycleSetting::MaxMessages(n) => max_messages = Some(n),
                }
            }
            LifecycleConfig {
                max_tokens,
                max_messages,
                span: s(span),
            }
        })
}

#[derive(Clone)]
enum LifecycleSetting {
    MaxTokens(Spanned<i64>),
    MaxMessages(Spanned<i64>),
}

// ── Test Config ──────────────────────────────────────────────────────────

pub(crate) fn test_config_parser(
) -> impl Parser<TokenKind, TestConfig, Error = Simple<TokenKind>> + Clone {
    tok(TokenKind::Test)
        .ignore_then(string_lit())
        .then(
            tok(TokenKind::Config)
                .ignore_then(tok(TokenKind::Colon))
                .ignore_then(ident())
                .or_not(),
        )
        .then(
            // Skip test body for now — consume balanced braces
            none_of(TokenKind::RBrace)
                .repeated()
                .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace)),
        )
        .map_with_span(|((name, config_name), _body), span| TestConfig {
            name,
            config_name,
            input: None,
            tool_stubs: vec![],
            expectations: vec![],
            model: None,
            span: s(span),
        })
}

// ── Intent Config ─────────────────────────────────────────────────────────

/// Parse a single named intent body (for inline and top-level):
/// `name { description: "..." fields { ... } }`
pub(crate) fn intent_body_parser(
) -> impl Parser<TokenKind, IntentDeclaration, Error = Simple<TokenKind>> + Clone {
    let desc = tok(TokenKind::Description)
        .ignore_then(tok(TokenKind::Colon))
        .ignore_then(string_lit())
        .or_not();

    let fields = tok(TokenKind::Fields)
        .ignore_then(
            type_config_decl_block_parser()
                .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace)),
        )
        .or_not()
        .map(|opt| opt.unwrap_or_default());

    let example_args = tok(TokenKind::AtExample)
        .ignore_then(
            named_args_parser()
                .delimited_by(tok(TokenKind::LParen), tok(TokenKind::RParen))
        );

    ident()
        .then(
            desc.then(fields)
                .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace))
                .then(example_args.repeated()),
        )
        .map_with_span(|(name, ((description, fields), examples)), span| IntentDeclaration {
            exported: false,
            name,
            description,
            fields,
            examples,
            span: s(span),
        })
}

/// Parse the `+`-composed intent expression:
/// atom = Ident | `{` intent_body+ `}`
/// expr = atom (`+` atom)*
fn intent_expr_parser() -> impl Parser<TokenKind, IntentExpr, Error = Simple<TokenKind>> + Clone {
    let inline_block = intent_body_parser()
        .repeated()
        .at_least(1)
        .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace))
        .map(IntentExpr::Inline);

    let atom = inline_block.or(ident().map(IntentExpr::Ref));

    atom.clone()
        .then(tok(TokenKind::Plus).ignore_then(atom).repeated())
        .map(|(first, rest)| {
            rest.into_iter().fold(first, |acc, next| {
                IntentExpr::Compose(Box::new(acc), Box::new(next))
            })
        })
}

pub(crate) fn intent_config_parser(
) -> impl Parser<TokenKind, AgentConfig, Error = Simple<TokenKind>> + Clone {
    tok(TokenKind::Intent)
        .ignore_then(tok(TokenKind::Colon))
        .ignore_then(intent_expr_parser())
        .map_with_span(|expr, span| {
            AgentConfig::Intent(IntentConfig {
                expr,
                span: s(span),
            })
        })
}

// ── Agent Config (combined) ──────────────────────────────────────────────

pub(crate) fn agent_config_parser(
) -> impl Parser<TokenKind, AgentConfig, Error = Simple<TokenKind>> + Clone + 'static {
    let input_config = tok(TokenKind::Input)
        .ignore_then(choice((
            // input: Text
            tok(TokenKind::Colon)
                .ignore_then(type_expr_parser())
                .map(InputShape::Direct),
            // input { ... }
            type_config_decl_block_parser()
                .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace))
                .map(InputShape::Properties),
        )))
        .map_with_span(|shape, span| {
            AgentConfig::Input(InputConfig {
                shape,
                span: s(span),
            })
        });

    let example_args = tok(TokenKind::AtExample)
        .ignore_then(
            named_args_parser()
                .delimited_by(tok(TokenKind::LParen), tok(TokenKind::RParen))
        );

    let output_props = tok(TokenKind::Output)
        .ignore_then(
            tok(TokenKind::Colon)
                .ignore_then(
                    // Union parser: Text | A | B or A | B
                    // Accept both Text keyword and identifiers
                    choice((
                        tok(TokenKind::TextType).map_with_span(|_, span| Spanned {
                            value: "Text".to_string(),
                            span: s(span),
                        }),
                        ident(),
                    ))
                        .then(
                            tok(TokenKind::Pipe)
                                .ignore_then(choice((
                                    tok(TokenKind::TextType).map_with_span(|_, span| Spanned {
                                        value: "Text".to_string(),
                                        span: s(span),
                                    }),
                                    ident(),
                                )))
                                .repeated()
                                .at_least(1),
                        )
                        .then(example_args.clone().repeated())
                        .map_with_span(|((first, rest), examples), span| {
                            let mut all = vec![first];
                            all.extend(rest);
                            OutputConfig {
                                shape: OutputShape::Union(all),
                                examples,
                                span: s(span),
                            }
                        })
                        .or(type_expr_parser()
                            .then(tok(TokenKind::AtDesc).ignore_then(string_lit()).or_not())
                            .then(example_args.clone().repeated())
                            .map_with_span(|((ty, desc), examples), span| OutputConfig {
                                shape: OutputShape::Direct { ty, desc },
                                examples,
                                span: s(span),
                            })),
                )
                .or(type_config_decl_parser()
                    .then(tok(TokenKind::AtDesc).ignore_then(string_lit()).or_not())
                    .map(|(decl, desc)| OutputProperty {
                        decl,
                        description: desc,
                    })
                    .then_ignore(tok(TokenKind::Comma).or_not())
                    .repeated()
                    .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace))
                    .then(example_args.repeated())
                    .map_with_span(|(props, examples), span| OutputConfig {
                        shape: OutputShape::Properties(props),
                        examples,
                        span: s(span),
                    })),
        )
        .map(AgentConfig::Output);

    let context_config = tok(TokenKind::Context)
        .ignore_then(
            type_config_decl_block_parser()
                .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace)),
        )
        .map_with_span(|props, span| {
            AgentConfig::Context(ContextConfig {
                properties: props,
                span: s(span),
            })
        });

    let tool_single = tok(TokenKind::Tool)
        .ignore_then(tool_function_parser())
        .map(AgentConfig::Tool);

    let tools_block = tok(TokenKind::Tools)
        .ignore_then(
            tool_function_parser()
                .repeated()
                .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace)),
        )
        .map(AgentConfig::Tools);

    let helpers_config = tok(TokenKind::Helpers)
        .ignore_then(
            helper_ref_parser()
                .repeated()
                .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace)),
        )
        .map_with_span(|helpers, span| {
            AgentConfig::Helpers(HelpersConfig {
                helpers,
                span: s(span),
            })
        });

    let example_args = tok(TokenKind::AtExample)
        .ignore_then(
            named_args_parser()
                .delimited_by(tok(TokenKind::LParen), tok(TokenKind::RParen))
        );

    let workflow = tok(TokenKind::Workflow)
        .ignore_then(ident())
        .then(
            type_config_decl_parser()
                .separated_by(tok(TokenKind::Comma))
                .allow_trailing()
                .delimited_by(tok(TokenKind::LParen), tok(TokenKind::RParen)),
        )
        .then(
            tok(TokenKind::Colon)
                .ignore_then(type_expr_parser())
                .or_not(),
        )
        .then(workflow_body_parser().delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace)))
        .then(example_args.repeated())
        .map_with_span(
            |((((name, params), ret_ty), (desc, tool_configs, body)), examples), span| {
                AgentConfig::Workflow(WorkflowConfig {
                    name,
                    params,
                    return_type: ret_ty.unwrap_or(TypeExpr::String(s(span.clone()))),
                    description: desc,
                    tool_configs,
                    body,
                    examples,
                    span: s(span),
                })
            },
        );

    let model_config = agent_model_config_parser().map(AgentConfig::Model);
    let lifecycle = lifecycle_parser().map(AgentConfig::Lifecycle);
    let test = test_config_parser().map(AgentConfig::Test);
    let intent = intent_config_parser();

    choice((
        input_config,
        output_props,
        context_config,
        tools_block,
        tool_single,
        helpers_config,
        workflow,
        model_config,
        lifecycle,
        test,
    ))
    .or(intent)
    .recover_with(nested_delimiters(
        TokenKind::LBrace,
        TokenKind::RBrace,
        [(TokenKind::LParen, TokenKind::RParen)],
        |span| {
            AgentConfig::Input(InputConfig {
                shape: InputShape::Properties(vec![]),
                span: s(span),
            })
        },
    ))
    .boxed()
}

fn workflow_body_parser() -> impl Parser<
    TokenKind,
    (Option<Spanned<String>>, Vec<ToolFunction>, Vec<Statement>),
    Error = Simple<TokenKind>,
> + Clone {
    let desc = tok(TokenKind::Description)
        .ignore_then(tok(TokenKind::Colon))
        .ignore_then(string_lit())
        .or_not();

    let tools = choice((
        tok(TokenKind::Tool)
            .ignore_then(tool_function_parser())
            .map(|t| vec![t]),
        tok(TokenKind::Tools).ignore_then(
            tool_function_parser()
                .repeated()
                .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace)),
        ),
    ))
    .repeated()
    .map(|v| v.into_iter().flatten().collect::<Vec<_>>());

    let stmts = statement_parser().repeated();

    desc.then(tools).then(stmts).map(|((d, t), s)| (d, t, s))
}
