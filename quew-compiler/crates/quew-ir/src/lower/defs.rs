use std::sync::Arc;

use indexmap::IndexMap;
use quew_ast::{
    AnnotationArgs, ConfigField, Expr, FieldDef, FunctionDecl, Item, Lit, ModelDecl, Module, Param,
    ParamBinding, Provider, ToolDecl, ToolEntry, ToolsDecl, TypeDecl, TypeExpr,
};
use quew_checker::CheckResult;
use quew_interner::Interner;
use quew_lexer::AnnotationKind;

use crate::defs::{
    AgentDef, Definitions, DisclosureMode, FunctionDef, ModelDef, ProtocolMode, ProviderKind,
    ToolDef, ToolKind, ToolParam, TypeDef,
};
use crate::graph::AgentGraph;
use crate::types::{IrField, IrType};

/// Walk every top-level `Item` and populate the `Definitions` registry.
/// Also emits sub-graphs for function bodies into `graphs`.
pub fn lower_definitions(
    module: &Module,
    _check: &CheckResult,
    interner: &Arc<Interner>,
    definitions: &mut Definitions,
    graphs: &mut IndexMap<String, AgentGraph>,
) {
    for item in &module.items {
        match item {
            Item::Type(decl) => lower_type(decl, interner, definitions),
            Item::Model(decl) => lower_model(decl, interner, definitions),
            Item::Tool(decl) => lower_tool(decl, interner, definitions),
            Item::Tools(decl) => lower_tools_group(decl, interner, definitions),
            Item::Function(decl) => lower_function(decl, interner, definitions, graphs),
            Item::Agent(decl) => {
                let context = decl.annotations.iter().find_map(|ann| {
                    if ann.kind == AnnotationKind::Context {
                        if let AnnotationArgs::Type(TypeExpr::Named(name, _)) = &ann.args {
                            return Some(*name);
                        }
                    }
                    None
                });

                definitions.agents.insert(
                    decl.name,
                    AgentDef {
                        input: Some(lower_type_expr(&decl.param.ty, interner)),
                        output: decl
                            .return_ty
                            .as_ref()
                            .map(|ty| lower_type_expr(ty, interner)),
                        context,
                        protocol: protocol_mode(decl),
                        graph_ref: format!("agent:{}", interner.resolve(decl.name)),
                    },
                );
            }
            Item::Let(_) => { /* top-level let: not yet supported */ }
        }
    }
}

fn protocol_mode(decl: &quew_ast::AgentDecl) -> ProtocolMode {
    for ann in &decl.annotations {
        match ann.kind {
            AnnotationKind::Native => return ProtocolMode::Native,
            AnnotationKind::Block => return ProtocolMode::Block,
            _ => {}
        }
    }
    ProtocolMode::Block
}

fn lower_type(decl: &TypeDecl, interner: &Arc<Interner>, defs: &mut Definitions) {
    let fields = decl
        .fields
        .iter()
        .map(|field| (field.name, lower_field(field, interner)))
        .collect();

    defs.types.insert(decl.name, TypeDef { fields });
}

fn lower_model(decl: &ModelDecl, interner: &Arc<Interner>, defs: &mut Definitions) {
    defs.models.insert(
        decl.name,
        lower_model_def(&decl.provider, &decl.config, interner),
    );
}

fn lower_tool(decl: &ToolDecl, interner: &Arc<Interner>, defs: &mut Definitions) {
    defs.tools.insert(
        decl.name,
        lower_host_tool(
            decl.name,
            &decl.params,
            &decl.return_ty,
            decl.desc.as_ref().map(|d| d.value),
            interner,
        ),
    );
}

fn lower_tools_group(decl: &ToolsDecl, interner: &Arc<Interner>, defs: &mut Definitions) {
    for entry in &decl.entries {
        defs.tools
            .insert(entry.name, lower_tool_entry(entry, interner));
    }

    if let Some(name) = decl.name {
        defs.tools.insert(
            name,
            ToolDef {
                kind: ToolKind::Group {
                    members: decl.entries.iter().map(|entry| entry.name).collect(),
                    disclosure: DisclosureMode::Lazy,
                },
                description: decl.desc.as_ref().map(|d| d.value),
            },
        );
    }
}

