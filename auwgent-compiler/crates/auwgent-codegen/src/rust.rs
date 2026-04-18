use crate::common::{array_at,collect_custom_provider_ids,collect_helper_tools,collect_required_providers,collect_workflow_tools,join_sections,merge_tool_defs,string_at};
use serde_json::{Map,Value};
use std::collections::BTreeSet;

pub fn generate(ir:&Value,_base_name:&str)->String{
 let agent_name=string_at(ir,&["name"]).unwrap_or("Agent");
 let workflow_tools=collect_workflow_tools(ir);
 let helper_tools=collect_helper_tools(ir);
 let all_tools=merge_tool_defs(array_at(ir,&["tools"]),workflow_tools.into_iter().chain(helper_tools.into_iter()).collect());
 let has_tools=!all_tools.is_empty();
 let workflows=array_at(ir,&["workflows"]);
 let helpers=array_at(ir,&["helpers"]);
 let custom_intents=collect_custom_intents(ir);
 let has_workflows=!workflows.is_empty();
 let has_helpers=!helpers.is_empty();
 let has_components=!array_at(ir,&["components"]).is_empty();
 let has_context=ir.get("context").and_then(Value::as_object).map(|c|!c.is_empty()).unwrap_or(false);
 let required_providers=collect_required_providers(ir);
 let custom_provider_ids=collect_custom_provider_ids(ir);
 let mut sections=vec![format!("// Auto-generated Rust bindings for {agent_name}"),"// Do not edit manually".to_string(),String::new(),"use serde_json::{Map as JsonMap, Value as JsonValue};".to_string(),"use std::marker::PhantomData;".to_string(),String::new(),generate_runtime_support()];
 if let Some(types)=ir.get("types").and_then(Value::as_object){sections.push(generate_custom_types(types));}
 sections.push(generate_named_shape(&format!("{agent_name}Input"),unwrap_input_fields(ir.get("input")).as_ref()));
 sections.push(generate_output_type(agent_name,ir.get("output")));
 if has_context{sections.push(generate_named_shape(&format!("{agent_name}Context"),ir.get("context")));}
 if has_tools{sections.push(generate_tools(agent_name,&all_tools));}
 sections.push(generate_intent_name_enum(agent_name,has_tools,has_workflows,has_helpers,has_components,&custom_intents));
 sections.push(generate_custom_intent_types(agent_name,ir));
 sections.push(generate_core_intents(agent_name,ir.get("output")));
 if has_tools{sections.push(generate_callable_family(agent_name,"Tool",&all_tools,"name","params","returns",true));}
 if has_workflows{sections.push(generate_callable_family(agent_name,"Workflow",workflows,"flowName","flowParams","returns",false));}
 if has_helpers{sections.push(generate_callable_family(agent_name,"Helper",helpers,"name","input","output",false));}
 if has_components{sections.push(generate_component_intents(agent_name));}
 sections.push(generate_top_level_intent_enums(agent_name,has_tools,has_workflows,has_helpers,has_components,&custom_intents));
 sections.push(generate_decode_functions(agent_name,has_tools,has_workflows,has_helpers,has_components,&custom_intents));
 sections.push(generate_handler_traits(agent_name));
 sections.push(generate_api_keys(agent_name,&required_providers,&custom_provider_ids));
 sections.push(generate_middleware_trait(agent_name));
 sections.push(generate_config(agent_name,has_tools,has_context,!required_providers.is_empty()));
 sections.push(generate_agent(agent_name));
 sections.push(generate_aliases(agent_name,has_tools,has_context,!required_providers.is_empty()));
 join_sections(&sections)
}

fn generate_runtime_support()->String{[
"pub type IntentControl = JsonValue;".to_string(),
"pub type SessionState = JsonValue;".to_string(),
"#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]\npub struct NoArgs {}\n".to_string(),
"#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]\npub struct PartialTextIntentValue {\n    #[serde(flatten)]\n    pub raw: JsonMap<String, JsonValue>,\n}\n".to_string(),
"#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]\npub struct PartialStructuredIntentValue<T> {\n    #[serde(flatten)]\n    pub raw: JsonMap<String, JsonValue>,\n    #[serde(skip)]\n    pub marker: PhantomData<T>,\n}\n".to_string(),
].join("\n")}

