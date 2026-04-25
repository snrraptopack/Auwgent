use crate::common::{join_sections,string_at};
use crate::generation_plan::CodegenPlan;
use serde_json::{Map,Value};
use std::collections::BTreeSet;

const AGENT_TYPE: &str = "AuwgentAgent";
const CONFIG_TYPE: &str = "AuwgentConfig";
const INPUT_TYPE: &str = "AuwgentInput";
const OUTPUT_TYPE: &str = "AuwgentOutput";
const BASE_OUTPUT_TYPE: &str = "AuwgentBaseOutput";
const CONTEXT_TYPE: &str = "AuwgentContext";
const API_KEYS_TYPE: &str = "AuwgentApiKeys";
const TOOLS_TRAIT: &str = "AuwgentTools";
const TOOLS_REGISTRY: &str = "AuwgentToolsRegistry";
const INTENT_ENUM: &str = "AuwgentIntent";
const INTENT_PARTIAL_ENUM: &str = "AuwgentIntentPartial";
const INTENT_NAME_ENUM: &str = "AuwgentIntentName";
const INTENT_HANDLER_TRAIT: &str = "AuwgentIntentHandler";
const PARTIAL_HANDLER_TRAIT: &str = "AuwgentBasePartialIntentHandler";
const MIDDLEWARE_TRAIT: &str = "AuwgentMiddleware";
const MIDDLEWARE_REGISTRY: &str = "AuwgentMiddlewareRegistry";
const INTENTS_VIEW: &str = "Intents";
const RESPONSE_TEXT_TYPE: &str = "ResponseText";
const RESPONSE_SCHEMA_TYPE: &str = "ResponseSchema";
const ERROR_INTENT_TYPE: &str = "ErrorIntent";

pub fn generate(plan:&CodegenPlan,base_name:&str)->String{
 let ir=plan.ir();
 let agent_name=plan.agent_name();
 let all_tools=plan.tools();
 let workflows=plan.workflows();
 let helpers=plan.helpers();
 let output_helpers=plan.output_helpers();
 let custom_intents=plan.custom_intents();
 let has_tools=plan.has_tools();
 let has_workflows=plan.has_workflows();
 let has_helpers=plan.has_helpers();
 let has_components=plan.has_components();
 let has_context=plan.has_context();
 let required_providers=plan.required_providers();
 let custom_provider_ids=plan.custom_provider_ids();
 let mut emitted_shapes=BTreeSet::new();
 let mut sections=vec![format!("// Auto-generated Rust bindings for {agent_name}"),"// Do not edit manually".to_string(),String::new(),"use async_trait::async_trait;".to_string(),"use auwgent_sdk_rust as sdk;".to_string(),"use serde_json::Value as JsonValue;".to_string(),"use std::sync::Arc;".to_string(),String::new(),generate_runtime_support()];
 if let Some(types)=ir.get("types").and_then(Value::as_object){sections.push(generate_custom_types(types));}
 sections.push(generate_input_type(agent_name,ir.get("input")));
 for helper in output_helpers{sections.push(generate_helper_output_type(helper));}
 sections.push(generate_output_type(agent_name,ir.get("output"),output_helpers));
 if has_context{sections.push(generate_named_shape(CONTEXT_TYPE,ir.get("context")));}
 if has_tools{sections.push(generate_tools(agent_name,all_tools,&mut emitted_shapes));}
 sections.push(generate_intent_name_enum(agent_name,has_tools,has_workflows,has_helpers,has_components,custom_intents));
 sections.push(generate_custom_intent_types(agent_name,plan));
 sections.push(generate_core_intents(agent_name,ir.get("output")));
 if has_tools{sections.push(generate_callable_family(agent_name,"Tool",all_tools,"name","params","returns",true,&mut emitted_shapes));}
 if has_workflows{sections.push(generate_callable_family(agent_name,"Workflow",workflows,"flowName","flowParams","returns",false,&mut emitted_shapes));}
 if has_helpers{sections.push(generate_callable_family(agent_name,"Helper",helpers,"name","input","output",false,&mut emitted_shapes));}
 if has_components{sections.push(generate_component_intents(agent_name));}
 sections.push(generate_top_level_intent_enums(agent_name,has_tools,has_workflows,has_helpers,has_components,custom_intents));
 sections.push(generate_decode_functions(agent_name,has_tools,has_workflows,has_helpers,has_components,custom_intents));
 sections.push(generate_handler_traits(agent_name,has_tools,has_workflows,has_helpers,has_components));
 sections.push(generate_api_keys(agent_name,required_providers,custom_provider_ids));
 sections.push(generate_middleware_trait(agent_name));
 sections.push(generate_config(agent_name,has_tools,has_context,plan.has_api_keys()));
 sections.push(generate_agent(agent_name,base_name,has_tools,has_context,plan.has_api_keys()));
 sections.push(generate_aliases(agent_name,has_tools,has_workflows,has_helpers,has_context,plan.has_api_keys()));
 join_sections(&sections)
}

fn generate_runtime_support()->String{[
"pub type IntentControl = sdk::IntentControl;".to_string(),
"pub type SessionState = sdk::SessionState;".to_string(),
"pub type Session = SessionState;".to_string(),
"pub type Context = sdk::MiddlewareContext;".to_string(),
"#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]\npub struct NoArgs {}\n".to_string(),
"#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n#[serde(rename_all = \"snake_case\")]\npub enum PartialIntentMode {\n    Text,\n    Structured,\n}\n".to_string(),
"#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub struct PartialIntentEnvelope {\n    pub partial: bool,\n    pub complete: bool,\n    pub mode: PartialIntentMode,\n    pub segment: i64,\n    pub raw: String,\n}\n".to_string(),
"#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub struct PartialTextIntentValue {\n    #[serde(flatten)]\n    pub envelope: PartialIntentEnvelope,\n    pub text: String,\n    #[serde(default)]\n    pub delta: Option<String>,\n}\n".to_string(),
"#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub struct PartialStructuredIntentValue<T> {\n    #[serde(flatten)]\n    pub envelope: PartialIntentEnvelope,\n    #[serde(flatten)]\n    pub value: T,\n}\n".to_string(),
].join("\n")}

fn generate_input_type(agent_name:&str,input:Option<&Value>)->String{
 let _=agent_name;
 if input.is_none(){return format!("pub type {INPUT_TYPE} = String;\n");}
 generate_named_shape(INPUT_TYPE,unwrap_input_fields(input).as_ref())
}

fn generate_custom_types(types:&Map<String,Value>)->String{let mut blocks=Vec::new();for(type_name,type_def)in types{blocks.push(generate_named_shape(type_name,Some(type_def)));}blocks.join("\n")}

fn generate_helper_output_type(helper:&Value)->String{
 let helper_name=string_at(helper,&["name"]).unwrap_or("Helper");
 generate_named_shape(&format!("{helper_name}Output"),helper.get("output"))
}

