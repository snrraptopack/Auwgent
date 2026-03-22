use serde_json::{Map, Value};
use std::collections::BTreeSet;

pub fn array_at<'a>(value: &'a Value, path: &[&str]) -> &'a [Value] {
    path.iter()
        .try_fold(value, |current, key| current.get(*key))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

pub fn object_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Map<String, Value>> {
    path.iter()
        .try_fold(value, |current, key| current.get(*key))
        .and_then(Value::as_object)
}

pub fn string_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    path.iter()
        .try_fold(value, |current, key| current.get(*key))
        .and_then(Value::as_str)
}

pub fn join_sections(sections: &[String]) -> String {
    sections
        .iter()
        .filter(|section| !section.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn collect_workflow_tools(ir: &Value) -> Vec<Value> {
    let mut tools = Vec::new();
    for workflow in array_at(ir, &["workflows"]) {
        if let Some(workflow_tools) = workflow.get("tools").and_then(Value::as_array) {
            tools.extend(workflow_tools.iter().cloned());
        }
    }
    tools
}

pub fn collect_helper_tools(ir: &Value) -> Vec<Value> {
    let mut tools = Vec::new();
    for helper in array_at(ir, &["helpers"]) {
        // Collect helper's own tools
        if let Some(helper_tools) = helper.get("tools").and_then(Value::as_array) {
            tools.extend(helper_tools.iter().cloned());
        }
        // Collect helper's workflow tools
        for workflow in array_at(helper, &["workflows"]) {
            if let Some(workflow_tools) = workflow.get("tools").and_then(Value::as_array) {
                tools.extend(workflow_tools.iter().cloned());
            }
        }
    }
    tools
}

pub fn merge_tool_defs(base: &[Value], extra: Vec<Value>) -> Vec<Value> {
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

pub fn collect_required_providers(ir: &Value) -> BTreeSet<String> {
    let mut providers = collect_providers_from_model_config(ir.get("modelConfig"));
    for helper in array_at(ir, &["helpers"]) {
        providers.extend(collect_providers_from_model_config(helper.get("modelConfig")));
    }
    providers
}

pub fn collect_custom_provider_ids(ir: &Value) -> BTreeSet<String> {
    let mut custom_ids = collect_custom_ids_from_model_config(ir.get("modelConfig"));
    for helper in array_at(ir, &["helpers"]) {
        custom_ids.extend(collect_custom_ids_from_model_config(helper.get("modelConfig")));
    }
    custom_ids
}

fn collect_providers_from_model_config(model_config: Option<&Value>) -> BTreeSet<String> {
    let mut providers = BTreeSet::new();
    let Some(configs) = model_config.and_then(Value::as_array) else {
        return providers;
    };

    for config in configs {
        // Default config
        if let Some(provider) = string_at(config, &["defaultConfig", "model", "type"]) {
            providers.insert(provider.to_string());
        }
        if let Some(provider) = string_at(config, &["defaultConfig", "embedding", "type"]) {
            providers.insert(provider.to_string());
        }

        // Named configs
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
        // Check default config model
        if string_at(config, &["defaultConfig", "model", "type"]) == Some("custom") {
            if let Some(id) = string_at(config, &["defaultConfig", "model", "id"]) {
                custom_ids.insert(id.to_string());
            }
        }
        // Check default config embedding
        if string_at(config, &["defaultConfig", "embedding", "type"]) == Some("custom") {
            if let Some(id) = string_at(config, &["defaultConfig", "embedding", "id"]) {
                custom_ids.insert(id.to_string());
            }
        }

        // Check named configs
        if let Some(named_configs) = config.get("namedConfig").and_then(Value::as_array) {
            for named in named_configs {
                // model
                if string_at(named, &["model", "type"]) == Some("custom") {
                    if let Some(id) = string_at(named, &["model", "id"]) {
                        custom_ids.insert(id.to_string());
                    }
                }
                // embedding
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

pub fn collect_transferred_helpers(ir: &Value) -> Vec<Value> {
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

pub fn collect_handoff_helpers(ir: &Value) -> Vec<Value> {
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

pub fn merge_helpers(primary: Vec<Value>, secondary: Vec<Value>) -> Vec<Value> {
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

fn collect_transfer_targets_from_statements(statements: Option<&Value>, found: &mut BTreeSet<String>) {
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