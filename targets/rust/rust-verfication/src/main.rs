pub mod observations;

pub mod main_agent {
    #![allow(clippy::all)]
    #![allow(dead_code)]

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test_case/main.agent.rs"
    ));
}

use observations::test_middleware_lifecycle::test_middleware_lifecycle_driver;
use observations::test_static_intents::test_static_intents;

#[tokio::main]
async fn main() {
    //test_promt_generation().await;
    test_static_intents().await;
    test_middleware_lifecycle_driver().await;
}
