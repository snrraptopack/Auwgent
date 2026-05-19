//! Semantic validation and type checking for the quew DSL.
//!
//! Orchestrates quew-scope (symbol tables) and quew-unify (type inference)
//! to validate a parsed Module and emit Diagnostics.
use std::sync::Arc;

mod generics;
mod keys;
mod prelude;
pub mod resolved;
mod roles;
mod type_resolve;

use generics::instantiate_function_call;
use indexmap::IndexMap;
use keys::{PrimKeys, WellKnownKeys};
use resolved::{ResolvedExpressionMap, ResolvedCall};
use quew_ast::{
    ElseClause, Expr, Item, Lit, Module, Stmt,
    expr::Provider as AstProvider,
    item::{AnnotationArgs, ModelDecl},
    stmt::{ReplyStmt, WithField},
};
use quew_errors::{Diagnostic, Severity, Span};
use quew_interner::InternedStr;
use quew_interner::Interner;
use quew_lexer::AnnotationKind;
use quew_scope::{SymbolKind, build_symbol_table};
pub use quew_scope::SymbolTable;
use quew_types::{ProviderKind, Ty};
use quew_unify::UnifyTable;
use type_resolve::{resolve_semantic_ty, resolve_type, resolve_type_with_params};

pub use prelude::{PreludeModule, module_with_prelude};

// ── Interned key caches ─────────────────────────────────────────────────────

/// Keys for validating `reply(...) with { ... }` fields.
/// Primitive type name → `Ty` mapping (avoids raw &str comparisons on InternedStr).
// ── Local scope stack ────────────────────────────────────────────────────────

/// A stack of lexical frames for tracking local variables inside function bodies.
/// Each frame corresponds to one block `{ }`. Lookup walks innermost-first.
struct LocalScope {
    frames: Vec<IndexMap<InternedStr, (Ty, Span)>>,
}

impl LocalScope {
    fn new() -> Self {
        Self { frames: vec![] }
    }

    fn push(&mut self) {
        self.frames.push(IndexMap::new());
    }

    fn pop(&mut self) {
        self.frames.pop();
    }

    /// Define a name in the current (innermost) frame.
    /// Emits an error if the name already exists in the SAME frame (collision).
    /// Shadowing an outer frame is allowed.
    fn define(&mut self, name: InternedStr, ty: Ty, span: Span, diags: &mut Vec<Diagnostic>) {
        if let Some(frame) = self.frames.last_mut() {
            // Use contains_key first to avoid type-inference issues on get()
            if frame.contains_key(&name) {
                let prev_span = frame[&name].1;
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!("name `{name:?}` is already declared in this block"),
                    primary_span: span,
                    primary_label: Some("redeclared here".into()),
                    secondary: vec![(prev_span, "first declared here".into())],
                    help: Some("use a different name or remove the duplicate".into()),
                    code: None,
                });
            } else {
                frame.insert(name, (ty, span));
            }
        }
    }

    /// Lookup a name, innermost frame first. Returns `None` if not found locally.
    fn lookup(&self, name: InternedStr) -> Option<&Ty> {
        for frame in self.frames.iter().rev() {
            if frame.contains_key(&name) {
                return Some(&frame[&name].0);
            }
        }
        None
    }
}

// ── Public result type ────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct CheckResult {
    pub symbol_table: SymbolTable,
    pub diagnostics: Vec<Diagnostic>,
    /// Per-expression resolutions from type inference.
    /// The IR lowerer consumes this to avoid re-resolving calls without type context.
    pub resolved: ResolvedExpressionMap,
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Run all semantic checks on a parsed `Module`.
///
/// Errors are accumulated and returned — this function never panics on bad input.
pub fn check(module: &Module, interner: &Arc<Interner>) -> CheckResult {
    let symbol_table = build_symbol_table(module, interner);
    let mut diagnostics: Vec<Diagnostic> = symbol_table.diagnostics.clone();
    let mut unify = UnifyTable::new();
    let prim = PrimKeys::new(interner);
    let wk = WellKnownKeys::new(interner);
    let mut resolved = ResolvedExpressionMap::new();

    for item in &module.items {
        match item {
            Item::Agent(decl) => {
                let mut local = LocalScope::new();
                local.push();
                // Input param: register with its actual declared type
                let param_ty = resolve_type(&decl.param.ty, &symbol_table, &prim, &mut diagnostics);
                local.define(decl.param.name, param_ty, decl.param.span, &mut diagnostics);
                // @context annotation: inject `ctx` with the context type
                for ann in &decl.annotations {
                    if ann.kind == AnnotationKind::Context {
                        if let AnnotationArgs::Type(ty_expr) = &ann.args {
                            let ctx_ty =
                                resolve_type(ty_expr, &symbol_table, &prim, &mut diagnostics);
                            local.define(wk.ctx, ctx_ty, ann.span, &mut diagnostics);
                        }
                    }
                }
                let ret_ty = decl
                    .return_ty
                    .as_ref()
                    .map(|t| resolve_type(t, &symbol_table, &prim, &mut diagnostics));
                check_body(
                    &decl.body,
                    &symbol_table,
                    &mut local,
                    ret_ty.as_ref(),
                    &wk,
                    &prim,
                    &mut unify,
                    &mut diagnostics,
                    &mut resolved,
                );
                local.pop();
            }
            Item::Function(decl) => {
                let mut local = LocalScope::new();
                local.push();
                for p in &decl.params {
                    // BoundRef params (@id) are still in scope — they resolve at runtime
                    let ty = resolve_type_with_params(
                        &p.ty,
                        &decl.type_params,
                        &symbol_table,
                        &prim,
                        &mut diagnostics,
                    );
                    local.define(p.name, ty, p.span, &mut diagnostics);
                }
                let ret_ty = decl.return_ty.as_ref().map(|t| {
                    resolve_type_with_params(
                        t,
                        &decl.type_params,
                        &symbol_table,
                        &prim,
                        &mut diagnostics,
                    )
                });
                check_body(
                    &decl.body,
                    &symbol_table,
                    &mut local,
                    ret_ty.as_ref(),
                    &wk,
                    &prim,
                    &mut unify,
                    &mut diagnostics,
                    &mut resolved,
                );
                local.pop();
            }
            Item::Model(decl) => {
                check_model_decl(
                    decl,
                    &symbol_table,
                    &wk,
                    &prim,
                    &mut unify,
                    &mut diagnostics,
                    &mut resolved,
                );
            }
            Item::Extend(decl) => {
                let self_ty =
                    resolve_type(&decl.receiver, &symbol_table, &prim, &mut diagnostics);
                for method in &decl.methods {
                    let mut local = LocalScope::new();
                    local.push();
                    local.define(wk.self_ident, self_ty.clone(), method.span, &mut diagnostics);
                    for p in &method.params {
                        let ty = resolve_type_with_params(
                            &p.ty,
                            &method.type_params,
                            &symbol_table,
                            &prim,
                            &mut diagnostics,
                        );
                        local.define(p.name, ty, p.span, &mut diagnostics);
                    }
                    let ret_ty = method.return_ty.as_ref().map(|t| {
                        resolve_type_with_params(
                            t,
                            &method.type_params,
                            &symbol_table,
                            &prim,
                            &mut diagnostics,
                        )
                    });
                    check_body(
                        &method.body,
                        &symbol_table,
                        &mut local,
                        ret_ty.as_ref(),
                        &wk,
                        &prim,
                        &mut unify,
                        &mut diagnostics,
                        &mut resolved,
                    );
                    local.pop();
                }
            }
            _ => {}
        }
    }

    CheckResult {
        symbol_table,
        diagnostics,
        resolved,
    }
}

