use std::sync::Arc;

use indexmap::IndexMap;
use quew_ast::{
    AnnotationArgs, BuiltinTypeMeta, BuiltinVisibility, ConfigField, Expr, FieldDef, FunctionDecl,
    Item, Lit, ModelDecl, Module, Param, ParamBinding, Provider, ToolDecl, ToolEntry, ToolsDecl,
    TypeDecl, TypeExpr,
};
use quew_checker::CheckResult;
use quew_interner::{InternedStr, Interner};
use quew_lexer::AnnotationKind;

use crate::defs::{
    AgentDef, Definitions, DisclosureMode, ExtensionDef, FunctionDef, IrRoleBinding, IrRoleKey,
    IrTypeVisibility, ModelDef, ProtocolMode, ProviderKind, ToolDef, ToolKind, ToolParam, TypeDef,
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
            Item::Function(decl) => lower_function(decl, _check, interner, definitions, graphs),
            Item::Extend(decl) => lower_extend(decl, _check, interner, definitions, graphs),
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

fn lower_extend(
    decl: &quew_ast::ExtendDecl,
    check: &CheckResult,
    interner: &Arc<Interner>,
    defs: &mut Definitions,
    graphs: &mut IndexMap<String, AgentGraph>,
) {
    let receiver = lower_type_expr(&decl.receiver, interner);
    for method in &decl.methods {
        let params: IndexMap<InternedStr, IrType> = method
            .params
            .iter()
            .map(|param| {
                (
                    param.name,
                    lower_type_expr_with_params(&param.ty, &method.type_params, interner),
                )
            })
            .collect();
        let graph_ref = format!(
            "extension:{}:{}",
            type_ref_name(&receiver, interner),
            interner.resolve(method.name)
        );

        // Build parameter list with `self` prepended.
        let mut all_params: IndexMap<InternedStr, IrType> = IndexMap::new();
        all_params.insert(
            interner.intern("self"),
            receiver.clone(),
        );
        for (k, v) in params.iter() {
            all_params.insert(*k, v.clone());
        }

        // Lower the method body into a graph.
        let graph = super::graph_lower::lower_function_graph(
            graph_ref.clone(),
            &all_params,
            &method.body,
            check,
            interner,
            defs,
        );
        graphs.insert(graph_ref.clone(), graph);

        defs.extensions.push(ExtensionDef {
            receiver: receiver.clone(),
            method_name: method.name,
            type_params: method.type_params.clone(),
            params,
            returns: method
                .return_ty
                .as_ref()
                .map(|ty| lower_type_expr_with_params(ty, &method.type_params, interner))
                .unwrap_or(IrType::Void),
            graph_ref,
        });
    }
}

fn type_ref_name(ty: &IrType, interner: &Arc<Interner>) -> String {
    match ty {
        IrType::String | IrType::Text => "string".into(),
        IrType::Number => "number".into(),
        IrType::Float => "float".into(),
        IrType::Bool => "bool".into(),
        IrType::Null => "null".into(),
        IrType::Void => "void".into(),
        IrType::Named(name) | IrType::GenericParam(name) | IrType::AgentOutput(name) => {
            interner.resolve(*name).to_string()
        }
        IrType::GenericInstance { name, .. } => interner.resolve(*name).to_string(),
        IrType::Object(_) => "object".into(),
        IrType::Array(_) => "array".into(),
        IrType::Union(_) => "union".into(),
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
        .map(|field| {
            (
                field.name,
                lower_field_with_params(field, &decl.type_params, interner),
            )
        })
        .collect();

    defs.types.insert(
        decl.name,
        TypeDef {
            type_params: decl.type_params.clone(),
            fields,
            visibility: ir_visibility(&decl.builtin),
        },
    );

    if let BuiltinTypeMeta::Builtin {
        role: Some(role), ..
    } = &decl.builtin
    {
        defs.roles.insert(
            IrRoleKey {
                keyword: role.keyword,
                place: role.place,
            },
            IrRoleBinding {
                type_name: decl.name,
            },
        );
    }
}

fn ir_visibility(meta: &BuiltinTypeMeta) -> IrTypeVisibility {
    match meta {
        BuiltinTypeMeta::User => IrTypeVisibility::User,
        BuiltinTypeMeta::Builtin {
            visibility: BuiltinVisibility::Public,
            ..
        } => IrTypeVisibility::PublicBuiltin,
        BuiltinTypeMeta::Builtin {
            visibility: BuiltinVisibility::Internal,
            ..
        } => IrTypeVisibility::InternalBuiltin,
    }
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
    check: &CheckResult,
    interner: &Arc<Interner>,
    defs: &mut Definitions,
    graphs: &mut IndexMap<String, AgentGraph>,
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
        let params: IndexMap<InternedStr, IrType> = decl
            .params
            .iter()
            .map(|param| {
                (
                    param.name,
                    lower_type_expr_with_params(&param.ty, &decl.type_params, interner),
                )
            })
            .collect();

        // Lower the function body into a graph.
        let graph = super::graph_lower::lower_function_graph(
            graph_ref.clone(),
            &params,
            &decl.body,
            check,
            interner,
            defs,
        );
        graphs.insert(graph_ref.clone(), graph);

        defs.functions.insert(
            decl.name,
            FunctionDef {
                type_params: decl.type_params.clone(),
                params,
                returns: decl
                    .return_ty
                    .as_ref()
                    .map(|ty| lower_type_expr_with_params(ty, &decl.type_params, interner))
                    .unwrap_or(IrType::Void),
                native: decl.native.as_ref().map(|native| native.id.value),
                graph_ref,
            },
        );
    }
}

pub(crate) fn lower_type_expr(expr: &TypeExpr, interner: &Arc<Interner>) -> IrType {
    lower_type_expr_with_params(expr, &[], interner)
}

pub(crate) fn lower_type_expr_with_params(
    expr: &TypeExpr,
    type_params: &[InternedStr],
    interner: &Arc<Interner>,
) -> IrType {
    match expr {
        TypeExpr::Named(name, _) => match interner.resolve(*name) {
            "string" => IrType::String,
            "number" => IrType::Number,
            "float" => IrType::Float,
            "bool" => IrType::Bool,
            "null" => IrType::Null,
            "void" => IrType::Void,
            "Text" => IrType::Text,
            _ if type_params.contains(name) => IrType::GenericParam(*name),
            _ => IrType::Named(*name),
        },
        TypeExpr::Union(members, _) => IrType::Union(
            members
                .iter()
                .map(|member| lower_type_expr_with_params(member, type_params, interner))
                .collect(),
        ),
        TypeExpr::Optional(inner, _) => IrType::Union(vec![
            lower_type_expr_with_params(inner, type_params, interner),
            IrType::Null,
        ]),
        TypeExpr::Generic(name, args, _) => IrType::GenericInstance {
            name: *name,
            args: args
                .iter()
                .map(|arg| lower_type_expr_with_params(arg, type_params, interner))
                .collect(),
        },
    }
}

fn lower_field_with_params(
    field: &FieldDef,
    type_params: &[InternedStr],
    interner: &Arc<Interner>,
) -> IrField {
    IrField {
        ty: lower_type_expr_with_params(&field.ty, type_params, interner),
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

    fn generic(interner: &Arc<Interner>, name: &str, args: Vec<TypeExpr>) -> TypeExpr {
        TypeExpr::Generic(interner.intern(name), args, sp())
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
            type_params: vec![],
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
            builtin: quew_ast::BuiltinTypeMeta::User,
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
    fn lower_generic_type_decl_preserves_params_and_generic_fields() {
        let interner = interner();
        let mut defs = Definitions::default();
        let t = interner.intern("T");
        let decl = TypeDecl {
            name: interner.intern("Box"),
            type_params: vec![t],
            fields: vec![FieldDef {
                name: interner.intern("value"),
                ty: named(&interner, "T"),
                optional: false,
                span: sp(),
            }],
            builtin: quew_ast::BuiltinTypeMeta::User,
            span: sp(),
        };

        lower_type(&decl, &interner, &mut defs);

        let def = &defs.types[&decl.name];
        assert_eq!(def.type_params, vec![t]);
        assert_eq!(
            def.fields[&interner.intern("value")].ty,
            IrType::GenericParam(t)
        );
    }

    #[test]
    fn lower_builtin_type_preserves_visibility() {
        let interner = interner();
        let mut defs = Definitions::default();
        let decl = TypeDecl {
            name: interner.intern("Text"),
            type_params: vec![],
            fields: vec![FieldDef {
                name: interner.intern("value"),
                ty: named(&interner, "string"),
                optional: false,
                span: sp(),
            }],
            builtin: BuiltinTypeMeta::public(),
            span: sp(),
        };

        lower_type(&decl, &interner, &mut defs);

        assert_eq!(
            defs.types[&decl.name].visibility,
            IrTypeVisibility::PublicBuiltin
        );
    }

    #[test]
    fn lower_role_bound_builtin_type_preserves_role_binding() {
        let interner = interner();
        let mut defs = Definitions::default();
        let t = interner.intern("T");
        let tool = interner.intern("tool");
        let value = interner.intern("value");
        let decl = TypeDecl {
            name: interner.intern("ToolResult"),
            type_params: vec![t],
            fields: vec![FieldDef {
                name: interner.intern("data"),
                ty: named(&interner, "T"),
                optional: false,
                span: sp(),
            }],
            builtin: BuiltinTypeMeta::Builtin {
                visibility: BuiltinVisibility::Public,
                role: Some(quew_ast::RoleBindingSyntax {
                    keyword: tool,
                    place: value,
                    span: sp(),
                }),
            },
            span: sp(),
        };

        lower_type(&decl, &interner, &mut defs);

        let key = IrRoleKey {
            keyword: tool,
            place: value,
        };
        assert_eq!(defs.roles[&key].type_name, decl.name);
        assert_eq!(defs.types[&decl.name].type_params, vec![t]);
        assert_eq!(
            defs.types[&decl.name].visibility,
            IrTypeVisibility::PublicBuiltin
        );
    }

    #[test]
    fn lower_generic_function_preserves_params_and_instantiated_return() {
        let interner = interner();
        let mut defs = Definitions::default();
        let mut graphs = IndexMap::new();
        let t = interner.intern("T");
        let decl = FunctionDecl {
            annotations: vec![],
            builtin: quew_ast::BuiltinFunctionMeta::User,
            native: None,
            name: interner.intern("wrap"),
            type_params: vec![t],
            params: vec![Param {
                binding: ParamBinding::Normal,
                name: interner.intern("value"),
                ty: named(&interner, "T"),
                optional: false,
                span: sp(),
            }],
            return_ty: Some(generic(&interner, "Box", vec![named(&interner, "T")])),
            body: vec![],
            span: sp(),
        };

        let check = CheckResult {
            symbol_table: quew_checker::SymbolTable::default(),
            diagnostics: vec![],
            resolved: quew_checker::resolved::ResolvedExpressionMap::default(),
        };
        lower_function(&decl, &check, &interner, &mut defs, &mut graphs);

        let def = &defs.functions[&decl.name];
        assert_eq!(def.type_params, vec![t]);
        assert_eq!(
            def.params[&interner.intern("value")],
            IrType::GenericParam(t)
        );
        assert_eq!(
            def.returns,
            IrType::GenericInstance {
                name: interner.intern("Box"),
                args: vec![IrType::GenericParam(t)]
            }
        );
        assert_eq!(def.native, None);
    }

    #[test]
    fn lower_native_builtin_function_preserves_native_id() {
        let interner = interner();
        let mut defs = Definitions::default();
        let mut graphs = IndexMap::new();
        let native_id = interner.intern("std.string.is_empty");
        let decl = FunctionDecl {
            annotations: vec![],
            builtin: quew_ast::BuiltinFunctionMeta::internal(),
            native: Some(quew_ast::NativeBinding {
                id: quew_ast::StringLit {
                    value: native_id,
                    kind: quew_ast::StringKind::Regular,
                    span: sp(),
                },
                span: sp(),
            }),
            name: interner.intern("string_is_empty"),
            type_params: vec![],
            params: vec![Param {
                binding: ParamBinding::Normal,
                name: interner.intern("value"),
                ty: named(&interner, "string"),
                optional: false,
                span: sp(),
            }],
            return_ty: Some(named(&interner, "bool")),
            body: vec![],
            span: sp(),
        };

        let check = CheckResult {
            symbol_table: quew_checker::SymbolTable::default(),
            diagnostics: vec![],
            resolved: quew_checker::resolved::ResolvedExpressionMap::default(),
        };
        lower_function(&decl, &check, &interner, &mut defs, &mut graphs);

        let def = &defs.functions[&decl.name];
        assert_eq!(def.native, Some(native_id));
    }

    #[test]
    fn lower_extension_method_preserves_receiver_and_signature() {
        let interner = interner();
        let mut defs = Definitions::default();
        let decl = quew_ast::ExtendDecl {
            receiver: named(&interner, "string"),
            methods: vec![FunctionDecl {
                annotations: vec![],
                builtin: quew_ast::BuiltinFunctionMeta::User,
                native: None,
                name: interner.intern("contains"),
                type_params: vec![],
                params: vec![Param {
                    binding: ParamBinding::Normal,
                    name: interner.intern("substring"),
                    ty: named(&interner, "string"),
                    optional: false,
                    span: sp(),
                }],
                return_ty: Some(named(&interner, "bool")),
                body: vec![],
                span: sp(),
            }],
            span: sp(),
        };

        let mut graphs = IndexMap::new();
        let check = CheckResult {
            symbol_table: quew_checker::SymbolTable::default(),
            diagnostics: vec![],
            resolved: quew_checker::resolved::ResolvedExpressionMap::default(),
        };
        lower_extend(&decl, &check, &interner, &mut defs, &mut graphs);

        assert_eq!(defs.extensions.len(), 1);
        let method = &defs.extensions[0];
        assert_eq!(method.receiver, IrType::String);
        assert_eq!(method.method_name, interner.intern("contains"));
        assert_eq!(method.params[&interner.intern("substring")], IrType::String);
        assert_eq!(method.returns, IrType::Bool);
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
