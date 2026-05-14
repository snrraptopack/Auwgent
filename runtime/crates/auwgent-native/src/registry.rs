use crate::schema::{
    build_helper_input_schema, build_output_schema, build_schema_from_params,
};
use auwgent_ir_schema::AgentIR;
use serde_json::{Value, json};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════
// NATIVE CALLABLE REGISTRY
// ═══════════════════════════════════════════════════════════════════════════

/// Action kind determines how a native function call is routed at runtime.
/// The provider-visible name carries the prefix (e.g. `tool_search`), so
/// dispatch is a simple prefix split — no map lookup needed for routing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionKind {
    Tool,
    Workflow,
    Helper,
}

/// Metadata for one callable surface exposed to the LLM provider.
#[derive(Debug, Clone)]
pub struct NativeCallableEntry {
    /// Provider-visible name, e.g. `"tool_search"`, `"workflow_route"`, `"helper_summarize"`
    pub provider_name: String,
    /// Original IR name without prefix, e.g. `"search"`
    pub canonical_name: String,
    pub action_kind: ActionKind,
    pub description: Option<String>,
    /// JSON Schema for the function parameters
    pub input_schema: Value,
}

/// Registry of all callable surfaces for native mode.
///
/// Built once from the agent IR and used to:
/// - produce provider-native tool lists (OpenAI, Gemini)
/// - route incoming native calls by prefix
/// - generate output schemas
#[derive(Debug, Clone, Default)]
pub struct NativeCallableRegistry {
    pub entries: HashMap<String, NativeCallableEntry>,
    /// Pre-built output schema, if the agent defines one
    pub output_schema: Option<Value>,
}

impl NativeCallableRegistry {
    /// Build a registry from the compiled agent IR.
    ///
    /// `strict` controls whether schemas are generated for OpenAI strict mode
    /// (`additionalProperties: false`, all fields required, optional → nullable).
    pub fn build(ir: &AgentIR, strict: bool) -> Self {
        let mut entries = HashMap::new();

        // Tools
        for tool in &ir.tools {
            let provider_name = format!("tool_{}", tool.name);
            let schema = build_schema_from_params(&tool.params.0, ir.types.as_ref(), strict);
            entries.insert(
                provider_name.clone(),
                NativeCallableEntry {
                    provider_name,
                    canonical_name: tool.name.clone(),
                    action_kind: ActionKind::Tool,
                    description: tool.description.clone(),
                    input_schema: schema,
                },
            );
        }

        // Workflows
        for workflow in &ir.workflows {
            let provider_name = format!("workflow_{}", workflow.name);
            let schema = build_schema_from_params(&workflow.params.0, ir.types.as_ref(), strict);
            entries.insert(
                provider_name.clone(),
                NativeCallableEntry {
                    provider_name,
                    canonical_name: workflow.name.clone(),
                    action_kind: ActionKind::Workflow,
                    description: workflow.description.clone(),
                    input_schema: schema,
                },
            );
        }

        // Helpers
        for helper in &ir.helpers {
            let provider_name = format!("helper_{}", helper.name);
            let schema = build_helper_input_schema(
                helper.input.as_ref().map(|v| &v.0),
                ir.types.as_ref(),
                strict,
            );
            entries.insert(
                provider_name.clone(),
                NativeCallableEntry {
                    provider_name,
                    canonical_name: helper.name.clone(),
                    action_kind: ActionKind::Helper,
                    description: helper.description.clone(),
                    input_schema: schema,
                },
            );
        }

        let output_schema = ir
            .output
            .as_ref()
            .and_then(|o| build_output_schema(&o.0, ir.types.as_ref(), strict));

        Self {
            entries,
            output_schema,
        }
    }

    pub fn get(&self, provider_name: &str) -> Option<&NativeCallableEntry> {
        self.entries.get(provider_name)
    }

    /// Route a provider-visible name to its action kind and canonical name.
    ///
    /// Splits on the first `_` just like the orchestrator uses intent names.
    /// Returns `None` if the prefix is not a known action kind.
    pub fn route(provider_name: &str) -> Option<(ActionKind, &str)> {
        let (kind, name) = provider_name.split_once('_')?;
        match kind {
            "tool" => Some((ActionKind::Tool, name)),
            "workflow" => Some((ActionKind::Workflow, name)),
            "helper" => Some((ActionKind::Helper, name)),
            _ => None,
        }
    }

    /// Generate OpenAI-compatible `tools` array.
    ///
    /// Shape: `[{ "type": "function", "function": { "name": "...", "description": "...", "parameters": {...}, "strict": true } }]`
    pub fn openai_tools(&self) -> Vec<Value> {
        self.entries
            .values()
            .map(|entry| {
                json!({
                    "type": "function",
                    "function": {
                        "name": entry.provider_name,
                        "description": entry.description.clone().unwrap_or_default(),
                        "parameters": entry.input_schema.clone(),
                        "strict": true,
                    }
                })
            })
            .collect()
    }

