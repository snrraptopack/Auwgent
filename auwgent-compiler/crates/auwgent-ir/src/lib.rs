//! # auwgent-ir
//!
//! Lowers a checked AST into the IR JSON format consumed by the Rust runtime.
//! The output must be **identical** to Langium's `generator.ts` output.

use auwgent_ast::*;
use serde_json::{json, Map, Value};

/// Lower a parsed AST model into IR JSON value.
/// Returns the JSON for the first agent found, or error if none.
pub fn lower(model: &Model) -> Result<Value, Vec<String>> {
    // Collect type declarations for the types map
    let mut type_decls: Vec<&TypeDeclaration> = Vec::new();
    let mut prompts: Vec<&NamedPrompt> = Vec::new();
    let mut helpers_vec: Vec<&Helper> = Vec::new();
    let mut agent: Option<&Agent> = None;

    for element in &model.elements {
        match element {
            Element::Agent(a) => agent = Some(a),
            Element::Helper(h) => helpers_vec.push(h),
            Element::TypeDecl(td) => type_decls.push(td),
            Element::NamedPrompt(p) => prompts.push(p),
            Element::ModelDef(_) => {}
        }
    }

    let agent = agent.ok_or_else(|| vec!["no agent found in file".to_string()])?;

    let mut ir = Map::new();
    ir.insert("name".into(), json!(agent.name.value));

    // Process agent configs
    let mut model_config = Vec::new();
    let mut input_val: Value = Value::Null;
    let mut output_val: Value = Value::Null;
    let mut context_val: Value = Value::Null;
    let mut tools_val: Vec<Value> = Vec::new();
    let mut workflows_val: Vec<Value> = Vec::new();
    let mut helpers_ir: Vec<Value> = Vec::new();
    let tests_val: Vec<Value> = Vec::new();
    let mut helper_tool_grants: Map<String, Value> = Map::new();
    let mut helper_handoff: Map<String, Value> = Map::new();
    let mut helpers_config: Option<&HelpersConfig> = None;

    for config in &agent.configs {
        match config {
            AgentConfig::Input(ic) => {
                input_val = lower_properties(&ic.properties);
            }
            AgentConfig::Output(oc) => {
                output_val = lower_output(&oc.shape);
            }
            AgentConfig::Context(cc) => {
                context_val = lower_properties(&cc.properties);
            }
            AgentConfig::Tool(tf) => {
                tools_val.push(lower_tool(tf));
            }
            AgentConfig::Tools(tfs) => {
                for tf in tfs {
                    tools_val.push(lower_tool(tf));
                }
            }
            AgentConfig::Model(mc) => {
                model_config.push(lower_agent_model_config(mc, &prompts));
            }
            AgentConfig::Workflow(wf) => {
                workflows_val.push(lower_workflow(wf));
            }
            AgentConfig::Helpers(hc) => {
                helpers_config = Some(hc);
            }
            AgentConfig::Lifecycle(_lc) => {
                // TODO: lifecycle
            }
            AgentConfig::Test(_tc) => {
                // TODO: tests
            }
        }
    }

    // Process helpers config (grants + handoff)
    if let Some(hc) = helpers_config {
        for href in &hc.helpers {
            // Build tool grants
            if href.with_all_tools {
                helper_tool_grants.insert(href.name.value.clone(), json!("all"));
            } else if !href.granted_tools.is_empty() {
                let tool_names: Vec<Value> =
                    href.granted_tools.iter().map(|t| json!(t.value)).collect();
                helper_tool_grants.insert(href.name.value.clone(), Value::Array(tool_names));
            }

            // Build handoff
            if href.handoff_user {
                if href.handoff_then_continue {
                    helper_handoff.insert(href.name.value.clone(), json!("thenContinue"));
                } else {
                    helper_handoff.insert(href.name.value.clone(), json!("user"));
                }
            }
        }
    }

    // Process helper definitions
    for helper in &helpers_vec {
        helpers_ir.push(lower_helper(helper, &prompts));
    }

    ir.insert("modelConfig".into(), Value::Array(model_config));
    ir.insert("input".into(), input_val);
    ir.insert("output".into(), output_val);
    ir.insert("context".into(), context_val);
    ir.insert("tools".into(), Value::Array(tools_val));
    ir.insert("workflows".into(), Value::Array(workflows_val));
    ir.insert("helpers".into(), Value::Array(helpers_ir));
    ir.insert("tests".into(), Value::Array(tests_val));

    // Types map
    if !type_decls.is_empty() {
        let mut types_map = Map::new();
        for td in &type_decls {
            types_map.insert(td.name.value.clone(), lower_type_declaration(td));
        }
        ir.insert("types".into(), Value::Object(types_map));
    }

    // Helper grants & handoff
    if !helper_tool_grants.is_empty() {
        ir.insert("helperToolGrants".into(), Value::Object(helper_tool_grants));
    }
    if !helper_handoff.is_empty() {
        ir.insert("helperHandoff".into(), Value::Object(helper_handoff));
    }

    Ok(Value::Object(ir))
}

