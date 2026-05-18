//! Per-file symbol table for the quew compiler.
pub mod roles;

use indexmap::IndexMap;
use quew_ast::{
    AnnotationArgs, BuiltinTypeMeta, BuiltinVisibility as AstBuiltinVisibility, FieldDef,
    FunctionDecl, Item, Module, Param, ParamBinding, Provider, ToolDecl, ToolEntry, TypeExpr,
};
use quew_errors::{Diagnostic, Severity, Span};
use quew_interner::{InternedStr, Interner};
use quew_lexer::AnnotationKind;
use quew_types::{AgentTy, FunctionTy, ProviderKind, ToolTy, Ty};
pub use roles::{BuiltinVisibility, RoleBinding, RoleKey, RoleRegistry};

// ── SymbolKind ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Let,
    Function,
    Agent,
    Tool,
    ToolGroup,
    Type,
    Model,
    Param,
    Local,
}

// ── Symbol ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Symbol {
    pub ty: Ty,
    pub kind: SymbolKind,
    pub def_span: Span,
    pub type_params: Vec<InternedStr>,
    pub visibility: BuiltinVisibility,
}

// ── SymbolTable ───────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct SymbolTable {
    /// Top-level names declared in the file (ordered).
    pub globals: IndexMap<InternedStr, Symbol>,
    /// Compiler role bindings declared by builtin types.
    pub roles: RoleRegistry,
    pub diagnostics: Vec<Diagnostic>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a name in globals; emit a duplicate error if already present.
    fn define_global(&mut self, name: InternedStr, sym: Symbol) {
        if let Some(prev) = self.globals.get(&name) {
            self.diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message: format!("duplicate definition of `{name:?}`"),
                primary_span: sym.def_span,
                primary_label: Some("defined again here".into()),
                secondary: vec![],
                help: Some(format!("first defined at {:?}", prev.def_span)),
                code: None,
            });
        } else {
            self.globals.insert(name, sym);
        }
    }
}

fn symbol(
    ty: Ty,
    kind: SymbolKind,
    def_span: Span,
    type_params: Vec<InternedStr>,
    visibility: BuiltinVisibility,
) -> Symbol {
    Symbol {
        ty,
        kind,
        def_span,
        type_params,
        visibility,
    }
}

fn visibility_from_builtin(meta: &BuiltinTypeMeta) -> BuiltinVisibility {
    match meta {
        BuiltinTypeMeta::User => BuiltinVisibility::User,
        BuiltinTypeMeta::Builtin {
            visibility: AstBuiltinVisibility::Public,
            ..
        } => BuiltinVisibility::PublicBuiltin,
        BuiltinTypeMeta::Builtin {
            visibility: AstBuiltinVisibility::Internal,
            ..
        } => BuiltinVisibility::InternalBuiltin,
    }
}

// ── TypeExpr → Ty lowering ────────────────────────────────────────────────────

/// Convert a syntactic `TypeExpr` to a semantic `Ty`.
/// Unknown named types become `Ty::Error` with a diagnostic queued.
pub fn lower_type(expr: &TypeExpr, diags: &mut Vec<Diagnostic>) -> Ty {
    lower_type_with_params(expr, &[], diags)
}

fn lower_type_with_params(
    expr: &TypeExpr,
    type_params: &[InternedStr],
    diags: &mut Vec<Diagnostic>,
) -> Ty {
    match expr {
        TypeExpr::Named(name, span) => {
            if type_params.contains(name) {
                return Ty::GenericParam(*name);
            }
            // Check well-known primitive names
            // InternedStr is opaque so we cannot compare to &str directly;
            // the checker will resolve user-defined names. Here we return
            // Ty::Error only for completely unknown kinds — the checker
            // resolves named types against the symbol table.
            let _ = (name, span, diags);
            // Defer full resolution to the checker; return a placeholder.
            // This function only lowers structural shape, not names.
            Ty::Named(*name)
        }
        TypeExpr::Optional(inner, _) => {
            lower_type_with_params(inner, type_params, diags).optional()
        }
        TypeExpr::Union(arms, _) => {
            let lowered: Vec<Ty> = arms
                .iter()
                .map(|a| lower_type_with_params(a, type_params, diags))
                .collect();
            Ty::Union(lowered).flatten_union()
        }
        TypeExpr::Generic(name, args, _) => {
            // Generics are structural; lower args but leave name unresolved
            let args = args
                .iter()
                .map(|arg| lower_type_with_params(arg, type_params, diags))
                .collect();
            Ty::GenericInstance { name: *name, args }
        }
    }
}

