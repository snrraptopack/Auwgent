use crate::ast::{ASTValue, TokenKind};
/// TypeScript-style object parser for @@out blocks
/// Supports both unquoted keys (TS-style) and quoted keys (JSON-style)
use crate::tokenizer::Tokenizer;
use std::collections::HashMap;

pub fn parse_ts_object(input: &str) -> Result<ASTValue, String> {
    let mut parser = TSObjectParser::new(input);
    parser.parse_value()
}

pub fn parse_assignment_object(input: &str) -> Result<HashMap<String, ASTValue>, String> {
    match parse_ts_object(&format!("{{{}}}", input)) {
        Ok(ASTValue::Object(obj)) => Ok(obj),
        Ok(_) => Err("Expected assignment object".to_string()),
        Err(_) => parse_indented_assignment_object(input),
    }
}

#[derive(Debug, Clone)]
struct IndentedLine {
    indent: usize,
    text: String,
}

fn parse_indented_assignment_object(input: &str) -> Result<HashMap<String, ASTValue>, String> {
    let lines = collect_indented_lines(input);
    if lines.is_empty() {
        return Ok(HashMap::new());
    }

    let mut index = 0;
    parse_indented_object(&lines, &mut index, lines[0].indent)
}

fn collect_indented_lines(input: &str) -> Vec<IndentedLine> {
    input
        .lines()
        .filter_map(|line| {
            let trimmed_end = line.trim_end();
            if trimmed_end.trim().is_empty() {
                return None;
            }

            let indent = line
                .chars()
                .take_while(|ch| ch.is_whitespace())
                .map(|ch| if ch == '\t' { 4 } else { 1 })
                .sum();

            Some(IndentedLine {
                indent,
                text: trimmed_end.trim_start().to_string(),
            })
        })
        .collect()
}

fn parse_indented_object(
    lines: &[IndentedLine],
    index: &mut usize,
    indent: usize,
) -> Result<HashMap<String, ASTValue>, String> {
    let mut map = HashMap::new();

    while *index < lines.len() {
        let line = &lines[*index];
        if line.indent < indent {
            break;
        }
        if line.indent > indent {
            return Err(format!("Unexpected indentation near '{}'", line.text));
        }
        if line.text.starts_with('-') {
            return Err(format!("Unexpected array item near '{}'", line.text));
        }

        let (key, value) = parse_object_entry(lines, index)?;
        map.insert(key, value);
    }

    Ok(map)
}

fn parse_object_entry(
    lines: &[IndentedLine],
    index: &mut usize,
) -> Result<(String, ASTValue), String> {
    let line = &lines[*index];
    let (key, value_text) = split_key_value(&line.text)
        .ok_or_else(|| format!("Expected 'key: value' or 'key = value' pair near '{}'", line.text))?;
    let current_indent = line.indent;
    *index += 1;

    let value = if value_text.is_empty() {
        parse_nested_value(lines, index, current_indent)?
    } else {
        parse_ts_object(&value_text)?
    };

    Ok((key, value))
}

fn parse_nested_value(
    lines: &[IndentedLine],
    index: &mut usize,
    parent_indent: usize,
) -> Result<ASTValue, String> {
    if *index >= lines.len() || lines[*index].indent <= parent_indent {
        return Ok(ASTValue::Null);
    }

    let child_indent = lines[*index].indent;
    if lines[*index].text.starts_with('-') {
        Ok(ASTValue::Array(parse_indented_array(
            lines,
            index,
            child_indent,
        )?))
    } else {
        Ok(ASTValue::Object(parse_indented_object(
            lines,
            index,
            child_indent,
        )?))
    }
}

fn parse_indented_array(
    lines: &[IndentedLine],
    index: &mut usize,
    indent: usize,
) -> Result<Vec<ASTValue>, String> {
    let mut items = Vec::new();

    while *index < lines.len() {
        let line = &lines[*index];
        if line.indent < indent {
            break;
        }
        if line.indent > indent {
            return Err(format!("Unexpected indentation near '{}'", line.text));
        }
        if !line.text.starts_with('-') {
            break;
        }

        let remainder = line.text[1..].trim_start();
        let current_indent = line.indent;
        *index += 1;

        let value = if remainder.is_empty() {
            parse_nested_value(lines, index, current_indent)?
        } else if let Some((key, value_text)) = split_key_value(remainder) {
            let mut object = HashMap::new();
            let first_value = if value_text.is_empty() {
                parse_nested_value(lines, index, current_indent)?
            } else {
                parse_ts_object(&value_text)?
            };
            object.insert(key, first_value);

            if *index < lines.len() && lines[*index].indent > current_indent {
                let nested_indent = lines[*index].indent;
                let nested_object = parse_indented_object(lines, index, nested_indent)?;
                for (nested_key, nested_value) in nested_object {
                    object.insert(nested_key, nested_value);
                }
            }

            ASTValue::Object(object)
        } else {
            parse_ts_object(remainder)?
        };

        items.push(value);
    }

    Ok(items)
}

