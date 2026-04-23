/// Block scanner for the bracket protocol
/// Scans for:
/// - [response_text]...[/response_text]
/// - [tool_call: name]...[/tool]
/// - [workflow_call: name]...[/workflow]
/// - [helper_call: name]...[/helper]
/// - [component: name, c_id:"instance_id"]...[/component]
/// - [render_component]...[/render_component]
/// - [schema: name]...[/schema]
/// - [custom: name]...[/custom]
/// - [result]...[/result]
/// - [error]...[/error]

#[derive(Debug, Clone, PartialEq)]
pub enum BlockType {
    Chat,
    Tool,
    Workflow,
    Helper,
    Component,
    RenderComponent,
    Out,
    Result,
    Error,
    Custom(String), // For custom intent blocks like @@my_custom_intent
}

#[derive(Debug, Clone)]
pub struct Block {
    pub block_type: BlockType,
    pub content: String,
    pub target_name: Option<String>,
    pub instance_id: Option<String>,
}

pub struct BlockScanner {
    chars: Vec<char>,
    pos: usize,
}

impl BlockScanner {
    const RESPONSE_TEXT_OPEN_BRACKET: &'static str = "[response_text]";
    const RESPONSE_TEXT_CLOSE_BRACKET: &'static str = "[/response_text]";
    const INCOMPLETE_HEADER_PREFIXES: [&'static str; 16] = [
        "[response_text]",
        "[/response_text]",
        "[tool_call:",
        "[/tool_call]",
        "[workflow_call:",
        "[/workflow]",
        "[helper_call:",
        "[/helper]",
        "[component:",
        "[/component]",
        "[render_component]",
        "[/render_component]",
        "[schema:",
        "[/schema]",
        "[custom:",
        "[/custom]",
    ];

