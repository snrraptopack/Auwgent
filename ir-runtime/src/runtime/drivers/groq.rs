// use crate::runtime::drivers::ModelDriver;
// use crate::runtime::session::{Message, Role};
// use async_trait::async_trait;
// use futures_util::{Stream, StreamExt};
// use reqwest::Client;
// use serde_json::{Value, json};
// use std::pin::Pin;

// pub struct GROQDRIVER{
//     client:Client,
//     api_key:String,
// }

// impl GROQDRIVER{
//     pub fn new(api_key:String) -> Self{
//         Self{
//             client:Client::new(),
//             api_key,
//         }
//     }
// }

// #[async_trait]
// impl ModelDriver for GROQDRIVER {
//     async fn stream_generate(
//         &self,
//         model: &str,
//         messages: &[Message],
//         config: Option<Value>,
//     ) -> Result<Pin<Box<dyn Stream<Item = Result<crate::runtime::drivers::ModelEvent, String>> + Send>>, String> {
//         let url = format!("https://api.groq.com/openai/v1/chat/completions");

// }
