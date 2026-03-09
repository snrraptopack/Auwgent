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
    let mut model_defs: Vec<&ModelDefinition> = Vec::new();
    let mut prompts: Vec<&NamedPrompt> = Vec::new();
    let mut helpers_vec: Vec<&Helper> = Vec::new();
    let mut agent: Option<&Agent> = None;

    for element in &model.elements {
        match element {
            Element::Agent(a) => agent = Some(a),
            Element::Helper(h) => helpers_vec.push(h),
            Element::TypeDecl(td) => type_decls.push(td),
            Element::ModelDef(md) => model_defs.push(md),
            Element::NamedPrompt(p) => prompts.push(p),
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
                output_val = lower_output(&oc.shape, &type_decls);
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
                model_config.push(lower_agent_model_config(mc, &prompts, &model_defs));
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
        helpers_ir.push(lower_helper(helper, &prompts, &model_defs));
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

fn lower_output(shape: &OutputShape, type_decls: &[&TypeDeclaration]) -> Value {
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
                } else {
                    prop.insert("description".into(), json!("no description"));
                }
                map.insert(p.decl.name.value.clone(), Value::Object(prop));
            }
            Value::Object(map)
        }
        OutputShape::Union(types) => {
            let mut variants = Map::new();
            let mut all_variants_resolved = true;

            for variant in types {
                if let Some(type_decl) = type_decls.iter().find(|td| td.name.value == variant.value) {
                    variants.insert(
                        variant.value.clone(),
                        lower_output_type_decl_fields(type_decl),
                    );
                } else {
                    all_variants_resolved = false;
                    break;
                }
            }

            if all_variants_resolved && !variants.is_empty() {
                return json!({ "__variants": variants });
            }

            let names: Vec<Value> = types.iter().map(|t| json!(t.value)).collect();
            json!({ "type": "union", "options": names })
        }
        OutputShape::Direct { ty, desc } => {
            if let TypeExpr::TypeRef(name) = ty {
                if let Some(type_decl) = type_decls.iter().find(|td| td.name.value == name.value) {
                    return lower_output_type_decl_fields(type_decl);
                }
            }

            let mut obj = Map::new();
            obj.insert("type".into(), lower_type_expr_value(ty));
            if let Some(d) = desc {
                obj.insert("description".into(), json!(d.value));
            }
            Value::Object(obj)
        }
    }
}

fn lower_output_type_decl_fields(type_decl: &TypeDeclaration) -> Value {
    let mut map = Map::new();
    for field in &type_decl.fields {
        let mut prop = Map::new();
        prop.insert("type".into(), lower_type_expr_value(&field.ty));
        prop.insert("optional".into(), json!(field.optional));
        if let Some(field_desc) = &field.description {
            prop.insert("description".into(), json!(field_desc.value));
        }
        map.insert(field.name.value.clone(), Value::Object(prop));
    }
    Value::Object(map)
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

fn lower_agent_model_config(
    mc: &AgentModelConfig,
    prompts: &[&NamedPrompt],
    model_defs: &[&ModelDefinition],
) -> Value {
    let mut obj = Map::new();
    obj.insert(
        "defaultConfig".into(),
        lower_model_config(&mc.default_config, prompts, model_defs),
    );

    let named: Vec<Value> = mc
        .named_configs
        .iter()
        .map(|nc| lower_named_model_config(nc, prompts, model_defs))
        .collect();
    obj.insert("namedConfig".into(), Value::Array(named));

    Value::Object(obj)
}

fn lower_named_model_config(
    nc: &NamedModelConfig,
    prompts: &[&NamedPrompt],
    model_defs: &[&ModelDefinition],
) -> Value {
    let mut obj = match lower_model_config(&nc.config, prompts, model_defs) {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    obj.insert("configName".into(), json!(nc.name.value));
    Value::Object(obj)
}

fn lower_model_config(
    mc: &ModelConfig,
    prompts: &[&NamedPrompt],
    model_defs: &[&ModelDefinition],
) -> Value {
    let mut obj = Map::new();

    // Model provider
    obj.insert("model".into(), lower_model_provider_ref(&mc.model, model_defs));

    // Prompt
    if let Some(expr) = &mc.prompt_expr {
        obj.insert("prompt".into(), lower_prompt_expr(expr, prompts));
    } else if !mc.prompt_block.is_empty() {
        let parts: Vec<Value> = mc
            .prompt_block
            .iter()
            .filter_map(|ps| lower_prompt_statement(ps, prompts))
            .collect();
        obj.insert("prompt".into(), json!({ "type": "block", "value": parts }));
    } else {
        obj.insert("prompt".into(), Value::Null);
    }

    Value::Object(obj)
}

fn lower_model_provider_ref(mpr: &ModelProviderRef, model_defs: &[&ModelDefinition]) -> Value {
    match mpr {
        ModelProviderRef::Inline(p) => lower_model_provider(p),
        ModelProviderRef::Ref(name) => model_defs
            .iter()
            .find(|model| model.name.value == name.value)
            .map(|model| lower_model_provider(&model.provider))
            .unwrap_or_else(|| json!({ "type": "modelRef", "name": name.value })),
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
                obj.insert("config".into(), lower_model_provider_config(c));
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
                obj.insert("config".into(), lower_model_provider_config(c));
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
                obj.insert("config".into(), lower_model_provider_config(c));
            }
            Value::Object(obj)
        }
    }
}

