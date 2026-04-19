use crate::common::{join_sections,string_at};
use crate::generation_plan::CodegenPlan;
use serde_json::{Map,Value};
use std::collections::BTreeSet;

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
 let mut sections=vec![format!("// Auto-generated Rust bindings for {agent_name}"),"// Do not edit manually".to_string(),String::new(),"use auwgent_sdk_rust as sdk;".to_string(),"use serde_json::{Map as JsonMap, Value as JsonValue};".to_string(),"use std::marker::PhantomData;".to_string(),"use std::sync::{Arc, Mutex};".to_string(),String::new(),generate_runtime_support()];
 if let Some(types)=ir.get("types").and_then(Value::as_object){sections.push(generate_custom_types(types));}
 sections.push(generate_named_shape(&format!("{agent_name}Input"),unwrap_input_fields(ir.get("input")).as_ref()));
 for helper in output_helpers{sections.push(generate_helper_output_type(helper));}
 sections.push(generate_output_type(agent_name,ir.get("output"),output_helpers));
 if has_context{sections.push(generate_named_shape(&format!("{agent_name}Context"),ir.get("context")));}
 if has_tools{sections.push(generate_tools(agent_name,all_tools));}
 sections.push(generate_intent_name_enum(agent_name,has_tools,has_workflows,has_helpers,has_components,custom_intents));
 sections.push(generate_custom_intent_types(agent_name,plan));
 sections.push(generate_core_intents(agent_name,ir.get("output")));
 if has_tools{sections.push(generate_callable_family(agent_name,"Tool",all_tools,"name","params","returns",true));}
 if has_workflows{sections.push(generate_callable_family(agent_name,"Workflow",workflows,"flowName","flowParams","returns",false));}
 if has_helpers{sections.push(generate_callable_family(agent_name,"Helper",helpers,"name","input","output",false));}
 if has_components{sections.push(generate_component_intents(agent_name));}
 sections.push(generate_top_level_intent_enums(agent_name,has_tools,has_workflows,has_helpers,has_components,custom_intents));
 sections.push(generate_decode_functions(agent_name,has_tools,has_workflows,has_helpers,has_components,custom_intents));
 sections.push(generate_handler_traits(agent_name));
 sections.push(generate_api_keys(agent_name,required_providers,custom_provider_ids));
 sections.push(generate_middleware_trait(agent_name));
 sections.push(generate_config(agent_name,has_tools,has_context,plan.has_api_keys()));
 sections.push(generate_agent(agent_name,base_name,has_tools,has_context,plan.has_api_keys()));
 sections.push(generate_aliases(agent_name,has_tools,has_context,plan.has_api_keys()));
 join_sections(&sections)
}

fn generate_runtime_support()->String{[
"pub type IntentControl = sdk::IntentControl;".to_string(),
"pub type SessionState = sdk::SessionState;".to_string(),
"#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]\npub struct NoArgs {}\n".to_string(),
"#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]\npub struct PartialTextIntentValue {\n    #[serde(flatten)]\n    pub raw: JsonMap<String, JsonValue>,\n}\n".to_string(),
"#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]\npub struct PartialStructuredIntentValue<T> {\n    #[serde(flatten)]\n    pub raw: JsonMap<String, JsonValue>,\n    #[serde(skip)]\n    pub marker: PhantomData<T>,\n}\n".to_string(),
].join("\n")}

fn generate_custom_types(types:&Map<String,Value>)->String{let mut blocks=Vec::new();for(type_name,type_def)in types{blocks.push(generate_named_shape(type_name,Some(type_def)));}blocks.join("\n")}

fn generate_helper_output_type(helper:&Value)->String{
 let helper_name=string_at(helper,&["name"]).unwrap_or("Helper");
 generate_named_shape(&format!("{helper_name}Output"),helper.get("output"))
}