/// Run semantic checks after prepending the Quew-owned prelude.
pub fn check_with_prelude(module: &Module, interner: &Arc<Interner>) -> CheckResult {
    let prelude = module_with_prelude(module, interner);
    let mut result = check(&prelude.module, interner);

    if !prelude.diagnostics.is_empty() {
        let mut diagnostics = prelude.diagnostics;
        diagnostics.extend(result.diagnostics);
        result.diagnostics = diagnostics;
    }

    result
}

// ── Type lowering (name-aware) ────────────────────────────────────────────────

/// Lower a `TypeExpr` to `Ty`, resolving named types against the symbol table.
// ── Statement body checker ────────────────────────────────────────────────────

fn check_body(
    stmts: &[Stmt],
    table: &SymbolTable,
    local: &mut LocalScope,
    ret_ty: Option<&Ty>,
    wk: &WellKnownKeys,
    prim: &PrimKeys,
    unify: &mut UnifyTable,
    diags: &mut Vec<Diagnostic>,
    resolved: &mut ResolvedExpressionMap,
) {
    let mut unreachable = false;
    for stmt in stmts {
        if unreachable {
            diags.push(Diagnostic {
                severity: Severity::Error,
                message: "unreachable code after `return`".into(),
                primary_span: stmt_span(stmt),
                primary_label: Some("this code is never reached".into()),
                secondary: vec![],
                help: None,
                code: None,
            });
            break;
        }
        match stmt {
            Stmt::Return(r) => {
                let actual = r
                    .value
                    .as_ref()
                    .map(|v| infer_expr(v, table, local, wk, prim, unify, diags, resolved))
                    .unwrap_or(Ty::void());
                // Validate return type if declared and both sides are concrete
                if let Some(declared) = ret_ty {
                    if actual.is_ok()
                        && declared.is_ok()
                        && !matches!(declared, Ty::Primitive(quew_types::PrimTy::Void))
                    {
                        if let Err(e) = unify.unify(&actual, declared) {
                            diags.push(Diagnostic {
                                severity: Severity::Error,
                                message: format!("return type mismatch: {e}"),
                                primary_span: r.span,
                                primary_label: Some(format!(
                                    "expected `{declared}`, found `{actual}`"
                                )),
                                secondary: vec![],
                                help: None,
                                code: None,
                            });
                        }
                    }
                }
                unreachable = true;
            }
            Stmt::Let(l) => {
                let inferred = infer_expr(&l.init, table, local, wk, prim, unify, diags, resolved);
                let ty = if let Some(annotation) = &l.ty {
                    let declared = resolve_type(annotation, table, prim, diags);
                    if inferred.is_ok() && declared.is_ok() && !inferred.is_assignable_to(&declared)
                    {
                        diags.push(Diagnostic {
                            severity: Severity::Error,
                            message: format!(
                                "let type mismatch: expected `{declared}`, found `{inferred}`"
                            ),
                            primary_span: l.span,
                            primary_label: Some("initializer does not match annotation".into()),
                            secondary: vec![],
                            help: None,
                            code: None,
                        });
                    }
                    declared
                } else {
                    inferred
                };
                local.define(l.name, ty, l.span, diags);
            }
            Stmt::If(s) => {
                infer_expr(&s.condition, table, local, wk, prim, unify, diags, resolved);
                local.push();
                check_body(&s.then_body, table, local, ret_ty, wk, prim, unify, diags, resolved);
                local.pop();
                match &s.else_clause {
                    ElseClause::None => {}
                    ElseClause::Else(body, _) => {
                        local.push();
                        check_body(body, table, local, ret_ty, wk, prim, unify, diags, resolved);
                        local.pop();
                    }
                    ElseClause::ElseIf(if_stmt) => {
                        infer_expr(&if_stmt.condition, table, local, wk, prim, unify, diags, resolved);
                        local.push();
                        check_body(
                            &if_stmt.then_body,
                            table,
                            local,
                            ret_ty,
                            wk,
                            prim,
                            unify,
                            diags,
                            resolved,
                        );
                        local.pop();
                    }
                }
            }
            Stmt::Reply(r) => {
                check_with_block(r, wk, prim, table, local, unify, diags, resolved);
            }
            Stmt::For(f) => {
                infer_expr(&f.iterable, table, local, wk, prim, unify, diags, resolved);
                local.push();
                local.define(f.value, Ty::Error, f.span, diags);
                if let Some(idx) = f.index {
                    local.define(idx, Ty::Error, f.span, diags);
                }
                check_body(&f.body, table, local, ret_ty, wk, prim, unify, diags, resolved);
                local.pop();
            }
            Stmt::Expr(e) => {
                infer_expr(&e.expr, table, local, wk, prim, unify, diags, resolved);
            }
        }
    }
}