fn generate_output_type(_agent_name:&str,value:Option<&Value>,output_helpers:&[Value])->String{
 let Some(value)=value else{
 if output_helpers.is_empty(){return format!("pub type {OUTPUT_TYPE} = JsonValue;\n");}
  let base_name=BASE_OUTPUT_TYPE.to_string();
  let mut enum_variants=vec![format!("    Base({base_name}),")];
  for helper in output_helpers{
   if let Some(helper_name)=string_at(helper,&["name"]){
    let helper_output=format!("{}Output",helper_name);
    enum_variants.push(format!("    {}({helper_output}),",to_rust_type_name(helper_name)));
   }
  }
  return format!("#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]\npub struct {base_name};\n\n#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n#[serde(untagged)]\npub enum {OUTPUT_TYPE} {{\n{}\n}}\n",enum_variants.join("\n"));
 };
 if let Some(variants)=value.get("__variants").and_then(Value::as_object){
  let mut blocks=Vec::new();let mut enum_variants=Vec::new();
  for(variant_name,variant_props)in variants{
   let case_name=format!("{}OutputCase",to_rust_type_name(variant_name));
   let props=variant_props.as_object().cloned().unwrap_or_default();
   blocks.push(generate_struct(&case_name,&props));
   enum_variants.push(format!("    #[serde(rename = \"{variant_name}\")]\n    {}({case_name}),",to_rust_type_name(variant_name)));
  }
  blocks.push(format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n#[serde(tag = \"type\")]\npub enum {OUTPUT_TYPE} {{\n{}\n}}\n",enum_variants.join("\n")));
  return blocks.join("\n");
 }
 if output_helpers.is_empty(){return generate_named_shape(OUTPUT_TYPE,Some(value));}
 let mut blocks=Vec::new();
 let base_name=BASE_OUTPUT_TYPE.to_string();
 if value.is_null(){
  blocks.push(format!("#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]\npub struct {base_name};\n"));
 }else{
  blocks.push(generate_named_shape(&base_name,Some(value)));
 }
 let mut enum_variants=vec![format!("    Base({base_name}),")];
 for helper in output_helpers{
  if let Some(helper_name)=string_at(helper,&["name"]){
   let helper_output=format!("{}Output",helper_name);
   enum_variants.push(format!("    {}({helper_output}),",to_rust_type_name(helper_name)));
  }
 }
 blocks.push(format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n#[serde(untagged)]\npub enum {OUTPUT_TYPE} {{\n{}\n}}\n",enum_variants.join("\n")));
 blocks.join("\n")
}

fn generate_named_shape(name:&str,value:Option<&Value>)->String{if let Some(properties)=shape_properties(value,false){return generate_struct(name,&properties);}format!("pub type {name} = {};\n",rust_type(value,false,"JsonValue"))}

fn generate_struct(name:&str,properties:&Map<String,Value>)->String{
 let mut fields=Vec::new();
 for(prop_name,prop_info)in properties{
  if prop_name.starts_with('@')||prop_name.starts_with("__"){continue;}
  let optional=prop_info.get("optional").and_then(Value::as_bool).unwrap_or(false);
  let field_name=to_rust_field_name(prop_name);
  let field_type=rust_type(Some(prop_info),optional,"JsonValue");
  let rename=if field_name!=prop_name.as_str(){format!("    #[serde(rename = \"{prop_name}\")]\n")}else{String::new()};
  fields.push(format!("{rename}    pub {field_name}: {field_type},"));
 }
 if fields.is_empty(){return format!("#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]\npub struct {name};\n");}
 format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub struct {name} {{\n{}\n}}\n",fields.join("\n"))
}

fn generate_tools(agent_name:&str,tools:&[Value],emitted_shapes:&mut BTreeSet<String>)->String{
 let _=agent_name;
 let mut result_aliases=Vec::new();let mut methods=Vec::new();let mut tool_names=Vec::new();let mut invocations=Vec::new();
 for tool in tools{
  let Some(tool_name)=string_at(tool,&["name"]) else{continue;};
  let pascal=to_rust_type_name(tool_name);let method_name=to_rust_field_name(tool_name);
  let args_type=shape_type_or_no_args(&format!("{pascal}Args"),tool.get("params"),&mut result_aliases,emitted_shapes);
  let result_alias=format!("{pascal}Result");
  result_aliases.push(format!("pub type {result_alias} = {};\n",rust_type(tool.get("returns"),false,"()")));
  if args_type=="NoArgs"{
   methods.push(format!("    fn {method_name}(&self) -> {pascal}Result;"));
  }else{
   methods.push(format!("    fn {method_name}(&self, args: {pascal}Args) -> {pascal}Result;"));
  }
  tool_names.push(format!("\"{tool_name}\""));
  if args_type=="NoArgs"{
   invocations.push(format!("            \"{tool_name}\" => {{\n                let tools = Arc::clone(&self.0);\n                Box::pin(async move {{\n                    let _: NoArgs = serde_json::from_value(args).map_err(|e| e.to_string())?;\n                    let result = tools.{method_name}();\n                    serde_json::to_value(result).map_err(|e| e.to_string())\n                }})\n            }}"));
  }else{
   invocations.push(format!("            \"{tool_name}\" => {{\n                let tools = Arc::clone(&self.0);\n                Box::pin(async move {{\n                    let parsed: {pascal}Args = serde_json::from_value(args).map_err(|e| e.to_string())?;\n                    let result = tools.{method_name}(parsed);\n                    serde_json::to_value(result).map_err(|e| e.to_string())\n                }})\n            }}"));
  }
 }
 format!("{}\npub trait {TOOLS_TRAIT}: Send + Sync + 'static {{\n{}\n}}\n\n#[derive(Clone)]\npub struct {TOOLS_REGISTRY}(pub Arc<dyn {TOOLS_TRAIT}>);\n\nimpl<T> From<T> for {TOOLS_REGISTRY}\nwhere\n    T: {TOOLS_TRAIT},\n{{\n    fn from(value: T) -> Self {{\n        Self(Arc::new(value))\n    }}\n}}\n\nimpl sdk::ToolRegistrar for {TOOLS_REGISTRY} {{\n    fn tool_names(&self) -> &'static [&'static str] {{\n        &[{}]\n    }}\n\n    fn invoke_tool(\n        &self,\n        name: &'static str,\n        args: JsonValue,\n    ) -> sdk::BoxFuture<'static, sdk::AuwgentResult<JsonValue>> {{\n        match name {{\n{}\n            _ => Box::pin(async move {{ Err(format!(\"Unknown tool: {{name}}\")) }}),\n        }}\n    }}\n}}\n",result_aliases.join("\n"),methods.join("\n"),tool_names.join(", "),invocations.join(",\n"))
}

fn generate_intent_name_enum(_agent_name:&str,has_tools:bool,has_workflows:bool,has_helpers:bool,has_components:bool,custom_intents:&[String])->String{
 let mut names=vec!["ResponseText".to_string(),"ResponseSchema".to_string(),"Error".to_string()];
 if has_tools{names.extend(["ToolCall".to_string(),"ToolResult".to_string(),"ToolError".to_string(),"ToolSkipped".to_string()]);}
 if has_workflows{names.extend(["WorkflowCall".to_string(),"WorkflowResult".to_string()]);}
 if has_helpers{names.extend(["HelperCall".to_string(),"HelperResult".to_string()]);}
 if has_components{names.extend(["Component".to_string(),"RenderComponent".to_string()]);}
 for custom_intent in custom_intents{names.push(to_rust_type_name(custom_intent));}
 let variants=names.iter().map(|name|format!("    {name},")).collect::<Vec<_>>().join("\n");
 format!("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum {INTENT_NAME_ENUM} {{\n{variants}\n}}\n")
}

fn generate_custom_intent_types(agent_name:&str,plan:&CodegenPlan)->String{
 let _=agent_name;
 let mut blocks=Vec::new();
 for (name,item) in plan.custom_intent_defs(){blocks.push(generate_named_shape(&format!("{}Intent",to_rust_type_name(name)),item.get("fields")));}
 blocks.join("\n")
}

fn generate_core_intents(_agent_name:&str,output:Option<&Value>)->String{
 let response_schema=if let Some(variants)=output.and_then(|value|value.get("__variants")).and_then(Value::as_object){
  let enum_variants=variants.keys().map(|variant_name|{let case_name=format!("{}OutputCase",to_rust_type_name(variant_name));format!("    #[serde(rename = \"{variant_name}\")]\n    {}({case_name}),",to_rust_type_name(variant_name))}).collect::<Vec<_>>().join("\n");
  format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n#[serde(tag = \"type\", content = \"response\")]\npub enum {RESPONSE_SCHEMA_TYPE} {{\n{enum_variants}\n}}\n")
 }else{format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub struct {RESPONSE_SCHEMA_TYPE} {{\n    #[serde(rename = \"type\")]\n    pub kind: String,\n    pub response: {OUTPUT_TYPE},\n}}\n")};
 [format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub struct {RESPONSE_TEXT_TYPE} {{\n    pub text: String,\n}}\n"),response_schema,format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub struct {ERROR_INTENT_TYPE} {{\n    pub message: String,\n}}\n")].join("\n")
}
fn generate_callable_family(agent_name:&str,family_name:&str,items:&[Value],name_key:&str,args_key:&str,result_key:&str,include_error_and_skipped:bool,emitted_shapes:&mut BTreeSet<String>)->String{
 if items.is_empty(){return String::new();}
 let _=agent_name;
 let call_name=format!("{family_name}Call");
 let result_name=format!("{family_name}Result");
 let skipped_name=format!("{family_name}Skipped");
 let error_name=format!("{family_name}Error");
 let mut blocks=Vec::new();let mut call_variants=Vec::new();let mut result_variants=Vec::new();let mut skipped_variants=Vec::new();
 for item in items{
  let Some(item_name)=string_at(item,&[name_key]) else{continue;};
  let pascal=to_rust_type_name(item_name);
  let args_type=shape_type_or_no_args(&format!("{pascal}{family_name}Args"),item.get(args_key),&mut blocks,emitted_shapes);
  let result_type=rust_type(item.get(result_key),false,"()");
  call_variants.push(enum_struct_variant(item_name,&pascal,"args",&args_type));
  result_variants.push(format!("    #[serde(rename = \"{item_name}\")]\n    {pascal} {{\n        args: {args_type},\n        result: {result_type},\n        #[serde(default)]\n        overridden: bool,\n    }},"));
  if include_error_and_skipped{skipped_variants.push(enum_struct_variant(item_name,&pascal,"args",&args_type));}
 }
 blocks.push(format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n#[serde(tag = \"type\")]\npub enum {call_name} {{\n{}\n}}\n",call_variants.join("\n")));
 blocks.push(format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n#[serde(tag = \"name\")]\npub enum {result_name} {{\n{}\n}}\n",result_variants.join("\n")));
 if include_error_and_skipped{
  blocks.push(format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n#[serde(tag = \"type\")]\npub enum {skipped_name} {{\n{}\n}}\n",skipped_variants.join("\n")));
  blocks.push(format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub struct {error_name} {{\n    pub tool: String,\n    pub message: String,\n}}\n"));
 }
 blocks.join("\n")
}

fn enum_struct_variant(item_name:&str,pascal:&str,field_name:&str,field_type:&str)->String{if field_type=="NoArgs"{format!("    #[serde(rename = \"{item_name}\")]\n    {pascal},")}else{format!("    #[serde(rename = \"{item_name}\")]\n    {pascal} {{\n        {field_name}: {field_type},\n    }},")}}

fn generate_component_intents(agent_name:&str)->String{
 let _=agent_name;
 ["pub type ComponentIntent = JsonValue;\n".to_string(),"pub type RenderComponentIntent = JsonValue;\n".to_string()].join("\n")
}

fn generate_top_level_intent_enums(agent_name:&str,has_tools:bool,has_workflows:bool,has_helpers:bool,has_components:bool,custom_intents:&[String])->String{
 let _=agent_name;
 let mut intent_variants=vec![format!("    ResponseText({RESPONSE_TEXT_TYPE}),"),format!("    ResponseSchema({RESPONSE_SCHEMA_TYPE}),"),format!("    Error({ERROR_INTENT_TYPE}),")];
 let mut partial_variants=vec!["    ResponseText(PartialTextIntentValue),".to_string(),format!("    ResponseSchema(PartialStructuredIntentValue<{RESPONSE_SCHEMA_TYPE}>),"),format!("    Error(PartialStructuredIntentValue<{ERROR_INTENT_TYPE}>),")];
 if has_tools{
  intent_variants.extend(["    ToolCall(ToolCall),".to_string(),"    ToolResult(ToolResult),".to_string(),"    ToolError(ToolError),".to_string(),"    ToolSkipped(ToolSkipped),".to_string()]);
  partial_variants.extend(["    ToolCall(PartialStructuredIntentValue<ToolCall>),".to_string(),"    ToolResult(PartialStructuredIntentValue<ToolResult>),".to_string(),"    ToolError(PartialStructuredIntentValue<ToolError>),".to_string(),"    ToolSkipped(PartialStructuredIntentValue<ToolSkipped>),".to_string()]);
 }
 if has_workflows{
  intent_variants.extend(["    WorkflowCall(WorkflowCall),".to_string(),"    WorkflowResult(WorkflowResult),".to_string()]);
  partial_variants.extend(["    WorkflowCall(PartialStructuredIntentValue<WorkflowCall>),".to_string(),"    WorkflowResult(PartialStructuredIntentValue<WorkflowResult>),".to_string()]);
 }
 if has_helpers{
  intent_variants.extend(["    HelperCall(HelperCall),".to_string(),"    HelperResult(HelperResult),".to_string()]);
  partial_variants.extend(["    HelperCall(PartialStructuredIntentValue<HelperCall>),".to_string(),"    HelperResult(PartialStructuredIntentValue<HelperResult>),".to_string()]);
 }
 if has_components{
  intent_variants.extend(["    Component(ComponentIntent),".to_string(),"    RenderComponent(RenderComponentIntent),".to_string()]);
  partial_variants.extend(["    Component(PartialStructuredIntentValue<ComponentIntent>),".to_string(),"    RenderComponent(PartialStructuredIntentValue<RenderComponentIntent>),".to_string()]);
 }
 for custom_intent in custom_intents{let pascal=to_rust_type_name(custom_intent);let type_name=format!("{pascal}Intent");intent_variants.push(format!("    {pascal}({type_name}),"));partial_variants.push(format!("    {pascal}(PartialStructuredIntentValue<{type_name}>),"));}
 format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub enum {INTENT_ENUM} {{\n{}\n}}\n\n#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub enum {INTENT_PARTIAL_ENUM} {{\n{}\n}}\n",intent_variants.join("\n"),partial_variants.join("\n"))
}

fn generate_decode_functions(agent_name:&str,has_tools:bool,has_workflows:bool,has_helpers:bool,has_components:bool,custom_intents:&[String])->String{
 let _=agent_name;
 let mut name_cases=vec![format!("        \"response_text\" => Some({INTENT_NAME_ENUM}::ResponseText),"),format!("        \"response_schema\" => Some({INTENT_NAME_ENUM}::ResponseSchema),"),format!("        \"error\" => Some({INTENT_NAME_ENUM}::Error),")];
 let mut intent_cases=vec![format!("        {INTENT_NAME_ENUM}::ResponseText => serde_json::from_value(value).ok().map({INTENT_ENUM}::ResponseText),"),format!("        {INTENT_NAME_ENUM}::ResponseSchema => serde_json::from_value(value).ok().map({INTENT_ENUM}::ResponseSchema),"),format!("        {INTENT_NAME_ENUM}::Error => serde_json::from_value(value).ok().map({INTENT_ENUM}::Error),")];
 let mut partial_cases=vec![format!("        {INTENT_NAME_ENUM}::ResponseText => serde_json::from_value(value).ok().map({INTENT_PARTIAL_ENUM}::ResponseText),"),format!("        {INTENT_NAME_ENUM}::ResponseSchema => serde_json::from_value(value).ok().map({INTENT_PARTIAL_ENUM}::ResponseSchema),"),format!("        {INTENT_NAME_ENUM}::Error => serde_json::from_value(value).ok().map({INTENT_PARTIAL_ENUM}::Error),")];
 if has_tools{
  name_cases.extend([format!("        \"tool_call\" => Some({INTENT_NAME_ENUM}::ToolCall),"),format!("        \"tool_result\" => Some({INTENT_NAME_ENUM}::ToolResult),"),format!("        \"tool_error\" => Some({INTENT_NAME_ENUM}::ToolError),"),format!("        \"tool_skipped\" => Some({INTENT_NAME_ENUM}::ToolSkipped),")]);
  intent_cases.extend([format!("        {INTENT_NAME_ENUM}::ToolCall => serde_json::from_value(value).ok().map({INTENT_ENUM}::ToolCall),"),format!("        {INTENT_NAME_ENUM}::ToolResult => serde_json::from_value(value).ok().map({INTENT_ENUM}::ToolResult),"),format!("        {INTENT_NAME_ENUM}::ToolError => serde_json::from_value(value).ok().map({INTENT_ENUM}::ToolError),"),format!("        {INTENT_NAME_ENUM}::ToolSkipped => serde_json::from_value(value).ok().map({INTENT_ENUM}::ToolSkipped),")]);
  partial_cases.extend([format!("        {INTENT_NAME_ENUM}::ToolCall => serde_json::from_value(value).ok().map({INTENT_PARTIAL_ENUM}::ToolCall),"),format!("        {INTENT_NAME_ENUM}::ToolResult => serde_json::from_value(value).ok().map({INTENT_PARTIAL_ENUM}::ToolResult),"),format!("        {INTENT_NAME_ENUM}::ToolError => serde_json::from_value(value).ok().map({INTENT_PARTIAL_ENUM}::ToolError),"),format!("        {INTENT_NAME_ENUM}::ToolSkipped => serde_json::from_value(value).ok().map({INTENT_PARTIAL_ENUM}::ToolSkipped),")]);
 }
 if has_workflows{
  name_cases.extend([format!("        \"workflow_call\" => Some({INTENT_NAME_ENUM}::WorkflowCall),"),format!("        \"workflow_result\" => Some({INTENT_NAME_ENUM}::WorkflowResult),")]);
  intent_cases.extend([format!("        {INTENT_NAME_ENUM}::WorkflowCall => serde_json::from_value(value).ok().map({INTENT_ENUM}::WorkflowCall),"),format!("        {INTENT_NAME_ENUM}::WorkflowResult => serde_json::from_value(value).ok().map({INTENT_ENUM}::WorkflowResult),")]);
  partial_cases.extend([format!("        {INTENT_NAME_ENUM}::WorkflowCall => serde_json::from_value(value).ok().map({INTENT_PARTIAL_ENUM}::WorkflowCall),"),format!("        {INTENT_NAME_ENUM}::WorkflowResult => serde_json::from_value(value).ok().map({INTENT_PARTIAL_ENUM}::WorkflowResult),")]);
 }
 if has_helpers{
  name_cases.extend([format!("        \"helper_call\" => Some({INTENT_NAME_ENUM}::HelperCall),"),format!("        \"helper_result\" => Some({INTENT_NAME_ENUM}::HelperResult),")]);
  intent_cases.extend([format!("        {INTENT_NAME_ENUM}::HelperCall => serde_json::from_value(value).ok().map({INTENT_ENUM}::HelperCall),"),format!("        {INTENT_NAME_ENUM}::HelperResult => serde_json::from_value(value).ok().map({INTENT_ENUM}::HelperResult),")]);
  partial_cases.extend([format!("        {INTENT_NAME_ENUM}::HelperCall => serde_json::from_value(value).ok().map({INTENT_PARTIAL_ENUM}::HelperCall),"),format!("        {INTENT_NAME_ENUM}::HelperResult => serde_json::from_value(value).ok().map({INTENT_PARTIAL_ENUM}::HelperResult),")]);
 }
 if has_components{
  name_cases.extend([format!("        \"component\" => Some({INTENT_NAME_ENUM}::Component),"),format!("        \"render_component\" => Some({INTENT_NAME_ENUM}::RenderComponent),")]);
  intent_cases.extend([format!("        {INTENT_NAME_ENUM}::Component => Some({INTENT_ENUM}::Component(value)),"),format!("        {INTENT_NAME_ENUM}::RenderComponent => Some({INTENT_ENUM}::RenderComponent(value)),")]);
  partial_cases.extend([format!("        {INTENT_NAME_ENUM}::Component => serde_json::from_value(value).ok().map({INTENT_PARTIAL_ENUM}::Component),"),format!("        {INTENT_NAME_ENUM}::RenderComponent => serde_json::from_value(value).ok().map({INTENT_PARTIAL_ENUM}::RenderComponent),")]);
 }
 for custom_intent in custom_intents{let pascal=to_rust_type_name(custom_intent);name_cases.push(format!("        \"{custom_intent}\" => Some({INTENT_NAME_ENUM}::{pascal}),"));intent_cases.push(format!("        {INTENT_NAME_ENUM}::{pascal} => serde_json::from_value(value).ok().map({INTENT_ENUM}::{pascal}),"));partial_cases.push(format!("        {INTENT_NAME_ENUM}::{pascal} => serde_json::from_value(value).ok().map({INTENT_PARTIAL_ENUM}::{pascal}),"));}
 format!("impl {INTENT_NAME_ENUM} {{\n    pub fn parse(name: &str) -> Option<Self> {{\n        match name {{\n{}\n            _ => None,\n        }}\n    }}\n}}\n\nimpl {INTENT_ENUM} {{\n    pub fn decode(name: {INTENT_NAME_ENUM}, value: JsonValue) -> Option<Self> {{\n        match name {{\n{}\n        }}\n    }}\n}}\n\nimpl {INTENT_PARTIAL_ENUM} {{\n    pub fn decode(name: {INTENT_NAME_ENUM}, value: JsonValue) -> Option<Self> {{\n        match name {{\n{}\n        }}\n    }}\n}}\n",name_cases.join("\n"),intent_cases.join("\n"),partial_cases.join("\n"))
}

fn generate_handler_traits(agent_name:&str,has_tools:bool,has_workflows:bool,has_helpers:bool,has_components:bool)->String{
 let _=agent_name;
 let mut wrapper_defs=Vec::new();
 let mut name_arms=vec![
  format!("            {INTENT_ENUM}::ResponseText(_) => \"response_text\","),
  format!("            {INTENT_ENUM}::ResponseSchema(_) => \"response_schema\","),
  format!("            {INTENT_ENUM}::Error(_) => \"error\","),
 ];
 let mut args_arms=Vec::new();
 let mut dispatch_arms=vec![
  format!("            {INTENT_ENUM}::ResponseText(value) => self.response_text(value, agent_name),"),
  format!("            {INTENT_ENUM}::ResponseSchema(value) => self.response_schema(value, agent_name),"),
  format!("            {INTENT_ENUM}::Error(value) => self.error(value, agent_name),"),
 ];
 if has_tools{
  wrapper_defs.extend([
   "#[derive(Debug, Clone)]\npub struct ToolCalls {\n    pub kind: ToolCall,\n}\n".to_string(),
   "#[derive(Debug, Clone)]\npub struct ToolResults {\n    pub kind: ToolResult,\n}\n".to_string(),
   "#[derive(Debug, Clone)]\npub struct ToolErrors {\n    pub kind: ToolError,\n}\n".to_string(),
   "#[derive(Debug, Clone)]\npub struct ToolSkippeds {\n    pub kind: ToolSkipped,\n}\n".to_string(),
  ]);
  name_arms.extend([
   format!("            {INTENT_ENUM}::ToolCall(..) => \"tool_call\","),
   format!("            {INTENT_ENUM}::ToolResult(..) => \"tool_result\","),
   format!("            {INTENT_ENUM}::ToolError(..) => \"tool_error\","),
   format!("            {INTENT_ENUM}::ToolSkipped(..) => \"tool_skipped\","),
  ]);
  args_arms.extend([
   format!("            {INTENT_ENUM}::ToolCall(intent) => serde_json::to_value(intent.clone()),"),
   format!("            {INTENT_ENUM}::ToolResult(intent) => serde_json::to_value(intent.clone()),"),
   format!("            {INTENT_ENUM}::ToolError(intent) => serde_json::to_value(intent.clone()),"),
   format!("            {INTENT_ENUM}::ToolSkipped(intent) => serde_json::to_value(intent.clone()),"),
  ]);
  dispatch_arms.extend([
   format!("            {INTENT_ENUM}::ToolCall(value) => self.tool_call(&ToolCalls {{ kind: value.clone() }}, agent_name),"),
   format!("            {INTENT_ENUM}::ToolResult(value) => self.tool_result(&ToolResults {{ kind: value.clone() }}, agent_name),"),
   format!("            {INTENT_ENUM}::ToolError(value) => self.tool_error(&ToolErrors {{ kind: value.clone() }}, agent_name),"),
   format!("            {INTENT_ENUM}::ToolSkipped(value) => self.tool_skipped(&ToolSkippeds {{ kind: value.clone() }}, agent_name),"),
  ]);
 }
 if has_workflows{
  wrapper_defs.extend([
   "#[derive(Debug, Clone)]\npub struct WorkflowCalls {\n    pub kind: WorkflowCall,\n}\n".to_string(),
   "#[derive(Debug, Clone)]\npub struct WorkflowResults {\n    pub kind: WorkflowResult,\n}\n".to_string(),
  ]);
  name_arms.extend([
   format!("            {INTENT_ENUM}::WorkflowCall(..) => \"workflow_call\","),
   format!("            {INTENT_ENUM}::WorkflowResult(..) => \"workflow_result\","),
  ]);
  args_arms.extend([
   format!("            {INTENT_ENUM}::WorkflowCall(intent) => serde_json::to_value(intent.clone()),"),
   format!("            {INTENT_ENUM}::WorkflowResult(intent) => serde_json::to_value(intent.clone()),"),
  ]);
  dispatch_arms.extend([
   format!("            {INTENT_ENUM}::WorkflowCall(value) => self.workflow_call(&WorkflowCalls {{ kind: value.clone() }}, agent_name),"),
   format!("            {INTENT_ENUM}::WorkflowResult(value) => self.workflow_result(&WorkflowResults {{ kind: value.clone() }}, agent_name),"),
  ]);
 }
 if has_helpers{
  wrapper_defs.extend([
   "#[derive(Debug, Clone)]\npub struct HelperCalls {\n    pub kind: HelperCall,\n}\n".to_string(),
   "#[derive(Debug, Clone)]\npub struct HelperResults {\n    pub kind: HelperResult,\n}\n".to_string(),
  ]);
  name_arms.extend([
   format!("            {INTENT_ENUM}::HelperCall(..) => \"helper_call\","),
   format!("            {INTENT_ENUM}::HelperResult(..) => \"helper_result\","),
  ]);
  args_arms.extend([
   format!("            {INTENT_ENUM}::HelperCall(intent) => serde_json::to_value(intent.clone()),"),
   format!("            {INTENT_ENUM}::HelperResult(intent) => serde_json::to_value(intent.clone()),"),
  ]);
  dispatch_arms.extend([
   format!("            {INTENT_ENUM}::HelperCall(value) => self.helper_call(&HelperCalls {{ kind: value.clone() }}, agent_name),"),
   format!("            {INTENT_ENUM}::HelperResult(value) => self.helper_result(&HelperResults {{ kind: value.clone() }}, agent_name),"),
  ]);
 }
 if has_components{
  name_arms.extend([
   format!("            {INTENT_ENUM}::Component(..) => \"component\","),
   format!("            {INTENT_ENUM}::RenderComponent(..) => \"render_component\","),
  ]);
  dispatch_arms.extend([
   format!("            {INTENT_ENUM}::Component(..) => self.component(intent, agent_name),"),
   format!("            {INTENT_ENUM}::RenderComponent(..) => self.render_component(intent, agent_name),"),
  ]);
 }
 let tool_error_message_arm=if has_tools{format!("            {INTENT_ENUM}::ToolError(intent) => &intent.message,\n")}else{String::new()};
 let tool_methods=if has_tools{"\n    fn tool_call(&self, _value: &ToolCalls, _agent: &str) {}\n    fn tool_result(&self, _value: &ToolResults, _agent: &str) {}\n    fn tool_error(&self, _value: &ToolErrors, _agent: &str) {}\n    fn tool_skipped(&self, _value: &ToolSkippeds, _agent: &str) {}".to_string()}else{String::new()};
 let workflow_methods=if has_workflows{"\n    fn workflow_call(&self, _value: &WorkflowCalls, _agent: &str) {}\n    fn workflow_result(&self, _value: &WorkflowResults, _agent: &str) {}".to_string()}else{String::new()};
 let helper_methods=if has_helpers{"\n    fn helper_call(&self, _value: &HelperCalls, _agent: &str) {}\n    fn helper_result(&self, _value: &HelperResults, _agent: &str) {}".to_string()}else{String::new()};
 let component_methods=if has_components{format!("\n    fn component(&self, _intent: &{INTENTS_VIEW}, _agent: &str) {{}}\n    fn render_component(&self, _intent: &{INTENTS_VIEW}, _agent: &str) {{}}")}else{String::new()};
 format!("#[derive(Debug, Clone)]\npub struct {INTENTS_VIEW} {{\n    inner: {INTENT_ENUM},\n}}\n\nimpl {INTENTS_VIEW} {{\n    pub fn new(inner: {INTENT_ENUM}) -> Self {{\n        Self {{ inner }}\n    }}\n\n    pub fn raw(&self) -> &{INTENT_ENUM} {{\n        &self.inner\n    }}\n\n    pub fn name(&self) -> &'static str {{\n        match &self.inner {{\n{}\n        }}\n    }}\n\n    pub fn text(&self) -> &str {{\n        match &self.inner {{\n            {INTENT_ENUM}::ResponseText(intent) => &intent.text,\n            _ => panic!(\"intent does not contain text\"),\n        }}\n    }}\n\n    pub fn message(&self) -> &str {{\n        match &self.inner {{\n            {INTENT_ENUM}::Error(intent) => &intent.message,\n{}            _ => panic!(\"intent does not contain a message\"),\n        }}\n    }}\n\n    pub fn response<T>(&self) -> T\n    where\n        T: serde::de::DeserializeOwned,\n    {{\n        let value = match &self.inner {{\n            {INTENT_ENUM}::ResponseSchema(intent) => serde_json::to_value(intent.response.clone()),\n            _ => panic!(\"intent does not contain a response\"),\n        }}.expect(\"response should serialize\");\n        serde_json::from_value(value).expect(\"response should deserialize\")\n    }}\n\n    pub fn value<T>(&self) -> T\n    where\n        T: serde::de::DeserializeOwned,\n    {{\n        let value = match &self.inner {{\n{}\n            _ => panic!(\"intent does not contain a typed value\"),\n        }}.expect(\"intent value should serialize\");\n        serde_json::from_value(value).expect(\"intent value should deserialize\")\n    }}\n}}\n\n{}\npub trait {INTENT_HANDLER_TRAIT}: Send + Sync + 'static {{\n    fn response_text(&self, _value: &{RESPONSE_TEXT_TYPE}, _agent: &str) {{}}\n    fn response_schema(&self, _value: &{RESPONSE_SCHEMA_TYPE}, _agent: &str) {{}}{}{}{}{}\n    fn error(&self, _value: &{ERROR_INTENT_TYPE}, _agent: &str) {{}}\n    fn any(&self, _intent: &{INTENTS_VIEW}, _agent: &str) {{}}\n\n    fn dispatch(&self, intent: &{INTENTS_VIEW}, agent_name: &str) -> Option<IntentControl> {{\n        self.any(intent, agent_name);\n        match intent.raw() {{\n{}\n        }}\n        None\n    }}\n}}\n\npub trait {PARTIAL_HANDLER_TRAIT} {{\n    fn on_intent_partial(&self, intent: {INTENT_PARTIAL_ENUM}, agent_name: &str) {{ let _ = (intent, agent_name); }}\n\n    fn dispatch_partial(&self, intent: {INTENT_PARTIAL_ENUM}, agent_name: &str) {{\n        self.on_intent_partial(intent, agent_name)\n    }}\n}}\n",name_arms.join("\n"),tool_error_message_arm,args_arms.join("\n"),wrapper_defs.join("\n"),tool_methods,workflow_methods,helper_methods,component_methods,dispatch_arms.join("\n"))
}
fn generate_api_keys(agent_name:&str,required_providers:&BTreeSet<String>,custom_provider_ids:&BTreeSet<String>)->String{
 if required_providers.is_empty(){return String::new();}
 let _=agent_name;
 let mut fields=Vec::new();
 let mut init_fields=Vec::new();
 let mut custom_sets=Vec::new();
 if required_providers.contains("openai"){fields.push("    pub openai_api_key: Option<String>,".to_string());init_fields.push("            openai_api_key: value.openai_api_key,".to_string());}
 if required_providers.contains("gemini"){fields.push("    pub gemini_api_key: Option<String>,".to_string());init_fields.push("            gemini_api_key: value.gemini_api_key,".to_string());}
 if required_providers.contains("groq"){fields.push("    pub groq_api_key: Option<String>,".to_string());init_fields.push("            groq_api_key: value.groq_api_key,".to_string());}
 for id in custom_provider_ids{
  let field_name=format!("{}_api_key",to_rust_field_name(&id.replace('-',"_")));
  fields.push(format!("    pub {field_name}: Option<String>,"));
  custom_sets.push(format!("        if let Some(api_key) = value.{field_name} {{\n            custom_api_keys.insert(\"{id}\".to_string(), api_key);\n        }}"));
 }
 let body=if custom_provider_ids.is_empty(){
  format!("        sdk::AuwgentApiKeys {{\n{}\n            ..sdk::AuwgentApiKeys::default()\n        }}",init_fields.join("\n"))
 }else{
  format!("        let mut custom_api_keys = std::collections::HashMap::new();\n{}\n        sdk::AuwgentApiKeys {{\n{}\n            custom_api_keys,\n            ..sdk::AuwgentApiKeys::default()\n        }}",custom_sets.join("\n"),init_fields.join("\n"))
 };
 format!("#[derive(Debug, Clone, Default)]\npub struct {API_KEYS_TYPE} {{\n{}\n}}\n\nimpl From<{API_KEYS_TYPE}> for sdk::AuwgentApiKeys {{\n    fn from(value: {API_KEYS_TYPE}) -> Self {{\n{}\n    }}\n}}\n",fields.join("\n"),body)
}

 fn generate_middleware_trait(agent_name:&str)->String{let _=agent_name;format!("#[async_trait]\npub trait {MIDDLEWARE_TRAIT}: Send + Sync + 'static {{\n    fn name(&self) -> &'static str {{\n        std::any::type_name::<Self>()\n    }}\n\n    fn target(&self) -> Option<Vec<String>> {{\n        None\n    }}\n\n    async fn on_run_start(&self, session: Session, _ctx: &Context) -> Session {{\n        session\n    }}\n\n    async fn on_llm_start(&self, prompt: String, _ctx: &Context) -> String {{\n        prompt\n    }}\n\n    async fn on_intent(&self, _intent: &{INTENTS_VIEW}, _ctx: &Context) -> Option<IntentControl> {{\n        None\n    }}\n\n    async fn on_intent_partial(&self, _intent: &{INTENT_PARTIAL_ENUM}, _ctx: &Context) {{}}\n\n    async fn on_llm_end(&self, _response: &JsonValue, _ctx: &Context) {{}}\n\n    async fn on_run_complete(&self, _session: &Session, _ctx: &Context) {{}}\n\n    async fn on_error(&self, _error: &JsonValue, _session: Option<&Session>, _ctx: &Context) -> bool {{\n        false\n    }}\n}}\n\n#[derive(Clone)]\npub struct {MIDDLEWARE_REGISTRY}(pub sdk::MiddlewareRegistry);\n\nstruct MiddlewareAdapter<T>(T);\n\n#[async_trait]\nimpl<T> sdk::Middleware for MiddlewareAdapter<T>\nwhere\n    T: {MIDDLEWARE_TRAIT},\n{{\n    fn name(&self) -> &'static str {{\n        self.0.name()\n    }}\n\n    fn target(&self) -> Option<Vec<String>> {{\n        self.0.target()\n    }}\n\n    async fn on_run_start(\n        &self,\n        session: SessionState,\n        ctx: &mut sdk::MiddlewareContext,\n    ) -> sdk::AuwgentResult<SessionState> {{\n        Ok(self.0.on_run_start(session, ctx).await)\n    }}\n\n    async fn on_llm_start(\n        &self,\n        prompt: String,\n        ctx: &mut sdk::MiddlewareContext,\n    ) -> sdk::AuwgentResult<Option<String>> {{\n        Ok(Some(self.0.on_llm_start(prompt, ctx).await))\n    }}\n\n    async fn on_intent(\n        &self,\n        name: &str,\n        value: &JsonValue,\n        ctx: &mut sdk::MiddlewareContext,\n    ) -> sdk::AuwgentResult<Option<IntentControl>> {{\n        let Some(intent_name) = {INTENT_NAME_ENUM}::parse(name) else {{\n            return Ok(None);\n        }};\n        let Some(intent) = {INTENT_ENUM}::decode(intent_name, value.clone()) else {{\n            return Ok(None);\n        }};\n        let intent = {INTENTS_VIEW}::new(intent);\n        Ok(self.0.on_intent(&intent, ctx).await)\n    }}\n\n    async fn on_intent_partial(\n        &self,\n        name: &str,\n        value: &JsonValue,\n        ctx: &mut sdk::MiddlewareContext,\n    ) -> sdk::AuwgentResult<()> {{\n        if let Some(intent_name) = {INTENT_NAME_ENUM}::parse(name)\n            && let Some(intent) = {INTENT_PARTIAL_ENUM}::decode(intent_name, value.clone())\n        {{\n            self.0.on_intent_partial(&intent, ctx).await;\n        }}\n        Ok(())\n    }}\n\n    async fn on_llm_end(\n        &self,\n        response: &JsonValue,\n        ctx: &mut sdk::MiddlewareContext,\n    ) -> sdk::AuwgentResult<()> {{\n        self.0.on_llm_end(response, ctx).await;\n        Ok(())\n    }}\n\n    async fn on_run_complete(\n        &self,\n        session: &SessionState,\n        ctx: &mut sdk::MiddlewareContext,\n    ) -> sdk::AuwgentResult<()> {{\n        self.0.on_run_complete(session, ctx).await;\n        Ok(())\n    }}\n\n    async fn on_error(\n        &self,\n        error: &JsonValue,\n        session: Option<&SessionState>,\n        ctx: &mut sdk::MiddlewareContext,\n    ) -> sdk::AuwgentResult<bool> {{\n        Ok(self.0.on_error(error, session, ctx).await)\n    }}\n}}\n\nimpl<T> From<T> for {MIDDLEWARE_REGISTRY}\nwhere\n    T: {MIDDLEWARE_TRAIT},\n{{\n    fn from(value: T) -> Self {{\n        Self(Arc::new(MiddlewareAdapter(value)))\n    }}\n}}\n\nimpl From<sdk::MiddlewareRegistry> for {MIDDLEWARE_REGISTRY} {{\n    fn from(value: sdk::MiddlewareRegistry) -> Self {{\n        Self(value)\n    }}\n}}\n")}

fn generate_config(agent_name:&str,has_tools:bool,has_context:bool,has_api_keys:bool)->String{
 let _=agent_name;
 let mut fields=Vec::new();
 if has_tools{fields.push("    pub tools: TTools,".to_string());}
 fields.push("    pub middleware: Vec<TMiddleware>,".to_string());
 if has_context{fields.push(format!("    pub context: {CONTEXT_TYPE},"));}
 if has_api_keys{fields.push(format!("    pub api_keys: {API_KEYS_TYPE},"));}
 let generics=if has_tools{format!("<TTools = {TOOLS_REGISTRY}, TMiddleware = {MIDDLEWARE_REGISTRY}>")}else{format!("<TMiddleware = {MIDDLEWARE_REGISTRY}>")};
 format!("#[derive(Clone)]\npub struct {CONFIG_TYPE}{generics} {{\n{}\n}}\n",fields.join("\n"))
}

fn generate_agent(agent_name:&str,base_name:&str,has_tools:bool,has_context:bool,has_api_keys:bool)->String{
 let snake_agent_name=to_rust_field_name(agent_name);
 let inner_tools_type=if has_tools{TOOLS_REGISTRY.to_string()}else{"()".to_string()};
 let tools_value=if has_tools{"config.tools.into()".to_string()}else{"()".to_string()};
 let config_type=if has_tools{format!("{CONFIG_TYPE}<TTools, TMiddleware>")}else{format!("{CONFIG_TYPE}<TMiddleware>")};
 let generics=if has_tools{"<TTools, TMiddleware>"}else{"<TMiddleware>"};
 let where_clause=if has_tools{format!("where\n    TTools: Into<{TOOLS_REGISTRY}>,\n    TMiddleware: Into<{MIDDLEWARE_REGISTRY}>,")}else{format!("where\n    TMiddleware: Into<{MIDDLEWARE_REGISTRY}>,")};
 let context_value=if has_context{"Some(serde_json::to_value(config.context).map_err(|e| e.to_string())?)".to_string()}else{"None".to_string()};
 let api_keys_value=if has_api_keys{"config.api_keys.into()".to_string()}else{"sdk::AuwgentApiKeys::default()".to_string()};
 format!("pub struct {AGENT_TYPE} {{\n    inner: sdk::TypedAuwgent<{inner_tools_type}>,\n}}\n\nimpl std::ops::Deref for {AGENT_TYPE} {{\n    type Target = sdk::TypedAuwgent<{inner_tools_type}>;\n\n    fn deref(&self) -> &Self::Target {{\n        &self.inner\n    }}\n}}\n\nimpl {AGENT_TYPE} {{\n    pub fn on_intent<H>(&self, handler: H)\n    where\n        H: {INTENT_HANDLER_TRAIT},\n    {{\n        let handler = Arc::new(handler);\n        self.inner.on_decoded_intent({INTENT_NAME_ENUM}::parse, {INTENT_ENUM}::decode, move |intent, agent_name| {{\n            let intent = {INTENTS_VIEW}::new(intent);\n            handler.dispatch(&intent, agent_name)\n        }});\n    }}\n\n    pub fn on_intent_raw<F>(&self, handler: F)\n    where\n        F: FnMut({INTENT_ENUM}, &str) -> Option<IntentControl> + Send + 'static,\n    {{\n        self.inner.on_decoded_intent({INTENT_NAME_ENUM}::parse, {INTENT_ENUM}::decode, handler);\n    }}\n\n    pub fn on_intent_handler<H>(&self, handler: H)\n    where\n        H: {INTENT_HANDLER_TRAIT},\n    {{\n        self.on_intent(handler);\n    }}\n\n    pub fn on_intent_partial<F>(&self, handler: F)\n    where\n        F: FnMut({INTENT_PARTIAL_ENUM}, &str) + Send + 'static,\n    {{\n        self.inner.on_decoded_intent_partial({INTENT_NAME_ENUM}::parse, {INTENT_PARTIAL_ENUM}::decode, handler);\n    }}\n\n    pub fn on_intent_partial_handler<H>(&self, handler: H)\n    where\n        H: {PARTIAL_HANDLER_TRAIT} + Send + Sync + 'static,\n    {{\n        let handler = Arc::new(handler);\n        self.on_intent_partial(move |intent, agent_name| {{\n            handler.dispatch_partial(intent, agent_name);\n        }});\n    }}\n\n    pub async fn run(&self, input: Option<{INPUT_TYPE}>) -> sdk::AuwgentResult<SessionState> {{\n        let input = input.map(serde_json::to_value).transpose().map_err(|e| e.to_string())?;\n        self.inner.run(input).await\n    }}\n}}\n\npub fn create_{snake_agent_name}{generics}(config: {config_type}) -> sdk::AuwgentResult<{AGENT_TYPE}>\n{where_clause}\n{{\n    let ir = sdk::parse_ir(include_str!(\"./{base_name}.agent.json\"))?;\n    let middleware = config.middleware.into_iter().map(|item| {{\n        let registry: {MIDDLEWARE_REGISTRY} = item.into();\n        registry.0\n    }}).collect();\n    let sdk_config = sdk::AuwgentConfig {{\n        tools: {tools_value},\n        middleware,\n        context: {context_value},\n        api_keys: {api_keys_value},\n    }};\n    let inner = sdk::create_auwgent(ir, sdk_config)?;\n    Ok({AGENT_TYPE} {{ inner }})\n}}\n\npub fn auwgent{generics}(config: {config_type}) -> sdk::AuwgentResult<{AGENT_TYPE}>\n{where_clause}\n{{\n    create_{snake_agent_name}(config)\n}}\n")}

fn generate_aliases(agent_name:&str,has_tools:bool,has_workflows:bool,has_helpers:bool,has_context:bool,has_api_keys:bool)->String{
 let _=(agent_name,has_tools,has_workflows,has_helpers,has_context,has_api_keys);
 String::new()
}

fn unwrap_input_fields(value:Option<&Value>)->Option<Value>{let value=value?;if value.get("kind").and_then(Value::as_str)==Some("properties"){return value.get("fields").cloned();}match value.get("kind").and_then(Value::as_str){Some("direct")=>None,_=>Some(value.clone())}}

fn shape_properties(value:Option<&Value>,unwrap_input_kind:bool)->Option<Map<String,Value>>{
 let value=value?;let obj=value.as_object()?;
 if unwrap_input_kind&&obj.get("kind").and_then(Value::as_str)==Some("properties"){return obj.get("fields").and_then(Value::as_object).cloned();}
 if obj.get("properties").and_then(Value::as_object).is_some(){return obj.get("properties").and_then(Value::as_object).cloned();}
 if obj.get("fields").and_then(Value::as_object).is_some(){return obj.get("fields").and_then(Value::as_object).cloned();}
 if obj.contains_key("__variants"){return None;}
 if !obj.contains_key("type")&&!obj.contains_key("kind"){return Some(obj.clone());}
 if obj.get("type").and_then(Value::as_str)==Some("object"){return obj.get("properties").and_then(Value::as_object).cloned();}
 None
}

fn is_empty_shape(value:Option<&Value>,unwrap_input_kind:bool)->bool{shape_properties(value,unwrap_input_kind).map(|properties|properties.iter().filter(|(name,_)|!name.starts_with('@')&&!name.starts_with("__")).count()==0).unwrap_or(false)}

fn shape_type_or_no_args(name:&str,shape:Option<&Value>,blocks:&mut Vec<String>,emitted_shapes:&mut BTreeSet<String>)->String{
 if is_empty_shape(shape,false){
  "NoArgs".to_string()
 }else{
  if emitted_shapes.insert(name.to_string()){
   blocks.push(generate_named_shape(name,shape));
  }
  name.to_string()
 }
}

fn rust_type(value:Option<&Value>,optional:bool,fallback:&str)->String{let base=rust_type_base(value,fallback);if optional{format!("Option<{base}>")}else{base}}

fn rust_type_base(value:Option<&Value>,fallback:&str)->String{
 let Some(value)=value else{return fallback.to_string();};
 if let Some(properties)=shape_properties(Some(value),false)&&!properties.is_empty(){return "JsonValue".to_string();}
 if let Some(obj)=value.as_object(){
  if let Some(type_name)=obj.get("type").and_then(Value::as_str){return match type_name{"string"=>"String".to_string(),"number"|"int"|"float"=>"f64".to_string(),"boolean"|"bool"=>"bool".to_string(),"array"=>{let item_type=rust_type(obj.get("items"),false,"JsonValue");format!("Vec<{item_type}>")},"typeRef"=>obj.get("name").and_then(Value::as_str).map(to_rust_type_name).unwrap_or_else(||fallback.to_string()),"object"=>"JsonValue".to_string(),_=>fallback.to_string(),};}
  if let Some(type_obj)=obj.get("type").and_then(Value::as_object)&&let Some(type_name)=type_obj.get("type").and_then(Value::as_str){return match type_name{"typeRef"=>type_obj.get("name").and_then(Value::as_str).map(to_rust_type_name).unwrap_or_else(||fallback.to_string()),"array"=>{let item_type=rust_type(type_obj.get("items"),false,"JsonValue");format!("Vec<{item_type}>")},"object"=>"JsonValue".to_string(),_=>fallback.to_string(),};}
 }
 match value{Value::String(_)=>"String".to_string(),Value::Number(_)=>"f64".to_string(),Value::Bool(_)=>"bool".to_string(),Value::Array(_)=>"Vec<JsonValue>".to_string(),Value::Object(_)=>"JsonValue".to_string(),Value::Null=>fallback.to_string(),}
}

fn to_rust_type_name(name:&str)->String{let mut out=String::new();let mut uppercase_next=true;for ch in name.chars(){if ch.is_ascii_alphanumeric(){if uppercase_next{out.extend(ch.to_uppercase());uppercase_next=false;}else{out.extend(ch.to_lowercase());}}else{uppercase_next=true;}}if out.is_empty(){"Unknown".to_string()}else{out}}

fn to_rust_field_name(name:&str)->String{let mut out=String::new();let mut underscore=false;for ch in name.chars(){if ch.is_ascii_alphanumeric(){if underscore&&!out.is_empty(){out.push('_');}out.extend(ch.to_lowercase());underscore=false;}else{underscore=true;}}if out.is_empty(){"value".to_string()}else{out}}

#[cfg(test)]
mod tests {
 use super::*;use crate::generation_plan::CodegenPlan;use serde_json::json;
 #[test]
 fn emits_typed_intents_and_conditional_config(){let ir=json!({"name":"Hello","context":{"user_id":{"type":"string","optional":false}},"tools":[{"name":"get_details","params":{},"returns":{"type":"typeRef","name":"Person"}},{"name":"get_location","params":{"id":{"type":"string","optional":false}},"returns":{"type":"string"}}],"helpers":[{"name":"Joker","input":null,"output":null,"customIntents":[{"name":"helper_prompt","fields":{"message":{"type":"string","optional":false}}}]}],"output":{"__variants":{"Output":{"name":{"type":"string","optional":false}},"Fallback":{"message":{"type":"string","optional":false}}}},"types":{"Person":{"properties":{"name":{"type":"string","optional":false},"age":{"type":"number","optional":false}}}},"customIntents":[{"name":"ask_user","fields":{"question":{"type":"string","optional":false}}}],"modelConfig":[{"defaultConfig":{"model":{"type":"openai","modelName":"gpt-4.1"}}}]});let output=generate(&CodegenPlan::new(ir),"hello");assert!(output.contains("pub enum AuwgentIntent"));assert!(output.contains("pub enum AuwgentIntentPartial"));assert!(output.contains("impl AuwgentIntentName"));assert!(output.contains("pub fn parse(name: &str) -> Option<Self>"));assert!(output.contains("impl AuwgentIntent {"));assert!(output.contains("pub fn decode(name: AuwgentIntentName, value: JsonValue) -> Option<Self>"));assert!(output.contains("impl AuwgentIntentPartial {"));assert!(output.contains("pub struct Intents"));assert!(output.contains("pub trait AuwgentIntentHandler"));assert!(output.contains("pub fn on_intent<H>(&self, handler: H)"));assert!(output.contains("pub fn on_intent_raw<F>(&self, handler: F)"));assert!(output.contains("on_decoded_intent(AuwgentIntentName::parse, AuwgentIntent::decode"));assert!(output.contains("on_decoded_intent_partial(AuwgentIntentName::parse, AuwgentIntentPartial::decode"));assert!(output.contains("fn tool_names(&self) -> &'static [&'static str]"));assert!(output.contains("fn invoke_tool("));assert!(output.contains("pub trait AuwgentTools: Send + Sync + 'static"));assert!(output.contains("impl<T> From<T> for AuwgentToolsRegistry"));assert!(!output.contains("impl AuwgentToolsRegistry"));assert!(output.contains("pub struct AuwgentConfig<TTools = AuwgentToolsRegistry, TMiddleware = AuwgentMiddlewareRegistry>"));assert!(output.contains("pub tools: TTools,"));assert!(output.contains("pub middleware: Vec<TMiddleware>,"));assert!(output.contains("let registry: AuwgentMiddlewareRegistry = item.into();"));assert!(output.contains("tools: config.tools.into(),"));assert!(output.contains("pub trait AuwgentMiddleware: Send + Sync + 'static"));assert!(!output.contains("pub use "));assert!(output.contains("pub struct HelperPromptIntent"));assert!(output.contains("pub struct AskUserIntent"));assert!(!output.contains("use std::sync::{Arc, Mutex};"));}
 #[test]
 fn omits_conditional_fields_when_not_needed(){let ir=json!({"name":"Mini","tools":[],"helpers":[],"components":[],"modelConfig":[]});let output=generate(&CodegenPlan::new(ir),"mini");assert!(output.contains("pub struct AuwgentConfig<TMiddleware = AuwgentMiddlewareRegistry>"));assert!(output.contains("pub middleware: Vec<TMiddleware>,"));assert!(!output.contains("pub tools: TTools,"));assert!(!output.contains("pub context: AuwgentContext,"));assert!(!output.contains("pub api_keys: AuwgentApiKeys,"));assert!(output.contains("pub trait AuwgentIntentHandler"));assert!(output.contains("pub fn on_intent_raw<F>(&self, handler: F)"));assert!(output.contains("tools: (),"));assert!(output.contains("context: None,"));assert!(output.contains("api_keys: sdk::AuwgentApiKeys::default()"));}
}