fn generate_output_type(agent_name:&str,value:Option<&Value>,output_helpers:&[Value])->String{
 let Some(value)=value else{
  if output_helpers.is_empty(){return format!("pub type {agent_name}Output = JsonValue;\n");}
  let base_name=format!("{agent_name}BaseOutput");
  let mut enum_variants=vec![format!("    Base({base_name}),")];
  for helper in output_helpers{
   if let Some(helper_name)=string_at(helper,&["name"]){
    let helper_output=format!("{}Output",helper_name);
    enum_variants.push(format!("    {}({helper_output}),",to_rust_type_name(helper_name)));
   }
  }
  return format!("#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]\npub struct {base_name};\n\n#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n#[serde(untagged)]\npub enum {agent_name}Output {{\n{}\n}}\n",enum_variants.join("\n"));
 };
 if let Some(variants)=value.get("__variants").and_then(Value::as_object){
  let mut blocks=Vec::new();let mut enum_variants=Vec::new();
  for(variant_name,variant_props)in variants{
   let case_name=format!("{agent_name}{}OutputCase",to_rust_type_name(variant_name));
   let props=variant_props.as_object().cloned().unwrap_or_default();
   blocks.push(generate_struct(&case_name,&props));
   enum_variants.push(format!("    #[serde(rename = \"{variant_name}\")]\n    {}({case_name}),",to_rust_type_name(variant_name)));
  }
  blocks.push(format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n#[serde(tag = \"type\")]\npub enum {agent_name}Output {{\n{}\n}}\n",enum_variants.join("\n")));
  return blocks.join("\n");
 }
 if output_helpers.is_empty(){return generate_named_shape(&format!("{agent_name}Output"),Some(value));}
 let mut blocks=Vec::new();
 let base_name=format!("{agent_name}BaseOutput");
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
 blocks.push(format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n#[serde(untagged)]\npub enum {agent_name}Output {{\n{}\n}}\n",enum_variants.join("\n")));
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

fn generate_tools(agent_name:&str,tools:&[Value])->String{
 let mut result_aliases=Vec::new();let mut methods=Vec::new();let mut registrations=Vec::new();
 for tool in tools{
  let Some(tool_name)=string_at(tool,&["name"]) else{continue;};
  let pascal=to_rust_type_name(tool_name);let method_name=to_rust_field_name(tool_name);
  let args_type=shape_type_or_no_args(&format!("{agent_name}{pascal}ToolArgs"),tool.get("params"),&mut result_aliases);
  let result_alias=format!("{agent_name}{pascal}ToolResultValue");
  result_aliases.push(format!("pub type {result_alias} = {};\n",rust_type(tool.get("returns"),false,"()")));
  methods.push(format!("    fn {method_name}(&self, args: {args_type}) -> {result_alias};"));
  registrations.push(format!("        let tools = Arc::clone(self);\n        native.register_tool_fn(\"{tool_name}\", move |args| {{\n            let tools = Arc::clone(&tools);\n            Box::pin(async move {{\n                let parsed: {args_type} = serde_json::from_value(args).map_err(|e| e.to_string())?;\n                let result = tools.{method_name}(parsed);\n                serde_json::to_value(result).map_err(|e| e.to_string())\n            }})\n        }});"));
 }
 format!("{}\npub trait {agent_name}Tools: Send + Sync {{\n{}\n}}\n\npub type {agent_name}ToolsRegistry = Arc<dyn {agent_name}Tools>;\n\nimpl sdk::ToolRegistrar for {agent_name}ToolsRegistry {{\n    fn register_tools(&self, native: &sdk::AuwgentNative) -> sdk::AuwgentResult<()> {{\n{}\n        Ok(())\n    }}\n}}\n",result_aliases.join("\n"),methods.join("\n"),registrations.join("\n"))
}

fn generate_intent_name_enum(agent_name:&str,has_tools:bool,has_workflows:bool,has_helpers:bool,has_components:bool,custom_intents:&[String])->String{
 let mut names=vec!["ResponseText".to_string(),"ResponseSchema".to_string(),"Error".to_string()];
 if has_tools{names.extend(["ToolCall".to_string(),"ToolResult".to_string(),"ToolError".to_string(),"ToolSkipped".to_string()]);}
 if has_workflows{names.extend(["WorkflowCall".to_string(),"WorkflowResult".to_string()]);}
 if has_helpers{names.extend(["HelperCall".to_string(),"HelperResult".to_string()]);}
 if has_components{names.extend(["Component".to_string(),"RenderComponent".to_string()]);}
 for custom_intent in custom_intents{names.push(to_rust_type_name(custom_intent));}
 let variants=names.iter().map(|name|format!("    {name},")).collect::<Vec<_>>().join("\n");
 format!("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum {agent_name}IntentName {{\n{variants}\n}}\n")
}

fn generate_custom_intent_types(agent_name:&str,plan:&CodegenPlan)->String{
 let mut blocks=Vec::new();
 for (name,item) in plan.custom_intent_defs(){blocks.push(generate_named_shape(&format!("{agent_name}{}Intent",to_rust_type_name(name)),item.get("fields")));}
 blocks.join("\n")
}

fn generate_core_intents(agent_name:&str,output:Option<&Value>)->String{
 let response_schema=if let Some(variants)=output.and_then(|value|value.get("__variants")).and_then(Value::as_object){
  let enum_variants=variants.keys().map(|variant_name|{let case_name=format!("{agent_name}{}OutputCase",to_rust_type_name(variant_name));format!("    #[serde(rename = \"{variant_name}\")]\n    {}({case_name}),",to_rust_type_name(variant_name))}).collect::<Vec<_>>().join("\n");
  format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n#[serde(tag = \"type\", content = \"response\")]\npub enum {agent_name}ResponseSchemaIntent {{\n{enum_variants}\n}}\n")
 }else{format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub struct {agent_name}ResponseSchemaIntent {{\n    #[serde(rename = \"type\")]\n    pub kind: String,\n    pub response: {agent_name}Output,\n}}\n")};
 [format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub struct {agent_name}ResponseTextIntent {{\n    pub text: String,\n}}\n"),response_schema,format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub struct {agent_name}ErrorIntent {{\n    pub message: String,\n}}\n")].join("\n")
}
fn generate_callable_family(agent_name:&str,family_name:&str,items:&[Value],name_key:&str,args_key:&str,result_key:&str,include_error_and_skipped:bool)->String{
 if items.is_empty(){return String::new();}
 let mut blocks=Vec::new();let mut call_variants=Vec::new();let mut result_variants=Vec::new();let mut skipped_variants=Vec::new();
 for item in items{
  let Some(item_name)=string_at(item,&[name_key]) else{continue;};
  let pascal=to_rust_type_name(item_name);
  let args_type=shape_type_or_no_args(&format!("{agent_name}{pascal}{family_name}Args"),item.get(args_key),&mut blocks);
  let result_type=rust_type(item.get(result_key),false,"()");
  call_variants.push(enum_struct_variant(item_name,&pascal,"args",&args_type));
  result_variants.push(format!("    #[serde(rename = \"{item_name}\")]\n    {pascal} {{\n        args: {args_type},\n        result: {result_type},\n        #[serde(default)]\n        overridden: bool,\n    }},"));
  if include_error_and_skipped{skipped_variants.push(enum_struct_variant(item_name,&pascal,"args",&args_type));}
 }
 blocks.push(format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n#[serde(tag = \"type\")]\npub enum {agent_name}{family_name}CallIntent {{\n{}\n}}\n",call_variants.join("\n")));
 blocks.push(format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n#[serde(tag = \"name\")]\npub enum {agent_name}{family_name}ResultIntent {{\n{}\n}}\n",result_variants.join("\n")));
 if include_error_and_skipped{
  blocks.push(format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n#[serde(tag = \"type\")]\npub enum {agent_name}{family_name}SkippedIntent {{\n{}\n}}\n",skipped_variants.join("\n")));
  blocks.push(format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub struct {agent_name}{family_name}ErrorIntent {{\n    pub tool: String,\n    pub message: String,\n}}\n"));
 }
 blocks.join("\n")
}

fn enum_struct_variant(item_name:&str,pascal:&str,field_name:&str,field_type:&str)->String{if field_type=="NoArgs"{format!("    #[serde(rename = \"{item_name}\")]\n    {pascal},")}else{format!("    #[serde(rename = \"{item_name}\")]\n    {pascal} {{\n        {field_name}: {field_type},\n    }},")}}

fn generate_component_intents(agent_name:&str)->String{[format!("pub type {agent_name}ComponentIntent = JsonValue;\n"),format!("pub type {agent_name}RenderComponentIntent = JsonValue;\n")].join("\n")}

fn generate_top_level_intent_enums(agent_name:&str,has_tools:bool,has_workflows:bool,has_helpers:bool,has_components:bool,custom_intents:&[String])->String{
 let mut intent_variants=vec![format!("    ResponseText({agent_name}ResponseTextIntent),"),format!("    ResponseSchema({agent_name}ResponseSchemaIntent),"),format!("    Error({agent_name}ErrorIntent),")];
 let mut partial_variants=vec!["    ResponseText(PartialTextIntentValue),".to_string(),format!("    ResponseSchema(PartialStructuredIntentValue<{agent_name}ResponseSchemaIntent>),"),format!("    Error(PartialStructuredIntentValue<{agent_name}ErrorIntent>),")];
 if has_tools{
  intent_variants.extend([format!("    ToolCall({agent_name}ToolCallIntent),"),format!("    ToolResult({agent_name}ToolResultIntent),"),format!("    ToolError({agent_name}ToolErrorIntent),"),format!("    ToolSkipped({agent_name}ToolSkippedIntent),")]);
  partial_variants.extend([format!("    ToolCall(PartialStructuredIntentValue<{agent_name}ToolCallIntent>),"),format!("    ToolResult(PartialStructuredIntentValue<{agent_name}ToolResultIntent>),"),format!("    ToolError(PartialStructuredIntentValue<{agent_name}ToolErrorIntent>),"),format!("    ToolSkipped(PartialStructuredIntentValue<{agent_name}ToolSkippedIntent>),")]);
 }
 if has_workflows{
  intent_variants.extend([format!("    WorkflowCall({agent_name}WorkflowCallIntent),"),format!("    WorkflowResult({agent_name}WorkflowResultIntent),")]);
  partial_variants.extend([format!("    WorkflowCall(PartialStructuredIntentValue<{agent_name}WorkflowCallIntent>),"),format!("    WorkflowResult(PartialStructuredIntentValue<{agent_name}WorkflowResultIntent>),")]);
 }
 if has_helpers{
  intent_variants.extend([format!("    HelperCall({agent_name}HelperCallIntent),"),format!("    HelperResult({agent_name}HelperResultIntent),")]);
  partial_variants.extend([format!("    HelperCall(PartialStructuredIntentValue<{agent_name}HelperCallIntent>),"),format!("    HelperResult(PartialStructuredIntentValue<{agent_name}HelperResultIntent>),")]);
 }
 if has_components{
  intent_variants.extend([format!("    Component({agent_name}ComponentIntent),"),format!("    RenderComponent({agent_name}RenderComponentIntent),")]);
  partial_variants.extend([format!("    Component(PartialStructuredIntentValue<{agent_name}ComponentIntent>),"),format!("    RenderComponent(PartialStructuredIntentValue<{agent_name}RenderComponentIntent>),")]);
 }
 for custom_intent in custom_intents{let pascal=to_rust_type_name(custom_intent);let type_name=format!("{agent_name}{pascal}Intent");intent_variants.push(format!("    {pascal}({type_name}),"));partial_variants.push(format!("    {pascal}(PartialStructuredIntentValue<{type_name}>),"));}
 format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub enum {agent_name}Intent {{\n{}\n}}\n\n#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub enum {agent_name}IntentPartial {{\n{}\n}}\n",intent_variants.join("\n"),partial_variants.join("\n"))
}

fn generate_decode_functions(agent_name:&str,has_tools:bool,has_workflows:bool,has_helpers:bool,has_components:bool,custom_intents:&[String])->String{
 let mut name_cases=vec![format!("        \"response_text\" => Some({agent_name}IntentName::ResponseText),"),format!("        \"response_schema\" => Some({agent_name}IntentName::ResponseSchema),"),format!("        \"error\" => Some({agent_name}IntentName::Error),")];
 let mut intent_cases=vec![format!("        {agent_name}IntentName::ResponseText => serde_json::from_value(value).ok().map({agent_name}Intent::ResponseText),"),format!("        {agent_name}IntentName::ResponseSchema => serde_json::from_value(value).ok().map({agent_name}Intent::ResponseSchema),"),format!("        {agent_name}IntentName::Error => serde_json::from_value(value).ok().map({agent_name}Intent::Error),")];
 let mut partial_cases=vec![format!("        {agent_name}IntentName::ResponseText => serde_json::from_value(value).ok().map({agent_name}IntentPartial::ResponseText),"),format!("        {agent_name}IntentName::ResponseSchema => serde_json::from_value(value).ok().map({agent_name}IntentPartial::ResponseSchema),"),format!("        {agent_name}IntentName::Error => serde_json::from_value(value).ok().map({agent_name}IntentPartial::Error),")];
 if has_tools{
  name_cases.extend([format!("        \"tool_call\" => Some({agent_name}IntentName::ToolCall),"),format!("        \"tool_result\" => Some({agent_name}IntentName::ToolResult),"),format!("        \"tool_error\" => Some({agent_name}IntentName::ToolError),"),format!("        \"tool_skipped\" => Some({agent_name}IntentName::ToolSkipped),")]);
  intent_cases.extend([format!("        {agent_name}IntentName::ToolCall => serde_json::from_value(value).ok().map({agent_name}Intent::ToolCall),"),format!("        {agent_name}IntentName::ToolResult => serde_json::from_value(value).ok().map({agent_name}Intent::ToolResult),"),format!("        {agent_name}IntentName::ToolError => serde_json::from_value(value).ok().map({agent_name}Intent::ToolError),"),format!("        {agent_name}IntentName::ToolSkipped => serde_json::from_value(value).ok().map({agent_name}Intent::ToolSkipped),")]);
  partial_cases.extend([format!("        {agent_name}IntentName::ToolCall => serde_json::from_value(value).ok().map({agent_name}IntentPartial::ToolCall),"),format!("        {agent_name}IntentName::ToolResult => serde_json::from_value(value).ok().map({agent_name}IntentPartial::ToolResult),"),format!("        {agent_name}IntentName::ToolError => serde_json::from_value(value).ok().map({agent_name}IntentPartial::ToolError),"),format!("        {agent_name}IntentName::ToolSkipped => serde_json::from_value(value).ok().map({agent_name}IntentPartial::ToolSkipped),")]);
 }
 if has_workflows{
  name_cases.extend([format!("        \"workflow_call\" => Some({agent_name}IntentName::WorkflowCall),"),format!("        \"workflow_result\" => Some({agent_name}IntentName::WorkflowResult),")]);
  intent_cases.extend([format!("        {agent_name}IntentName::WorkflowCall => serde_json::from_value(value).ok().map({agent_name}Intent::WorkflowCall),"),format!("        {agent_name}IntentName::WorkflowResult => serde_json::from_value(value).ok().map({agent_name}Intent::WorkflowResult),")]);
  partial_cases.extend([format!("        {agent_name}IntentName::WorkflowCall => serde_json::from_value(value).ok().map({agent_name}IntentPartial::WorkflowCall),"),format!("        {agent_name}IntentName::WorkflowResult => serde_json::from_value(value).ok().map({agent_name}IntentPartial::WorkflowResult),")]);
 }
 if has_helpers{
  name_cases.extend([format!("        \"helper_call\" => Some({agent_name}IntentName::HelperCall),"),format!("        \"helper_result\" => Some({agent_name}IntentName::HelperResult),")]);
  intent_cases.extend([format!("        {agent_name}IntentName::HelperCall => serde_json::from_value(value).ok().map({agent_name}Intent::HelperCall),"),format!("        {agent_name}IntentName::HelperResult => serde_json::from_value(value).ok().map({agent_name}Intent::HelperResult),")]);
  partial_cases.extend([format!("        {agent_name}IntentName::HelperCall => serde_json::from_value(value).ok().map({agent_name}IntentPartial::HelperCall),"),format!("        {agent_name}IntentName::HelperResult => serde_json::from_value(value).ok().map({agent_name}IntentPartial::HelperResult),")]);
 }
 if has_components{
  name_cases.extend([format!("        \"component\" => Some({agent_name}IntentName::Component),"),format!("        \"render_component\" => Some({agent_name}IntentName::RenderComponent),")]);
  intent_cases.extend([format!("        {agent_name}IntentName::Component => Some({agent_name}Intent::Component(value)),"),format!("        {agent_name}IntentName::RenderComponent => Some({agent_name}Intent::RenderComponent(value)),")]);
  partial_cases.extend([format!("        {agent_name}IntentName::Component => serde_json::from_value(value).ok().map({agent_name}IntentPartial::Component),"),format!("        {agent_name}IntentName::RenderComponent => serde_json::from_value(value).ok().map({agent_name}IntentPartial::RenderComponent),")]);
 }
 for custom_intent in custom_intents{let pascal=to_rust_type_name(custom_intent);name_cases.push(format!("        \"{custom_intent}\" => Some({agent_name}IntentName::{pascal}),"));intent_cases.push(format!("        {agent_name}IntentName::{pascal} => serde_json::from_value(value).ok().map({agent_name}Intent::{pascal}),"));partial_cases.push(format!("        {agent_name}IntentName::{pascal} => serde_json::from_value(value).ok().map({agent_name}IntentPartial::{pascal}),"));}
 format!("pub fn parse_intent_name(name: &str) -> Option<{agent_name}IntentName> {{\n    match name {{\n{}\n        _ => None,\n    }}\n}}\n\npub fn decode_intent(name: {agent_name}IntentName, value: JsonValue) -> Option<{agent_name}Intent> {{\n    match name {{\n{}\n    }}\n}}\n\npub fn decode_intent_partial(name: {agent_name}IntentName, value: JsonValue) -> Option<{agent_name}IntentPartial> {{\n    match name {{\n{}\n    }}\n}}\n",name_cases.join("\n"),intent_cases.join("\n"),partial_cases.join("\n"))
}

fn generate_handler_traits(agent_name:&str)->String{format!("pub trait {agent_name}BaseIntentHandler {{\n    fn on_intent(&mut self, intent: {agent_name}Intent, agent_name: &str) -> Option<IntentControl> {{ let _ = (intent, agent_name); None }}\n}}\n\npub trait {agent_name}BasePartialIntentHandler {{\n    fn on_intent_partial(&mut self, intent: {agent_name}IntentPartial, agent_name: &str) {{ let _ = (intent, agent_name); }}\n}}\n\npub fn dispatch_intent<H: {agent_name}BaseIntentHandler>(handler: &mut H, intent: {agent_name}Intent, agent_name: &str) -> Option<IntentControl> {{\n    handler.on_intent(intent, agent_name)\n}}\n\npub fn dispatch_partial_intent<H: {agent_name}BasePartialIntentHandler>(handler: &mut H, intent: {agent_name}IntentPartial, agent_name: &str) {{\n    handler.on_intent_partial(intent, agent_name)\n}}\n")}
fn generate_api_keys(agent_name:&str,required_providers:&BTreeSet<String>,custom_provider_ids:&BTreeSet<String>)->String{
 if required_providers.is_empty(){return String::new();}
 let mut fields=Vec::new();
 let mut direct_sets=vec!["            openai_api_key: None,".to_string(),"            gemini_api_key: None,".to_string(),"            groq_api_key: None,".to_string()];
 let mut custom_sets=Vec::new();
 if required_providers.contains("openai"){fields.push("    pub openai_api_key: Option<String>,".to_string());direct_sets[0]="            openai_api_key: value.openai_api_key,".to_string();}
 if required_providers.contains("gemini"){fields.push("    pub gemini_api_key: Option<String>,".to_string());direct_sets[1]="            gemini_api_key: value.gemini_api_key,".to_string();}
 if required_providers.contains("groq"){fields.push("    pub groq_api_key: Option<String>,".to_string());direct_sets[2]="            groq_api_key: value.groq_api_key,".to_string();}
 for id in custom_provider_ids{let field_name=format!("{}_api_key",to_rust_field_name(&id.replace('-',"_")));fields.push(format!("    pub {field_name}: Option<String>,"));custom_sets.push(format!("        if let Some(api_key) = value.{field_name} {{\n            custom_api_keys.insert(\"{id}\".to_string(), api_key);\n        }}"));}
 format!("#[derive(Debug, Clone, Default)]\npub struct {agent_name}ApiKeys {{\n{}\n}}\n\nimpl From<{agent_name}ApiKeys> for sdk::AuwgentApiKeys {{\n    fn from(value: {agent_name}ApiKeys) -> Self {{\n        let mut custom_api_keys = std::collections::HashMap::new();\n{}\n        Self {{\n{}\n            custom_api_keys,\n        }}\n    }}\n}}\n",fields.join("\n"),custom_sets.join("\n"),direct_sets.join("\n"))
}

fn generate_middleware_trait(agent_name:&str)->String{format!("pub use sdk::MiddlewareContext;\n\npub trait {agent_name}Middleware: sdk::Middleware {{}}\n\nimpl<T> {agent_name}Middleware for T where T: sdk::Middleware + ?Sized {{}}\n\npub type {agent_name}MiddlewareRegistry = sdk::MiddlewareRegistry;\n")}

fn generate_config(agent_name:&str,has_tools:bool,has_context:bool,has_api_keys:bool)->String{
 let mut fields=Vec::new();
 if has_tools{fields.push(format!("    pub tools: {agent_name}ToolsRegistry,"));}
 fields.push(format!("    pub middleware: Vec<{agent_name}MiddlewareRegistry>,"));
 if has_context{fields.push(format!("    pub context: {agent_name}Context,"));}
 if has_api_keys{fields.push(format!("    pub api_keys: {agent_name}ApiKeys,"));}
 let tools_registry=if has_tools{String::new()}else{format!("pub type {agent_name}ToolsRegistry = ();")};
 format!("{tools_registry}\n#[derive(Clone)]\npub struct {agent_name}Config {{\n{}\n}}\n",fields.join("\n"))
}

fn generate_agent(agent_name:&str,base_name:&str,has_tools:bool,has_context:bool,has_api_keys:bool)->String{
 let snake_agent_name=to_rust_field_name(agent_name);
 let inner_tools_type=if has_tools{format!("{agent_name}ToolsRegistry")}else{"()".to_string()};
 let tools_value=if has_tools{"config.tools".to_string()}else{"()".to_string()};
 let context_value=if has_context{"Some(serde_json::to_value(config.context).map_err(|e| e.to_string())?)".to_string()}else{"None".to_string()};
 let api_keys_value=if has_api_keys{"config.api_keys.into()".to_string()}else{"sdk::AuwgentApiKeys::default()".to_string()};
 format!("pub struct {agent_name}Agent {{\n    inner: sdk::TypedAuwgent<{inner_tools_type}>,\n}}\n\nimpl {agent_name}Agent {{\n    pub fn on_intent<F>(&self, handler: F)\n    where\n        F: FnMut({agent_name}Intent, &str) -> Option<IntentControl> + Send + 'static,\n    {{\n        let handler = Arc::new(Mutex::new(handler));\n        self.inner.on_intent(move |name, value, agent_name| {{\n            let handler = Arc::clone(&handler);\n            Box::pin(async move {{\n                let intent_name = parse_intent_name(&name)?;\n                let intent = decode_intent(intent_name, value)?;\n                let mut handler = handler.lock().ok()?;\n                (*handler)(intent, &agent_name)\n            }})\n        }});\n    }}\n\n    pub fn on_intent_partial<F>(&self, handler: F)\n    where\n        F: FnMut({agent_name}IntentPartial, &str) + Send + 'static,\n    {{\n        let handler = Arc::new(Mutex::new(handler));\n        self.inner.on_intent_partial(move |name, value, agent_name| {{\n            if let Some(intent_name) = parse_intent_name(&name)\n                && let Some(intent) = decode_intent_partial(intent_name, value)\n                && let Ok(mut handler) = handler.lock()\n            {{\n                (*handler)(intent, &agent_name);\n            }}\n        }});\n    }}\n\n    pub fn on_intent_handler<H>(&self, handler: H)\n    where\n        H: {agent_name}BaseIntentHandler + Send + 'static,\n    {{\n        let handler = Arc::new(Mutex::new(handler));\n        self.on_intent(move |intent, agent_name| {{\n            let mut handler = handler.lock().ok()?;\n            dispatch_intent(&mut *handler, intent, agent_name)\n        }});\n    }}\n\n    pub fn on_intent_partial_handler<H>(&self, handler: H)\n    where\n        H: {agent_name}BasePartialIntentHandler + Send + 'static,\n    {{\n        let handler = Arc::new(Mutex::new(handler));\n        self.on_intent_partial(move |intent, agent_name| {{\n            if let Ok(mut handler) = handler.lock() {{\n                dispatch_partial_intent(&mut *handler, intent, agent_name);\n            }}\n        }});\n    }}\n\n    pub async fn run(&self, input: Option<{agent_name}Input>) -> sdk::AuwgentResult<SessionState> {{\n        let input = input.map(serde_json::to_value).transpose().map_err(|e| e.to_string())?;\n        self.inner.run(input).await\n    }}\n\n    pub fn export_session(&self) -> sdk::AuwgentResult<SessionState> {{\n        self.inner.export_session()\n    }}\n\n    pub fn import_session(&self, session: &SessionState) -> sdk::AuwgentResult<()> {{\n        self.inner.import_session(session)\n    }}\n\n    pub fn clear_session(&self) {{\n        self.inner.clear_session();\n    }}\n\n    pub fn get_metadata(&self) -> sdk::AuwgentResult<sdk::RunMetadata> {{\n        self.inner.get_metadata()\n    }}\n\n    pub fn raw(&self) -> &sdk::TypedAuwgent<{inner_tools_type}> {{\n        &self.inner\n    }}\n}}\n\npub fn create_{snake_agent_name}(config: {agent_name}Config) -> sdk::AuwgentResult<{agent_name}Agent> {{\n    let ir = sdk::parse_ir(include_str!(\"./{base_name}.agent.json\"))?;\n    let sdk_config = sdk::AuwgentConfig {{\n        tools: {tools_value},\n        middleware: config.middleware,\n        context: {context_value},\n        api_keys: {api_keys_value},\n    }};\n    let inner = sdk::create_auwgent(ir, sdk_config)?;\n    Ok({agent_name}Agent {{ inner }})\n}}\n\npub fn auwgent(config: {agent_name}Config) -> sdk::AuwgentResult<{agent_name}Agent> {{\n    create_{snake_agent_name}(config)\n}}\n")}

fn generate_aliases(agent_name:&str,has_tools:bool,has_context:bool,has_api_keys:bool)->String{
 let mut aliases=vec![format!("pub use {agent_name}Agent as AuwgentAgent;"),format!("pub use {agent_name}Config as AuwgentConfig;"),format!("pub use {agent_name}Intent as AuwgentIntent;"),format!("pub use {agent_name}IntentPartial as AuwgentIntentPartial;"),format!("pub use {agent_name}IntentName as AuwgentIntentName;"),format!("pub use {agent_name}BaseIntentHandler as AuwgentBaseIntentHandler;"),format!("pub use {agent_name}BasePartialIntentHandler as AuwgentBasePartialIntentHandler;"),format!("pub use {agent_name}Middleware as AuwgentMiddleware;"),format!("pub use {agent_name}MiddlewareRegistry as AuwgentMiddlewareRegistry;")];
 if has_context{aliases.push(format!("pub use {agent_name}Context as AuwgentContext;"));}
 if has_tools{aliases.push(format!("pub use {agent_name}Tools as AuwgentTools;"));}
 if has_api_keys{aliases.push(format!("pub use {agent_name}ApiKeys as AuwgentApiKeys;"));}
 aliases.join("\n")
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

fn shape_type_or_no_args(name:&str,shape:Option<&Value>,blocks:&mut Vec<String>)->String{if is_empty_shape(shape,false){"NoArgs".to_string()}else{blocks.push(generate_named_shape(name,shape));name.to_string()}}

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
 fn emits_typed_intents_and_conditional_config(){let ir=json!({"name":"Hello","context":{"user_id":{"type":"string","optional":false}},"tools":[{"name":"get_details","params":{},"returns":{"type":"typeRef","name":"Person"}},{"name":"get_location","params":{"id":{"type":"string","optional":false}},"returns":{"type":"string"}}],"helpers":[{"name":"Joker","input":null,"output":null,"customIntents":[{"name":"helper_prompt","fields":{"message":{"type":"string","optional":false}}}]}],"output":{"__variants":{"Output":{"name":{"type":"string","optional":false}},"Fallback":{"message":{"type":"string","optional":false}}}},"types":{"Person":{"properties":{"name":{"type":"string","optional":false},"age":{"type":"number","optional":false}}}},"customIntents":[{"name":"ask_user","fields":{"question":{"type":"string","optional":false}}}],"modelConfig":[{"defaultConfig":{"model":{"type":"openai","modelName":"gpt-4.1"}}}]});let output=generate(&CodegenPlan::new(ir),"hello");assert!(output.contains("pub enum HelloIntent"));assert!(output.contains("pub enum HelloIntentPartial"));assert!(output.contains("ToolCall(HelloToolCallIntent)"));assert!(output.contains("ResponseSchema(HelloResponseSchemaIntent)"));assert!(output.contains("pub fn parse_intent_name"));assert!(output.contains("FnMut(HelloIntent, &str) -> Option<IntentControl>"));assert!(output.contains("pub enum HelloToolCallIntent"));assert!(output.contains("GetLocation {"));assert!(output.contains("pub enum HelloResponseSchemaIntent"));assert!(output.contains("#[serde(tag = \"type\", content = \"response\")]"));assert!(output.contains("pub trait HelloTools: Send + Sync"));assert!(output.contains("impl sdk::ToolRegistrar for HelloToolsRegistry"));assert!(output.contains("pub struct HelloConfig"));assert!(output.contains("pub tools: HelloToolsRegistry,"));assert!(output.contains("pub middleware: Vec<HelloMiddlewareRegistry>,"));assert!(output.contains("pub context: HelloContext,"));assert!(output.contains("pub api_keys: HelloApiKeys,"));assert!(output.contains("sdk::parse_ir(include_str!(\"./hello.agent.json\"))"));assert!(output.contains("sdk::create_auwgent(ir, sdk_config)?"));assert!(output.contains("pub fn auwgent(config: HelloConfig) -> sdk::AuwgentResult<HelloAgent>"));assert!(output.contains("pub use HelloIntent as AuwgentIntent;"));assert!(output.contains("pub use HelloIntentPartial as AuwgentIntentPartial;"));assert!(output.contains("pub use HelloTools as AuwgentTools;"));assert!(output.contains("pub trait HelloMiddleware: sdk::Middleware"));assert!(output.contains("pub struct HelloHelperPromptIntent"));}
 #[test]
 fn omits_conditional_fields_when_not_needed(){let ir=json!({"name":"Mini","tools":[],"helpers":[],"components":[],"modelConfig":[]});let output=generate(&CodegenPlan::new(ir),"mini");assert!(output.contains("pub struct MiniConfig"));assert!(output.contains("pub middleware: Vec<MiniMiddlewareRegistry>,"));assert!(!output.contains("pub tools: MiniToolsRegistry,"));assert!(!output.contains("pub context: MiniContext,"));assert!(!output.contains("pub api_keys: MiniApiKeys,"));assert!(output.contains("FnMut(MiniIntent, &str) -> Option<IntentControl>"));assert!(output.contains("tools: (),"));assert!(output.contains("context: None,"));assert!(output.contains("api_keys: sdk::AuwgentApiKeys::default()"));}
}
