//! Lower a `reply(...) with { ... }` block into `ReplyConfig`.

use std::sync::Arc;

use indexmap::IndexMap;
use quew_ast::{Expr, Lit, Provider, WithBlock};
use quew_checker::CheckResult;
use quew_interner::{InternedStr, Interner};

use crate::defs::{Definitions, ModelDef, ProviderKind};
use crate::graph::{AgentRef, IrPrompt, ModelRef, ReplyConfig, ToolRef};

use super::ctx::LowerCtx;
use super::expr::lower_expr_as_ref;

/// Lower the `with { ... }` block of a `reply(...)` statement.
pub fn lower_reply_config(
    with_block: &WithBlock,
    check: &CheckResult,
    interner: &Arc<Interner>,
    defs: &mut Definitions,
    ctx: &mut LowerCtx,
) -> ReplyConfig {
    let keys = Keys::new(interner);
    let mut prompt = IrPrompt::Literal(interner.intern_static(""));
    let mut model = None;
    let mut fallback = None;
    let mut retry = None;
    let mut max_turn = None;
    let mut tools = Vec::new();
    let mut builtin = Vec::new();
    let mut agents = Vec::new();

    for field in &with_block.fields {
        if field.key == keys.prompt {
            if let Expr::Lit(Lit::String(value)) = &field.value {
                prompt = IrPrompt::Literal(value.value);
            }
        } else if field.key == keys.model {
            model = Some(lower_model_ref(&field.value, defs, interner));
        } else if field.key == keys.fallback {
            fallback = Some(lower_model_ref(&field.value, defs, interner));
        } else if field.key == keys.retry {
            retry = int_field(&field.value);
        } else if field.key == keys.max_turn {
            max_turn = int_field(&field.value);
        } else if field.key == keys.tools {
            tools = lower_tool_refs(&field.value, check, ctx);
        } else if field.key == keys.builtin {
            builtin = lower_ident_array(&field.value);
        } else if field.key == keys.agents {
            agents = lower_agent_refs(&field.value);
        }
    }

    ReplyConfig {
        prompt,
        model: model.unwrap_or_else(|| ModelRef::Named(interner.intern_static("<missing-model>"))),
        fallback,
        retry,
        max_turn,
        tools,
        builtin,
        agents,
    }
}

struct Keys {
    model: InternedStr,
    fallback: InternedStr,
    prompt: InternedStr,
    tools: InternedStr,
    builtin: InternedStr,
    agents: InternedStr,
    retry: InternedStr,
    max_turn: InternedStr,
}

impl Keys {
    fn new(interner: &Interner) -> Self {
        Self {
            model: interner.intern("model"),
            fallback: interner.intern("fallback"),
            prompt: interner.intern("prompt"),
            tools: interner.intern("tools"),
            builtin: interner.intern("builtin"),
            agents: interner.intern("agents"),
            retry: interner.intern("retry"),
            max_turn: interner.intern("maxTurn"),
        }
    }
}

fn lower_model_ref(expr: &Expr, defs: &mut Definitions, interner: &Arc<Interner>) -> ModelRef {
    match expr {
        Expr::Ident(ident) => ModelRef::Named(ident.name),
        Expr::Provider(provider) => {
            let key = interner.intern(&format!(
                "__inline_{}_{}",
                provider_name(provider.provider),
                interner.resolve(provider.model_name.value)
            ));
            let def = ModelDef {
                provider: lower_provider(provider.provider),
                model_name: provider.model_name.value,
                config: provider
                    .config
                    .iter()
                    .map(|field| (field.key, config_value(&field.value, interner)))
                    .collect(),
            };
            defs.models.entry(key).or_insert_with(|| def.clone());
            ModelRef::Inline { key, def }
        }
        _ => panic!("lowering bug: checker accepted non-model expression in reply config"),
    }
}

fn lower_provider(provider: Provider) -> ProviderKind {
    match provider {
        Provider::Gemini => ProviderKind::Gemini,
        Provider::OpenAi => ProviderKind::OpenAi,
        Provider::Groq => ProviderKind::Groq,
    }
}

fn provider_name(provider: Provider) -> &'static str {
    match provider {
        Provider::Gemini => "gemini",
        Provider::OpenAi => "openai",
        Provider::Groq => "groq",
    }
}

fn lower_tool_refs(expr: &Expr, check: &CheckResult, ctx: &mut LowerCtx) -> Vec<ToolRef> {
    match expr {
        Expr::Array(array) => array
            .elements
            .iter()
            .map(|element| lower_tool_ref(element, check, ctx))
            .collect(),
        Expr::Ident(_) | Expr::Call(_) => vec![lower_tool_ref(expr, check, ctx)],
        _ => Vec::new(),
    }
}

fn lower_tool_ref(expr: &Expr, check: &CheckResult, ctx: &mut LowerCtx) -> ToolRef {
    match expr {
        Expr::Ident(ident) => ToolRef {
            name: ident.name,
            host_args: IndexMap::new(),
        },
        Expr::Call(call) => {
            let name = match call.callee.as_ref() {
                Expr::Ident(ident) => ident.name,
                _ => panic!("lowering bug: non-identifier tool call"),
            };
            let mut host_args = IndexMap::new();
            for arg in &call.args {
                let data = lower_expr_as_ref(arg, check, ctx);
                if let Some(slot) = data.slot {
                    host_args.insert(slot, data);
                }
            }
            ToolRef { name, host_args }
        }
        _ => panic!("lowering bug: invalid tool reference expression"),
    }
}

fn lower_agent_refs(expr: &Expr) -> Vec<AgentRef> {
    match expr {
        Expr::Array(array) => array.elements.iter().filter_map(agent_ref).collect(),
        _ => agent_ref(expr).into_iter().collect(),
    }
}

fn agent_ref(expr: &Expr) -> Option<AgentRef> {
    if let Expr::Ident(ident) = expr {
        Some(AgentRef {
            name: ident.name,
            handoff: crate::graph::AgentCallMode::BlackBox,
        })
    } else {
        None
    }
}

fn lower_ident_array(expr: &Expr) -> Vec<InternedStr> {
    match expr {
        Expr::Array(array) => array.elements.iter().filter_map(ident_name).collect(),
        _ => ident_name(expr).into_iter().collect(),
    }
}

fn ident_name(expr: &Expr) -> Option<InternedStr> {
    if let Expr::Ident(ident) = expr {
        Some(ident.name)
    } else {
        None
    }
}

fn int_field(expr: &Expr) -> Option<u32> {
    if let Expr::Lit(Lit::Int(value, _)) = expr {
        (*value).try_into().ok()
    } else {
        None
    }
}

fn config_value(expr: &Expr, interner: &Arc<Interner>) -> String {
    match expr {
        Expr::Lit(Lit::String(s)) => interner.resolve(s.value).to_string(),
        Expr::Lit(Lit::Int(value, _)) => value.to_string(),
        Expr::Lit(Lit::Float(value, _)) => value.to_string(),
        Expr::Lit(Lit::Bool(value, _)) => value.to_string(),
        Expr::Lit(Lit::Null(_)) => "null".to_string(),
        Expr::Ident(ident) => interner.resolve(ident.name).to_string(),
        _ => "<expr>".to_string(),
    }
}