fn split_key_value(text: &str) -> Option<(String, String)> {
    let separator_index = text.find(':').or_else(|| text.find('='))?;
    let key = text[..separator_index].trim();
    if key.is_empty() {
        return None;
    }

    Some((
        key.to_string(),
        text[separator_index + 1..].trim().to_string(),
    ))
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
                let ident = s.clone();
                self.advance();

                if self.current == TokenKind::OpenParen {
                    self.advance();
                    let args = self.parse_object_like_args(TokenKind::CloseParen)?;
                    if self.current == TokenKind::CloseParen {
                        self.advance();
                    }
                    Ok(ASTValue::Call { name: ident, args })
                } else {
                    Ok(ASTValue::String(ident))
                }
            }
            _ => Err(format!("Unexpected token: {:?}", self.current)),
        }
    }

    fn parse_object_like_args(
        &mut self,
        close_token: TokenKind,
    ) -> Result<HashMap<String, ASTValue>, String> {
        let mut map = HashMap::new();

        while self.current != TokenKind::EOF && self.current != close_token {
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
                _ => return Err(format!("Expected argument name, got: {:?}", self.current)),
            };

            if self.current != TokenKind::Equals {
                return Err(format!("Expected '=' or ':', got: {:?}", self.current));
            }
            self.advance();

            let value = self.parse_value()?;
            map.insert(key, value);

            if self.current == TokenKind::Comma {
                self.advance();
            }
        }

        Ok(map)
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
                _ => return Err(format!("Expected key, got: {:?}", self.current)),
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
            assert_eq!(
                obj.get("session_id"),
                Some(&ASTValue::String("123".to_string()))
            );
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
            assert_eq!(
                obj.get("session_id"),
                Some(&ASTValue::String("123".to_string()))
            );
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
                assert_eq!(
                    user.get("id"),
                    Some(&ASTValue::String("usr_123".to_string()))
                );
                assert_eq!(
                    user.get("name"),
                    Some(&ASTValue::String("Nana".to_string()))
                );
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

    #[test]
    fn test_assignment_object_supports_colon_syntax() {
        let input = "session_id: \"123\"\ncount: 42";
        let result = parse_assignment_object(input).unwrap();

        assert_eq!(
            result.get("session_id"),
            Some(&ASTValue::String("123".to_string()))
        );
        assert_eq!(result.get("count"), Some(&ASTValue::Number(42.0)));
    }

    #[test]
    fn test_assignment_object_supports_equals_syntax() {
        let input = "session_id = \"123\"\ncount = 42";
        let result = parse_assignment_object(input).unwrap();

        assert_eq!(
            result.get("session_id"),
            Some(&ASTValue::String("123".to_string()))
        );
        assert_eq!(result.get("count"), Some(&ASTValue::Number(42.0)));
    }

    #[test]
    fn test_assignment_object_supports_mixed_assignment_syntax() {
        let input = "session_id: \"123\"\ncount = 42";
        let result = parse_assignment_object(input).unwrap();

        assert_eq!(
            result.get("session_id"),
            Some(&ASTValue::String("123".to_string()))
        );
        assert_eq!(result.get("count"), Some(&ASTValue::Number(42.0)));
    }

    #[test]
    fn test_assignment_object_supports_call_expression_values() {
        let input = "action_onclick: delete(id: \"123\")";
        let result = parse_assignment_object(input).unwrap();

        assert_eq!(
            result.get("action_onclick"),
            Some(&ASTValue::Call {
                name: "delete".to_string(),
                args: HashMap::from([(
                    "id".to_string(),
                    ASTValue::String("123".to_string())
                )]),
            })
        );
    }
}