/// Lower a `TypeExpr` using a primitive name map for well-known types.
/// Returns `None` for names that must be resolved from the symbol table.
pub fn lower_primitive(name: &str) -> Option<Ty> {
    match name {
        "string" => Some(Ty::string()),
        "number" => Some(Ty::number()),
        "float" => Some(Ty::float()),
        "bool" => Some(Ty::bool_ty()),
        "void" => Some(Ty::void()),
        "null" => Some(Ty::null()),
        _ => None,
    }
}

// ── Param lowering ────────────────────────────────────────────────────────────

fn lower_param_ty(param: &Param, type_params: &[InternedStr], diags: &mut Vec<Diagnostic>) -> Ty {
    lower_type_with_params(&param.ty, type_params, diags)
}

// ── Provider lowering ─────────────────────────────────────────────────────────

fn lower_provider(p: &Provider) -> ProviderKind {
    match p {
        Provider::Gemini => ProviderKind::Gemini,
        Provider::OpenAi => ProviderKind::OpenAi,
        Provider::Groq => ProviderKind::Groq,
    }
}

// ── Record type from TypeDecl fields ─────────────────────────────────────────

fn lower_record_with_params(
    fields: &[FieldDef],
    type_params: &[InternedStr],
    diags: &mut Vec<Diagnostic>,
) -> Ty {
    let mut map = IndexMap::new();
    for f in fields {
        let mut ty = lower_type_with_params(&f.ty, type_params, diags);
        if f.optional {
            ty = ty.optional();
        }
        map.insert(f.name, ty);
    }
    Ty::Record(map)
}

fn validate_type_params(type_params: &[InternedStr], span: Span, diags: &mut Vec<Diagnostic>) {
    let mut seen = IndexMap::<InternedStr, ()>::new();
    for param in type_params {
        if seen.contains_key(param) {
            diags.push(Diagnostic {
                severity: Severity::Error,
                message: format!("duplicate generic parameter `{param:?}`"),
                primary_span: span,
                primary_label: Some("generic parameter declared more than once".into()),
                secondary: vec![],
                help: None,
                code: None,
            });
        } else {
            seen.insert(*param, ());
        }
    }
}

// ── Tool param splitting ──────────────────────────────────────────────────────

/// Split params into (bound_params, model_params).
fn split_params(
    params: &[Param],
    diags: &mut Vec<Diagnostic>,
) -> (Vec<(InternedStr, Ty)>, Vec<(InternedStr, Ty)>) {
    split_params_with_type_params(params, &[], diags)
}

fn split_params_with_type_params(
    params: &[Param],
    type_params: &[InternedStr],
    diags: &mut Vec<Diagnostic>,
) -> (Vec<(InternedStr, Ty)>, Vec<(InternedStr, Ty)>) {
    let mut bound = vec![];
    let mut model = vec![];
    for p in params {
        let ty = lower_param_ty(p, type_params, diags);
        match p.binding {
            ParamBinding::BoundRef => bound.push((p.name, ty)),
            ParamBinding::Normal => model.push((p.name, ty)),
        }
    }
    (bound, model)
}

/// Extract bound_params from `@tool(name: Type, ...)` annotation.
fn extract_tool_annotation_params(
    func: &FunctionDecl,
    diags: &mut Vec<Diagnostic>,
) -> Vec<(InternedStr, Ty)> {
    for ann in &func.annotations {
        if ann.kind == AnnotationKind::Tool {
            if let AnnotationArgs::Params(params) = &ann.args {
                return params
                    .iter()
                    .map(|p| (p.name, lower_param_ty(p, &[], diags)))
                    .collect();
            }
        }
    }
    vec![]
}

