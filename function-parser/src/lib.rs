pub mod ast;
pub mod block_scanner;
pub mod function;
pub mod parser;
pub mod tokenizer;
pub mod ts_object;

pub use ast::*;
pub use block_scanner::{Block, BlockScanner, BlockType};
pub use function::{FunctionCall, parse_function_calls};
pub use parser::Parser;
pub use tokenizer::Tokenizer;
pub use ts_object::{parse_assignment_object, parse_ts_object};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_intent() {
        let input = r#"
            thought(
              explain = "I will break this down step by step."
            )
        "#;

        let mut parser = Parser::new(input);
        let intents = parser.parse();

        assert_eq!(intents.len(), 1);
        assert_eq!(intents[0].name, "thought");
        assert!(intents[0].fields.contains_key("explain"));
    }

    #[test]
    fn test_nested_object_intent() {
        let input = r#"
            helper_call(
              type = "StoryTeller"
              args = {
                city = "Accra"
                days = 3
                themes = [
                  "horror",
                  "night"
                ]
              }
            )
        "#;

        let mut parser = Parser::new(input);
        let intents = parser.parse();

        assert_eq!(intents.len(), 1);
        assert_eq!(intents[0].name, "helper_call");
        assert_eq!(
            intents[0].fields.get("type"),
            Some(&ASTValue::String("StoryTeller".to_string()))
        );

        if let Some(ASTValue::Object(args)) = intents[0].fields.get("args") {
            assert_eq!(
                args.get("city"),
                Some(&ASTValue::String("Accra".to_string()))
            );
            assert_eq!(args.get("days"), Some(&ASTValue::Number(3.0)));

            if let Some(ASTValue::Array(themes)) = args.get("themes") {
                assert_eq!(themes.len(), 2);
                assert_eq!(themes[0], ASTValue::String("horror".to_string()));
                assert_eq!(themes[1], ASTValue::String("night".to_string()));
            } else {
                panic!("themes is not an array");
            }
        } else {
            panic!("args is not an object");
        }
    }

    #[test]
    fn test_complex_array_of_objects() {
        let input = r#"
            analytics_event(
              user = {
                id = "usr_123"
                premium = true
                metrics = [ 99.5, 42.0 ]
              }
              session_history = [
                {
                  event: "login",
                  timestamp: "2023-11-01T12:00:00Z"
                  metadata = {
                    ip = "192.168.1.1"
                  }
                },
                {
                  event = "purchase"
                  amount = 45.99
                  cart = [
                    { item_id = "book_1", qty = 2 },
                    { item_id = "book_2", qty = 1 }
                  ]
                }
              ]
              notes = "This tests extreme nesting.
It has strings with newlines.
It uses colons instead of equals for assignment."
            )
        "#;

        let mut parser = Parser::new(input);
        let intents = parser.parse();

        assert_eq!(intents.len(), 1);
        let intent = &intents[0];
        assert_eq!(intent.name, "analytics_event");

        // Test user object
        if let Some(ASTValue::Object(user)) = intent.fields.get("user") {
            assert_eq!(
                user.get("id"),
                Some(&ASTValue::String("usr_123".to_string()))
            );
            assert_eq!(user.get("premium"), Some(&ASTValue::Boolean(true)));
            if let Some(ASTValue::Array(metrics)) = user.get("metrics") {
                assert_eq!(metrics.len(), 2);
                assert_eq!(metrics[0], ASTValue::Number(99.5));
            } else {
                panic!("metrics not array");
            }
        } else {
            panic!("user not object");
        }

        // Test session history (array of objects)
        if let Some(ASTValue::Array(history)) = intent.fields.get("session_history") {
            assert_eq!(history.len(), 2);

            // First history object
            if let ASTValue::Object(event1) = &history[0] {
                assert_eq!(
                    event1.get("event"),
                    Some(&ASTValue::String("login".to_string()))
                ); // Used a colon
                if let Some(ASTValue::Object(meta)) = event1.get("metadata") {
                    assert_eq!(
                        meta.get("ip"),
                        Some(&ASTValue::String("192.168.1.1".to_string()))
                    );
                } else {
                    panic!("metadata not object");
                }
            } else {
                panic!("history[0] not object");
            }

            // Second history object
            if let ASTValue::Object(event2) = &history[1] {
                assert_eq!(
                    event2.get("event"),
                    Some(&ASTValue::String("purchase".to_string()))
                );
                assert_eq!(event2.get("amount"), Some(&ASTValue::Number(45.99)));

                // Nested nested array of objects
                if let Some(ASTValue::Array(cart)) = event2.get("cart") {
                    assert_eq!(cart.len(), 2);
                    if let ASTValue::Object(item1) = &cart[0] {
                        assert_eq!(
                            item1.get("item_id"),
                            Some(&ASTValue::String("book_1".to_string()))
                        );
                        assert_eq!(item1.get("qty"), Some(&ASTValue::Number(2.0)));
                    } else {
                        panic!("cart[0] not object");
                    }
                } else {
                    panic!("cart not array");
                }
            } else {
                panic!("history[1] not object");
            }
        } else {
            panic!("session_history not array");
        }

        // Test multiline string
        if let Some(ASTValue::String(notes)) = intent.fields.get("notes") {
            assert!(notes.starts_with("This tests extreme nesting."));
            assert!(notes.contains("It uses colons instead of equals"));
        } else {
            panic!("notes not string");
        }
    }
    #[test]
    fn test_multiple_attempts_with_registered_intents() {
        // Test parser recovery when model makes multiple attempts
        let input = r#"response_schema(
  type = "ContextCompilerOutput"
workflow_call(
  type = "first"
response_schema(
  type = "StraightResponse"
response_schema(
  type = "Final"
  response = {
    data = "complete"
  }
)"#;

        let mut registered = std::collections::HashSet::new();
        registered.insert("response_schema".to_string());
        registered.insert("workflow_call".to_string());

        let mut parser = Parser::with_registered_intents(input, registered);
        let intents = parser.parse();

        // Should find all 4 intents
        assert_eq!(intents.len(), 4);

        // Verify the intents
        assert_eq!(intents[0].name, "response_schema");
        assert!(!intents[0].is_complete);

        assert_eq!(intents[1].name, "workflow_call");
        assert!(!intents[1].is_complete);

        assert_eq!(intents[2].name, "response_schema");
        assert!(!intents[2].is_complete);

        assert_eq!(intents[3].name, "response_schema");
        assert!(intents[3].is_complete);

        // Last response_schema should have the complete data
        if let Some(ASTValue::Object(response)) = intents[3].fields.get("response") {
            assert!(response.contains_key("data"));
        } else {
            panic!("Last response_schema should have response object");
        }
    }
}