fn generate_custom_types(types:&Map<String,Value>)->String{let mut blocks=Vec::new();for(type_name,type_def)in types{blocks.push(generate_named_shape(type_name,Some(type_def)));}blocks.join("\n")}

fn generate_output_type(agent_name:&str,value:Option<&Value>)->String{
 let Some(value)=value else{return format!("pub type {agent_name}Output = JsonValue;\n");};
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
 generate_named_shape(&format!("{agent_name}Output"),Some(value))
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
 let mut result_aliases=Vec::new();let mut methods=Vec::new();
 for tool in tools{
  let Some(tool_name)=string_at(tool,&["name"]) else{continue;};
  let pascal=to_rust_type_name(tool_name);let method_name=to_rust_field_name(tool_name);
  let args_type=shape_type_or_no_args(&format!("{agent_name}{pascal}ToolArgs"),tool.get("params"),&mut result_aliases);
  let result_alias=format!("{agent_name}{pascal}ToolResultValue");
  result_aliases.push(format!("pub type {result_alias} = {};\n",rust_type(tool.get("returns"),false,"()")));
  methods.push(format!("    fn {method_name}(&self, args: {args_type}) -> {result_alias};"));
 }
 format!("{}\npub trait {agent_name}Tools {{\n{}\n}}\n",result_aliases.join("\n"),methods.join("\n"))
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

fn generate_custom_intent_types(agent_name:&str,ir:&Value)->String{
 let mut blocks=Vec::new();
 if let Some(items)=ir.get("customIntents").and_then(Value::as_array){for item in items{let Some(name)=string_at(item,&["name"]) else{continue;};blocks.push(generate_named_shape(&format!("{agent_name}{}Intent",to_rust_type_name(name)),item.get("fields")));}}
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
 let mut intent_cases=vec![format!("        {agent_name}IntentName::ResponseText => Some({agent_name}Intent::ResponseText(serde_json::from_value(value).unwrap())),"),format!("        {agent_name}IntentName::ResponseSchema => Some({agent_name}Intent::ResponseSchema(serde_json::from_value(value).unwrap())),"),format!("        {agent_name}IntentName::Error => Some({agent_name}Intent::Error(serde_json::from_value(value).unwrap())),")];
 let mut partial_cases=vec![format!("        {agent_name}IntentName::ResponseText => Some({agent_name}IntentPartial::ResponseText(serde_json::from_value(value).unwrap())),"),format!("        {agent_name}IntentName::ResponseSchema => Some({agent_name}IntentPartial::ResponseSchema(serde_json::from_value(value).unwrap())),"),format!("        {agent_name}IntentName::Error => Some({agent_name}IntentPartial::Error(serde_json::from_value(value).unwrap())),")];
 if has_tools{
  intent_cases.extend([format!("        {agent_name}IntentName::ToolCall => Some({agent_name}Intent::ToolCall(serde_json::from_value(value).unwrap())),"),format!("        {agent_name}IntentName::ToolResult => Some({agent_name}Intent::ToolResult(serde_json::from_value(value).unwrap())),"),format!("        {agent_name}IntentName::ToolError => Some({agent_name}Intent::ToolError(serde_json::from_value(value).unwrap())),"),format!("        {agent_name}IntentName::ToolSkipped => Some({agent_name}Intent::ToolSkipped(serde_json::from_value(value).unwrap())),")]);
  partial_cases.extend([format!("        {agent_name}IntentName::ToolCall => Some({agent_name}IntentPartial::ToolCall(serde_json::from_value(value).unwrap())),"),format!("        {agent_name}IntentName::ToolResult => Some({agent_name}IntentPartial::ToolResult(serde_json::from_value(value).unwrap())),"),format!("        {agent_name}IntentName::ToolError => Some({agent_name}IntentPartial::ToolError(serde_json::from_value(value).unwrap())),"),format!("        {agent_name}IntentName::ToolSkipped => Some({agent_name}IntentPartial::ToolSkipped(serde_json::from_value(value).unwrap())),")]);
 }
 if has_workflows{
  intent_cases.extend([format!("        {agent_name}IntentName::WorkflowCall => Some({agent_name}Intent::WorkflowCall(serde_json::from_value(value).unwrap())),"),format!("        {agent_name}IntentName::WorkflowResult => Some({agent_name}Intent::WorkflowResult(serde_json::from_value(value).unwrap())),")]);
  partial_cases.extend([format!("        {agent_name}IntentName::WorkflowCall => Some({agent_name}IntentPartial::WorkflowCall(serde_json::from_value(value).unwrap())),"),format!("        {agent_name}IntentName::WorkflowResult => Some({agent_name}IntentPartial::WorkflowResult(serde_json::from_value(value).unwrap())),")]);
 }
 if has_helpers{
  intent_cases.extend([format!("        {agent_name}IntentName::HelperCall => Some({agent_name}Intent::HelperCall(serde_json::from_value(value).unwrap())),"),format!("        {agent_name}IntentName::HelperResult => Some({agent_name}Intent::HelperResult(serde_json::from_value(value).unwrap())),")]);
  partial_cases.extend([format!("        {agent_name}IntentName::HelperCall => Some({agent_name}IntentPartial::HelperCall(serde_json::from_value(value).unwrap())),"),format!("        {agent_name}IntentName::HelperResult => Some({agent_name}IntentPartial::HelperResult(serde_json::from_value(value).unwrap())),")]);
 }
 if has_components{
  intent_cases.extend([format!("        {agent_name}IntentName::Component => Some({agent_name}Intent::Component(value)),"),format!("        {agent_name}IntentName::RenderComponent => Some({agent_name}Intent::RenderComponent(value)),")]);
  partial_cases.extend([format!("        {agent_name}IntentName::Component => Some({agent_name}IntentPartial::Component(serde_json::from_value(value).unwrap())),"),format!("        {agent_name}IntentName::RenderComponent => Some({agent_name}IntentPartial::RenderComponent(serde_json::from_value(value).unwrap())),")]);
 }
 for custom_intent in custom_intents{let pascal=to_rust_type_name(custom_intent);intent_cases.push(format!("        {agent_name}IntentName::{pascal} => Some({agent_name}Intent::{pascal}(serde_json::from_value(value).unwrap())),"));partial_cases.push(format!("        {agent_name}IntentName::{pascal} => Some({agent_name}IntentPartial::{pascal}(serde_json::from_value(value).unwrap())),"));}
 format!("pub fn decode_intent(name: {agent_name}IntentName, value: JsonValue) -> Option<{agent_name}Intent> {{\n    match name {{\n{}\n    }}\n}}\n\npub fn decode_intent_partial(name: {agent_name}IntentName, value: JsonValue) -> Option<{agent_name}IntentPartial> {{\n    match name {{\n{}\n    }}\n}}\n",intent_cases.join("\n"),partial_cases.join("\n"))
}

fn generate_handler_traits(agent_name:&str)->String{format!("pub trait {agent_name}BaseIntentHandler {{\n    fn on_intent(&mut self, intent: {agent_name}Intent, agent_name: &str) -> Option<IntentControl> {{ let _ = (intent, agent_name); None }}\n}}\n\npub trait {agent_name}BasePartialIntentHandler {{\n    fn on_intent_partial(&mut self, intent: {agent_name}IntentPartial, agent_name: &str) {{ let _ = (intent, agent_name); }}\n}}\n\npub fn dispatch_intent<H: {agent_name}BaseIntentHandler>(handler: &mut H, intent: {agent_name}Intent, agent_name: &str) -> Option<IntentControl> {{\n    handler.on_intent(intent, agent_name)\n}}\n\npub fn dispatch_partial_intent<H: {agent_name}BasePartialIntentHandler>(handler: &mut H, intent: {agent_name}IntentPartial, agent_name: &str) {{\n    handler.on_intent_partial(intent, agent_name)\n}}\n")}
fn generate_api_keys(agent_name:&str,required_providers:&BTreeSet<String>,custom_provider_ids:&BTreeSet<String>)->String{
 if required_providers.is_empty(){return String::new();}
 let mut fields=Vec::new();
 if required_providers.contains("openai"){fields.push("    pub openai_api_key: Option<String>,".to_string());}
 if required_providers.contains("gemini"){fields.push("    pub gemini_api_key: Option<String>,".to_string());}
 if required_providers.contains("groq"){fields.push("    pub groq_api_key: Option<String>,".to_string());}
 for id in custom_provider_ids{fields.push(format!("    pub {}_api_key: Option<String>,",to_rust_field_name(&id.replace('-',"_"))))}
 format!("#[derive(Debug, Clone, Default)]\npub struct {agent_name}ApiKeys {{\n{}\n}}\n",fields.join("\n"))
}

fn generate_middleware_trait(agent_name:&str)->String{format!("pub type MiddlewareContext = JsonMap<String, JsonValue>;\n\npub trait {agent_name}Middleware {{\n    fn name(&self) -> &'static str;\n\n    fn on_run_start(&mut self, session: SessionState, ctx: &mut MiddlewareContext) -> SessionState {{ let _ = ctx; session }}\n    fn on_llm_start(&mut self, prompt: String, ctx: &mut MiddlewareContext) -> Option<String> {{ let _ = (prompt, ctx); None }}\n    fn on_intent(&mut self, intent: {agent_name}Intent, ctx: &mut MiddlewareContext) -> Option<IntentControl> {{ let _ = (intent, ctx); None }}\n    fn on_intent_partial(&mut self, intent: {agent_name}IntentPartial, ctx: &mut MiddlewareContext) {{ let _ = (intent, ctx); }}\n    fn on_llm_end(&mut self, response: JsonValue, ctx: &mut MiddlewareContext) {{ let _ = (response, ctx); }}\n    fn on_run_complete(&mut self, final_session: SessionState, ctx: &mut MiddlewareContext) {{ let _ = (final_session, ctx); }}\n    fn on_error(&mut self, error: JsonValue, session: Option<SessionState>, ctx: &mut MiddlewareContext) -> bool {{ let _ = (error, session, ctx); false }}\n}}\n")}

fn generate_config(agent_name:&str,has_tools:bool,has_context:bool,has_api_keys:bool)->String{
 let mut fields=Vec::new();
 if has_tools{fields.push(format!("    pub tools: {agent_name}ToolsRegistry,"));}
 fields.push(format!("    pub middleware: Vec<{agent_name}MiddlewareRegistry>,"));
 if has_context{fields.push(format!("    pub context: {agent_name}Context,"));}
 if has_api_keys{fields.push(format!("    pub api_keys: {agent_name}ApiKeys,"));}
 let tools_registry=if has_tools{format!("pub type {agent_name}ToolsRegistry = Box<dyn {agent_name}Tools>;")}else{format!("pub type {agent_name}ToolsRegistry = ();")};
 format!("{tools_registry}\npub type {agent_name}MiddlewareRegistry = Box<dyn {agent_name}Middleware>;\n\n#[derive(Debug)]\npub struct {agent_name}Config {{\n{}\n}}\n",fields.join("\n"))
}

fn generate_agent(agent_name:&str)->String{format!("pub struct {agent_name}Agent {{\n    pub config: {agent_name}Config,\n}}\n\nimpl {agent_name}Agent {{\n    pub fn on_intent<F>(&mut self, _handler: F)\n    where\n        F: FnMut({agent_name}Intent, &str) -> Option<IntentControl> + 'static,\n    {{}}\n\n    pub fn on_intent_partial<F>(&mut self, _handler: F)\n    where\n        F: FnMut({agent_name}IntentPartial, &str) + 'static,\n    {{}}\n\n    pub fn on_intent_handler<H>(&mut self, _handler: H)\n    where\n        H: {agent_name}BaseIntentHandler + 'static,\n    {{}}\n\n    pub fn on_intent_partial_handler<H>(&mut self, _handler: H)\n    where\n        H: {agent_name}BasePartialIntentHandler + 'static,\n    {{}}\n}}\n\npub fn create_{snake_agent_name}(config: {agent_name}Config) -> {agent_name}Agent {{\n    {agent_name}Agent {{ config }}\n}}\n\npub fn auwgent(config: {agent_name}Config) -> {agent_name}Agent {{\n    create_{snake_agent_name}(config)\n}}\n",snake_agent_name=to_rust_field_name(agent_name))}

fn generate_aliases(agent_name:&str,has_tools:bool,has_context:bool,has_api_keys:bool)->String{
 let mut aliases=vec![format!("pub use {agent_name}Agent as AuwgentAgent;"),format!("pub use {agent_name}Config as AuwgentConfig;"),format!("pub use {agent_name}Intent as AuwgentIntent;"),format!("pub use {agent_name}IntentPartial as AuwgentIntentPartial;"),format!("pub use {agent_name}IntentName as AuwgentIntentName;"),format!("pub use {agent_name}BaseIntentHandler as AuwgentBaseIntentHandler;"),format!("pub use {agent_name}BasePartialIntentHandler as AuwgentBasePartialIntentHandler;"),format!("pub use {agent_name}MiddlewareRegistry as AuwgentMiddleware;")];
 if has_context{aliases.push(format!("pub use {agent_name}Context as AuwgentContext;"));}
 if has_tools{aliases.push(format!("pub use {agent_name}Tools as AuwgentTools;"));}
 if has_api_keys{aliases.push(format!("pub use {agent_name}ApiKeys as AuwgentApiKeys;"));}
 aliases.join("\n")
}

fn collect_custom_intents(ir:&Value)->Vec<String>{let mut intents=BTreeSet::new();if let Some(items)=ir.get("customIntents").and_then(Value::as_array){for item in items{if let Some(name)=string_at(item,&["name"]){intents.insert(name.to_string());}}}intents.into_iter().collect()}

fn unwrap_input_fields(value:Option<&Value>)->Option<Value>{let value=value?;if value.get("kind").and_then(Value::as_str)==Some("properties"){return value.get("fields").cloned();}Some(value.clone())}

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
 use super::*;use serde_json::json;
 #[test]
 fn emits_typed_intents_and_conditional_config(){let ir=json!({"name":"Hello","context":{"user_id":{"type":"string","optional":false}},"tools":[{"name":"get_details","params":{},"returns":{"type":"typeRef","name":"Person"}},{"name":"get_location","params":{"id":{"type":"string","optional":false}},"returns":{"type":"string"}}],"helpers":[{"name":"Joker","input":null,"output":null}],"output":{"__variants":{"Output":{"name":{"type":"string","optional":false}},"Fallback":{"message":{"type":"string","optional":false}}}},"types":{"Person":{"properties":{"name":{"type":"string","optional":false},"age":{"type":"number","optional":false}}}},"customIntents":[{"name":"ask_user","fields":{"question":{"type":"string","optional":false}}}],"modelConfig":[{"defaultConfig":{"model":{"type":"openai","modelName":"gpt-4.1"}}}]});let output=generate(&ir,"hello");assert!(output.contains("pub enum HelloIntent"));assert!(output.contains("pub enum HelloIntentPartial"));assert!(output.contains("ToolCall(HelloToolCallIntent)"));assert!(output.contains("ResponseSchema(HelloResponseSchemaIntent)"));assert!(output.contains("pub fn decode_intent"));assert!(output.contains("FnMut(HelloIntent, &str) -> Option<IntentControl>"));assert!(output.contains("pub enum HelloToolCallIntent"));assert!(output.contains("GetLocation {"));assert!(output.contains("pub enum HelloResponseSchemaIntent"));assert!(output.contains("#[serde(tag = \"type\", content = \"response\")]") );assert!(output.contains("pub struct HelloConfig"));assert!(output.contains("pub tools: HelloToolsRegistry,"));assert!(output.contains("pub middleware: Vec<HelloMiddlewareRegistry>,"));assert!(output.contains("pub context: HelloContext,"));assert!(output.contains("pub api_keys: HelloApiKeys,"));assert!(output.contains("pub use HelloIntent as AuwgentIntent;"));assert!(output.contains("pub use HelloIntentPartial as AuwgentIntentPartial;"));assert!(output.contains("pub use HelloTools as AuwgentTools;"));}
 #[test]
 fn omits_conditional_fields_when_not_needed(){let ir=json!({"name":"Mini","tools":[],"helpers":[],"components":[],"modelConfig":[]});let output=generate(&ir,"mini");assert!(output.contains("pub struct MiniConfig"));assert!(output.contains("pub middleware: Vec<MiniMiddlewareRegistry>,"));assert!(!output.contains("pub tools: MiniToolsRegistry,"));assert!(!output.contains("pub context: MiniContext,"));assert!(!output.contains("pub api_keys: MiniApiKeys,"));assert!(output.contains("FnMut(MiniIntent, &str) -> Option<IntentControl>"));}
}
