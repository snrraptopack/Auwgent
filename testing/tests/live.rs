use auwgent_testing::fixture::*;
use async_trait::async_trait;

use serde_json::json;
struct Tools;
struct Logger;

#[async_trait]
impl  AuwgentMiddleware for Logger{
    fn name(&self) ->  &'static str {
        "logger"
    }

    async fn on_run_start(&self,session:Session,ctx:Context)->Session{
        session
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

impl AuwgentIntentHandler for MyHandler {
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

    fn response_schema(&self,value: &ResponseSchema,_agent: &str) {
       match value{
        ResponseSchema::FactOutput { response } =>  print(""),
        _ => { println!("")}
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

    let _session = agent.run(Some("hello")).await.unwrap();

}
