// You don't need to manually import `async_trait` for the main function itself
use crate::main_agent::auwgent;
use crate::observations::agent_config::get_agent_config;

pub async fn test_promt_generation() {
    let config = get_agent_config(vec![]);

    let Ok(agent) = auwgent(config) else {
        println!("failed to load config");
        return;
    };
    let Ok(prompt) = agent.generate_prompt(None) else {
        println!("failed to generate prompt");
        return;
    };

    println!("{prompt}");

    assert!(
        prompt.contains("user_name"),
        "expected prompt to contain user_name"
    );

    assert!(
        prompt.contains("[tool_call"),
        "expected prompt to contain prompt containing tool call"
    );

    assert!(
        prompt.contains("[helper_call"),
        "expected prompt to contain prompt containing helper call"
    );

    assert!(
        prompt.contains("[workflow_call"),
        "expected prompt to contain prompt containing workflow call"
    );

    assert!(
        prompt.contains("get_location"),
        "expected prompt to contain get_location"
    );

    assert!(
        prompt.contains("marks_and_location"),
        "expected prompt to contain marks_and_location"
    );

    assert!(
        prompt.contains("Joker"),
        "expected prompt to contain Joker"
    )

}