    /// Generate Gemini-compatible `tools` array.
    ///
    /// Shape: `[{ "functionDeclarations": [{ "name": "...", "description": "...", "parameters": {...} }] }]`
    pub fn gemini_tools(&self) -> Vec<Value> {
        let declarations: Vec<Value> = self
            .entries
            .values()
            .map(|entry| {
                json!({
                    "name": entry.provider_name,
                    "description": entry.description.clone().unwrap_or_default(),
                    "parameters": entry.input_schema.clone(),
                })
            })
            .collect();

        if declarations.is_empty() {
            vec![]
        } else {
            vec![json!({ "functionDeclarations": declarations })]
        }
    }

    /// Generate OpenAI-compatible structured output schema.
    ///
    /// Shape for Chat Completions:
    /// `{ "type": "json_schema", "json_schema": { "name": "AuwgentOutput", "schema": {...}, "strict": true } }`
    pub fn openai_output_format(&self) -> Option<Value> {
        let schema = self.output_schema.as_ref()?;
        Some(json!({
            "type": "json_schema",
            "json_schema": {
                "name": "AuwgentOutput",
                "schema": schema.clone(),
                "strict": true,
            }
        }))
    }

    /// Generate Gemini-compatible structured output schema.
    ///
    /// Shape: `{ "text": { "mimeType": "application/json", "schema": {...} } }`
    pub fn gemini_output_format(&self) -> Option<Value> {
        let schema = self.output_schema.as_ref()?;
        Some(json!({
            "text": {
                "mimeType": "application/json",
                "schema": schema.clone(),
            }
        }))
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_ir() -> AgentIR {
        serde_json::from_value(json!({
            "name": "TestAgent",
            "modelConfig": [{
                "defaultConfig": {
                    "model": { "type": "modelRef", "name": "test" },
                    "prompt": { "type": "literal", "value": "hi" }
                }
            }],
            "tools": [{
                "name": "search",
                "description": "Search the web",
                "params": {
                    "query": { "type": "string", "optional": false }
                },
                "returns": "string"
            }],
            "workflows": [{
                "flowName": "route",
                "flowParams": {
                    "target": { "type": "string", "optional": false }
                },
                "returns": "string",
                "body": []
            }],
            "helpers": [{
                "name": "Summarize",
                "description": "Summarize text",
                "input": {
                    "kind": "properties",
                    "fields": {
                        "text": { "type": "string", "optional": false }
                    }
                },
                "modelConfig": [],
                "tools": [],
                "workflows": [],
                "examples": []
            }],
            "output": {
                "status": { "type": "string", "optional": false }
            }
        }))
        .expect("valid test ir")
    }

    #[test]
    fn registry_contains_all_entries() {
        let ir = test_ir();
        let reg = NativeCallableRegistry::build(&ir, false);

        assert_eq!(reg.entries.len(), 3);
        assert!(reg.entries.contains_key("tool_search"));
        assert!(reg.entries.contains_key("workflow_route"));
        assert!(reg.entries.contains_key("helper_Summarize"));
    }

    #[test]
    fn entry_metadata_correct() {
        let ir = test_ir();
        let reg = NativeCallableRegistry::build(&ir, false);

        let tool = reg.get("tool_search").unwrap();
        assert_eq!(tool.canonical_name, "search");
        assert_eq!(tool.action_kind, ActionKind::Tool);
        assert_eq!(tool.description.as_deref(), Some("Search the web"));

        let wf = reg.get("workflow_route").unwrap();
        assert_eq!(wf.canonical_name, "route");
        assert_eq!(wf.action_kind, ActionKind::Workflow);

        let helper = reg.get("helper_Summarize").unwrap();
        assert_eq!(helper.canonical_name, "Summarize");
        assert_eq!(helper.action_kind, ActionKind::Helper);
    }

    #[test]
    fn prefix_routing() {
        assert_eq!(
            NativeCallableRegistry::route("tool_search"),
            Some((ActionKind::Tool, "search"))
        );
        assert_eq!(
            NativeCallableRegistry::route("workflow_route_case"),
            Some((ActionKind::Workflow, "route_case"))
        );
        assert_eq!(
            NativeCallableRegistry::route("helper_tool_helper"),
            Some((ActionKind::Helper, "tool_helper"))
        );
        assert_eq!(NativeCallableRegistry::route("unknown_search"), None);
        assert_eq!(NativeCallableRegistry::route("search"), None);
    }

    #[test]
    fn openai_tools_format() {
        let ir = test_ir();
        let reg = NativeCallableRegistry::build(&ir, false);
        let tools = reg.openai_tools();

        assert_eq!(tools.len(), 3);

        let tool_fn = &tools[0]["function"];
        assert!(
            tool_fn["name"].as_str().unwrap().starts_with("tool_")
                || tool_fn["name"].as_str().unwrap().starts_with("workflow_")
                || tool_fn["name"].as_str().unwrap().starts_with("helper_")
        );
        assert_eq!(tool_fn["strict"], true);
        assert!(tool_fn["parameters"].get("properties").is_some());
        assert_eq!(tool_fn["parameters"]["additionalProperties"], false);
    }

    #[test]
    fn gemini_tools_format() {
        let ir = test_ir();
        let reg = NativeCallableRegistry::build(&ir, false);
        let tools = reg.gemini_tools();

        assert_eq!(tools.len(), 1);
        let decls = tools[0]["functionDeclarations"].as_array().unwrap();
        assert_eq!(decls.len(), 3);

        let decl = &decls[0];
        assert!(
            decl["name"].as_str().unwrap().starts_with("tool_")
                || decl["name"].as_str().unwrap().starts_with("workflow_")
                || decl["name"].as_str().unwrap().starts_with("helper_")
        );
        assert!(decl["parameters"].get("properties").is_some());
    }

    #[test]
    fn empty_registry_produces_empty_tools() {
        let ir = serde_json::from_value(json!({
            "name": "Empty",
            "modelConfig": [{
                "defaultConfig": {
                    "model": { "type": "modelRef", "name": "test" },
                    "prompt": { "type": "literal", "value": "" }
                }
            }]
        }))
        .expect("valid ir");
        let reg = NativeCallableRegistry::build(&ir, false);
        assert!(reg.openai_tools().is_empty());
        assert!(reg.gemini_tools().is_empty());
    }

    #[test]
    fn output_schema_present() {
        let ir = test_ir();
        let reg = NativeCallableRegistry::build(&ir, false);
        assert!(reg.output_schema.is_some());
        assert_eq!(
            reg.output_schema.unwrap()["properties"]["status"]["type"],
            "string"
        );
    }

    #[test]
    fn openai_output_format() {
        let ir = test_ir();
        let reg = NativeCallableRegistry::build(&ir, false);
        let fmt = reg.openai_output_format().unwrap();
        assert_eq!(fmt["type"], "json_schema");
        assert_eq!(fmt["json_schema"]["name"], "AuwgentOutput");
        assert_eq!(fmt["json_schema"]["strict"], true);
    }

    #[test]
    fn gemini_output_format() {
        let ir = test_ir();
        let reg = NativeCallableRegistry::build(&ir, false);
        let fmt = reg.gemini_output_format().unwrap();
        assert_eq!(fmt["text"]["mimeType"], "application/json");
    }

    #[test]
    fn no_output_schema_when_ir_has_none() {
        let mut ir = test_ir();
        ir.output = None;
        let reg = NativeCallableRegistry::build(&ir, false);
        assert!(reg.output_schema.is_none());
        assert!(reg.openai_output_format().is_none());
        assert!(reg.gemini_output_format().is_none());
    }

    #[test]
    fn name_collision_resolved_by_prefix() {
        let ir = serde_json::from_value(json!({
            "name": "CollisionTest",
            "modelConfig": [{
                "defaultConfig": {
                    "model": { "type": "modelRef", "name": "test" },
                    "prompt": { "type": "literal", "value": "hi" }
                }
            }],
            "tools": [{
                "name": "search",
                "params": { "q": { "type": "string", "optional": false } },
                "returns": "string"
            }],
            "workflows": [{
                "flowName": "search",
                "flowParams": { "target": { "type": "string", "optional": false } },
                "returns": "string",
                "body": []
            }],
            "helpers": [{
                "name": "search",
                "input": null,
                "modelConfig": [],
                "tools": [],
                "workflows": [],
                "examples": []
            }]
        }))
        .expect("valid ir");

        let reg = NativeCallableRegistry::build(&ir, false);
        assert_eq!(reg.entries.len(), 3);

        let tool = reg.get("tool_search").unwrap();
        assert_eq!(tool.action_kind, ActionKind::Tool);
        assert!(tool.input_schema["properties"].get("q").is_some());

        let wf = reg.get("workflow_search").unwrap();
        assert_eq!(wf.action_kind, ActionKind::Workflow);
        assert!(wf.input_schema["properties"].get("target").is_some());

        let helper = reg.get("helper_search").unwrap();
        assert_eq!(helper.action_kind, ActionKind::Helper);
        assert!(helper.input_schema["properties"].get("input").is_some());
    }
}