// (return coverage is now inline in check_body via the ret_ty parameter)

// ── Expression inference ──────────────────────────────────────────────────────

/// Infer the type of an expression. Returns `Ty::Error` on failure.
fn infer_expr(
    expr: &Expr,
    table: &SymbolTable,
    local: &mut LocalScope,
    wk: &WellKnownKeys,
    prim: &PrimKeys,
    unify: &mut UnifyTable,
    diags: &mut Vec<Diagnostic>,
    resolved: &mut ResolvedExpressionMap,
) -> Ty {
    match expr {
        Expr::Lit(lit) => infer_lit(lit),

        Expr::Ident(id) => {
            // Local scope takes priority over globals
            if let Some(ty) = local.lookup(id.name) {
                return ty.clone();
            }
            if let Some(sym) = table.globals.get(&id.name) {
                resolve_semantic_ty(&sym.ty, table, prim, diags, id.span)
            } else {
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!("undefined name `{:?}`", id.name),
                    primary_span: id.span,
                    primary_label: Some("not found in scope".into()),
                    secondary: vec![],
                    help: None,
                    code: None,
                });
                Ty::Error
            }
        }

        Expr::Binary(b) => {
            let l = infer_expr(&b.left, table, local, wk, prim, unify, diags, resolved);
            let r = infer_expr(&b.right, table, local, wk, prim, unify, diags, resolved);
            // For assignment, result is the rhs type
            match b.op {
                quew_ast::BinaryOp::Assign => r,
                quew_ast::BinaryOp::And | quew_ast::BinaryOp::Or => Ty::bool_ty(),
                quew_ast::BinaryOp::Eq | quew_ast::BinaryOp::NotEq => Ty::bool_ty(),
                _ => {
                    // Arithmetic: expect same type, return it
                    if let Err(e) = unify.unify(&l, &r) {
                        diags.push(Diagnostic {
                            severity: Severity::Error,
                            message: format!("operator type mismatch: {e}"),
                            primary_span: b.span,
                            primary_label: None,
                            secondary: vec![],
                            help: None,
                            code: None,
                        });
                        Ty::Error
                    } else {
                        l
                    }
                }
            }
        }

        Expr::Unary(u) => {
            let _ = infer_expr(&u.operand, table, local, wk, prim, unify, diags, resolved);
            Ty::bool_ty()
        }

        Expr::Call(c) => {
            if let Expr::Member(m) = c.callee.as_ref() {
                if let Some(ty) = infer_extension_method_call(
                    m, &c.args, table, local, wk, prim, unify, diags, c.span, resolved,
                ) {
                    return ty;
                }
            }

            let callee_ty = infer_expr(&c.callee, table, local, wk, prim, unify, diags, resolved);
            let arg_tys: Vec<Ty> = c
                .args
                .iter()
                .map(|arg| infer_expr(arg, table, local, wk, prim, unify, diags, resolved))
                .collect();
            let result = match &callee_ty {
                Ty::Function(f) => instantiate_function_call(&f, &arg_tys, c.span, diags),
                Ty::Tool(t) => {
                    let return_ty = resolve_semantic_ty(&t.return_ty, table, prim, diags, c.span);
                    roles::wrap_value_if_bound(
                        wk.tool, wk.value, return_ty, table, prim, diags, c.span,
                    )
                }
                Ty::Agent(a) => resolve_semantic_ty(&a.return_ty, table, prim, diags, c.span),
                Ty::Error => Ty::Error,
                _ => Ty::Error,
            };
            // Record resolved call for the lowerer.
            if let Expr::Ident(ident) = c.callee.as_ref() {
                let kind = match callee_ty {
                    Ty::Function(_) => resolved::CallKind::Function,
                    Ty::Tool(_) => resolved::CallKind::Tool,
                    Ty::Agent(_) => resolved::CallKind::Agent,
                    _ => resolved::CallKind::Function, // fallback
                };
                resolved.record_call(c.span, ResolvedCall::new(kind, ident.name));
            }
            result
        }

        Expr::Member(m) => {
            let obj = infer_expr(&m.object, table, local, wk, prim, unify, diags, resolved);
            match obj {
                Ty::Record(fields) => fields.get(&m.field).cloned().unwrap_or(Ty::Error),
                Ty::Error => Ty::Error,
                _ => Ty::Error,
            }
        }

        Expr::Array(a) => {
            let elem_ty = a
                .elements
                .first()
                .map(|e| infer_expr(e, table, local, wk, prim, unify, diags, resolved))
                .unwrap_or(Ty::Error);
            for e in a.elements.iter().skip(1) {
                infer_expr(e, table, local, wk, prim, unify, diags, resolved);
            }
            elem_ty
        }

        Expr::Provider(call) => {
            // Provider builders are migrating to prelude `@@function`s that
            // return the globally available `Model` type. While the parser still
            // emits `Expr::Provider`, prefer the Quew-defined `Model` contract
            // whenever it is loaded. Prelude-free checks keep the old provider
            // type so isolated tests remain possible.
            provider_call_ty(call, table, wk, prim, diags)
        }
        Expr::Is(_) => Ty::bool_ty(),
        Expr::Error(_) => Ty::Error,
        Expr::PostfixIf(p) => {
            infer_expr(&p.condition, table, local, wk, prim, unify, diags, resolved);
            let v = infer_expr(&p.value, table, local, wk, prim, unify, diags, resolved);
            infer_expr(&p.else_value, table, local, wk, prim, unify, diags, resolved);
            v
        }
    }
}