    pub fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += 1;
        Some(ch)
    }

    fn check_literal(&self, literal: &str) -> bool {
        let literal_chars: Vec<char> = literal.chars().collect();
        if self.pos + literal_chars.len() > self.chars.len() {
            return false;
        }

        for (idx, ch) in literal_chars.iter().enumerate() {
            if self.chars[self.pos + idx] != *ch {
                return false;
            }
        }

        true
    }

    fn consume_literal(&mut self, literal: &str) -> bool {
        if !self.check_literal(literal) {
            return false;
        }

        self.pos += literal.chars().count();
        true
    }

    fn check_incomplete_literal_prefix(&self, literal: &str) -> bool {
        let literal_len = literal.chars().count();
        let remaining_len = self.chars.len().saturating_sub(self.pos);
        if remaining_len == 0 || remaining_len >= literal_len {
            return false;
        }

        let remaining: String = self.chars[self.pos..].iter().collect();
        literal.starts_with(&remaining)
    }

    fn check_incomplete_response_text_open(&self) -> bool {
        self.check_incomplete_literal_prefix(Self::RESPONSE_TEXT_OPEN_BRACKET)
    }

    fn check_incomplete_response_text_close(&self) -> bool {
        self.check_incomplete_literal_prefix(Self::RESPONSE_TEXT_CLOSE_BRACKET)
    }

    fn check_incomplete_known_header_prefix(&self) -> bool {
        if self.peek() != Some('[') || self.try_read_header().is_some() {
            return false;
        }

        let remaining: String = self.chars[self.pos..].iter().collect();
        Self::INCOMPLETE_HEADER_PREFIXES
            .iter()
            .any(|literal| remaining.starts_with(literal))
    }

    fn try_read_header(&self) -> Option<String> {
        if self.peek() != Some('[') {
            return None;
        }

        let mut idx = self.pos + 1;
        let mut header = String::new();
        while let Some(ch) = self.chars.get(idx) {
            if *ch == ']' {
                return Some(header.trim().to_string());
            }
            header.push(*ch);
            idx += 1;
        }

        None
    }

    /// Returns the raw (untrimmed) character count between `[` and `]`.
    fn raw_header_len(&self) -> usize {
        if self.peek() != Some('[') {
            return 0;
        }
        let mut idx = self.pos + 1;
        let mut count = 0;
        while let Some(ch) = self.chars.get(idx) {
            if *ch == ']' {
                return count;
            }
            count += 1;
            idx += 1;
        }
        0
    }

    fn consume_header(&mut self) -> Option<String> {
        let header = self.try_read_header()?;
        // Use the raw (untrimmed) length between [ and ] for accurate advancement.
        // try_read_header trims whitespace from the header, but we must advance
        // past the full raw header including any trailing spaces before `]`.
        let raw_len = self.raw_header_len();
        self.pos += raw_len + 2; // +2 for '[' and ']'
        Some(header)
    }

    fn is_known_closing_header(&self, header: &str) -> bool {
        matches!(
            header.trim(),
            "/tool_call"
                | "/workflow"
                | "/helper"
                | "/component"
                | "/render_component"
                | "/schema"
                | "/custom"
                | "/result"
                | "/error"
                | "/response_text"
        )
    }

    fn is_known_opening_header_kind(&self, header: &str) -> bool {
        let header = header.trim();
        if header.eq_ignore_ascii_case("response_text")
            || header.eq_ignore_ascii_case("render_component")
            || header.eq_ignore_ascii_case("result")
            || header.eq_ignore_ascii_case("error")
        {
            return true;
        }

        matches!(
            header.split_once(':').map(|(kind, _)| kind.trim()),
            Some("tool_call")
                | Some("workflow_call")
                | Some("helper_call")
                | Some("component")
                | Some("schema")
                | Some("custom")
        )
    }

    fn parse_header(
        &self,
        header: &str,
    ) -> Option<(BlockType, Option<String>, Option<String>, &'static str)> {
        let header = header.trim();

        if header.eq_ignore_ascii_case("response_text") {
            return Some((
                BlockType::Chat,
                None,
                None,
                Self::RESPONSE_TEXT_CLOSE_BRACKET,
            ));
        }

        if header.eq_ignore_ascii_case("result") {
            return Some((BlockType::Result, None, None, "[/result]"));
        }

        if header.eq_ignore_ascii_case("error") {
            return Some((BlockType::Error, None, None, "[/error]"));
        }

        if header.eq_ignore_ascii_case("render_component") {
            return Some((BlockType::RenderComponent, None, None, "[/render_component]"));
        }

        let (kind, target) = header.split_once(':')?;
        let kind = kind.trim();
        let target = target.trim();
        if target.is_empty() {
            return None;
        }

        match kind {
            "tool_call" => {
                if target
                    .chars()
                    .any(|ch| ch.is_whitespace() || matches!(ch, '[' | ']' | '<' | '>'))
                {
                    return None;
                }
                Some((BlockType::Tool, Some(target.to_string()), None, "[/tool_call]"))
            }
            "workflow_call" => {
                if target
                    .chars()
                    .any(|ch| ch.is_whitespace() || matches!(ch, '[' | ']' | '<' | '>'))
                {
                    return None;
                }
                Some((
                    BlockType::Workflow,
                    Some(target.to_string()),
                    None,
                    "[/workflow]",
                ))
            }
            "helper_call" => {
                if target
                    .chars()
                    .any(|ch| ch.is_whitespace() || matches!(ch, '[' | ']' | '<' | '>'))
                {
                    return None;
                }
                Some((BlockType::Helper, Some(target.to_string()), None, "[/helper]"))
            }
            "component" => {
                let (component_name, instance_id) = parse_component_header_target(target)?;
                Some((
                    BlockType::Component,
                    Some(component_name),
                    Some(instance_id),
                    "[/component]",
                ))
            }
            "schema" => {
                if target
                    .chars()
                    .any(|ch| ch.is_whitespace() || matches!(ch, '[' | ']' | '<' | '>'))
                {
                    return None;
                }
                Some((BlockType::Out, Some(target.to_string()), None, "[/schema]"))
            }
            "custom" => Some((
                BlockType::Custom(target.to_string()),
                Some(target.to_string()),
                None,
                "[/custom]",
            )),
            _ => None,
        }
    }

    fn close_literal_for_header_kind(&self, header: &str) -> Option<&'static str> {
        let header = header.trim();

        if header.eq_ignore_ascii_case("response_text") {
            return Some(Self::RESPONSE_TEXT_CLOSE_BRACKET);
        }

        if header.eq_ignore_ascii_case("render_component") {
            return Some("[/render_component]");
        }

        if header.eq_ignore_ascii_case("result") {
            return Some("[/result]");
        }

        if header.eq_ignore_ascii_case("error") {
            return Some("[/error]");
        }

        match header.split_once(':').map(|(kind, _)| kind.trim()) {
            Some("tool_call") => Some("[/tool_call]"),
            Some("workflow_call") => Some("[/workflow]"),
            Some("helper_call") => Some("[/helper]"),
            Some("component") => Some("[/component]"),
            Some("schema") => Some("[/schema]"),
            Some("custom") => Some("[/custom]"),
            _ => None,
        }
    }

    fn read_until_literal_or_eof(&mut self, literal: &str) -> String {
        let mut content = String::new();

        while self.pos < self.chars.len() {
            if self.check_literal(literal) {
                break;
            }

            if self.check_literal(Self::RESPONSE_TEXT_OPEN_BRACKET) {
                break;
            }

            if self.check_literal(Self::RESPONSE_TEXT_CLOSE_BRACKET) {
                break;
            }

            if self.check_incomplete_response_text_open() {
                break;
            }

            if self.check_incomplete_response_text_close() {
                break;
            }

            if self.check_incomplete_known_header_prefix() {
                break;
            }

            if let Some(header) = self.try_read_header()
                && (self.parse_header(&header).is_some() || self.is_known_closing_header(&header))
            {
                break;
            }

            if let Some(ch) = self.advance() {
                content.push(ch);
            }
        }

        content.trim().to_string()
    }

    pub fn scan(&mut self) -> Vec<Block> {
        let mut blocks = Vec::new();
        let mut implicit_chat = String::new();

        while self.pos < self.chars.len() {
            if self.check_literal(Self::RESPONSE_TEXT_OPEN_BRACKET) {
                if !implicit_chat.trim().is_empty() {
                    blocks.push(Block {
                        block_type: BlockType::Chat,
                        content: implicit_chat.trim().to_string(),
                        target_name: None,
                        instance_id: None,
                    });
                    implicit_chat.clear();
                }

                self.consume_literal(Self::RESPONSE_TEXT_OPEN_BRACKET);
                let content = self.read_until_literal_or_eof(Self::RESPONSE_TEXT_CLOSE_BRACKET);
                self.consume_literal(Self::RESPONSE_TEXT_CLOSE_BRACKET);
                blocks.push(Block {
                    block_type: BlockType::Chat,
                    content,
                    target_name: None,
                    instance_id: None,
                });
            } else if self.check_incomplete_response_text_open()
                || self.check_incomplete_response_text_close()
                || self.check_incomplete_known_header_prefix()
            {
                if !implicit_chat.trim().is_empty() {
                    blocks.push(Block {
                        block_type: BlockType::Chat,
                        content: implicit_chat.trim().to_string(),
                        target_name: None,
                        instance_id: None,
                    });
                    implicit_chat.clear();
                }
                break;
            } else if let Some(header) = self.try_read_header() {
                if let Some((block_type, target_name, instance_id, close_literal)) =
                    self.parse_header(&header)
                {
                    if !implicit_chat.trim().is_empty() {
                        blocks.push(Block {
                            block_type: BlockType::Chat,
                            content: implicit_chat.trim().to_string(),
                            target_name: None,
                            instance_id: None,
                        });
                        implicit_chat.clear();
                    }

                    self.consume_header();
                    let content = self.read_until_literal_or_eof(close_literal);
                    self.consume_literal(close_literal);
                    blocks.push(Block {
                        block_type,
                        content,
                        target_name,
                        instance_id,
                    });
                } else if self.is_known_opening_header_kind(&header) {
                    self.consume_header();
                    if let Some(close_literal) = self.close_literal_for_header_kind(&header) {
                        self.read_until_literal_or_eof(close_literal);
                        self.consume_literal(close_literal);
                    }
                } else if self.is_known_closing_header(&header) {
                    self.consume_header();
                } else if let Some(ch) = self.advance() {
                    implicit_chat.push(ch);
                }
            } else if let Some(ch) = self.advance() {
                implicit_chat.push(ch);
            }
        }

        // Flush any remaining implicit chat
        if !implicit_chat.trim().is_empty() {
            blocks.push(Block {
                block_type: BlockType::Chat,
                content: implicit_chat.trim().to_string(),
                target_name: None,
                instance_id: None,
            });
        }

        blocks
    }
}

