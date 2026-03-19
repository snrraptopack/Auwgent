// Preservation tests - verify existing parser behavior is unchanged
// These tests MUST PASS on both unfixed and fixed code

use super::orchestrator::Orchestrator;
use super::parser::parse;
use super::types::*;
use std::sync::{Arc, Mutex};

#[cfg(test)]
mod preservation_tests {
    use super::*;

    #[test]
    fn test_simple_key_value_pairs() {
        // Preservation: Simple key-value pairs parse correctly
        let input = "key1: value1\nkey2: value2\nkey3: value3";
        let result = parse(input, None);
        
        assert!(result.ast.is_some());
        if let Some(ASTNode::Mapping(root)) = result.ast {
            assert_eq!(root.entries.len(), 3);
            assert_eq!(root.entries[0].key, "key1");
            assert_eq!(root.entries[1].key, "key2");
            assert_eq!(root.entries[2].key, "key3");
        }
    }

    #[test]
    fn test_nested_mappings() {
        // Preservation: Nested mappings parse correctly
        let input = "parent:\n  child1: value1\n  child2: value2";
        let result = parse(input, None);
        
        assert!(result.ast.is_some());
        if let Some(ASTNode::Mapping(root)) = result.ast {
            let parent = root.entries.iter().find(|e| e.key == "parent").unwrap();
            if let ASTNode::Mapping(child_map) = &parent.value {
                assert_eq!(child_map.entries.len(), 2);
            }
        }
    }

    #[test]
    fn test_sequences() {
        // Preservation: Sequences parse correctly
        let input = "items:\n  - item1\n  - item2\n  - item3";
        let result = parse(input, None);
        
        assert!(result.ast.is_some());
        if let Some(ASTNode::Mapping(root)) = result.ast {
            let items = root.entries.iter().find(|e| e.key == "items").unwrap();
            if let ASTNode::Sequence(seq) = &items.value {
                assert_eq!(seq.items.len(), 3);
            }
        }
    }

    #[test]
    fn test_quoted_strings() {
        // Preservation: Quoted strings with escapes parse correctly
        let input = r#"text: "escaped\nstring""#;
        let result = parse(input, None);
        
        assert!(result.ast.is_some());
        if let Some(ASTNode::Mapping(root)) = result.ast {
            let text = root.entries.iter().find(|e| e.key == "text").unwrap();
            if let ASTNode::Scalar(scalar) = &text.value {
                assert!(scalar.quoted);
                assert!(scalar.value.contains('\n'));
            }
        }
    }

    #[test]
    fn test_standard_booleans() {
        // Preservation: Standard booleans coerce correctly
        let input = "enabled: true\ndisabled: false";
        let result = parse(input, None);
        
        assert!(result.ast.is_some());
        // Type coercion happens in builder, just verify parsing works
    }

    #[test]
    fn test_standard_numbers() {
        // Preservation: Standard numbers coerce correctly
        let input = "int: 42\nfloat: 3.14\nsci: 1.5e10";
        let result = parse(input, None);
        
        assert!(result.ast.is_some());
        // Type coercion happens in builder, just verify parsing works
    }

    #[test]
    fn test_normal_pipe_blocks() {
        // Preservation: Normal pipe blocks with normal chunk sizes parse correctly
        let input = "text: |\n  Line 1\n  Line 2\n  Line 3";
        let result = parse(input, None);
        
        assert!(result.ast.is_some());
        if let Some(ASTNode::Mapping(root)) = result.ast {
            let text = root.entries.iter().find(|e| e.key == "text").unwrap();
            if let ASTNode::Scalar(scalar) = &text.value {
                assert!(scalar.value.contains("Line 1"));
                assert!(scalar.value.contains("Line 2"));
                assert!(scalar.value.contains("Line 3"));
            }
        }
    }

    #[test]
    fn test_intent_detection() {
        // Preservation: Intent detection works correctly
        let input = "intent1:\n  text: Hello World";
        
        let mut orchestrator = Orchestrator::new(None);
        orchestrator.register_intent("intent1");

        let intents = Arc::new(Mutex::new(Vec::new()));
        let intents_clone = intents.clone();
        orchestrator.on_intent_ready(Arc::new(move |name, value| {
            intents_clone.lock().unwrap().push((name, value));
        }));

        orchestrator.write(input);
        orchestrator.end();

        let final_intents = intents.lock().unwrap();
        assert_eq!(final_intents.len(), 1);
        assert_eq!(final_intents[0].0, "intent1");
    }
}
