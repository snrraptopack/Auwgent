use crate::common::{array_at, object_at, string_at};
use serde_json::Value;
use std::collections::BTreeSet;

/// Canonical semantic view of an agent IR for code generation.
///
/// This is intentionally target-agnostic:
/// - it decides what exists in the IR
/// - it centralizes conditional feature discovery
/// - it merges semantic sources like top-level tools, workflow tools, helper tools,
///   transferred helpers, handoff helpers, and custom intents
///
/// Renderers should consume this plan and only decide how to express it in their
/// target language.
#[derive(Debug, Clone)]
pub struct CodegenPlan {
    ir: Value,
    agent_name: String,
    tools: Vec<Value>,
    workflows: Vec<Value>,
    helpers: Vec<Value>,
    output_helpers: Vec<Value>,
    custom_intents: Vec<String>,
    custom_intent_defs: Vec<(String, Value)>,
    required_providers: BTreeSet<String>,
    custom_provider_ids: BTreeSet<String>,
    has_tools: bool,
    has_workflows: bool,
    has_helpers: bool,
    has_components: bool,
    has_context: bool,
    has_api_keys: bool,
}

impl CodegenPlan {
    pub fn new(ir: Value) -> Self {
        let agent_name = string_at(&ir, &["name"]).unwrap_or("Agent").to_string();

        let tools = merge_tool_defs(
            array_at(&ir, &["tools"]),
            collect_workflow_tools(&ir)
                .into_iter()
                .chain(collect_helper_tools(&ir))
                .collect(),
        );

        let workflows = array_at(&ir, &["workflows"]).to_vec();
        let helpers = array_at(&ir, &["helpers"]).to_vec();
        let output_helpers = merge_helpers(
            collect_transferred_helpers(&ir),
            collect_handoff_helpers(&ir),
        );
        let required_providers = collect_required_providers(&ir);
        let custom_provider_ids = collect_custom_provider_ids(&ir);
        let custom_intent_defs = collect_custom_intent_defs(&ir);
        let custom_intents = custom_intent_defs
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();

        let has_tools = !tools.is_empty();
        let has_workflows = !workflows.is_empty();
        let has_helpers = !helpers.is_empty();
        let has_components = !array_at(&ir, &["components"]).is_empty();
        let has_context = ir
            .get("context")
            .and_then(Value::as_object)
            .map(|context| !context.is_empty())
            .unwrap_or(false);
        let has_api_keys = !required_providers.is_empty() || !custom_provider_ids.is_empty();

        Self {
            ir,
            agent_name,
            tools,
            workflows,
            helpers,
            output_helpers,
            custom_intents,
            custom_intent_defs,
            required_providers,
            custom_provider_ids,
            has_tools,
            has_workflows,
            has_helpers,
            has_components,
            has_context,
            has_api_keys,
        }
    }

    pub fn ir(&self) -> &Value {
        &self.ir
    }

    pub fn agent_name(&self) -> &str {
        &self.agent_name
    }

    pub fn tools(&self) -> &[Value] {
        &self.tools
    }

    pub fn workflows(&self) -> &[Value] {
        &self.workflows
    }

    pub fn helpers(&self) -> &[Value] {
        &self.helpers
    }

    pub fn output_helpers(&self) -> &[Value] {
        &self.output_helpers
    }

    pub fn custom_intents(&self) -> &[String] {
        &self.custom_intents
    }

    pub fn custom_intent_defs(&self) -> &[(String, Value)] {
        &self.custom_intent_defs
    }

    pub fn required_providers(&self) -> &BTreeSet<String> {
        &self.required_providers
    }

    pub fn custom_provider_ids(&self) -> &BTreeSet<String> {
        &self.custom_provider_ids
    }

    pub fn has_tools(&self) -> bool {
        self.has_tools
    }

    pub fn has_workflows(&self) -> bool {
        self.has_workflows
    }

    pub fn has_helpers(&self) -> bool {
        self.has_helpers
    }

    pub fn has_components(&self) -> bool {
        self.has_components
    }

    pub fn has_context(&self) -> bool {
        self.has_context
    }

    pub fn has_api_keys(&self) -> bool {
        self.has_api_keys
    }
}

fn collect_workflow_tools(ir: &Value) -> Vec<Value> {
    let mut tools = Vec::new();
    for workflow in array_at(ir, &["workflows"]) {
        if let Some(workflow_tools) = workflow.get("tools").and_then(Value::as_array) {
            tools.extend(workflow_tools.iter().cloned());
        }
    }
    tools
}

fn collect_helper_tools(ir: &Value) -> Vec<Value> {
    let mut tools = Vec::new();
    for helper in array_at(ir, &["helpers"]) {
        if let Some(helper_tools) = helper.get("tools").and_then(Value::as_array) {
            tools.extend(helper_tools.iter().cloned());
        }

        for workflow in array_at(helper, &["workflows"]) {
            if let Some(workflow_tools) = workflow.get("tools").and_then(Value::as_array) {
                tools.extend(workflow_tools.iter().cloned());
            }
        }
    }
    tools
}