fn parse_component_header_target(target: &str) -> Option<(String, String)> {
    let (component_name, metadata) = target.split_once(',')?;
    let component_name = component_name.trim();
    if component_name.is_empty()
        || component_name
            .chars()
            .any(|ch| ch.is_whitespace() || matches!(ch, '[' | ']' | '<' | '>'))
    {
        return None;
    }

    let (key, raw_value) = metadata.split_once(':')?;
    if key.trim() != "c_id" {
        return None;
    }

    let raw_value = raw_value.trim();
    if !raw_value.starts_with('"') || !raw_value.ends_with('"') || raw_value.len() < 2 {
        return None;
    }

    let instance_id = raw_value[1..raw_value.len() - 1].trim().to_string();
    if instance_id.is_empty()
        || instance_id
            .chars()
            .any(|ch| matches!(ch, '[' | ']' | '<' | '>' | '"'))
    {
        return None;
    }

    Some((component_name.to_string(), instance_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_response_text_block() {
        let input = "[response_text]Hello world[/response_text]";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, BlockType::Chat);
        assert_eq!(blocks[0].content, "Hello world");
    }

    #[test]
    fn test_tool_block() {
        let input = "[tool_call: fetch_user]\nid: \"123\"\n[/tool_call]";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, BlockType::Tool);
        assert_eq!(blocks[0].target_name, Some("fetch_user".to_string()));
        assert_eq!(blocks[0].instance_id, None);
        assert_eq!(blocks[0].content, "id: \"123\"");
    }

    #[test]
    fn test_component_block_with_required_instance_id() {
        let input = "[component: Button, c_id:\"confirm_order_button\"]\nlabel: \"Confirm\"\naction_onclick: \"confirm_order\"\n[/component]";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, BlockType::Component);
        assert_eq!(blocks[0].target_name, Some("Button".to_string()));
        assert_eq!(
            blocks[0].instance_id,
            Some("confirm_order_button".to_string())
        );
        assert!(blocks[0].content.contains("action_onclick"));
    }

    #[test]
    fn test_render_component_block() {
        let input = "[render_component]\nroot: \"checkout_screen\"\n[/render_component]";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, BlockType::RenderComponent);
        assert_eq!(blocks[0].target_name, None);
        assert_eq!(blocks[0].instance_id, None);
        assert_eq!(blocks[0].content, "root: \"checkout_screen\"");
    }

    #[test]
    fn test_implicit_chat() {
        let input = "Hello\n[tool_call: fetch]\nid: \"123\"\n[/tool_call]\nGoodbye";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].block_type, BlockType::Chat);
        assert_eq!(blocks[0].content, "Hello");
        assert_eq!(blocks[1].block_type, BlockType::Tool);
        assert_eq!(blocks[2].block_type, BlockType::Chat);
        assert_eq!(blocks[2].content, "Goodbye");
    }

    #[test]
    fn test_schema_with_target_name() {
        let input = "[schema: MySchema]\ndata: \"test\"\n[/schema]";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, BlockType::Out);
        assert_eq!(blocks[0].target_name, Some("MySchema".to_string()));
        assert!(blocks[0].content.contains("data"));
    }

    #[test]
    fn test_auto_closes_when_next_block_starts() {
        let input = "[response_text]Hello\n[tool_call: fetch_user]\nid: \"123\"\n[/tool_call]";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].block_type, BlockType::Chat);
        assert_eq!(blocks[0].content, "Hello");
        assert_eq!(blocks[1].block_type, BlockType::Tool);
        assert_eq!(blocks[1].target_name, Some("fetch_user".to_string()));
    }

    #[test]
    fn test_ignores_stray_closing_headers() {
        let input = "[tool_call: fetch_user]\nid: \"123\"\n[/workflow]\n[response_text]Done[/response_text]";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].block_type, BlockType::Tool);
        assert_eq!(blocks[0].content, "id: \"123\"");
        assert_eq!(blocks[1].block_type, BlockType::Chat);
        assert_eq!(blocks[1].content, "Done");
    }

    #[test]
    fn test_incomplete_response_text_open_is_not_emitted_as_chat() {
        let input = "[response_text";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert!(blocks.is_empty());
    }

    #[test]
    fn test_incomplete_response_text_open_flushes_prior_chat_only() {
        let input = "Hello[response_text";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, BlockType::Chat);
        assert_eq!(blocks[0].content, "Hello");
    }

    #[test]
    fn test_incomplete_response_text_close_is_not_emitted_as_chat() {
        let input = "[response_text]Hello[/response_text";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, BlockType::Chat);
        assert_eq!(blocks[0].content, "Hello");
    }

    #[test]
    fn test_response_text_accepts_bracket_close() {
        let input = "[response_text]Hello world[/response_text]";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, BlockType::Chat);
        assert_eq!(blocks[0].content, "Hello world");
    }

    #[test]
    fn test_rejects_malformed_tool_header_with_whitespace_in_target() {
        let input = "[tool_call: user_name To get your name]\n[/tool_call]";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert!(blocks.is_empty());
    }

    #[test]
    fn test_rejects_component_header_without_c_id() {
        let input = "[component: Button]\nlabel: \"Confirm\"\n[/component]";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert!(blocks.is_empty());
    }

    #[test]
    fn test_ignores_stray_response_text_closing_header() {
        let input = "[response_text]Hello[/response_text][/response_text]";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, BlockType::Chat);
        assert_eq!(blocks[0].content, "Hello");
    }

    #[test]
    fn test_incomplete_tool_header_is_not_emitted_as_chat() {
        let input = "[tool_call: user_name";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert!(blocks.is_empty());
    }

    #[test]
    fn test_incomplete_tool_header_flushes_prior_chat_only() {
        let input = "Thinking...[tool_call: user_name";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, BlockType::Chat);
        assert_eq!(blocks[0].content, "Thinking...");
    }

    #[test]
    fn test_response_text_and_schema_both_parsed() {
        // Exact model output pattern from the bug report
        let input = " \n[response_text]\nHiroshi is a 21-year-old student from Japan.\n[/response_text]\n[schema: Output ]\nname: Hiroshi\nage: 21\ncountry: Japan\nis_student: true\n[/schema]";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 2, "Expected 2 blocks (Chat + Out), got {}: {:?}", blocks.len(), blocks);
        assert_eq!(blocks[0].block_type, BlockType::Chat);
        assert_eq!(blocks[0].content, "Hiroshi is a 21-year-old student from Japan.");
        assert_eq!(blocks[1].block_type, BlockType::Out);
        assert_eq!(blocks[1].target_name, Some("Output".to_string()));
        assert!(blocks[1].content.contains("name: Hiroshi"));
        assert!(blocks[1].content.contains("age: 21"));
    }

    #[test]
    fn test_tool_call_llama_reproduce(){
        let input = " \n[tool_call: get_user_name_age] \n[/tool_call]\n[tool_call: get_location] \n[/tool_call]";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 2, "expected 2 tools got {} : {:?}", blocks.len(), blocks);
        assert_eq!(blocks[0].block_type, BlockType::Tool);
        assert_eq!(blocks[1].block_type, BlockType::Tool);
        assert_eq!(blocks[0].target_name, Some("get_user_name_age".to_string()));
        assert_eq!(blocks[1].target_name, Some("get_location".to_string()));
    }
}