fn lower_model_provider_config(obj: &ObjectLiteral) -> Value {
    json!({
        "type": "object",
        "value": lower_object_literal(obj)
    })
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
        Expr::FunctionCall(fc) => lower_prompt_function_call(fc, prompts),
        // prompt: SomePrompt
        Expr::VarRef(v) => lower_prompt_var_ref(v, prompts),
        // prompt: left + right
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
                "left": lower_prompt_expr(&bo.left, prompts),
                "right": lower_prompt_expr(&bo.right, prompts)
            })
        }
        Expr::InlinePrompt(ip) => {
            let parts: Vec<Value> = ip
                .parts
                .iter()
                .filter_map(|ps| lower_prompt_statement(ps, prompts))
                .collect();
            json!({ "type": "inlinePrompt", "parts": parts })
        }
        Expr::Grouped(inner, _) => lower_prompt_expr(inner, prompts),
        // prompt: someExpr
        _ => lower_expression(expr),
    }
}

fn lower_prompt_var_ref(v: &Spanned<String>, prompts: &[&NamedPrompt]) -> Value {
    let prompt_def = prompts.iter().find(|p| p.name.value == v.value);
    if let Some(pdef) = prompt_def {
        let mut obj = Map::new();
        obj.insert("type".into(), json!("promptRef"));
        obj.insert("name".into(), json!(v.value));

        let param_names: Vec<Value> = pdef.params.iter().map(|p| json!(p.name.value)).collect();
        obj.insert("params".into(), Value::Array(param_names));
        obj.insert("args".into(), Value::Array(vec![]));

        let body: Vec<Value> = pdef
            .body
            .iter()
            .filter_map(|ps| lower_prompt_statement(ps, prompts))
            .collect();
        obj.insert("value".into(), Value::Array(body));

        Value::Object(obj)
    } else {
        json!({ "type": "varRef", "value": v.value })
    }
}

fn lower_prompt_function_call(fc: &FunctionCall, prompts: &[&NamedPrompt]) -> Value {
    let prompt_def = prompts.iter().find(|p| p.name.value == fc.name.value);
    if let Some(pdef) = prompt_def {
        let mut obj = Map::new();
        obj.insert("type".into(), json!("promptRef"));
        obj.insert("name".into(), json!(fc.name.value));

        let param_names: Vec<Value> = pdef.params.iter().map(|p| json!(p.name.value)).collect();
        obj.insert("params".into(), Value::Array(param_names));

        let args: Vec<Value> = fc.args.iter().map(lower_expression).collect();
        obj.insert("args".into(), Value::Array(args));

        let body: Vec<Value> = pdef
            .body
            .iter()
            .filter_map(|ps| lower_prompt_statement(ps, prompts))
            .collect();
        obj.insert("value".into(), Value::Array(body));

        Value::Object(obj)
    } else {
        let args: Vec<Value> = fc.args.iter().map(lower_expression).collect();
        json!({
            "type": "functionCall",
            "value": fc.name.value,
            "args": args
        })
    }
}