fn lower_function(
    decl: &FunctionDecl,
    interner: &Arc<Interner>,
    defs: &mut Definitions,
    _graphs: &mut IndexMap<String, AgentGraph>,
) {
    let graph_ref = format!("function:{}", interner.resolve(decl.name));
    let is_tool = decl
        .annotations
        .iter()
        .any(|ann| ann.kind == AnnotationKind::Tool);
    let description = decl.annotations.iter().find_map(|ann| {
        if ann.kind == AnnotationKind::Desc {
            if let AnnotationArgs::String(s) = &ann.args {
                return Some(s.value);
            }
        }
        None
    });

    if is_tool {
        let mut model_params = IndexMap::new();
        let mut host_params = IndexMap::new();
        for param in &decl.params {
            let lowered = lower_tool_param(param, interner);
            match param.binding {
                ParamBinding::Normal => {
                    model_params.insert(param.name, lowered);
                }
                ParamBinding::BoundRef => {
                    host_params.insert(param.name, lowered);
                }
            }
        }

        defs.tools.insert(
            decl.name,
            ToolDef {
                kind: ToolKind::Dsl {
                    model_params,
                    host_params,
                    returns: decl
                        .return_ty
                        .as_ref()
                        .map(|ty| lower_type_expr(ty, interner))
                        .unwrap_or(IrType::Void),
                    graph_ref,
                },
                description,
            },
        );
    } else {
        let params = decl
            .params
            .iter()
            .map(|param| (param.name, lower_type_expr(&param.ty, interner)))
            .collect();

        defs.functions.insert(
            decl.name,
            FunctionDef {
                params,
                returns: decl
                    .return_ty
                    .as_ref()
                    .map(|ty| lower_type_expr(ty, interner))
                    .unwrap_or(IrType::Void),
                graph_ref,
            },
        );
    }
}

pub(crate) fn lower_type_expr(expr: &TypeExpr, interner: &Arc<Interner>) -> IrType {
    match expr {
        TypeExpr::Named(name, _) => match interner.resolve(*name) {
            "string" => IrType::String,
            "number" => IrType::Number,
            "float" => IrType::Float,
            "bool" => IrType::Bool,
            "null" => IrType::Null,
            "void" => IrType::Void,
            "Text" => IrType::Text,
            _ => IrType::Named(*name),
        },
        TypeExpr::Union(members, _) => IrType::Union(
            members
                .iter()
                .map(|member| lower_type_expr(member, interner))
                .collect(),
        ),
        TypeExpr::Optional(inner, _) => {
            IrType::Union(vec![lower_type_expr(inner, interner), IrType::Null])
        }
        TypeExpr::Generic(name, _, _) => IrType::Named(*name),
    }
}

fn lower_field(field: &FieldDef, interner: &Arc<Interner>) -> IrField {
    IrField {
        ty: lower_type_expr(&field.ty, interner),
        optional: field.optional,
    }
}

fn lower_model_def(
    provider: &quew_ast::ProviderCall,
    config: &[ConfigField],
    interner: &Arc<Interner>,
) -> ModelDef {
    let mut lowered_config: IndexMap<_, _> = provider
        .config
        .iter()
        .map(|field| (field.key, config_value(&field.value, interner)))
        .collect();
    for field in config {
        lowered_config.insert(field.key, config_value(&field.value, interner));
    }

    ModelDef {
        provider: lower_provider(provider.provider),
        model_name: provider.model_name.value,
        config: lowered_config,
    }
}

fn lower_provider(provider: Provider) -> ProviderKind {
    match provider {
        Provider::Gemini => ProviderKind::Gemini,
        Provider::OpenAi => ProviderKind::OpenAi,
        Provider::Groq => ProviderKind::Groq,
    }
}

fn lower_host_tool(
    _name: quew_interner::InternedStr,
    params: &[Param],
    return_ty: &TypeExpr,
    description: Option<quew_interner::InternedStr>,
    interner: &Arc<Interner>,
) -> ToolDef {
    ToolDef {
        kind: ToolKind::Host {
            params: params
                .iter()
                .map(|param| (param.name, lower_tool_param(param, interner)))
                .collect(),
            returns: lower_type_expr(return_ty, interner),
        },
        description,
    }
}

