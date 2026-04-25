use auwgent_testing::{build_agent, 
    fixture::{
        AuwgentConfig, 
        AuwgentContext,
        AuwgentTools,
        NoArgs,
        SimpleToolGetLocationToolResultValue,
        SimpleToolGetMarksToolArgs,
        SimpleToolGetMarksToolResultValue
    }, live_guard};
use serde_json::json;


struct Tools;

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


#[tokio::test]
#[ignore = "requires a real provider key"]
async fn live_run_smoke() {
    if let Err(reason) = live_guard() {
        eprintln!("skipping live test: {reason}");
        return;
    }

    let config = AuwgentConfig{
        api_keys:auwgent_testing::fixture::AuwgentApiKeys { groq_api_key: Some("helo".to_string()) },
        context:AuwgentContext{
            user_name: "A".to_string(),
            age: 10.0,
            id: "123".to_string()
        },
        middleware: vec![],
        tools:Tools
    };


    let agent = build_agent::<auwgent_testing::fixture::SimpleToolMiddlewareRegistry>(vec![]);
    let _session = agent
        .run(Some(json!("Call get_marks for user id user_42 and summarize it.")))
        .await
        .expect("live run should complete");
}