// ── build_symbol_table ────────────────────────────────────────────────────────

/// Walk a parsed `Module` and produce a `SymbolTable`.
///
/// Errors are accumulated into `SymbolTable::diagnostics`.
/// This function never panics on malformed input.
pub fn build_symbol_table(module: &Module, interner: &Interner) -> SymbolTable {
    let mut table = SymbolTable::new();

    for item in &module.items {
        // Each arm collects its own diagnostics into `d` to avoid aliased borrows.
        let mut d: Vec<Diagnostic> = vec![];

        match item {
            // ── agent ──────────────────────────────────────────────────────────
            Item::Agent(decl) => {
                let input_ty = lower_param_ty(&decl.param, &[], &mut d);
                let return_ty = decl
                    .return_ty
                    .as_ref()
                    .map(|t| lower_type(t, &mut d))
                    .unwrap_or_else(Ty::void);
                let ty = Ty::Agent(AgentTy {
                    input_name: decl.param.name,
                    input_ty: Box::new(input_ty),
                    return_ty: Box::new(return_ty),
                });
                table.diagnostics.extend(d);
                table.define_global(
                    decl.name,
                    symbol(
                        ty,
                        SymbolKind::Agent,
                        decl.span,
                        vec![],
                        BuiltinVisibility::User,
                    ),
                );
            }

            // ── function ───────────────────────────────────────────────────────
            Item::Function(decl) => {
                validate_type_params(&decl.type_params, decl.span, &mut d);
                let bound = extract_tool_annotation_params(decl, &mut d);
                let (_, model) =
                    split_params_with_type_params(&decl.params, &decl.type_params, &mut d);
                let return_ty = decl
                    .return_ty
                    .as_ref()
                    .map(|t| lower_type_with_params(t, &decl.type_params, &mut d))
                    .unwrap_or_else(Ty::void);
                let ty = if bound.is_empty() {
                    Ty::Function(FunctionTy {
                        type_params: decl.type_params.clone(),
                        params: model,
                        return_ty: Box::new(return_ty),
                    })
                } else {
                    validate_bound_params(decl, &bound, &mut d);
                    Ty::Tool(ToolTy {
                        bound_params: bound,
                        model_params: model,
                        return_ty: Box::new(return_ty),
                    })
                };
                let kind = if matches!(ty, Ty::Tool(_)) {
                    SymbolKind::Tool
                } else {
                    SymbolKind::Function
                };
                table.diagnostics.extend(d);
                table.define_global(
                    decl.name,
                    symbol(
                        ty,
                        kind,
                        decl.span,
                        decl.type_params.clone(),
                        BuiltinVisibility::User,
                    ),
                );
            }

            // ── tool ───────────────────────────────────────────────────────────
            Item::Tool(decl) => {
                table.diagnostics.extend(d);
                register_tool_decl(&mut table, decl);
            }

            // ── tools group ────────────────────────────────────────────────────
            Item::Tools(decl) => {
                for entry in &decl.entries {
                    register_tool_entry(&mut table, entry, &mut d);
                }
                if let Some(name) = decl.name {
                    table.diagnostics.extend(d);
                    table.define_global(
                        name,
                        symbol(
                            Ty::void(),
                            SymbolKind::ToolGroup,
                            decl.span,
                            vec![],
                            BuiltinVisibility::User,
                        ),
                    );
                } else {
                    table.diagnostics.extend(d);
                }
            }

            // ── type ───────────────────────────────────────────────────────────
            Item::Type(decl) => {
                validate_type_params(&decl.type_params, decl.span, &mut d);
                let ty = lower_record_with_params(&decl.fields, &decl.type_params, &mut d);
                table.diagnostics.extend(d);
                if let BuiltinTypeMeta::Builtin {
                    role: Some(role), ..
                } = &decl.builtin
                {
                    table.roles.register(
                        RoleKey {
                            keyword: role.keyword,
                            place: role.place,
                        },
                        RoleBinding {
                            type_name: decl.name,
                            span: role.span,
                        },
                        interner,
                        &mut table.diagnostics,
                    );
                }
                table.define_global(
                    decl.name,
                    symbol(
                        ty,
                        SymbolKind::Type,
                        decl.span,
                        decl.type_params.clone(),
                        visibility_from_builtin(&decl.builtin),
                    ),
                );
            }

            // ── model ──────────────────────────────────────────────────────────
            Item::Model(decl) => {
                table.diagnostics.extend(d);
                let ty = Ty::Provider(lower_provider(&decl.provider.provider));
                table.define_global(
                    decl.name,
                    symbol(
                        ty,
                        SymbolKind::Model,
                        decl.span,
                        vec![],
                        BuiltinVisibility::User,
                    ),
                );
            }

            // ── let ────────────────────────────────────────────────────────────
            Item::Let(decl) => {
                let ty = decl
                    .ty
                    .as_ref()
                    .map(|t| lower_type(t, &mut d))
                    .unwrap_or(Ty::Error);
                table.diagnostics.extend(d);
                table.define_global(
                    decl.name,
                    symbol(
                        ty,
                        SymbolKind::Let,
                        decl.span,
                        vec![],
                        BuiltinVisibility::User,
                    ),
                );
            }
        }
    }

    table
}

