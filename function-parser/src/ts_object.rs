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
    // For multi-line input, prefer the indented parser which correctly handles
    // LLM output patterns like multi-word unquoted strings and multi-line arrays.
    // The TS-style parser silently truncates "Auwgent SDK Launch" to "Auwgent".
    if input.contains('\n') {
        if let Ok(obj) = parse_indented_assignment_object(input) {
            return Ok(obj);
        }
    }
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
        // Standalone closing brackets/braces are end-of-object markers, not keys
        if line.text == "]" || line.text == "}" {
            *index += 1;
            break;
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
    } else if value_text.starts_with('[') || value_text.starts_with('{') {
        // Multi-line array/object: collect indented continuation lines
        let mut full_value = value_text.clone();
        while *index < lines.len() && lines[*index].indent > current_indent {
            full_value.push_str(&format!("\n{}", &lines[*index].text));
            *index += 1;
        }
        // Grab a closing bracket/brace at the same indent level
        if *index < lines.len()
            && lines[*index].indent == current_indent
            && (lines[*index].text == "]" || lines[*index].text == "}")
        {
            full_value.push_str(&format!("\n{}", &lines[*index].text));
            *index += 1;
        }
        parse_ts_object(&full_value)
            .unwrap_or_else(|_| ASTValue::String(full_value))
    } else {
        // Parse the value while avoiding tokenizer corruption of special chars
        // like @ (emails) and . (URLs/domains). Only route through parse_ts_object
        // when the value clearly looks like a structured or primitive type.
        let trimmed = value_text.trim();
        if trimmed.starts_with('"') || trimmed.starts_with('\'') {
            // Quoted string — parse normally to handle escapes
            parse_ts_object(&value_text)
                .unwrap_or_else(|_| ASTValue::String(value_text.to_string()))
        } else if trimmed.starts_with('[') || trimmed.starts_with('{') || trimmed.starts_with('(') {
            // Structural value — parse normally
            parse_ts_object(&value_text)
                .unwrap_or_else(|_| ASTValue::String(value_text.to_string()))
        } else if trimmed == "true" {
            ASTValue::Boolean(true)
        } else if trimmed == "false" {
            ASTValue::Boolean(false)
        } else if trimmed == "null" {
            ASTValue::Null
        } else if trimmed.chars().next().map(|c| c.is_ascii_digit() || c == '-').unwrap_or(false) {
            // Starts with digit or minus — try as number, fall back to string
            parse_ts_object(&value_text)
                .unwrap_or_else(|_| ASTValue::String(value_text.to_string()))
        } else {
            // Raw string — preserve as-is to avoid tokenizer mangling
            // special characters like @ in emails or . in URLs.
            // But if it looks like a call expression (contains '('), try parsing it.
            if trimmed.contains('(') {
                match parse_ts_object(&value_text) {
                    Ok(val @ ASTValue::Call { .. }) => val,
                    _ => ASTValue::String(value_text.to_string()),
                }
            } else {
                ASTValue::String(value_text.to_string())
            }
        }
    };

    Ok((key, value))
}