// ── Property Maps ────────────────────────────────────────────────────────

fn lower_properties(props: &[TypeConfigDecl]) -> Value {
    if props.is_empty() {
        return Value::Null;
    }
    let mut map = Map::new();
    for p in props {
        let mut prop = Map::new();
        prop.insert("type".into(), lower_type_expr_value(&p.ty));
        prop.insert("optional".into(), json!(p.optional));
        if let Some(desc) = &p.description {
            prop.insert("description".into(), json!(desc.value));
        }
        map.insert(p.name.value.clone(), Value::Object(prop));
    }
    Value::Object(map)
}

fn lower_output(shape: &OutputShape) -> Value {
    match shape {
        OutputShape::Properties(props) => {
            if props.is_empty() {
                return Value::Null;
            }
            let mut map = Map::new();
            for p in props {
                let mut prop = Map::new();
                prop.insert("type".into(), lower_type_expr_value(&p.decl.ty));
                prop.insert("optional".into(), json!(p.decl.optional));
                if let Some(desc) = &p.description {
                    prop.insert("description".into(), json!(desc.value));
                } else if let Some(desc) = &p.decl.description {
                    prop.insert("description".into(), json!(desc.value));
                }
                map.insert(p.decl.name.value.clone(), Value::Object(prop));
            }
            Value::Object(map)
        }
        OutputShape::Union(types) => {
            let names: Vec<Value> = types.iter().map(|t| json!(t.value)).collect();
            json!({ "type": "union", "options": names })
        }
        OutputShape::Direct { ty, desc } => {
            let mut obj = Map::new();
            obj.insert("type".into(), lower_type_expr_value(ty));
            if let Some(d) = desc {
                obj.insert("description".into(), json!(d.value));
            }
            Value::Object(obj)
        }
    }
}

// ── Type Expressions ─────────────────────────────────────────────────────

fn lower_type_expr_value(ty: &TypeExpr) -> Value {
    match ty {
        TypeExpr::String(_) => json!("string"),
        TypeExpr::Number(_) => json!("number"),
        TypeExpr::Boolean(_) => json!("boolean"),
        TypeExpr::Array { element, .. } => {
            json!({
                "type": "array",
                "items": lower_type_expr_value(element)
            })
        }
        TypeExpr::Object { properties, .. } => {
            let mut map = Map::new();
            map.insert("type".into(), json!("object"));
            let mut props = Map::new();
            for p in properties {
                let mut prop = Map::new();
                prop.insert("type".into(), lower_type_expr_value(&p.ty));
                prop.insert("optional".into(), json!(p.optional));
                props.insert(p.name.value.clone(), Value::Object(prop));
            }
            map.insert("properties".into(), Value::Object(props));
            Value::Object(map)
        }
        TypeExpr::TypeRef(name) => {
            json!({ "type": "typeRef", "name": name.value })
        }
        TypeExpr::Union { options, .. } => {
            let opts: Vec<Value> = options.iter().map(|o| json!(o.value)).collect();
            json!({ "type": "union", "options": opts })
        }
    }
}

// ── Tools ────────────────────────────────────────────────────────────────

