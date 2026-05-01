use crate::main_agent::auwgent;
use crate::observations::agent_config::get_agent_config;


pub async fn test_static_intents() {
    let config = get_agent_config(vec![]);

    let Ok(agent) = auwgent(config) else {
        println!("failed to load config");
        return;
    };

    let intents = vec![
        "[response_text] hello [/response_text]".to_string(),
        "[tool_call:get_location] [/tool_call]".to_string(),
        "[tool_call:get_marks]id:100[/tool_call]".to_string(),
        "[tool_call:get_] [/tool_call]".to_string(),
        "[custom:Loud]actions:take_action , reasons: nothing[/custom]
        [helper_call:Fact]input:called helper [/helper_call]".to_string(),
    ];

    agent.raw().write_chunk(intents[0].clone());
    let result = agent.raw().process_intents().await;
    println!("result: {:?}", result);

    agent.raw().write_chunk(intents[1].clone());
    let result = agent.raw().process_intents().await;
    println!("result: {:?}", result);


}