fn parse_nested_value(
    lines: &[IndentedLine],
    index: &mut usize,
    parent_indent: usize,
) -> Result<ASTValue, String> {
    if *index >= lines.len() {
        return Ok(ASTValue::Null);
    }

    let next_line = &lines[*index];

    // Lines at lower indent than parent → no nested value
    if next_line.indent < parent_indent {
        return Ok(ASTValue::Null);
    }

    // Lines at same indent as parent → only allow [ or { as value start
    // (anything else is the next sibling key, not a nested value)
    if next_line.indent == parent_indent {
        if next_line.text.starts_with('[') || next_line.text.starts_with('{') {
            // JSON-style array/object at same indent level
            let anchor_indent = next_line.indent;
            let mut collected = String::new();
            while *index < lines.len() && lines[*index].indent >= anchor_indent {
                if !collected.is_empty() {
                    collected.push('\n');
                }
                collected.push_str(&lines[*index].text);
                *index += 1;
            }
            return parse_ts_object(&collected)
                .map_err(|e| format!("Failed to parse nested JSON value: {}", e));
        }
        return Ok(ASTValue::Null);
    }

    // Child indent > parent indent
    let child_indent = next_line.indent;
    if next_line.text.starts_with('-') {
        Ok(ASTValue::Array(parse_indented_array(
            lines,
            index,
            child_indent,
        )?))
    } else if next_line.text.starts_with('[') || next_line.text.starts_with('{') {
        // JSON-style array/object spanning multiple indented lines.
        // Collect all lines at child indent or deeper, then parse as one value.
        let mut collected = String::new();
        while *index < lines.len() && lines[*index].indent >= child_indent {
            if !collected.is_empty() {
                collected.push('\n');
            }
            collected.push_str(&lines[*index].text);
            *index += 1;
        }
        parse_ts_object(&collected)
            .map_err(|e| format!("Failed to parse nested JSON value: {}", e))
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
        } else if remainder.starts_with('{') || remainder.starts_with('[') {
            parse_ts_object(remainder)?
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
                let mut ident = s.clone();
                self.advance();

                if self.current == TokenKind::OpenParen {
                    self.advance();
                    let args = self.parse_object_like_args(TokenKind::CloseParen)?;
                    if self.current == TokenKind::CloseParen {
                        self.advance();
                    }
                    Ok(ASTValue::Call { name: ident, args })
                } else {
                    // Concatenate consecutive identifiers into a multi-word string.
                    // LLMs often output unquoted multi-word values like:
                    //   title: Write documentation
                    // But stop if the next identifier is followed by : or = (it's a key).
                    while let TokenKind::Identifier(next) = &self.current {
                        let next_word = next.clone();
                        // Peek ahead: if this identifier is followed by : or =,
                        // it's the next key, not a continuation of this value.
                        let saved = self.tokenizer.save_state();
                        let saved_current = self.current.clone();
                        self.advance(); // consume the identifier
                        if self.current == TokenKind::Equals {
                            // It's a key — restore and stop concatenating
                            self.tokenizer.restore_state(saved);
                            self.current = saved_current;
                            break;
                        }
                        // Not a key — it's part of the multi-word value
                        ident.push(' ');
                        ident.push_str(&next_word);
                        // self.current is already advanced past the identifier
                    }
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

    #[test]
    fn test_assignment_object_unquoted_string_values() {
        // Exact schema content format from LLM output
        let input = "name: Hiroshi\nage: 21\ncountry: Japan\nis_student: true";
        let result = parse_assignment_object(input);
        assert!(result.is_ok(), "Failed to parse schema content: {:?}", result.err());
        let obj = result.unwrap();
        assert_eq!(obj.get("name"), Some(&ASTValue::String("Hiroshi".to_string())));
        assert_eq!(obj.get("age"), Some(&ASTValue::Number(21.0)));
        assert_eq!(obj.get("country"), Some(&ASTValue::String("Japan".to_string())));
        assert_eq!(obj.get("is_student"), Some(&ASTValue::Boolean(true)));
    }

    #[test]
    fn test_assignment_object_multi_word_unquoted_strings() {
        // LLM often outputs unquoted multi-word strings
        let input = "project_name: Auwgent SDK Launch\nstatus: active";
        let result = parse_assignment_object(input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
        let obj = result.unwrap();
        assert_eq!(
            obj.get("project_name"),
            Some(&ASTValue::String("Auwgent SDK Launch".to_string()))
        );
        assert_eq!(
            obj.get("status"),
            Some(&ASTValue::String("active".to_string()))
        );
    }

    #[test]
    fn test_assignment_object_multiline_array() {
        // LLM outputs arrays across multiple indented lines
        let input = "project_name: Auwgent SDK Launch\ntasks: [\n  { title: Write documentation, priority: high, completed: false },\n  { title: Fix buffer bugs, priority: medium, completed: true },\n  { title: Publish to npm, priority: low, completed: false }\n]";
        let result = parse_assignment_object(input);
        assert!(result.is_ok(), "Failed to parse multi-line array: {:?}", result.err());
        let obj = result.unwrap();
        assert_eq!(
            obj.get("project_name"),
            Some(&ASTValue::String("Auwgent SDK Launch".to_string()))
        );
        if let Some(ASTValue::Array(tasks)) = obj.get("tasks") {
            assert_eq!(tasks.len(), 3, "Expected 3 tasks, got {}", tasks.len());
        } else {
            panic!("tasks should be an array, got {:?}", obj.get("tasks"));
        }
    }

    #[test]
    fn test_assignment_object_dash_prefixed_inline_object_array() {
        let input = "project_name: Auwgent SDK Launch\ntasks:\n  - { title: Write documentation, priority: high, completed: false }\n  - { title: Fix buffer bugs, priority: medium, completed: true }\n  - { title: Publish to npm, priority: low, completed: false }";
        let result = parse_assignment_object(input);
        assert!(result.is_ok(), "Failed to parse task array: {:?}", result.err());
        let obj = result.unwrap();

        assert_eq!(
            obj.get("project_name"),
            Some(&ASTValue::String("Auwgent SDK Launch".to_string()))
        );

        let Some(ASTValue::Array(tasks)) = obj.get("tasks") else {
            panic!("tasks should be an array, got {:?}", obj.get("tasks"));
        };
        assert_eq!(tasks.len(), 3);

        let ASTValue::Object(first_task) = &tasks[0] else {
            panic!("first task should be an object, got {:?}", tasks[0]);
        };
        assert_eq!(
            first_task.get("title"),
            Some(&ASTValue::String("Write documentation".to_string()))
        );
        assert_eq!(
            first_task.get("priority"),
            Some(&ASTValue::String("high".to_string()))
        );
        assert_eq!(
            first_task.get("completed"),
            Some(&ASTValue::Boolean(false))
        );
    }

    #[test]
    fn test_assignment_object_nested_array_on_next_line() {
        // LLM puts the array value on the next indented line after "key:"
        let input = "company_name: SnrRaptoPack\ncompany_departments:\n  [\n    {\n      dept_name: Engineering,\n      employees:\n        [\n          { name: Alice, role: Lead Developer, salary: 95000 },\n          { name: Bob, role: Backend Engineer, salary: null }\n        ]\n    },\n    {\n      dept_name: Design,\n      employees:\n        [\n          { name: Clara, role: UI Designer, salary: 72000 }\n        ]\n    }\n  ]";
        let result = parse_assignment_object(input);
        assert!(result.is_ok(), "Failed to parse nested array: {:?}", result.err());
        let obj = result.unwrap();
        assert_eq!(
            obj.get("company_name"),
            Some(&ASTValue::String("SnrRaptoPack".to_string()))
        );
        if let Some(ASTValue::Array(depts)) = obj.get("company_departments") {
            assert_eq!(depts.len(), 2, "Expected 2 departments, got {}", depts.len());
        } else {
            panic!("company_departments should be an array, got {:?}", obj.get("company_departments"));
        }
    }

    #[test]
    fn test_assignment_object_same_indent_array_on_next_line() {
        // Exact model output pattern: [ at same indent as key (indent=0)
        let input = "company_name: SnrRaptoPack\ncompany_departments:\n[\n  {\n    dept_name: Engineering\n    employees:\n    [\n      {\n        name: Alice\n        role: Lead Developer\n        salary: 95000\n      }\n      {\n        name: Bob\n        role: Backend Engineer\n        salary: null\n      }\n    ]\n  }\n  {\n    dept_name: Design\n    employees:\n    [\n      {\n        name: Clara\n        role: UI Designer\n        salary: 72000\n      }\n    ]\n  }\n]";
        let result = parse_assignment_object(input);
        assert!(result.is_ok(), "Failed to parse same-indent array: {:?}", result.err());
        let obj = result.unwrap();
        assert_eq!(
            obj.get("company_name"),
            Some(&ASTValue::String("SnrRaptoPack".to_string()))
        );
        if let Some(ASTValue::Array(depts)) = obj.get("company_departments") {
            assert_eq!(depts.len(), 2, "Expected 2 departments, got {}", depts.len());
        } else {
            panic!("company_departments should be an array, got {:?}", obj.get("company_departments"));
        }
    }

    #[test]
    fn test_assignment_object_preserves_emails_and_urls() {
        // Emails and URLs contain @ and . which the tokenizer would mangle
        let input = "person_email: shawn@example.com\nperson_id: usr_777\naccount_active: true\naccount_plan: pro";
        let result = parse_assignment_object(input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
        let obj = result.unwrap();
        assert_eq!(
            obj.get("person_email"),
            Some(&ASTValue::String("shawn@example.com".to_string()))
        );
        assert_eq!(
            obj.get("person_id"),
            Some(&ASTValue::String("usr_777".to_string()))
        );
        assert_eq!(
            obj.get("account_active"),
            Some(&ASTValue::Boolean(true))
        );
        assert_eq!(
            obj.get("account_plan"),
            Some(&ASTValue::String("pro".to_string()))
        );
    }
}