fn lower_prompt_statement(ps: &PromptStatement, prompts: &[&NamedPrompt]) -> Option<Value> {
    match ps {
        PromptStatement::Expr(expr) => Some(lower_prompt_expression(expr, prompts)),
        PromptStatement::Example(eb) => Some(lower_prompt_example_block(eb)),
        PromptStatement::If(ifs) => Some(lower_statement(&Statement::If(ifs.clone()))),
        PromptStatement::Statement(stmt) => Some(lower_statement(stmt)),
    }
}

fn lower_prompt_example_block(eb: &ExampleBlock) -> Value {
    let mut examples = Vec::new();
    let mut current_user_text = String::new();

    for message in &eb.messages {
        if message.role.value == "user" {
            current_user_text = message.text.value.clone();
        } else if message.role.value == "assistant" {
            examples.push(json!({
                "user": current_user_text,
                "assistant": message.text.value
            }));
            current_user_text.clear();
        }
    }

    json!({
        "type": "promptExamples",
        "examples": examples
    })
}

fn lower_prompt_expression(expr: &Expr, prompts: &[&NamedPrompt]) -> Value {
    match expr {
        Expr::StringLit(s) => json!({ "type": "literal", "value": s.value }),
        Expr::MultilineStringLit(s) => {
            let template = process_template_string(&s.value);
            json!({ "type": "template", "value": template })
        }
        _ => lower_prompt_expr(expr, prompts),
    }
}

// ── Template String Processing ───────────────────────────────────────────