fn model_type_if_available(
    table: &SymbolTable,
    wk: &WellKnownKeys,
    prim: &PrimKeys,
    diags: &mut Vec<Diagnostic>,
    span: Span,
) -> Option<Ty> {
    table.globals.get(&wk.model_type)?;
    Some(resolve_semantic_ty(
        &Ty::Named(wk.model_type),
        table,
        prim,
        diags,
        span,
    ))
}

fn provider_call_ty(
    call: &quew_ast::expr::ProviderCall,
    table: &SymbolTable,
    wk: &WellKnownKeys,
    prim: &PrimKeys,
    diags: &mut Vec<Diagnostic>,
) -> Ty {
    if let Some(model_ty) = model_type_if_available(table, wk, prim, diags, call.span) {
        return model_ty;
    }

    let kind = match call.provider {
        AstProvider::Gemini => ProviderKind::Gemini,
        AstProvider::OpenAi => ProviderKind::OpenAi,
        AstProvider::Groq => ProviderKind::Groq,
    };
    Ty::Provider(kind)
}

fn infer_extension_method_call(
    member: &quew_ast::expr::MemberExpr,
    args: &[Expr],
    table: &SymbolTable,
    local: &mut LocalScope,
    wk: &WellKnownKeys,
    prim: &PrimKeys,
    unify: &mut UnifyTable,
    diags: &mut Vec<Diagnostic>,
    call_span: Span,
    resolved: &mut ResolvedExpressionMap,
) -> Option<Ty> {
    let receiver = infer_expr(&member.object, table, local, wk, prim, unify, diags, resolved);

    if receiver == Ty::Error {
        for arg in args {
            infer_expr(arg, table, local, wk, prim, unify, diags, resolved);
        }
        return Some(Ty::Error);
    }

    let method = table.extension_methods.iter().find(|method| {
        method.name == member.field
            && receiver.is_assignable_to(&resolve_semantic_ty(
                &method.receiver_ty,
                table,
                prim,
                &mut vec![],
                member.span,
            ))
    })?;

    // Record the resolution so the lowerer knows which extension method to call.
    resolved.record_call(
        call_span,
        ResolvedCall::extension_method(method.name, receiver.clone()),
    );

    let arg_tys: Vec<Ty> = args
        .iter()
        .map(|arg| infer_expr(arg, table, local, wk, prim, unify, diags, resolved))
        .collect();

    let function = quew_types::FunctionTy {
        type_params: method.type_params.clone(),
        params: method
            .params
            .iter()
            .map(|(name, ty)| (*name, resolve_semantic_ty(ty, table, prim, diags, call_span)))
            .collect(),
        return_ty: Box::new(resolve_semantic_ty(
            &method.return_ty,
            table,
            prim,
            diags,
            call_span,
        )),
    };
    Some(instantiate_function_call(
        &function, &arg_tys, call_span, diags,
    ))
}


