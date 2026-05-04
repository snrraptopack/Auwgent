pub mod observations;

pub mod main_agent {
    #![allow(clippy::all)]
    #![allow(dead_code)]

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test_case/main.agent.rs"
    ));
}

// use observations::test_bridge_surface::test_bridge_surface;
// use observations::test_middleware_lifecycle::test_middleware_lifecycle_driver;
// use observations::test_helper_custom_intents::test_helper_custom_intents;
// use observations::test_static_intents::test_static_intents;
// use observations::test_stack_resumption::test_stack_resumption;
use observations::live_test::live_test;

#[tokio::main]
async fn main() {
    //test_promt_generation().await;
    // test_static_intents().await;
    // test_helper_custom_intents().await;
    // test_stack_resumption().await;
    // test_bridge_surface().await;
    // test_middleware_lifecycle_driver().await;

    live_test().await;
}