fn merge_tool_defs(base: &[Value], extra: Vec<Value>) -> Vec<Value> {
    let mut merged = Vec::new();

    for tool in base.iter().chain(extra.iter()) {
        let Some(name) = string_at(tool, &["name"]) else {
            continue;
        };

        if let Some(index) = merged
            .iter()
            .position(|existing| string_at(existing, &["name"]) == Some(name))
        {
            merged[index] = tool.clone();
        } else {
            merged.push(tool.clone());
        }
    }

    merged
}

fn collect_required_providers(ir: &Value) -> BTreeSet<String> {
    let mut providers = collect_providers_from_model_config(ir.get("modelConfig"));
    for helper in array_at(ir, &["helpers"]) {
        providers.extend(collect_providers_from_model_config(
            helper.get("modelConfig"),
        ));
    }
    providers
}

fn collect_custom_provider_ids(ir: &Value) -> BTreeSet<String> {
    let mut custom_ids = collect_custom_ids_from_model_config(ir.get("modelConfig"));
    for helper in array_at(ir, &["helpers"]) {
        custom_ids.extend(collect_custom_ids_from_model_config(
            helper.get("modelConfig"),
        ));
    }
    custom_ids
        .into_iter()
        .filter(|id| !is_builtin_provider_alias(id))
        .collect()
}

fn is_builtin_provider_alias(id: &str) -> bool {
    let normalized: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect();

    matches!(
        normalized.as_str(),
        "gemini" | "geminiapi" | "openai" | "openaiapi" | "groq" | "groqapi"
    )
}

fn collect_providers_from_model_config(model_config: Option<&Value>) -> BTreeSet<String> {
    let mut providers = BTreeSet::new();
    let Some(configs) = model_config.and_then(Value::as_array) else {
        return providers;
    };

    for config in configs {
        if let Some(provider) = string_at(config, &["defaultConfig", "model", "type"]) {
            providers.insert(provider.to_string());
        }
        if let Some(provider) = string_at(config, &["defaultConfig", "embedding", "type"]) {
            providers.insert(provider.to_string());
        }

        if let Some(named_configs) = config.get("namedConfig").and_then(Value::as_array) {
            for named in named_configs {
                if let Some(provider) = string_at(named, &["model", "type"]) {
                    providers.insert(provider.to_string());
                }
                if let Some(provider) = string_at(named, &["embedding", "type"]) {
                    providers.insert(provider.to_string());
                }
            }
        }
    }

    providers
}

fn collect_custom_ids_from_model_config(model_config: Option<&Value>) -> BTreeSet<String> {
    let mut custom_ids = BTreeSet::new();
    let Some(configs) = model_config.and_then(Value::as_array) else {
        return custom_ids;
    };

    for config in configs {
        if string_at(config, &["defaultConfig", "model", "type"]) == Some("custom") {
            if let Some(id) = string_at(config, &["defaultConfig", "model", "id"]) {
                custom_ids.insert(id.to_string());
            }
        }

        if string_at(config, &["defaultConfig", "embedding", "type"]) == Some("custom") {
            if let Some(id) = string_at(config, &["defaultConfig", "embedding", "id"]) {
                custom_ids.insert(id.to_string());
            }
        }

        if let Some(named_configs) = config.get("namedConfig").and_then(Value::as_array) {
            for named in named_configs {
                if string_at(named, &["model", "type"]) == Some("custom") {
                    if let Some(id) = string_at(named, &["model", "id"]) {
                        custom_ids.insert(id.to_string());
                    }
                }

                if string_at(named, &["embedding", "type"]) == Some("custom") {
                    if let Some(id) = string_at(named, &["embedding", "id"]) {
                        custom_ids.insert(id.to_string());
                    }
                }
            }
        }
    }

    custom_ids
}