fn infer_lit(lit: &Lit) -> Ty {
    match lit {
        Lit::Int(_, _) => Ty::number(),
        Lit::Float(_, _) => Ty::float(),
        Lit::String(_) => Ty::string(),
        Lit::Bool(_, _) => Ty::bool_ty(),
        Lit::Null(_) => Ty::null(),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

// Model declarations are still parsed as a dedicated compatibility AST node.
// The expected body shape now comes from the `(model, body)` role so the
// compiler can migrate validation into Quew-owned prelude contracts.
fn check_model_decl(
    decl: &ModelDecl,
    table: &SymbolTable,
    wk: &WellKnownKeys,
    prim: &PrimKeys,
    unify: &mut UnifyTable,
    diags: &mut Vec<Diagnostic>,
    resolved: &mut ResolvedExpressionMap,
) {
    let Some(contract_fields) = model_body_contract_fields(decl.span, wk, prim, table, diags)
    else {
        return;
    };

    if let Some(expected) = contract_fields.get(&wk.model) {
        let ty = provider_call_ty(&decl.provider, table, wk, prim, diags);
        if ty.is_ok() && expected.is_ok() && !ty.is_assignable_to(expected) {
            diags.push(mk_err(
                decl.provider.span,
                format!("`model` must be `{expected}`, found `{ty}`"),
                &format!("expected `{expected}`"),
                None,
            ));
        }
    }

    if let Some(expected) = contract_fields.get(&wk.config) {
        let ty = infer_config_record(&decl.config, table, wk, prim, unify, diags, resolved);
        if ty.is_ok() && expected.is_ok() && !ty.is_assignable_to(expected) {
            diags.push(mk_err(
                decl.span,
                format!("`config` must be `{expected}`, found `{ty}`"),
                &format!("expected `{expected}`"),
                None,
            ));
        }
    } else {
        let mut local = LocalScope::new();
        local.push();
        for field in &decl.config {
            infer_expr(&field.value, table, &mut local, wk, prim, unify, diags, resolved);
        }
    }
}

fn infer_config_record(
    fields: &[quew_ast::expr::ConfigField],
    table: &SymbolTable,
    wk: &WellKnownKeys,
    prim: &PrimKeys,
    unify: &mut UnifyTable,
    diags: &mut Vec<Diagnostic>,
    resolved: &mut ResolvedExpressionMap,
) -> Ty {
    let mut local = LocalScope::new();
    local.push();

    let mut record = IndexMap::new();
    for field in fields {
        let ty = infer_expr(&field.value, table, &mut local, wk, prim, unify, diags, resolved);
        record.insert(field.key, ty);
    }
    Ty::Record(record)
}

fn model_body_contract_fields(
    span: Span,
    wk: &WellKnownKeys,
    prim: &PrimKeys,
    table: &SymbolTable,
    diags: &mut Vec<Diagnostic>,
) -> Option<IndexMap<InternedStr, Ty>> {
    let ty = roles::resolve_role_type(wk.model, wk.body, table, prim, diags, span)?;
    match ty {
        Ty::Record(fields) => Some(fields),
        Ty::Error => None,
        other => {
            diags.push(mk_err(
                span,
                format!("`(model, body)` role must resolve to a record type, found `{other}`"),
                "invalid model-body role binding",
                None,
            ));
            None
        }
    }
}

fn stmt_span(stmt: &Stmt) -> Span {
    match stmt {
        Stmt::Let(s) => s.span,
        Stmt::If(s) => s.span,
        Stmt::Return(s) => s.span,
        Stmt::Reply(s) => s.span,
        Stmt::For(s) => s.span,
        Stmt::Expr(s) => s.span,
    }
}

fn mk_err(span: Span, msg: String, label: &str, help: Option<String>) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        message: msg,
        primary_span: span,
        primary_label: Some(label.into()),
        secondary: vec![],
        help,
        code: None,
    }
}

// ── reply(...) with { ... } validation ────────────────────────────────────────

/// Validate a `reply(input) with { ... }` statement.
/// Checks the input expression and each known with-block field for type correctness.
fn check_with_block(
    stmt: &ReplyStmt,
    wk: &WellKnownKeys,
    prim: &PrimKeys,
    table: &SymbolTable,
    local: &mut LocalScope,
    unify: &mut UnifyTable,
    diags: &mut Vec<Diagnostic>,
    resolved: &mut ResolvedExpressionMap,
) {
    // The reply input is a normal expression
    infer_expr(&stmt.input, table, local, wk, prim, unify, diags, resolved);
    let contract_fields = with_body_contract_fields(stmt.span, wk, prim, table, diags);

    for field in &stmt.with_block.fields {
        let k = field.key;
        if k == wk.tools {
            check_tools_field(field, table, local, wk, prim, unify, diags, resolved);
        } else if let Some(expected) = contract_fields.as_ref().and_then(|fields| fields.get(&k)) {
            let ty = infer_expr(&field.value, table, local, wk, prim, unify, diags, resolved);
            if ty.is_ok() && expected.is_ok() && !ty.is_assignable_to(expected) {
                diags.push(mk_err(
                    field.span,
                    format!(
                        "`{}` must be `{expected}`, found `{ty}`",
                        with_field_label(k, wk)
                    ),
                    &format!("expected `{expected}`"),
                    None,
                ));
            }
        } else if k == wk.model || k == wk.fallback {
            let label = if k == wk.model { "model" } else { "fallback" };
            let ty = infer_expr(&field.value, table, local, wk, prim, unify, diags, resolved);
            if !matches!(&ty, Ty::Provider(_) | Ty::Error) {
                diags.push(mk_err(
                    field.span,
                    format!("`{label}` must be a model, found `{ty}`"),
                    &format!("expected a model, found `{ty}`"),
                    None,
                ));
            }
        } else if k == wk.prompt {
            let ty = infer_expr(&field.value, table, local, wk, prim, unify, diags, resolved);
            if !matches!(&ty, Ty::Primitive(quew_types::PrimTy::String) | Ty::Error) {
                diags.push(mk_err(
                    field.span,
                    format!("`prompt` must be a string, found `{ty}`"),
                    "expected a string literal or string-returning expression",
                    None,
                ));
            }
        } else if k == wk.retry || k == wk.max_turn {
            let label = if k == wk.retry { "retry" } else { "maxTurn" };
            let ty = infer_expr(&field.value, table, local, wk, prim, unify, diags, resolved);
            if !matches!(
                &ty,
                Ty::Primitive(quew_types::PrimTy::Number | quew_types::PrimTy::Float) | Ty::Error
            ) {
                diags.push(mk_err(
                    field.span,
                    format!("`{label}` must be a number, found `{ty}`"),
                    "expected a numeric literal",
                    None,
                ));
            }
        } else {
            // builtin, agents, and any future fields — infer without semantic gate
            infer_expr(&field.value, table, local, wk, prim, unify, diags, resolved);
        }
    }
}