fn lower_tool(tf: &ToolFunction) -> Value {
    let mut tool = Map::new();

    // Description (join all desc strings)
    let desc: String = tf
        .description
        .iter()
        .map(|d| d.value.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    tool.insert("description".into(), json!(desc));

    // Params
    let mut params = Map::new();
    for p in &tf.params {
        let mut param = Map::new();
        param.insert("type".into(), lower_type_expr_value(&p.ty));
        param.insert("optional".into(), json!(p.optional));
        params.insert(p.name.value.clone(), Value::Object(param));
    }
    tool.insert("params".into(), Value::Object(params));

    tool.insert("name".into(), json!(tf.name.value));

    // Returns
    tool.insert("returns".into(), lower_type_expr_value(&tf.returns));

    Value::Object(tool)
}

// ── Model Config ─────────────────────────────────────────────────────────

fn lower_agent_model_config(mc: &AgentModelConfig, prompts: &[&NamedPrompt]) -> Value {
    let mut obj = Map::new();
    obj.insert(
        "defaultConfig".into(),
        lower_model_config(&mc.default_config, prompts),
    );

    let named: Vec<Value> = mc
        .named_configs
        .iter()
        .map(|nc| {
            json!({
                "name": nc.name.value,
                "config": lower_model_config(&nc.config, prompts)
            })
        })
        .collect();
    obj.insert("namedConfig".into(), Value::Array(named));

    Value::Object(obj)
}

fn lower_model_config(mc: &ModelConfig, prompts: &[&NamedPrompt]) -> Value {
    let mut obj = Map::new();

    // Model provider
    obj.insert("model".into(), lower_model_provider_ref(&mc.model));

    // Prompt
    if let Some(expr) = &mc.prompt_expr {
        obj.insert("prompt".into(), lower_prompt_expr(expr, prompts));
    } else if !mc.prompt_block.is_empty() {
        let parts: Vec<Value> = mc
            .prompt_block
            .iter()
            .filter_map(|ps| lower_prompt_statement(ps))
            .collect();
        obj.insert("prompt".into(), json!({ "type": "block", "value": parts }));
    }

    Value::Object(obj)
}

fn lower_model_provider_ref(mpr: &ModelProviderRef) -> Value {
    match mpr {
        ModelProviderRef::Inline(p) => lower_model_provider(p),
        ModelProviderRef::Ref(name) => json!({ "type": "modelRef", "name": name.value }),
    }
}

fn lower_model_provider(mp: &ModelProvider) -> Value {
    match mp {
        ModelProvider::Gemini {
            model_name, config, ..
        } => {
            let mut obj = Map::new();
            obj.insert("type".into(), json!("gemini"));
            obj.insert("modelName".into(), json!(model_name.value));
            if let Some(c) = config {
                obj.insert("config".into(), lower_object_literal(c));
            }
            Value::Object(obj)
        }
        ModelProvider::OpenAI {
            model_name, config, ..
        } => {
            let mut obj = Map::new();
            obj.insert("type".into(), json!("openai"));
            obj.insert("modelName".into(), json!(model_name.value));
            if let Some(c) = config {
                obj.insert("config".into(), lower_object_literal(c));
            }
            Value::Object(obj)
        }
        ModelProvider::Custom {
            url,
            model_name,
            config,
            ..
        } => {
            let mut obj = Map::new();
            obj.insert("type".into(), json!("custom"));
            obj.insert("url".into(), json!(url.value));
            obj.insert("modelName".into(), json!(model_name.value));
            if let Some(c) = config {
                obj.insert("config".into(), lower_object_literal(c));
            }
            Value::Object(obj)
        }
    }
}

// ── Prompt Lowering ──────────────────────────────────────────────────────

fn lower_prompt_expr(expr: &Expr, prompts: &[&NamedPrompt]) -> Value {
    match expr {
        // prompt: "literal string"
        Expr::StringLit(s) => {
            json!({ "value": s.value, "type": "literal" })
        }
        // prompt: multiline """ ... {{ }} ... """
        Expr::MultilineStringLit(s) => {
            let template = process_template_string(&s.value);
            json!({ "type": "template", "value": template })
        }
        // prompt: managerPrompt(ctx.user_name)
        Expr::FunctionCall(fc) => {
            // Look up the prompt definition
            let prompt_def = prompts.iter().find(|p| p.name.value == fc.name.value);
            let mut obj = Map::new();
            obj.insert("type".into(), json!("promptRef"));
            obj.insert("name".into(), json!(fc.name.value));

            if let Some(pdef) = prompt_def {
                let param_names: Vec<Value> =
                    pdef.params.iter().map(|p| json!(p.name.value)).collect();
                obj.insert("params".into(), Value::Array(param_names));
            }

            let args: Vec<Value> = fc.args.iter().map(|a| lower_expression(a)).collect();
            obj.insert("args".into(), Value::Array(args));

            // Inline the resolved prompt body
            if let Some(pdef) = prompt_def {
                let body: Vec<Value> = pdef
                    .body
                    .iter()
                    .filter_map(|ps| lower_prompt_statement(ps))
                    .collect();
                obj.insert("value".into(), Value::Array(body));
            }

            Value::Object(obj)
        }
        // prompt: someExpr + "text"
        _ => lower_expression(expr),
    }
}

fn lower_prompt_statement(ps: &PromptStatement) -> Option<Value> {
    match ps {
        PromptStatement::Expr(expr) => Some(lower_prompt_expression(expr)),
        PromptStatement::Example(eb) => {
            let msgs: Vec<Value> = eb
                .messages
                .iter()
                .map(|m| {
                    json!({
                        "role": m.role.value,
                        "text": m.text.value
                    })
                })
                .collect();
            Some(json!({ "type": "example", "messages": msgs }))
        }
        PromptStatement::If(ifs) => {
            let then_parts: Vec<Value> = ifs
                .then_block
                .iter()
                .filter_map(|s| {
                    if let Statement::Return(r) = s {
                        Some(lower_expression(&r.value))
                    } else {
                        None
                    }
                })
                .collect();
            let else_parts: Vec<Value> = ifs
                .else_block
                .iter()
                .filter_map(|s| {
                    if let Statement::Return(r) = s {
                        Some(lower_expression(&r.value))
                    } else {
                        None
                    }
                })
                .collect();
            Some(json!({
                "type": "conditional",
                "condition": lower_condition(&ifs.condition),
                "then": then_parts,
                "else": else_parts
            }))
        }
    }
}

fn lower_prompt_expression(expr: &Expr) -> Value {
    match expr {
        Expr::StringLit(s) => json!({ "type": "literal", "value": s.value }),
        Expr::MultilineStringLit(s) => {
            let template = process_template_string(&s.value);
            json!({ "type": "template", "value": template })
        }
        _ => lower_expression(expr),
    }
}

// ── Template String Processing ───────────────────────────────────────────

/// Process `{{var}}` interpolations in multiline strings.
/// Produces an array of { type: "literal" } and { type: "varRef" } segments.
fn process_template_string(content: &str) -> Vec<Value> {
    let mut segments = Vec::new();
    let mut pos = 0;
    let bytes = content.as_bytes();

    while pos < bytes.len() {
        if pos + 1 < bytes.len() && bytes[pos] == b'{' && bytes[pos + 1] == b'{' {
            // Check for {{@schema(...)}}
            let inner_start = pos + 2;
            if let Some(end) = find_closing_braces(content, inner_start) {
                let inner = content[inner_start..end].trim();
                if inner.starts_with("@schema") {
                    // {{@schema(path)}}
                    let path = inner
                        .trim_start_matches("@schema")
                        .trim_start_matches('(')
                        .trim_end_matches(')')
                        .trim();
                    segments.push(json!({ "type": "schema", "path": path }));
                } else if inner.starts_with("#if") {
                    // Skip #if blocks for now — they're complex
                    segments.push(json!({ "type": "literal", "value": &content[pos..end+2] }));
                } else {
                    // {{variable}} or {{expr.prop}}
                    if inner.contains('.') {
                        let parts: Vec<&str> = inner.splitn(2, '.').collect();
                        segments.push(json!({
                            "type": "memberAccess",
                            "object": parts[0],
                            "property": parts[1]
                        }));
                    } else {
                        segments.push(json!({ "type": "varRef", "value": inner }));
                    }
                }
                pos = end + 2; // skip past }}
                continue;
            }
        }

        // Regular text — collect until next {{ or end
        let text_start = pos;
        while pos < bytes.len() {
            if pos + 1 < bytes.len() && bytes[pos] == b'{' && bytes[pos + 1] == b'{' {
                break;
            }
            pos += 1;
        }
        if pos > text_start {
            segments.push(json!({ "type": "literal", "value": &content[text_start..pos] }));
        }
    }

    segments
}

fn find_closing_braces(content: &str, start: usize) -> Option<usize> {
    let bytes = content.as_bytes();
    let mut i = start;
    while i + 1 < bytes.len() {
        if bytes[i] == b'}' && bytes[i + 1] == b'}' {
            return Some(i);
        }
        i += 1;
    }
    None
}

// ── Expression Lowering ──────────────────────────────────────────────────

fn lower_expression(expr: &Expr) -> Value {
    match expr {
        Expr::StringLit(s) => json!({ "type": "literal", "value": s.value }),
        Expr::MultilineStringLit(s) => {
            let template = process_template_string(&s.value);
            json!({ "type": "template", "value": template })
        }
        Expr::NumberLit(n) => json!({ "type": "number", "value": n.value }),
        Expr::BooleanLit(b) => json!({ "type": "boolean", "value": b.value }),
        Expr::Array(a) => {
            let elems: Vec<Value> = a.elements.iter().map(|e| lower_expression(e)).collect();
            json!({ "type": "array", "value": elems })
        }
        Expr::Object(o) => lower_object_literal(o),
        Expr::VarRef(v) => json!({ "type": "varRef", "value": v.value }),
        Expr::MemberAccess(ma) => {
            let mut chain = vec![ma.property.value.clone()];
            for c in &ma.chain {
                chain.push(c.value.clone());
            }
            json!({
                "type": "memberAccess",
                "object": ma.object.value,
                "property": chain.join(".")
            })
        }
        Expr::IndexAccess(ia) => {
            json!({
                "type": "indexAccess",
                "object": ia.object.value,
                "index": lower_expression(&ia.index)
            })
        }
        Expr::BinaryOp(bo) => {
            let op = match bo.op {
                BinOperator::Add => "+",
                BinOperator::Sub => "-",
                BinOperator::Mul => "*",
                BinOperator::Div => "/",
            };
            json!({
                "type": "binaryOp",
                "op": op,
                "left": lower_expression(&bo.left),
                "right": lower_expression(&bo.right)
            })
        }
        Expr::FunctionCall(fc) => {
            let args: Vec<Value> = fc.args.iter().map(|a| lower_expression(a)).collect();
            json!({
                "type": "functionCall",
                "name": fc.name.value,
                "args": args
            })
        }
        Expr::HelperCall(hc) => {
            let args: Vec<Value> = hc.args.iter().map(|a| lower_expression(a)).collect();
            json!({
                "type": "helperCall",
                "name": hc.helper.value,
                "args": args
            })
        }
        Expr::PromptCall(pc) => {
            let args: Vec<Value> = pc.args.iter().map(|a| lower_expression(a)).collect();
            json!({
                "type": "promptCall",
                "name": pc.prompt.value,
                "args": args
            })
        }
        Expr::ContextRef(cr) => {
            json!({ "type": "contextRef", "property": cr.property.value })
        }
        Expr::InlinePrompt(ip) => {
            let parts: Vec<Value> = ip
                .parts
                .iter()
                .filter_map(|ps| lower_prompt_statement(ps))
                .collect();
            json!({ "type": "block", "value": parts })
        }
        Expr::Grouped(inner, _) => lower_expression(inner),
    }
}

fn lower_object_literal(obj: &ObjectLiteral) -> Value {
    let mut map = Map::new();
    for p in &obj.properties {
        let val = p
            .value
            .as_ref()
            .map(|v| lower_expression(v))
            .unwrap_or(json!(true)); // shorthand: { name } → name: true
        map.insert(p.name.value.clone(), val);
    }
    Value::Object(map)
}

// ── Condition Lowering ───────────────────────────────────────────────────

fn lower_condition(cond: &Condition) -> Value {
    match cond {
        Condition::Comparison {
            left, op, right, ..
        } => {
            let op_str = match op {
                ComparisonOp::Eq => "==",
                ComparisonOp::Neq => "!=",
                ComparisonOp::Gt => ">",
                ComparisonOp::Lt => "<",
                ComparisonOp::Gte => ">=",
                ComparisonOp::Lte => "<=",
            };
            json!({
                "type": "comparison",
                "op": op_str,
                "left": lower_expression(left),
                "right": lower_expression(right)
            })
        }
        Condition::Logical {
            left, op, right, ..
        } => {
            let op_str = match op {
                LogicalOp::And => "&&",
                LogicalOp::Or => "||",
            };
            json!({
                "type": "logical",
                "op": op_str,
                "left": lower_condition(left),
                "right": lower_condition(right)
            })
        }
        Condition::Boolean { value, .. } => lower_expression(value),
    }
}

// ── Workflow Lowering ────────────────────────────────────────────────────

fn lower_workflow(wf: &WorkflowConfig) -> Value {
    let mut obj = Map::new();
    obj.insert("name".into(), json!(wf.name.value));
    obj.insert("description".into(), json!(wf.description.value));

    let params = lower_properties(&wf.params);
    obj.insert("params".into(), params);
    obj.insert("returns".into(), lower_type_expr_value(&wf.return_type));

    let tools: Vec<Value> = wf.tool_configs.iter().map(|t| lower_tool(t)).collect();
    obj.insert("tools".into(), Value::Array(tools));

    let body: Vec<Value> = wf.body.iter().map(|s| lower_statement(s)).collect();
    obj.insert("body".into(), Value::Array(body));

    Value::Object(obj)
}

fn lower_statement(stmt: &Statement) -> Value {
    match stmt {
        Statement::Let(ls) => {
            json!({
                "type": "let",
                "name": ls.name.value,
                "value": lower_expression(&ls.value)
            })
        }
        Statement::Assign(as_) => {
            json!({
                "type": "assign",
                "variable": as_.variable.value,
                "value": lower_expression(&as_.value)
            })
        }
        Statement::Return(rs) => {
            json!({
                "type": "return",
                "value": lower_expression(&rs.value)
            })
        }
        Statement::If(ifs) => {
            let then_stmts: Vec<Value> =
                ifs.then_block.iter().map(|s| lower_statement(s)).collect();
            let else_stmts: Vec<Value> =
                ifs.else_block.iter().map(|s| lower_statement(s)).collect();
            json!({
                "type": "if",
                "condition": lower_condition(&ifs.condition),
                "then": then_stmts,
                "else": else_stmts
            })
        }
        Statement::Transfer(ts) => {
            let args: Vec<Value> = ts.call.args.iter().map(|a| lower_expression(a)).collect();
            json!({
                "type": "transfer",
                "helper": ts.call.helper.value,
                "args": args,
                "thenContinue": ts.then_continue
            })
        }
        Statement::Parallel(ps) => {
            let stmts: Vec<Value> = ps.body.iter().map(|s| lower_statement(s)).collect();
            json!({ "type": "parallel", "body": stmts })
        }
    }
}

// ── Helper Lowering ──────────────────────────────────────────────────────

fn lower_helper(helper: &Helper, prompts: &[&NamedPrompt]) -> Value {
    let mut obj = Map::new();
    obj.insert("name".into(), json!(helper.name.value));
    obj.insert("description".into(), json!(helper.description.value));

    let mut model_config = Vec::new();
    let mut input_val: Value = Value::Null;
    let mut output_val: Value = Value::Null;
    let mut context_val: Value = Value::Null;
    let mut tools_val: Vec<Value> = Vec::new();
    let mut workflows_val: Vec<Value> = Vec::new();

    for config in &helper.configs {
        match config {
            AgentConfig::Input(ic) => input_val = lower_properties(&ic.properties),
            AgentConfig::Output(oc) => output_val = lower_output(&oc.shape),
            AgentConfig::Context(cc) => context_val = lower_properties(&cc.properties),
            AgentConfig::Tool(tf) => tools_val.push(lower_tool(tf)),
            AgentConfig::Tools(tfs) => {
                for tf in tfs {
                    tools_val.push(lower_tool(tf));
                }
            }
            AgentConfig::Model(mc) => {
                model_config.push(lower_agent_model_config(mc, prompts));
            }
            AgentConfig::Workflow(wf) => workflows_val.push(lower_workflow(wf)),
            _ => {}
        }
    }

    obj.insert("modelConfig".into(), Value::Array(model_config));
    obj.insert("input".into(), input_val);
    obj.insert("output".into(), output_val);
    obj.insert("context".into(), context_val);
    obj.insert("tools".into(), Value::Array(tools_val));
    obj.insert("workflows".into(), Value::Array(workflows_val));

    Value::Object(obj)
}

// ── Type Declaration Lowering ────────────────────────────────────────────

fn lower_type_declaration(td: &TypeDeclaration) -> Value {
    let mut obj = Map::new();
    obj.insert("isOutput".into(), json!(td.is_output));

    let mut props = Map::new();
    for field in &td.fields {
        let mut prop = Map::new();
        prop.insert("type".into(), lower_type_expr_value(&field.ty));
        prop.insert("optional".into(), json!(field.optional));
        if let Some(desc) = &field.description {
            prop.insert("description".into(), json!(desc.value));
        }
        props.insert(field.name.value.clone(), Value::Object(prop));
    }
    obj.insert("properties".into(), Value::Object(props));

    Value::Object(obj)
}