fn lower_tool_entry(entry: &ToolEntry, interner: &Arc<Interner>) -> ToolDef {
    lower_host_tool(
        entry.name,
        &entry.params,
        &entry.return_ty,
        entry.desc.as_ref().map(|d| d.value),
        interner,
    )
}

fn lower_tool_param(param: &Param, interner: &Arc<Interner>) -> ToolParam {
    ToolParam {
        ty: lower_type_expr(&param.ty, interner),
        optional: param.optional,
        description: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use quew_ast::{ProviderCall, StringKind, StringLit};
    use quew_errors::Span;

    fn interner() -> Arc<Interner> {
        Arc::new(Interner::new())
    }

    fn sp() -> Span {
        Span::new(0, 1)
    }

    fn named(interner: &Arc<Interner>, name: &str) -> TypeExpr {
        TypeExpr::Named(interner.intern(name), sp())
    }

    fn string_lit(interner: &Arc<Interner>, value: &str) -> StringLit {
        StringLit {
            value: interner.intern(value),
            kind: StringKind::Regular,
            span: sp(),
        }
    }

    fn param(interner: &Arc<Interner>, name: &str, ty: &str) -> Param {
        Param {
            binding: ParamBinding::Normal,
            name: interner.intern(name),
            ty: named(interner, ty),
            optional: false,
            span: sp(),
        }
    }

    #[test]
    fn lower_type_decl_preserves_field_order_and_optional_flags() {
        let interner = interner();
        let mut defs = Definitions::default();
        let decl = TypeDecl {
            name: interner.intern("User"),
            fields: vec![
                FieldDef {
                    name: interner.intern("id"),
                    ty: named(&interner, "string"),
                    optional: false,
                    span: sp(),
                },
                FieldDef {
                    name: interner.intern("age"),
                    ty: named(&interner, "number"),
                    optional: true,
                    span: sp(),
                },
            ],
            span: sp(),
        };

        lower_type(&decl, &interner, &mut defs);

        let fields = &defs.types[&decl.name].fields;
        let names: Vec<_> = fields.keys().map(|key| interner.resolve(*key)).collect();
        assert_eq!(names, vec!["id", "age"]);
        assert_eq!(fields[&interner.intern("id")].ty, IrType::String);
        assert!(fields[&interner.intern("age")].optional);
    }

    #[test]
    fn lower_model_decl_records_provider_model_and_config() {
        let interner = interner();
        let mut defs = Definitions::default();
        let top_k = interner.intern("topK");
        let temperature = interner.intern("temperature");
        let decl = ModelDecl {
            name: interner.intern("Gemini"),
            provider: ProviderCall {
                provider: Provider::Gemini,
                model_name: string_lit(&interner, "gemini-pro"),
                config: vec![ConfigField {
                    key: top_k,
                    value: Box::new(Expr::Lit(Lit::Int(40, sp()))),
                    span: sp(),
                }],
                span: sp(),
            },
            config: vec![ConfigField {
                key: temperature,
                value: Box::new(Expr::Lit(Lit::Float(0.2, sp()))),
                span: sp(),
            }],
            span: sp(),
        };

        lower_model(&decl, &interner, &mut defs);

        let model = &defs.models[&decl.name];
        assert_eq!(model.provider, ProviderKind::Gemini);
        assert_eq!(interner.resolve(model.model_name), "gemini-pro");
        assert_eq!(model.config[&top_k], "40");
        assert_eq!(model.config[&temperature], "0.2");
    }

    #[test]
    fn lower_host_tool_records_params_return_type_and_description() {
        let interner = interner();
        let mut defs = Definitions::default();
        let desc = string_lit(&interner, "Fetch weather");
        let decl = ToolDecl {
            name: interner.intern("getWeather"),
            params: vec![param(&interner, "city", "string")],
            return_ty: named(&interner, "Text"),
            desc: Some(desc.clone()),
            span: sp(),
        };

        lower_tool(&decl, &interner, &mut defs);

        let tool = &defs.tools[&decl.name];
        assert_eq!(tool.description, Some(desc.value));
        match &tool.kind {
            ToolKind::Host { params, returns } => {
                assert_eq!(params[&interner.intern("city")].ty, IrType::String);
                assert_eq!(*returns, IrType::Text);
            }
            other => panic!("expected host tool, got {other:?}"),
        }
    }
}
