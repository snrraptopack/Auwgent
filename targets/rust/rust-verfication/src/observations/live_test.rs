use serde_json::{Value, json};
use async_trait::async_trait;

use crate::{
    main_agent::{
        AuwgentIntentHandler, AuwgentMiddleware, AuwgentMiddlewareRegistry, Context, PartialTextIntentValue, ResponseText, Session, auwgent
    },
    observations::agent_config::get_agent_config_live
};

struct IntentLogger;
struct MiddlewareLogger;

impl AuwgentIntentHandler for IntentLogger {

    fn response_text(&self,value: &ResponseText,_agent: &str) {
        println!("{:?}",value)
    }
}



#[async_trait]
impl AuwgentMiddleware for MiddlewareLogger{
    fn name(&self)-> &'static str{
        "logger"
    }

    async fn on_run_start(&self,session:Session,ctx:&mut Context)->Session{
        ctx.data.insert("one".to_string(), Value::String("Hello".to_string()));
        session
    }

    async fn on_llm_start(&self,prompt:String,ctx:&mut Context)->String{
        println!("This is llm start");
          ctx.set_context(Value::String("secrete number : 100".to_string()));
        println!("the value: {:?}",ctx.data.get("one"));
        prompt + "repeat exactly what i said"
    }
}

pub async fn live_test(){
    let middleware:Vec<AuwgentMiddlewareRegistry> = vec![MiddlewareLogger.into()];
    let config = get_agent_config_live(
        middleware,
        Some("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".to_string())
    );

    let agent = auwgent(config).unwrap();

    agent.on_intent_handler(IntentLogger);

    let _session = agent.run(Some(json!("hello what is the seceret number"))).await;

    let Ok(metadata) = agent.get_metadata() else {
        println!("could not get metadata");
        return;
    };

    println!("{:?}",metadata.aggregate);
    let Ok(session) = _session else {
        return;
    };
    println!("{:?}",session.turns);

}
