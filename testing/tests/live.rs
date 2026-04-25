use auwgent_testing::fixture::*;

use serde_json::json;
struct Tools;
struct Logger;

impl  AuwgentMiddleware for Logger{
    fn name(&self) ->  &'static str {
        "logger"
    }
}

impl AuwgentTools for Tools {
    fn get_location(&self,_args:NoArgs) -> SimpleToolGetLocationToolResultValue {
        return "Tarkwa".to_string();
    }

    fn get_marks(&self,args:SimpleToolGetMarksToolArgs) -> SimpleToolGetMarksToolResultValue {
       if args.id.contains("1"){
        return "1,2,4,6".to_string()
       }

       "A, 3,5".to_string()
    }
}

struct MyHandler;

impl SimpleToolIntentHandler for MyHandler {
    fn response_text(&self, intent: &SimpleToolIntentView, _agent: &str) {
        println!("LLM text: {}", intent.text());
    }

    fn tool_call(&self, intent: &SimpleToolIntentView, _agent: &str) {
        let call: SimpleToolToolCallIntent = intent.args();
        match call {
            SimpleToolToolCallIntent::GetLocation => println!("tool call: get_location"),
            SimpleToolToolCallIntent::GetMarks { args } => println!("tool call: get_marks id={}", args.id),
        }
    }

    fn tool_result(&self, intent: &SimpleToolIntentView, _agent: &str) {
        let res: SimpleToolToolResultIntent = intent.args();
        match res {
            SimpleToolToolResultIntent::GetLocation { result, .. } => println!("tool result: location = {}", result),
            SimpleToolToolResultIntent::GetMarks { args, result, .. } => println!("marks for {} = {}", args.id, result),
        }
    }

    fn any(&self, intent: &SimpleToolIntentView, agent: &str) {
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
