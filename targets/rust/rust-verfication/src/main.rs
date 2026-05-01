pub mod observations;

pub mod main_agent{
     #![allow(clippy::all)]
    #![allow(dead_code)]

    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/test_case/main.agent.rs"));
}

use observations::test_prompt::test_promt_generation;

#[tokio::main]
async fn main() {
    test_promt_generation().await;
}
