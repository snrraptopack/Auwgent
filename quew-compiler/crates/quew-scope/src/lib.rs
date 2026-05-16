//! Per-file symbol table for the quew compiler.
use indexmap::IndexMap;
use quew_ast::{
    AnnotationArgs, FieldDef, FunctionDecl, Item, Module, Param, ParamBinding, Provider, ToolDecl,
    ToolEntry, TypeExpr,
};
use quew_errors::{Diagnostic, Severity, Span};
use quew_interner::InternedStr;
use quew_lexer::AnnotationKind;
use quew_types::{AgentTy, FunctionTy, ProviderKind, ToolTy, Ty};

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
}

// ── SymbolTable ───────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct SymbolTable {
    /// Top-level names declared in the file (ordered).
    pub globals: IndexMap<InternedStr, Symbol>,
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

// ── TypeExpr → Ty lowering ────────────────────────────────────────────────────

/// Convert a syntactic `TypeExpr` to a semantic `Ty`.
/// Unknown named types become `Ty::Error` with a diagnostic queued.
pub fn lower_type(expr: &TypeExpr, diags: &mut Vec<Diagnostic>) -> Ty {
    match expr {
        TypeExpr::Named(name, span) => {
            // Check well-known primitive names
            // InternedStr is opaque so we cannot compare to &str directly;
            // the checker will resolve user-defined names. Here we return
            // Ty::Error only for completely unknown kinds — the checker
            // resolves named types against the symbol table.
            let _ = (name, span, diags);
            // Defer full resolution to the checker; return a placeholder.
            // This function only lowers structural shape, not names.
            Ty::Error // will be replaced by the checker's name-resolution pass
        }
        TypeExpr::Optional(inner, _) => lower_type(inner, diags).optional(),
        TypeExpr::Union(arms, _) => {
            let lowered: Vec<Ty> = arms.iter().map(|a| lower_type(a, diags)).collect();
            Ty::Union(lowered).flatten_union()
        }
        TypeExpr::Generic(_, args, _) => {
            // Generics are structural; lower args but leave name unresolved
            let _ = args;
            Ty::Error
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

fn lower_param_ty(_param: &Param, diags: &mut Vec<Diagnostic>) -> Ty {
    lower_type(&_param.ty, diags)
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

fn lower_record(fields: &[FieldDef], diags: &mut Vec<Diagnostic>) -> Ty {
    let mut map = IndexMap::new();
    for f in fields {
        let mut ty = lower_type(&f.ty, diags);
        if f.optional {
            ty = ty.optional();
        }
        map.insert(f.name, ty);
    }
    Ty::Record(map)
}

// ── Tool param splitting ──────────────────────────────────────────────────────

/// Split params into (bound_params, model_params).
fn split_params(
    params: &[Param],
    diags: &mut Vec<Diagnostic>,
) -> (Vec<(InternedStr, Ty)>, Vec<(InternedStr, Ty)>) {
    let mut bound = vec![];
    let mut model = vec![];
    for p in params {
        let ty = lower_param_ty(p, diags);
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
                    .map(|p| (p.name, lower_param_ty(p, diags)))
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
pub fn build_symbol_table(module: &Module) -> SymbolTable {
    let mut table = SymbolTable::new();

    for item in &module.items {
        // Each arm collects its own diagnostics into `d` to avoid aliased borrows.
        let mut d: Vec<Diagnostic> = vec![];

        match item {
            // ── agent ──────────────────────────────────────────────────────────
            Item::Agent(decl) => {
                let input_ty = lower_param_ty(&decl.param, &mut d);
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
                    Symbol {
                        ty,
                        kind: SymbolKind::Agent,
                        def_span: decl.span,
                    },
                );
            }

            // ── function ───────────────────────────────────────────────────────
            Item::Function(decl) => {
                let bound = extract_tool_annotation_params(decl, &mut d);
                let (_, model) = split_params(&decl.params, &mut d);
                let return_ty = decl
                    .return_ty
                    .as_ref()
                    .map(|t| lower_type(t, &mut d))
                    .unwrap_or_else(Ty::void);
                let ty = if bound.is_empty() {
                    Ty::Function(FunctionTy {
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
                    Symbol {
                        ty,
                        kind,
                        def_span: decl.span,
                    },
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
                        Symbol {
                            ty: Ty::void(),
                            kind: SymbolKind::ToolGroup,
                            def_span: decl.span,
                        },
                    );
                } else {
                    table.diagnostics.extend(d);
                }
            }

            // ── type ───────────────────────────────────────────────────────────
            Item::Type(decl) => {
                let ty = lower_record(&decl.fields, &mut d);
                table.diagnostics.extend(d);
                table.define_global(
                    decl.name,
                    Symbol {
                        ty,
                        kind: SymbolKind::Type,
                        def_span: decl.span,
                    },
                );
            }

            // ── model ──────────────────────────────────────────────────────────
            Item::Model(decl) => {
                table.diagnostics.extend(d);
                let ty = Ty::Provider(lower_provider(&decl.provider.provider));
                table.define_global(
                    decl.name,
                    Symbol {
                        ty,
                        kind: SymbolKind::Model,
                        def_span: decl.span,
                    },
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
                    Symbol {
                        ty,
                        kind: SymbolKind::Let,
                        def_span: decl.span,
                    },
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
        Symbol {
            ty,
            kind: SymbolKind::Tool,
            def_span: decl.span,
        },
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
        Symbol {
            ty,
            kind: SymbolKind::Tool,
            def_span: entry.span,
        },
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
        let t = build_symbol_table(&empty_module());
        assert!(t.globals.is_empty());
        assert!(t.diagnostics.is_empty());
    }

    #[test]
    fn type_decl_registers_record() {
        let i = interner();
        let module = Module {
            items: vec![Item::Type(TypeDecl {
                name: intern(&i, "Response"),
                fields: vec![FieldDef {
                    name: intern(&i, "msg"),
                    ty: ty_str(&i),
                    optional: false,
                    span: sp(),
                }],
                span: sp(),
            })],
            span: sp(),
        };
        let t = build_symbol_table(&module);
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
        let t = build_symbol_table(&module);
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
        let t = build_symbol_table(&module);
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
                params: vec![normal_param(&i, "name")],
                return_ty: Some(ty_str(&i)),
                body: vec![],
                span: sp(),
            })],
            span: sp(),
        };
        let t = build_symbol_table(&module);
        let sym = t
            .globals
            .get(&intern(&i, "greet"))
            .expect("greet not found");
        assert_eq!(sym.kind, SymbolKind::Function);
        assert!(matches!(sym.ty, Ty::Function(_)));
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
        let t = build_symbol_table(&module);
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
                fields: vec![],
                span: sp(),
            })
        };
        let module = Module {
            items: vec![decl(), decl()],
            span: sp(),
        };
        let t = build_symbol_table(&module);
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
                params: vec![normal_param(&i, "isAdmin"), bound_param(&i, "id")],
                return_ty: Some(ty_str(&i)),
                body: vec![],
                span: sp(),
            })],
            span: sp(),
        };
        let t = build_symbol_table(&module);
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
                params: vec![bound_param(&i, "missing")],
                return_ty: None,
                body: vec![],
                span: sp(),
            })],
            span: sp(),
        };
        let t = build_symbol_table(&module);
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
        let t = build_symbol_table(&module);
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