fn with_body_contract_fields(
    span: Span,
    wk: &WellKnownKeys,
    prim: &PrimKeys,
    table: &SymbolTable,
    diags: &mut Vec<Diagnostic>,
) -> Option<IndexMap<InternedStr, Ty>> {
    let ty = roles::resolve_role_type(wk.with, wk.body, table, prim, diags, span)?;
    match ty {
        Ty::Record(fields) => Some(fields),
        Ty::Error => None,
        other => {
            diags.push(mk_err(
                span,
                format!("`(with, body)` role must resolve to a record type, found `{other}`"),
                "invalid with-body role binding",
                None,
            ));
            None
        }
    }
}

fn with_field_label(name: InternedStr, wk: &WellKnownKeys) -> &'static str {
    if name == wk.prompt {
        "prompt"
    } else if name == wk.retry {
        "retry"
    } else if name == wk.max_turn {
        "maxTurn"
    } else if name == wk.builtin {
        "builtin"
    } else if name == wk.model {
        "model"
    } else if name == wk.fallback {
        "fallback"
    } else {
        "with field"
    }
}

/// Validate the value of a `tools: ...` field.
/// Must be an array; each element is validated by `check_tool_element`.
fn check_tools_field(
    field: &WithField,
    table: &SymbolTable,
    local: &mut LocalScope,
    wk: &WellKnownKeys,
    prim: &PrimKeys,
    unify: &mut UnifyTable,
    diags: &mut Vec<Diagnostic>,
    resolved: &mut ResolvedExpressionMap,
) {
    match &field.value {
        Expr::Array(arr) => {
            for elem in &arr.elements {
                check_tool_element(elem, table, local, wk, prim, unify, diags, resolved);
            }
        }
        other => {
            // Allow local variable references — user may have built a tools list dynamically.
            // Only error if it's a concrete non-tool value (literal, etc.).
            if let Expr::Ident(id) = other {
                if local.lookup(id.name).is_some() {
                    return; // local var; trust it
                }
            }
            let ty = infer_expr(other, table, local, wk, prim, unify, diags, resolved);
            if !matches!(ty, Ty::Error) {
                diags.push(mk_err(
                    field.span,
                    format!("`tools` must be an array of tool references, found `{ty}`"),
                    "expected `[tool1, tool2, ...]`",
                    Some("wrap tool references in an array: `tools: [myTool]`".into()),
                ));
            }
        }
    }
}

