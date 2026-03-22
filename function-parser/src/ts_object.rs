/// TypeScript-style object parser for @@out blocks
/// Supports both unquoted keys (TS-style) and quoted keys (JSON-style)

use crate::tokenizer::Tokenizer;
use crate::ast::{TokenKind, ASTValue};
use std::collections::HashMap;

pub fn parse_ts_object(input: &str) -> Result<ASTValue, String> {
    let mut parser = TSObjectParser::new(input);
    parser.parse_value()
}

struct TSObjectParser {
    tokenizer: Tokenizer,
    current: TokenKind,
}

impl TSObjectParser {
    fn new(input: &str) -> Self {
        let mut tokenizer = Tokenizer::new(input);
        let current = tokenizer.next_token().kind;
        Self { tokenizer, current }
    }

    fn advance(&mut self) {
        self.current = self.tokenizer.next_token().kind;
    }

    fn parse_value(&mut self) -> Result<ASTValue, String> {
        match &self.current {
            TokenKind::StringLiteral(s) => {
                let val = ASTValue::String(s.clone());
                self.advance();
                Ok(val)
            }
            TokenKind::NumberLiteral(n) => {
                let val = ASTValue::Number(*n);
                self.advance();
                Ok(val)
            }
            TokenKind::BooleanLiteral(b) => {
                let val = ASTValue::Boolean(*b);
                self.advance();
                Ok(val)
            }
            TokenKind::Null => {
                self.advance();
                Ok(ASTValue::Null)
            }
            TokenKind::OpenBrace => {
                self.advance(); // consume '{'
                let obj = self.parse_object()?;
                if self.current == TokenKind::CloseBrace {
                    self.advance();
                }
                Ok(ASTValue::Object(obj))
            }
            TokenKind::OpenBracket => {
                self.advance(); // consume '['
                let arr = self.parse_array()?;
                if self.current == TokenKind::CloseBracket {
                    self.advance();
                }
                Ok(ASTValue::Array(arr))
            }
            TokenKind::Identifier(s) => {
                // Bare identifier - treat as string
                let val = ASTValue::String(s.clone());
                self.advance();
                Ok(val)
            }
            _ => Err(format!("Unexpected token: {:?}", self.current))
        }
    }

    fn parse_object(&mut self) -> Result<HashMap<String, ASTValue>, String> {
        let mut map = HashMap::new();

        while self.current != TokenKind::EOF && self.current != TokenKind::CloseBrace {
            // Parse key (can be identifier or string)
            let key = match &self.current {
                TokenKind::Identifier(s) => {
                    let k = s.clone();
                    self.advance();
                    k
                }
                TokenKind::StringLiteral(s) => {
                    let k = s.clone();
                    self.advance();
                    k
                }
                TokenKind::Comma => {
                    self.advance();
                    continue;
                }
                TokenKind::CloseBrace => break,
                _ => return Err(format!("Expected key, got: {:?}", self.current))
            };

            // Expect : or =
            if self.current != TokenKind::Equals {
                return Err(format!("Expected '=' or ':', got: {:?}", self.current));
            }
            self.advance();

            // Parse value
            let value = self.parse_value()?;
            map.insert(key, value);

            // Optional comma
            if self.current == TokenKind::Comma {
                self.advance();
            }
        }

        Ok(map)
    }

    fn parse_array(&mut self) -> Result<Vec<ASTValue>, String> {
        let mut arr = Vec::new();

        while self.current != TokenKind::EOF && self.current != TokenKind::CloseBracket {
            if self.current == TokenKind::Comma {
                self.advance();
                continue;
            }
            if self.current == TokenKind::CloseBracket {
                break;
            }

            let value = self.parse_value()?;
            arr.push(value);

            if self.current == TokenKind::Comma {
                self.advance();
            }
        }

        Ok(arr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unquoted_keys() {
        let input = r#"{session_id: "123", count: 42}"#;
        let result = parse_ts_object(input).unwrap();
        
        if let ASTValue::Object(obj) = result {
            assert_eq!(obj.get("session_id"), Some(&ASTValue::String("123".to_string())));
            assert_eq!(obj.get("count"), Some(&ASTValue::Number(42.0)));
        } else {
            panic!("Expected object");
        }
    }

    #[test]
    fn test_quoted_keys() {
        let input = r#"{"session_id": "123", "count": 42}"#;
        let result = parse_ts_object(input).unwrap();
        
        if let ASTValue::Object(obj) = result {
            assert_eq!(obj.get("session_id"), Some(&ASTValue::String("123".to_string())));
            assert_eq!(obj.get("count"), Some(&ASTValue::Number(42.0)));
        } else {
            panic!("Expected object");
        }
    }

    #[test]
    fn test_nested_objects() {
        let input = r#"{user: {id: "usr_123", name: "Nana"}}"#;
        let result = parse_ts_object(input).unwrap();
        
        if let ASTValue::Object(obj) = result {
            if let Some(ASTValue::Object(user)) = obj.get("user") {
                assert_eq!(user.get("id"), Some(&ASTValue::String("usr_123".to_string())));
                assert_eq!(user.get("name"), Some(&ASTValue::String("Nana".to_string())));
            } else {
                panic!("user should be object");
            }
        } else {
            panic!("Expected object");
        }
    }

    #[test]
    fn test_arrays() {
        let input = r#"{items: [1, 2, 3], tags: ["a", "b"]}"#;
        let result = parse_ts_object(input).unwrap();
        
        if let ASTValue::Object(obj) = result {
            if let Some(ASTValue::Array(items)) = obj.get("items") {
                assert_eq!(items.len(), 3);
            } else {
                panic!("items should be array");
            }
            if let Some(ASTValue::Array(tags)) = obj.get("tags") {
                assert_eq!(tags.len(), 2);
            } else {
                panic!("tags should be array");
            }
        } else {
            panic!("Expected object");
        }
    }
}
