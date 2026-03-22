/// Simplified function call parser for @@tool, @@workflow, @@helper blocks
/// Parses: tool_name(arg1 = "value", arg2 = {...})

use crate::tokenizer::Tokenizer;
use crate::ast::{TokenKind, ASTValue};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct FunctionCall {
    pub name: String,
    pub args: HashMap<String, ASTValue>,
}

pub fn parse_function_calls(input: &str) -> Vec<FunctionCall> {
    let mut parser = FunctionParser::new(input);
    parser.parse()
}

struct FunctionParser {
    tokenizer: Tokenizer,
    current: TokenKind,
}

impl FunctionParser {
    fn new(input: &str) -> Self {
        let mut tokenizer = Tokenizer::new(input);
        let current = tokenizer.next_token().kind;
        Self { tokenizer, current }
    }

    fn advance(&mut self) {
        self.current = self.tokenizer.next_token().kind;
    }

    fn parse(&mut self) -> Vec<FunctionCall> {
        let mut calls = Vec::new();

        while self.current != TokenKind::EOF {
            if let TokenKind::Identifier(name) = &self.current {
                let func_name = name.clone();
                self.advance();

                if self.current == TokenKind::OpenParen {
                    self.advance(); // consume '('
                    let args = self.parse_args();
                    
                    if self.current == TokenKind::CloseParen {
                        self.advance();
                    }

                    calls.push(FunctionCall {
                        name: func_name,
                        args,
                    });
                }
            } else {
                self.advance();
            }
        }

        calls
    }

    fn parse_args(&mut self) -> HashMap<String, ASTValue> {
        let mut args = HashMap::new();

        while self.current != TokenKind::EOF 
            && self.current != TokenKind::CloseParen 
            && self.current != TokenKind::CloseBrace {
            
            if let TokenKind::Identifier(key) = &self.current {
                let arg_name = key.clone();
                self.advance();

                if self.current == TokenKind::Equals {
                    self.advance(); // consume '=' or ':'
                    if let Ok(val) = self.parse_value() {
                        args.insert(arg_name, val);
                    }
                }
            } else if self.current == TokenKind::Comma {
                self.advance();
            } else {
                self.advance();
            }
        }

        args
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
            TokenKind::Identifier(s) => {
                // Bare identifier - treat as string
                let val = ASTValue::String(s.clone());
                self.advance();
                Ok(val)
            }
            TokenKind::OpenBrace => {
                self.advance(); // consume '{'
                let obj = self.parse_args();
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
            _ => Err(format!("Unexpected token: {:?}", self.current))
        }
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
    fn test_single_call() {
        let input = r#"fetch(id = "123")"#;
        let calls = parse_function_calls(input);

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "fetch");
        assert_eq!(calls[0].args.get("id"), Some(&ASTValue::String("123".to_string())));
    }

    #[test]
    fn test_multiple_calls() {
        let input = r#"
fetch(id = "123")
get(name = "test")
        "#;
        let calls = parse_function_calls(input);

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "fetch");
        assert_eq!(calls[1].name, "get");
    }

    #[test]
    fn test_nested_args() {
        let input = r#"process(config = {timeout = 30, retry = true})"#;
        let calls = parse_function_calls(input);

        assert_eq!(calls.len(), 1);
        if let Some(ASTValue::Object(config)) = calls[0].args.get("config") {
            assert_eq!(config.get("timeout"), Some(&ASTValue::Number(30.0)));
            assert_eq!(config.get("retry"), Some(&ASTValue::Boolean(true)));
        } else {
            panic!("config should be an object");
        }
    }
}
