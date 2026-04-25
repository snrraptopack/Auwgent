use auwgent_testing::fixture::*;

use serde_json::json;
struct Tools;
struct Logger;

impl  AuwgentMiddleware for Logger{
    fn name(&self) ->  &'static str {
        "logger"
    }

    async fn on_run_start(&self,session:Session,_ctx:&Context) ->  Session{
        session
    }

    async fn on_llm_end<'life0,'life1,'life2,'async_trait>(&'life0 self,_response: &'life1 serde_json::Value,_ctx: &'life2 Context) ->  ::core::pin::Pin<Box<dyn ::core::future::Future<Output = ()> + ::core::marker::Send+'async_trait> >where 'life0:'async_trait,'life1:'async_trait,'life2:'async_trait,Self:'async_trait {
        
    }
}

impl AuwgentTools for Tools {
    fn get_location(&self) -> GetLocationResult {
        return "Tarkwa".to_string();
    }

    fn get_marks(&self,args:GetMarksArgs) -> GetMarksResult {
       if args.id.contains("1"){
        return "1,2,4,6".to_string()
       }

       "A, 3,5".to_string()
    }
}

struct MyHandler;

impl SimpleToolIntentHandler for MyHandler {
    fn response_text(&self, value: &ResponseText, _agent: &str) {
        println!("LLM text: {}", value.text);
    }

    fn tool_call(&self, value: &ToolCalls, _agent: &str) {
        match &value.kind {
            ToolCall::GetLocation => println!("tool call: get_location"),
            ToolCall::GetMarks { args } => println!("tool call: get_marks id={}", args.id),
        }
    }

    fn tool_result(&self, value: &ToolResults, _agent: &str) {
        match &value.kind {
            ToolResult::GetLocation { result, .. } => println!("tool result: location = {}", result),
            ToolResult::GetMarks { args, result, .. } => println!("marks for {} = {}", args.id, result),
        }
    }

    fn any(&self, intent: &Intents, agent: &str) {
        // runs for every intent
        println!("intent {} from {}", intent.name(), agent);
    }
}


#[tokio::test]
#[ignore = "requires a real provider key"]
async fn live_run_smoke() {

    let config = AuwgentConfig{
        api_keys:AuwgentApiKeys { groq_api_key: Some("helo".to_string()) },
        context:AuwgentContext{
            user_name: "A".to_string(),
            age: 10.0,
            id: "123".to_string()
        },
        middleware: vec![Logger],
        tools:Tools
    };

    let agent = auwgent(config).unwrap();
    agent.on_intent_handler(MyHandler);

    let _session = agent.run(Some(json!("hello"))).await.unwrap();

}