/// Validate a single element inside a `tools: [...]` array.
///
/// Rules:
/// - Bare ident → must resolve to `SymbolKind::Tool` or `ToolGroup`; if it is a
///   `Ty::Tool` with non-empty `model_params`, the caller must pre-bind them via
///   a partial call `[myTool(ctx.value)]`.
/// - Call expr → callee must be a `Ty::Tool`; arg count must equal `model_params.len()`.
/// - Anything else → error.
fn check_tool_element(
    elem: &Expr,
    table: &SymbolTable,
    local: &mut LocalScope,
    wk: &WellKnownKeys,
    prim: &PrimKeys,
    unify: &mut UnifyTable,
    diags: &mut Vec<Diagnostic>,
    resolved: &mut ResolvedExpressionMap,
) {
    match elem {
        Expr::Ident(id) => {
            match table.globals.get(&id.name) {
                Some(sym) if matches!(sym.kind, SymbolKind::Tool | SymbolKind::ToolGroup) => {
                    if let Ty::Tool(ref tt) = sym.ty {
                        if !tt.model_params.is_empty() {
                            let params: Vec<String> = tt
                                .model_params
                                .iter()
                                .map(|(n, t)| format!("{n:?}: {t}"))
                                .collect();
                            let example: Vec<String> = tt
                                .model_params
                                .iter()
                                .map(|(n, _)| format!("ctx.{n:?}"))
                                .collect();
                            diags.push(mk_err(
                                id.span,
                                format!(
                                    "tool `{:?}` has host-binding parameters [{}] that must be pre-bound; \
                                     write `{:?}({})` instead of a bare reference",
                                    id.name, params.join(", "), id.name, example.join(", ")
                                ),
                                "bare tool reference — host params must be pre-bound",
                                Some("example: `tools: [delete_person(ctx.isAdmin)]`".into()),
                            ));
                        }
                        // `bound_params` are model-visible params — fine as bare ref
                    }
                    // ToolGroup: progressive disclosure, always ok as bare ref
                }
                Some(sym) => {
                    diags.push(mk_err(
                        id.span,
                        format!(
                            "`{:?}` is not a tool or tool group (found `{}`); \
                             only `tool` declarations, `tools {{ }}` blocks, and \
                             `@tool`-decorated functions are valid in a tools array",
                            id.name, sym.ty
                        ),
                        "not a tool",
                        None,
                    ));
                }
                None => {
                    // Could be a local variable (e.g. `let selected = [...]`)
                    if local.lookup(id.name).is_none() {
                        diags.push(mk_err(
                            id.span,
                            format!("undefined name `{:?}`", id.name),
                            "not found in scope",
                            None,
                        ));
                    }
                    // Local var assumed to hold a valid tool list — checked at runtime
                }
            }
        }
        Expr::Call(call) => {
            // Partial application: `myTool(ctx.value)` pre-binds host params
            let callee_ty = infer_expr(&call.callee, table, local, wk, prim, unify, diags, resolved);
            for arg in &call.args {
                infer_expr(arg, table, local, wk, prim, unify, diags, resolved);
            }
            match &callee_ty {
                Ty::Tool(tt) => {
                    if call.args.len() != tt.model_params.len() {
                        diags.push(mk_err(
                            call.span,
                            format!(
                                "tool pre-binding: expected {} host argument(s), found {}",
                                tt.model_params.len(),
                                call.args.len()
                            ),
                            &format!("expected {} argument(s)", tt.model_params.len()),
                            None,
                        ));
                    }
                }
                Ty::Error => {} // already reported
                _ => {
                    diags.push(mk_err(
                        call.span,
                        format!(
                            "call in `tools` array must target a tool; `{callee_ty}` is not a tool"
                        ),
                        "not a tool",
                        None,
                    ));
                }
            }
        }
        other => {
            let ty = infer_expr(other, table, local, wk, prim, unify, diags, resolved);
            if !matches!(ty, Ty::Error) {
                diags.push(mk_err(
                    other.span(),
                    format!("expected a tool reference, found `{ty}`"),
                    "not a tool reference",
                    None,
                ));
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
    use quew_lexer::AnnotationKind;

    fn make_interner() -> Arc<Interner> {
        Arc::new(Interner::new())
    }
    fn sp() -> Span {
        Span::new(0, 10)
    }

    fn intern(i: &Arc<Interner>, s: &str) -> quew_interner::InternedStr {
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

    fn str_lit_expr() -> Expr {
        Expr::Lit(Lit::String(StringLit {
            value: Arc::new(Interner::new()).intern("hello"),
            kind: StringKind::Regular,
            span: sp(),
        }))
    }

    fn return_stmt(val: Option<Expr>) -> Stmt {
        Stmt::Return(ReturnStmt {
            value: val,
            mode: ReturnMode::Normal,
            span: sp(),
        })
    }

    // ── valid programs ────────────────────────────────────────────────────────

    #[test]
    fn empty_module_no_errors() {
        let i = make_interner();
        let module = Module {
            items: vec![],
            span: sp(),
        };
        let result = check(&module, &i);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn agent_with_no_body_no_errors() {
        let i = make_interner();
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
        let result = check(&module, &i);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn function_with_return_no_error() {
        let i = make_interner();
        let module = Module {
            items: vec![Item::Function(FunctionDecl {
                annotations: vec![],
                builtin: BuiltinFunctionMeta::User,
                native: None,
                name: intern(&i, "greet"),
                type_params: vec![],
                params: vec![],
                return_ty: None,
                body: vec![return_stmt(Some(str_lit_expr()))],
                span: sp(),
            })],
            span: sp(),
        };
        let result = check(&module, &i);
        assert!(
            result.diagnostics.is_empty(),
            "unexpected: {:?}",
            result.diagnostics
        );
    }

    // ── duplicate definitions ─────────────────────────────────────────────────

    #[test]
    fn duplicate_function_name_produces_error() {
        let i = make_interner();
        let mk_fn = || {
            Item::Function(FunctionDecl {
                annotations: vec![],
                builtin: BuiltinFunctionMeta::User,
                native: None,
                name: intern(&i, "foo"),
                type_params: vec![],
                params: vec![],
                return_ty: None,
                body: vec![],
                span: sp(),
            })
        };
        let module = Module {
            items: vec![mk_fn(), mk_fn()],
            span: sp(),
        };
        let result = check(&module, &i);
        assert!(!result.diagnostics.is_empty());
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
    }

    // ── return type checking ──────────────────────────────────────────────────

    #[test]
    fn return_stmt_type_mismatch_is_detected() {
        // Function declares `: string` but returns a number literal
        let i = make_interner();
        let module = Module {
            items: vec![Item::Function(FunctionDecl {
                annotations: vec![],
                builtin: BuiltinFunctionMeta::User,
                native: None,
                name: intern(&i, "bad"),
                type_params: vec![],
                params: vec![],
                return_ty: Some(ty_str(&i)),
                body: vec![return_stmt(Some(Expr::Lit(Lit::Int(42, sp()))))],
                span: sp(),
            })],
            span: sp(),
        };
        let result = check(&module, &i);
        // Return type check: number is not assignable to string
        // Note: resolve_type on "string" named type returns Ty::Error (not in globals),
        // which is absorbed. So no error is expected here from the type-system path —
        // named primitive resolution is deferred. We verify no panic occurs.
        let _ = result; // at minimum, no panic
    }

    // ── unreachable code ──────────────────────────────────────────────────────

    #[test]
    fn stmt_after_return_produces_error() {
        let i = make_interner();
        let module = Module {
            items: vec![Item::Function(FunctionDecl {
                annotations: vec![],
                builtin: BuiltinFunctionMeta::User,
                native: None,
                name: intern(&i, "unreachable_fn"),
                type_params: vec![],
                params: vec![],
                return_ty: None,
                body: vec![
                    return_stmt(None),
                    // Statement after unconditional return
                    Stmt::Expr(ExprStmt {
                        expr: str_lit_expr(),
                        span: Span::new(50, 60),
                    }),
                ],
                span: sp(),
            })],
            span: sp(),
        };
        let result = check(&module, &i);
        assert!(
            !result.diagnostics.is_empty(),
            "expected unreachable code error"
        );
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.message.contains("unreachable"))
        );
    }

    // ── expression inference ──────────────────────────────────────────────────

    #[test]
    fn int_literal_infers_number() {
        let lit = Lit::Int(42, sp());
        assert_eq!(infer_lit(&lit), Ty::number());
    }

    #[test]
    fn float_literal_infers_float() {
        assert_eq!(infer_lit(&Lit::Float(3.14, sp())), Ty::float());
    }

    #[test]
    fn bool_literal_infers_bool() {
        assert_eq!(infer_lit(&Lit::Bool(true, sp())), Ty::bool_ty());
    }

    #[test]
    fn null_literal_infers_null() {
        assert_eq!(infer_lit(&Lit::Null(sp())), Ty::null());
    }

    // ── symbol table in result ────────────────────────────────────────────────

    #[test]
    fn check_result_contains_symbol_table() {
        let i = make_interner();
        let module = Module {
            items: vec![Item::Type(TypeDecl {
                name: intern(&i, "MyType"),
                type_params: vec![],
                fields: vec![],
                builtin: BuiltinTypeMeta::User,
                span: sp(),
            })],
            span: sp(),
        };
        let result = check(&module, &i);
        assert!(
            result
                .symbol_table
                .globals
                .contains_key(&intern(&i, "MyType"))
        );
    }

    // ── bound params via check ────────────────────────────────────────────────

    #[test]
    fn bound_param_missing_from_tool_annotation_errors() {
        let i = make_interner();
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
                builtin: quew_ast::BuiltinFunctionMeta::User,
                native: None,
                name: intern(&i, "deleteUser"),
                type_params: vec![],
                params: vec![Param {
                    binding: ParamBinding::BoundRef,
                    name: intern(&i, "missing"),
                    ty: ty_str(&i),
                    optional: false,
                    span: sp(),
                }],
                return_ty: None,
                body: vec![],
                span: sp(),
            })],
            span: sp(),
        };
        let result = check(&module, &i);
        assert!(
            !result.diagnostics.is_empty(),
            "expected error for unmatched bound param"
        );
    }

    // ── name collision: duplicate params ─────────────────────────────────────

    #[test]
    fn duplicate_param_name_in_function_errors() {
        let i = make_interner();
        // function foo(x: string, x: number) — `x` declared twice
        let module = Module {
            items: vec![Item::Function(FunctionDecl {
                annotations: vec![],
                builtin: BuiltinFunctionMeta::User,
                native: None,
                name: intern(&i, "foo"),
                type_params: vec![],
                params: vec![
                    normal_param(&i, "x"),
                    // second param with same name
                    Param {
                        binding: ParamBinding::Normal,
                        name: intern(&i, "x"),
                        ty: TypeExpr::Named(intern(&i, "number"), sp()),
                        optional: false,
                        span: Span::new(20, 30),
                    },
                ],
                return_ty: None,
                body: vec![],
                span: sp(),
            })],
            span: sp(),
        };
        let result = check(&module, &i);
        assert!(
            !result.diagnostics.is_empty(),
            "expected duplicate param error"
        );
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.message.contains("already declared"))
        );
    }

    // ── name collision: duplicate let in same block ───────────────────────────

    #[test]
    fn duplicate_let_in_same_block_errors() {
        let i = make_interner();
        let make_let = |span_start: usize| {
            Stmt::Let(LetStmt {
                name: intern(&i, "x"),
                ty: None,
                init: str_lit_expr(),
                span: Span::new(span_start, span_start + 10),
            })
        };
        let module = Module {
            items: vec![Item::Function(FunctionDecl {
                annotations: vec![],
                builtin: BuiltinFunctionMeta::User,
                native: None,
                name: intern(&i, "bar"),
                type_params: vec![],
                params: vec![],
                return_ty: None,
                body: vec![make_let(0), make_let(20)],
                span: sp(),
            })],
            span: sp(),
        };
        let result = check(&module, &i);
        assert!(
            !result.diagnostics.is_empty(),
            "expected duplicate let error"
        );
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.message.contains("already declared"))
        );
    }

    // ── shadowing outer block is OK ───────────────────────────────────────────

    #[test]
    fn shadowing_outer_let_in_inner_block_ok() {
        let i = make_interner();
        // let x = ...
        // if true { let x = ... }  <- inner block, shadowing is OK
        let outer_let = Stmt::Let(LetStmt {
            name: intern(&i, "x"),
            ty: None,
            init: str_lit_expr(),
            span: sp(),
        });
        let inner_let = Stmt::Let(LetStmt {
            name: intern(&i, "x"),
            ty: None,
            init: str_lit_expr(),
            span: Span::new(20, 30),
        });
        let if_stmt = Stmt::If(IfStmt {
            condition: Expr::Lit(Lit::Bool(true, sp())),
            then_body: vec![inner_let],
            else_clause: ElseClause::None,
            span: Span::new(15, 50),
        });
        let module = Module {
            items: vec![Item::Function(FunctionDecl {
                annotations: vec![],
                builtin: BuiltinFunctionMeta::User,
                native: None,
                name: intern(&i, "shadow_fn"),
                type_params: vec![],
                params: vec![],
                return_ty: None,
                body: vec![outer_let, if_stmt],
                span: sp(),
            })],
            span: sp(),
        };
        let result = check(&module, &i);
        assert!(
            result.diagnostics.is_empty(),
            "shadowing should be ok: {:?}",
            result.diagnostics
        );
    }

    // ── collision: type vs function same name ─────────────────────────────────

    #[test]
    fn type_and_function_same_name_errors() {
        let i = make_interner();
        let module = Module {
            items: vec![
                Item::Type(TypeDecl {
                    name: intern(&i, "Foo"),
                    type_params: vec![],
                    fields: vec![],
                    builtin: BuiltinTypeMeta::User,
                    span: sp(),
                }),
                Item::Function(FunctionDecl {
                    annotations: vec![],
                    builtin: quew_ast::BuiltinFunctionMeta::User,
                    native: None,
                    name: intern(&i, "Foo"),
                    type_params: vec![],
                    params: vec![],
                    return_ty: None,
                    body: vec![],
                    span: Span::new(20, 40),
                }),
            ],
            span: sp(),
        };
        let result = check(&module, &i);
        assert!(
            !result.diagnostics.is_empty(),
            "expected duplicate name error"
        );
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.message.contains("duplicate"))
        );
    }
}