/// Process `{{var}}` interpolations in multiline strings.
/// Produces an array of { type: "literal" } and { type: "varRef" } segments.
fn process_template_string(content: &str) -> Vec<Value> {
    parse_template_segments(content)
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

fn parse_template_segments(content: &str) -> Vec<Value> {
    let mut segments = Vec::new();
    let mut pos = 0;
    let bytes = content.as_bytes();

    while pos < bytes.len() {
        if let Some(tag_start) = find_next_tag(content, pos) {
            if tag_start > pos {
                segments.push(json!({ "type": "literal", "value": &content[pos..tag_start] }));
            }

            if let Some((segment, next_pos)) = parse_template_tag(content, tag_start) {
                segments.push(segment);
                pos = next_pos;
                continue;
            }

            segments.push(json!({ "type": "literal", "value": "{{" }));
            pos = tag_start + 2;
            continue;
        }

        if pos < content.len() {
            segments.push(json!({ "type": "literal", "value": &content[pos..] }));
        }
        break;
    }

    segments
}

fn find_next_tag(content: &str, start: usize) -> Option<usize> {
    content[start..].find("{{").map(|offset| start + offset)
}

fn parse_template_tag(content: &str, tag_start: usize) -> Option<(Value, usize)> {
    let inner_start = tag_start + 2;
    let tag_end = find_closing_braces(content, inner_start)?;
    let inner = content[inner_start..tag_end].trim();

    if inner.starts_with("@schema") {
        let path = inner
            .trim_start_matches("@schema")
            .trim_start_matches('(')
            .trim_end_matches(')')
            .trim();
        return Some((json!({ "type": "schemaDirective", "path": path }), tag_end + 2));
    }

    if let Some(condition) = inner.strip_prefix("#if") {
        return parse_inline_if_block(content, tag_start, tag_end + 2, condition.trim());
    }

    if inner == "else" || inner == "/if" {
        return None;
    }

    Some((lower_template_reference(inner), tag_end + 2))
}

fn parse_inline_if_block(
    content: &str,
    tag_start: usize,
    body_start: usize,
    condition: &str,
) -> Option<(Value, usize)> {
    let (then_end, else_range, close_range) = find_inline_if_bounds(content, body_start)?;

    let then_value = parse_template_segments(&content[body_start..then_end]);
    let else_value = else_range
        .map(|(else_start, else_end)| parse_template_segments(&content[else_start..else_end]))
        .unwrap_or_default();

    let next_pos = close_range.1;
    let inline_if = json!({
        "type": "inlineIf",
        "condition": lower_template_condition(condition),
        "then": then_value,
        "else": else_value
    });

    if tag_start >= next_pos {
        return None;
    }

    Some((inline_if, next_pos))
}

fn find_inline_if_bounds(
    content: &str,
    body_start: usize,
) -> Option<(usize, Option<(usize, usize)>, (usize, usize))> {
    let mut pos = body_start;
    let mut depth = 0usize;
    let mut else_tag: Option<(usize, usize)> = None;

    while let Some(tag_start) = find_next_tag(content, pos) {
        let inner_start = tag_start + 2;
        let tag_end = find_closing_braces(content, inner_start)?;
        let inner = content[inner_start..tag_end].trim();
        let next_pos = tag_end + 2;

        if inner.starts_with("#if") {
            depth += 1;
        } else if inner == "/if" {
            if depth == 0 {
                let then_end = else_tag.map(|(start, _)| start).unwrap_or(tag_start);
                let else_range = else_tag.map(|(_, end)| (end, tag_start));
                return Some((then_end, else_range, (tag_start, next_pos)));
            }
            depth -= 1;
        } else if inner == "else" && depth == 0 {
            else_tag = Some((tag_start, next_pos));
        }

        pos = next_pos;
    }

    None
}

fn lower_template_reference(inner: &str) -> Value {
    if inner.contains('.') {
        let parts: Vec<&str> = inner.split('.').collect();
        let object = parts.first().copied().unwrap_or("");
        let properties: Vec<&str> = parts.iter().skip(1).copied().collect();
        json!({
            "type": "memberAccess",
            "object": {
                "type": "varRef",
                "value": object
            },
            "properties": properties
        })
    } else {
        json!({ "type": "varRef", "value": inner })
    }
}

fn lower_template_condition(condition: &str) -> Value {
    for operator in ["==", "!=", ">=", "<=", ">", "<"] {
        if let Some((left, right)) = condition.split_once(operator) {
            return json!({
                "left": lower_template_condition_operand(left.trim()),
                "operator": operator,
                "right": lower_template_condition_operand(right.trim())
            });
        }
    }

    lower_template_condition_operand(condition.trim())
}

fn lower_template_condition_operand(operand: &str) -> Value {
    if operand.eq("true") {
        return json!({ "type": "literal", "value": true });
    }
    if operand.eq("false") {
        return json!({ "type": "literal", "value": false });
    }
    if let Ok(value) = operand.parse::<f64>() {
        return json!({ "type": "literal", "value": value });
    }
    if (operand.starts_with('"') && operand.ends_with('"'))
        || (operand.starts_with('\'') && operand.ends_with('\''))
    {
        return json!({
            "type": "literal",
            "value": operand[1..operand.len() - 1].to_string()
        });
    }

    lower_template_reference(operand)
}

// ── Expression Lowering ──────────────────────────────────────────────────

fn lower_expression(expr: &Expr) -> Value {
    match expr {
        Expr::StringLit(s) => json!({ "type": "literal", "value": s.value }),
        Expr::MultilineStringLit(s) => {
            let template = process_template_string(&s.value);
            json!({ "type": "template", "value": template })
        }
        Expr::NumberLit(n) => json!({ "type": "literal", "value": n.value }),
        Expr::BooleanLit(b) => json!({ "type": "literal", "value": b.value }),
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
                "object": {
                    "type": "varRef",
                    "value": ma.object.value
                },
                "properties": chain
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
                "value": fc.name.value,
                "args": args
            })
        }
        Expr::HelperCall(hc) => {
            let args: Vec<Value> = hc.args.iter().map(|a| lower_expression(a)).collect();
            json!({
                "type": "helperCall",
                "value": hc.helper.value,
                "args": args
            })
        }
        Expr::PromptCall(pc) => {
            let args: Vec<Value> = pc.args.iter().map(|a| lower_expression(a)).collect();
            json!({
                "type": "promptCall",
                "value": pc.prompt.value,
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
                .filter_map(|ps| lower_prompt_statement(ps, &[]))
                .collect();
            json!({ "type": "inlinePrompt", "parts": parts })
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
                "operator": op_str,
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
    obj.insert("flowName".into(), json!(wf.name.value));

    let params = if wf.params.is_empty() {
        Value::Object(Map::new())
    } else {
        lower_properties(&wf.params)
    };
    obj.insert("flowParams".into(), params);
    obj.insert("returns".into(), lower_type_expr_value(&wf.return_type));

    let body: Vec<Value> = wf.body.iter().map(|s| lower_statement(s)).collect();
    obj.insert("body".into(), Value::Array(body));

    obj.insert("description".into(), json!(wf.description.value));

    let tools: Vec<Value> = wf.tool_configs.iter().map(|t| lower_tool(t)).collect();
    obj.insert("tools".into(), Value::Array(tools));

    Value::Object(obj)
}

fn lower_statement(stmt: &Statement) -> Value {
    match stmt {
        Statement::Let(ls) => {
            json!({
                "type": "variableDeclaration",
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

fn lower_helper(
    helper: &Helper,
    prompts: &[&NamedPrompt],
    model_defs: &[&ModelDefinition],
) -> Value {
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
            AgentConfig::Output(oc) => output_val = lower_output(&oc.shape, &[]),
            AgentConfig::Context(cc) => context_val = lower_properties(&cc.properties),
            AgentConfig::Tool(tf) => tools_val.push(lower_tool(tf)),
            AgentConfig::Tools(tfs) => {
                for tf in tfs {
                    tools_val.push(lower_tool(tf));
                }
            }
            AgentConfig::Model(mc) => {
                model_config.push(lower_agent_model_config(mc, prompts, model_defs));
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

#[cfg(test)]
mod tests {
    use super::*;
    use auwgent_checker::check;
    use auwgent_lexer::tokenize;
    use auwgent_parser::parse;

    fn lower_source(source: &str) -> Value {
        let (tokens, lex_errors) = tokenize(source);
        assert!(lex_errors.is_empty(), "lexer errors: {lex_errors:?}");

        let (model, parse_errors) = parse(&tokens);
        assert!(parse_errors.is_empty(), "parse errors: {parse_errors:?}");

        let diagnostics = check(&model);
        assert!(diagnostics.is_empty(), "checker diagnostics: {diagnostics:?}");

        lower(&model).expect("IR lowering should succeed")
    }

    #[test]
    fn output_fields_default_description_and_provider_config_literals_match_langium() {
        let ir = lower_source(
            r#"
            agent Test {
                default config {
                    model: gemini("gemini-2.5-flash", {
                        thinking: "low",
                        maxToken: 2000
                    })
                    prompt: "Hello"
                }

                input {
                    text: string
                }

                output {
                    name: string
                    age: string
                }
            }
            "#,
        );

        assert_eq!(ir["modelConfig"][0]["defaultConfig"]["model"]["config"]["value"]["thinking"]["type"], json!("literal"));
        assert_eq!(ir["modelConfig"][0]["defaultConfig"]["model"]["config"]["value"]["maxToken"]["type"], json!("literal"));
        assert_eq!(ir["modelConfig"][0]["defaultConfig"]["model"]["config"]["value"]["maxToken"]["value"], json!(2000.0));
        assert_eq!(ir["output"]["name"]["description"], json!("no description"));
        assert_eq!(ir["output"]["age"]["description"], json!("no description"));
    }

    #[test]
    fn named_prompt_body_accepts_statement_level_if_and_return() {
        let ir = lower_source(
            r#"
            prompt One {
                """
                {{#if 1 == 10}}
                    "hel"
                {{else}}
                    "wow"
                {{/if}}
                """

                if (10 > 20) {
                    return ""
                } else {
                    return "wow"
                }
            }

            agent Test {
                default config {
                    model: gemini("gemini-2.5-flash", {
                        thinking: "low",
                        maxToken: 2000
                    })
                    prompt: "Hello" + One
                }

                input {
                    text: string
                }

                output {
                    name: string
                    age: string
                }
            }
            "#,
        );

        assert_eq!(ir["modelConfig"][0]["defaultConfig"]["prompt"]["type"], json!("binaryOp"));
        assert_eq!(ir["modelConfig"][0]["defaultConfig"]["prompt"]["right"]["type"], json!("promptRef"));
        assert_eq!(ir["modelConfig"][0]["defaultConfig"]["prompt"]["right"]["value"][0]["type"], json!("template"));
        assert_eq!(ir["modelConfig"][0]["defaultConfig"]["prompt"]["right"]["value"][1]["type"], json!("if"));
        assert_eq!(ir["modelConfig"][0]["defaultConfig"]["prompt"]["right"]["value"][1]["then"][0]["type"], json!("return"));
        assert_eq!(ir["modelConfig"][0]["defaultConfig"]["prompt"]["right"]["value"][1]["else"][0]["value"]["value"], json!("wow"));
    }

    #[test]
    fn direct_output_type_ref_is_flattened_but_types_map_is_preserved() {
        let ir = lower_source(
            r#"
            type Hey {
                name: string
                age: number
            }

            agent Test {
                default config {
                    model: gemini("gemini-2.5-flash")
                    prompt: "Hello"
                }

                input {
                    text: string
                }

                output: Hey
            }
            "#,
        );

        assert_eq!(ir["output"]["name"]["type"], json!("string"));
        assert_eq!(ir["output"]["age"]["type"], json!("number"));
        assert_eq!(ir["output"]["name"]["description"], Value::Null);
        assert_eq!(ir["types"]["Hey"]["properties"]["name"]["type"], json!("string"));
    }

    #[test]
    fn output_union_of_named_types_is_lowered_to_variants() {
        let ir = lower_source(
            r#"
            type Hey {
                name: string
                age: number
            }

            type A {
                wow: string
            }

            agent Test {
                default config {
                    model: gemini("gemini-2.5-flash")
                    prompt: "Hello"
                }

                input {
                    text: string
                }

                output: Hey | A
            }
            "#,
        );

        assert_eq!(ir["output"]["__variants"]["Hey"]["name"]["type"], json!("string"));
        assert_eq!(ir["output"]["__variants"]["Hey"]["age"]["type"], json!("number"));
        assert_eq!(ir["output"]["__variants"]["A"]["wow"]["type"], json!("string"));
        assert_eq!(ir["types"]["Hey"]["properties"]["name"]["type"], json!("string"));
        assert_eq!(ir["types"]["A"]["properties"]["wow"]["type"], json!("string"));
    }

    #[test]
    fn exported_model_definition_is_inlined_when_referenced() {
        let ir = lower_source(
            r#"
            export model Gemini {
                provider: gemini("gemini-2.5-flash")
            }

            prompt One {
                example {
                    user: "hello"
                    assistant: "how may i help you"
                }
            }

            agent Test {
                default config {
                    model: Gemini
                    prompt: "Hello" + One
                }

                input {
                    text: string
                }

                output {
                    name: string
                    age: string
                }
            }
            "#,
        );

        assert_eq!(ir["modelConfig"][0]["defaultConfig"]["model"]["type"], json!("gemini"));
        assert_eq!(ir["modelConfig"][0]["defaultConfig"]["model"]["modelName"], json!("gemini-2.5-flash"));
    }
}