// ── Helper: register a single ToolDecl ───────────────────────────────────────

fn register_tool_decl(table: &mut SymbolTable, decl: &ToolDecl) {
    let mut d = vec![];
    let (_, model) = split_params(&decl.params, &mut d);
    let return_ty = lower_type(&decl.return_ty, &mut d);
    table.diagnostics.extend(d);

    let ty = Ty::Tool(ToolTy {
        bound_params: vec![],
        model_params: model,
        return_ty: Box::new(return_ty),
    });
    table.define_global(
        decl.name,
        symbol(
            ty,
            SymbolKind::Tool,
            decl.span,
            vec![],
            BuiltinVisibility::User,
        ),
    );
}

fn register_tool_entry(table: &mut SymbolTable, entry: &ToolEntry, d: &mut Vec<Diagnostic>) {
    let (_, model) = split_params(&entry.params, d);
    let return_ty = lower_type(&entry.return_ty, d);
    let ty = Ty::Tool(ToolTy {
        bound_params: vec![],
        model_params: model,
        return_ty: Box::new(return_ty),
    });
    table.define_global(
        entry.name,
        symbol(
            ty,
            SymbolKind::Tool,
            entry.span,
            vec![],
            BuiltinVisibility::User,
        ),
    );
}

// ── Bound param validation ────────────────────────────────────────────────────