fn collect_transferred_helpers(ir: &Value) -> Vec<Value> {
    let mut transferred_names = BTreeSet::new();
    for workflow in array_at(ir, &["workflows"]) {
        collect_transfer_targets_from_statements(workflow.get("body"), &mut transferred_names);
    }

    array_at(ir, &["helpers"])
        .iter()
        .filter(|helper| {
            string_at(helper, &["name"])
                .map(|name| transferred_names.contains(name))
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

fn collect_handoff_helpers(ir: &Value) -> Vec<Value> {
    let handoff_names: BTreeSet<String> = object_at(ir, &["helperHandoff"])
        .map(|handoff| handoff.keys().cloned().collect())
        .unwrap_or_default();

    array_at(ir, &["helpers"])
        .iter()
        .filter(|helper| {
            string_at(helper, &["name"])
                .map(|name| handoff_names.contains(name))
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

fn merge_helpers(primary: Vec<Value>, secondary: Vec<Value>) -> Vec<Value> {
    let mut merged = Vec::new();

    for helper in primary.into_iter().chain(secondary.into_iter()) {
        let Some(name) = string_at(&helper, &["name"]) else {
            continue;
        };

        if merged
            .iter()
            .all(|existing| string_at(existing, &["name"]) != Some(name))
        {
            merged.push(helper);
        }
    }

    merged
}

fn collect_custom_intent_defs(ir: &Value) -> Vec<(String, Value)> {
    let mut defs = Vec::new();

    if let Some(items) = ir.get("customIntents").and_then(Value::as_array) {
        for item in items {
            if let Some(name) = string_at(item, &["name"]) {
                if !defs.iter().any(|(existing, _)| existing == name) {
                    defs.push((name.to_string(), item.clone()));
                }
            }
        }
    }

    for helper in array_at(ir, &["helpers"]) {
        if let Some(items) = helper.get("customIntents").and_then(Value::as_array) {
            for item in items {
                if let Some(name) = string_at(item, &["name"]) {
                    if !defs.iter().any(|(existing, _)| existing == name) {
                        defs.push((name.to_string(), item.clone()));
                    }
                }
            }
        }
    }

    defs
}

fn collect_transfer_targets_from_statements(
    statements: Option<&Value>,
    found: &mut BTreeSet<String>,
) {
    let Some(statements) = statements.and_then(Value::as_array) else {
        return;
    };

    for statement in statements {
        if string_at(statement, &["type"]) == Some("transfer") {
            if let Some(target) = string_at(statement, &["target", "value"]) {
                found.insert(target.to_string());
            }
        }

        if string_at(statement, &["type"]) == Some("if") {
            collect_transfer_targets_from_statements(statement.get("then"), found);
            collect_transfer_targets_from_statements(statement.get("else"), found);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_canonical_codegen_plan_from_multiple_ir_sources() {
        let ir = json!({
            "name": "Manager",
            "tools": [
                { "name": "top_tool", "params": {}, "returns": null }
            ],
            "workflows": [
                {
                    "flowName": "deleteAccount",
                    "flowParams": { "id": { "type": "string", "optional": false } },
                    "returns": { "deleted": { "type": "boolean", "optional": false } },
                    "tools": [
                        { "name": "workflow_tool", "params": {}, "returns": null }
                    ],
                    "body": [
                        {
                            "type": "transfer",
                            "target": { "value": "Reviewer" }
                        }
                    ]
                }
            ],
            "helpers": [
                {
                    "name": "Reviewer",
                    "input": { "kind": "properties", "fields": { "text": { "type": "string", "optional": false } } },
                    "output": { "approved": { "type": "boolean", "optional": false } },
                    "tools": [
                        { "name": "helper_tool", "params": {}, "returns": null }
                    ],
                    "customIntents": [
                        { "name": "ask_user", "fields": { "question": { "type": "string", "optional": false } } }
                    ],
                    "modelConfig": [
                        {
                            "defaultConfig": {
                                "model": { "type": "custom", "id": "my-groq" }
                            }
                        }
                    ]
                }
            ],
            "helperHandoff": {
                "Reviewer": true
            },
            "context": {
                "user_id": { "type": "string", "optional": false }
            },
            "components": [
                { "name": "Button" }
            ],
            "customIntents": [
                { "name": "main_action", "fields": { "value": { "type": "string", "optional": false } } }
            ],
            "modelConfig": [
                {
                    "defaultConfig": {
                        "model": { "type": "openai", "modelName": "gpt-4.1" }
                    }
                }
            ]
        });

        let plan = CodegenPlan::new(ir);

        assert_eq!(plan.agent_name(), "Manager");
        assert!(plan.has_tools());
        assert!(plan.has_workflows());
        assert!(plan.has_helpers());
        assert!(plan.has_components());
        assert!(plan.has_context());
        assert!(plan.has_api_keys());

        assert_eq!(plan.tools().len(), 3);
        assert_eq!(plan.workflows().len(), 1);
        assert_eq!(plan.helpers().len(), 1);
        assert_eq!(plan.output_helpers().len(), 1);

        assert!(plan.custom_intents().contains(&"main_action".to_string()));
        assert!(plan.custom_intents().contains(&"ask_user".to_string()));

        assert!(plan.required_providers().contains("openai"));
        assert!(plan.custom_provider_ids().contains("my-groq"));
    }

    #[test]
    fn filters_builtin_provider_aliases_from_custom_ids() {
        let ir = json!({
            "modelConfig": [
                {
                    "defaultConfig": {
                        "model": {
                            "type": "custom",
                            "id": "groq-api"
                        }
                    }
                }
            ],
            "helpers": [
                {
                    "modelConfig": [
                        {
                            "defaultConfig": {
                                "model": {
                                    "type": "custom",
                                    "id": "my-groq"
                                }
                            }
                        }
                    ]
                }
            ]
        });

        let plan = CodegenPlan::new(ir);

        assert!(!plan.custom_provider_ids().contains("groq-api"));
        assert!(plan.custom_provider_ids().contains("my-groq"));
    }
}