/// Verify that every `@name: T` (BoundRef) param in `decl.params` exists in
/// `bound_params` (from `@tool(...)`) with a compatible type.
fn validate_bound_params(
    decl: &FunctionDecl,
    bound_params: &[(InternedStr, Ty)],
    diags: &mut Vec<Diagnostic>,
) {
    for param in &decl.params {
        if param.binding != ParamBinding::BoundRef {
            continue;
        }

        let found = bound_params.iter().find(|(n, _)| *n == param.name);
        match found {
            None => {
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!(
                        "`@{:?}` is not declared in the `@tool(...)` annotation",
                        param.name
                    ),
                    primary_span: param.span,
                    primary_label: Some("unknown bound param".into()),
                    secondary: vec![],
                    help: Some("add it to `@tool(name: Type, ...)`".into()),
                    code: None,
                });
            }
            Some((_, ann_ty)) => {
                let param_ty = lower_type(&param.ty, diags);
                if !param_ty.is_assignable_to(ann_ty) && !ann_ty.is_assignable_to(&param_ty) {
                    diags.push(Diagnostic {
                        severity: Severity::Error,
                        message: format!(
                            "type mismatch for bound param `@{:?}`: \
                             annotation has `{}` but param declares `{}`",
                            param.name, ann_ty, param_ty
                        ),
                        primary_span: param.span,
                        primary_label: Some("type mismatch".into()),
                        secondary: vec![],
                        help: None,
                        code: None,
                    });
                }
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use quew_ast::*;
    use quew_errors::Span;
    use quew_interner::Interner;
    use std::sync::Arc;

    fn interner() -> Arc<Interner> {
        Arc::new(Interner::new())
    }
    fn sp() -> Span {
        Span::new(0, 10)
    }

    fn intern(i: &Arc<Interner>, s: &str) -> InternedStr {
        i.intern(s)
    }

    fn ty_str(i: &Arc<Interner>) -> TypeExpr {
        TypeExpr::Named(intern(i, "string"), sp())
    }

    fn normal_param(i: &Arc<Interner>, name: &str) -> Param {
        Param {
            binding: ParamBinding::Normal,
            name: intern(i, name),
            ty: ty_str(i),
            optional: false,
            span: sp(),
        }
    }

    fn bound_param(i: &Arc<Interner>, name: &str) -> Param {
        Param {
            binding: ParamBinding::BoundRef,
            name: intern(i, name),
            ty: ty_str(i),
            optional: false,
            span: sp(),
        }
    }

    fn empty_module() -> Module {
        Module {
            items: vec![],
            span: sp(),
        }
    }

    // ── basic registration ────────────────────────────────────────────────────

    #[test]
    fn empty_module_yields_empty_table() {
        let i = interner();
        let t = build_symbol_table(&empty_module(), &i);
        assert!(t.globals.is_empty());
        assert!(t.diagnostics.is_empty());
    }

    #[test]
    fn type_decl_registers_record() {
        let i = interner();
        let module = Module {
            items: vec![Item::Type(TypeDecl {
                name: intern(&i, "Response"),
                type_params: vec![],
                fields: vec![FieldDef {
                    name: intern(&i, "msg"),
                    ty: ty_str(&i),
                    optional: false,
                    span: sp(),
                }],
                builtin: BuiltinTypeMeta::User,
                span: sp(),
            })],
            span: sp(),
        };
        let t = build_symbol_table(&module, &i);
        let sym = t
            .globals
            .get(&intern(&i, "Response"))
            .expect("Response not found");
        assert_eq!(sym.kind, SymbolKind::Type);
        assert!(matches!(sym.ty, Ty::Record(_)));
    }

    #[test]
    fn model_decl_registers_provider() {
        let i = interner();
        let module = Module {
            items: vec![Item::Model(ModelDecl {
                name: intern(&i, "MyModel"),
                provider: ProviderCall {
                    provider: Provider::Gemini,
                    model_name: StringLit {
                        value: intern(&i, "gemini-pro"),
                        kind: StringKind::Regular,
                        span: sp(),
                    },
                    config: vec![],
                    span: sp(),
                },
                config: vec![],
                span: sp(),
            })],
            span: sp(),
        };
        let t = build_symbol_table(&module, &i);
        let sym = t
            .globals
            .get(&intern(&i, "MyModel"))
            .expect("MyModel not found");
        assert_eq!(sym.kind, SymbolKind::Model);
        assert_eq!(sym.ty, Ty::Provider(ProviderKind::Gemini));
    }

    #[test]
    fn agent_decl_registers_agent_ty() {
        let i = interner();
        let module = Module {
            items: vec![Item::Agent(AgentDecl {
                annotations: vec![],
                name: intern(&i, "ChatAgent"),
                param: normal_param(&i, "input"),
                return_ty: None,
                body: vec![],
                span: sp(),
            })],
            span: sp(),
        };
        let t = build_symbol_table(&module, &i);
        let sym = t
            .globals
            .get(&intern(&i, "ChatAgent"))
            .expect("ChatAgent not found");
        assert_eq!(sym.kind, SymbolKind::Agent);
        assert!(matches!(sym.ty, Ty::Agent(_)));
    }

    #[test]
    fn function_decl_registers_function_ty() {
        let i = interner();
        let module = Module {
            items: vec![Item::Function(FunctionDecl {
                annotations: vec![],
                name: intern(&i, "greet"),
                type_params: vec![],
                params: vec![normal_param(&i, "name")],
                return_ty: Some(ty_str(&i)),
                body: vec![],
                span: sp(),
            })],
            span: sp(),
        };
        let t = build_symbol_table(&module, &i);
        let sym = t
            .globals
            .get(&intern(&i, "greet"))
            .expect("greet not found");
        assert_eq!(sym.kind, SymbolKind::Function);
        assert!(matches!(sym.ty, Ty::Function(_)));
    }

    #[test]
    fn generic_function_registers_type_params_and_generic_param_types() {
        let i = interner();
        let t_param = intern(&i, "T");
        let module = Module {
            items: vec![Item::Function(FunctionDecl {
                annotations: vec![],
                name: intern(&i, "identity"),
                type_params: vec![t_param],
                params: vec![Param {
                    binding: ParamBinding::Normal,
                    name: intern(&i, "value"),
                    ty: TypeExpr::Named(t_param, sp()),
                    optional: false,
                    span: sp(),
                }],
                return_ty: Some(TypeExpr::Named(t_param, sp())),
                body: vec![],
                span: sp(),
            })],
            span: sp(),
        };

        let table = build_symbol_table(&module, &i);
        assert!(table.diagnostics.is_empty(), "{:?}", table.diagnostics);
        let sym = &table.globals[&intern(&i, "identity")];
        assert_eq!(sym.type_params, vec![t_param]);
        match &sym.ty {
            Ty::Function(function) => {
                assert_eq!(function.type_params, vec![t_param]);
                assert_eq!(function.params[0].1, Ty::GenericParam(t_param));
                assert_eq!(*function.return_ty, Ty::GenericParam(t_param));
            }
            other => panic!("expected function, got {other:?}"),
        }
    }

    #[test]
    fn duplicate_generic_type_params_error() {
        let i = interner();
        let t_param = intern(&i, "T");
        let module = Module {
            items: vec![Item::Type(TypeDecl {
                name: intern(&i, "Bad"),
                type_params: vec![t_param, t_param],
                fields: vec![],
                builtin: BuiltinTypeMeta::User,
                span: sp(),
            })],
            span: sp(),
        };

        let table = build_symbol_table(&module, &i);
        assert!(
            table
                .diagnostics
                .iter()
                .any(|d| d.message.contains("duplicate generic parameter")),
            "expected duplicate generic parameter diagnostic, got {:?}",
            table.diagnostics
        );
    }

    #[test]
    fn public_builtin_type_preserves_visibility() {
        let i = interner();
        let module = Module {
            items: vec![Item::Type(TypeDecl {
                name: intern(&i, "Text"),
                type_params: vec![],
                fields: vec![],
                builtin: BuiltinTypeMeta::public(),
                span: sp(),
            })],
            span: sp(),
        };

        let table = build_symbol_table(&module, &i);
        assert!(table.diagnostics.is_empty(), "{:?}", table.diagnostics);
        assert_eq!(
            table.globals[&intern(&i, "Text")].visibility,
            super::BuiltinVisibility::PublicBuiltin
        );
    }

    #[test]
    fn internal_builtin_type_preserves_visibility() {
        let i = interner();
        let module = Module {
            items: vec![Item::Type(TypeDecl {
                name: intern(&i, "InternalText"),
                type_params: vec![],
                fields: vec![],
                builtin: BuiltinTypeMeta::internal(),
                span: sp(),
            })],
            span: sp(),
        };

        let table = build_symbol_table(&module, &i);
        assert!(table.diagnostics.is_empty(), "{:?}", table.diagnostics);
        assert_eq!(
            table.globals[&intern(&i, "InternalText")].visibility,
            super::BuiltinVisibility::InternalBuiltin
        );
    }

    #[test]
    fn role_bound_generic_type_registers_role_and_params() {
        let i = interner();
        let t_param = intern(&i, "T");
        let module = Module {
            items: vec![Item::Type(TypeDecl {
                name: intern(&i, "ToolResult"),
                type_params: vec![t_param],
                fields: vec![],
                builtin: BuiltinTypeMeta::Builtin {
                    visibility: AstBuiltinVisibility::Public,
                    role: Some(RoleBindingSyntax {
                        keyword: intern(&i, "tool"),
                        place: intern(&i, "value"),
                        span: sp(),
                    }),
                },
                span: sp(),
            })],
            span: sp(),
        };

        let table = build_symbol_table(&module, &i);
        assert!(table.diagnostics.is_empty(), "{:?}", table.diagnostics);
        let key = RoleKey {
            keyword: intern(&i, "tool"),
            place: intern(&i, "value"),
        };
        assert_eq!(
            table.roles.bindings[&key].type_name,
            intern(&i, "ToolResult")
        );
        assert_eq!(
            table.globals[&intern(&i, "ToolResult")].type_params,
            vec![t_param]
        );
    }

    #[test]
    fn duplicate_role_binding_errors() {
        let i = interner();
        let role = || BuiltinTypeMeta::Builtin {
            visibility: AstBuiltinVisibility::Public,
            role: Some(RoleBindingSyntax {
                keyword: intern(&i, "tool"),
                place: intern(&i, "value"),
                span: sp(),
            }),
        };
        let decl = |name: &str| {
            Item::Type(TypeDecl {
                name: intern(&i, name),
                type_params: vec![],
                fields: vec![],
                builtin: role(),
                span: sp(),
            })
        };
        let module = Module {
            items: vec![decl("ToolResult"), decl("OtherToolResult")],
            span: sp(),
        };

        let table = build_symbol_table(&module, &i);
        assert!(
            table
                .diagnostics
                .iter()
                .any(|d| d.message.contains("duplicate role binding")),
            "expected duplicate role binding diagnostic, got {:?}",
            table.diagnostics
        );
    }

    #[test]
    fn unknown_role_keyword_and_place_error() {
        let i = interner();
        let module = Module {
            items: vec![Item::Type(TypeDecl {
                name: intern(&i, "BadRole"),
                type_params: vec![],
                fields: vec![],
                builtin: BuiltinTypeMeta::Builtin {
                    visibility: AstBuiltinVisibility::Public,
                    role: Some(RoleBindingSyntax {
                        keyword: intern(&i, "unknown"),
                        place: intern(&i, "elsewhere"),
                        span: sp(),
                    }),
                },
                span: sp(),
            })],
            span: sp(),
        };

        let table = build_symbol_table(&module, &i);
        assert!(
            table
                .diagnostics
                .iter()
                .any(|d| d.message.contains("unknown role keyword")),
            "expected unknown role keyword diagnostic, got {:?}",
            table.diagnostics
        );
        assert!(
            table
                .diagnostics
                .iter()
                .any(|d| d.message.contains("unknown role place")),
            "expected unknown role place diagnostic, got {:?}",
            table.diagnostics
        );
    }

    #[test]
    fn tool_decl_registers_tool_ty() {
        let i = interner();
        let module = Module {
            items: vec![Item::Tool(ToolDecl {
                name: intern(&i, "delete_user"),
                params: vec![normal_param(&i, "id")],
                return_ty: ty_str(&i),
                desc: None,
                span: sp(),
            })],
            span: sp(),
        };
        let t = build_symbol_table(&module, &i);
        let sym = t
            .globals
            .get(&intern(&i, "delete_user"))
            .expect("tool not found");
        assert_eq!(sym.kind, SymbolKind::Tool);
        if let Ty::Tool(tt) = &sym.ty {
            assert!(tt.bound_params.is_empty());
            assert_eq!(tt.model_params.len(), 1);
        } else {
            panic!("expected Ty::Tool");
        }
    }

    #[test]
    fn duplicate_name_produces_diagnostic() {
        let i = interner();
        let decl = || {
            Item::Type(TypeDecl {
                name: intern(&i, "Foo"),
                type_params: vec![],
                fields: vec![],
                builtin: BuiltinTypeMeta::User,
                span: sp(),
            })
        };
        let module = Module {
            items: vec![decl(), decl()],
            span: sp(),
        };
        let t = build_symbol_table(&module, &i);
        assert_eq!(t.diagnostics.len(), 1);
        assert_eq!(t.diagnostics[0].severity, Severity::Error);
        assert!(t.diagnostics[0].message.contains("duplicate"));
    }

    #[test]
    fn function_with_tool_annotation_registers_tool_ty() {
        let i = interner();
        let id_name = intern(&i, "id");
        let tool_ann = Annotation {
            kind: AnnotationKind::Tool,
            args: AnnotationArgs::Params(vec![Param {
                binding: ParamBinding::Normal,
                name: id_name,
                ty: ty_str(&i),
                optional: false,
                span: sp(),
            }]),
            span: sp(),
        };
        let module = Module {
            items: vec![Item::Function(FunctionDecl {
                annotations: vec![tool_ann],
                name: intern(&i, "deleteUser"),
                type_params: vec![],
                params: vec![normal_param(&i, "isAdmin"), bound_param(&i, "id")],
                return_ty: Some(ty_str(&i)),
                body: vec![],
                span: sp(),
            })],
            span: sp(),
        };
        let t = build_symbol_table(&module, &i);
        assert!(
            t.diagnostics.is_empty(),
            "unexpected diags: {:?}",
            t.diagnostics
        );
        let sym = t
            .globals
            .get(&intern(&i, "deleteUser"))
            .expect("deleteUser not found");
        if let Ty::Tool(tt) = &sym.ty {
            assert_eq!(tt.bound_params.len(), 1, "should have 1 bound param");
            assert_eq!(tt.model_params.len(), 1, "should have 1 model param");
        } else {
            panic!("expected Ty::Tool, got {:?}", sym.ty);
        }
    }

    #[test]
    fn bound_param_missing_from_annotation_produces_error() {
        let i = interner();
        // @tool(id: string) but param list has @missing: string
        let tool_ann = Annotation {
            kind: AnnotationKind::Tool,
            args: AnnotationArgs::Params(vec![Param {
                binding: ParamBinding::Normal,
                name: intern(&i, "id"),
                ty: ty_str(&i),
                optional: false,
                span: sp(),
            }]),
            span: sp(),
        };
        let module = Module {
            items: vec![Item::Function(FunctionDecl {
                annotations: vec![tool_ann],
                name: intern(&i, "deleteUser"),
                type_params: vec![],
                params: vec![bound_param(&i, "missing")],
                return_ty: None,
                body: vec![],
                span: sp(),
            })],
            span: sp(),
        };
        let t = build_symbol_table(&module, &i);
        assert!(
            !t.diagnostics.is_empty(),
            "expected error for unmatched bound param"
        );
        assert_eq!(t.diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn tools_group_registers_entries() {
        let i = interner();
        let entry = ToolEntry {
            name: intern(&i, "getWeather"),
            params: vec![normal_param(&i, "city")],
            return_ty: ty_str(&i),
            desc: None,
            span: sp(),
        };
        let module = Module {
            items: vec![Item::Tools(ToolsDecl {
                name: None,
                entries: vec![entry],
                desc: None,
                span: sp(),
            })],
            span: sp(),
        };
        let t = build_symbol_table(&module, &i);
        assert!(t.globals.contains_key(&intern(&i, "getWeather")));
    }

    #[test]
    fn lower_primitive_known_types() {
        assert_eq!(lower_primitive("string"), Some(Ty::string()));
        assert_eq!(lower_primitive("number"), Some(Ty::number()));
        assert_eq!(lower_primitive("bool"), Some(Ty::bool_ty()));
        assert_eq!(lower_primitive("void"), Some(Ty::void()));
        assert_eq!(lower_primitive("null"), Some(Ty::null()));
        assert_eq!(lower_primitive("MyType"), None);
    }
}
