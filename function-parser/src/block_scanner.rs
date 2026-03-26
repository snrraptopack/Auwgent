/// Block scanner for the tag/bracket protocol
/// Scans for:
/// - <response_text>...</response_text>
/// - [tool_call: name]...[/tool]
/// - [workflow_call: name]...[/workflow]
/// - [helper_call: name]...[/helper]
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
}

pub struct BlockScanner {
    chars: Vec<char>,
    pos: usize,
}

impl BlockScanner {
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

    fn consume_header(&mut self) -> Option<String> {
        let header = self.try_read_header()?;
        self.pos += header.chars().count() + 2;
        Some(header)
    }

    fn is_known_closing_header(&self, header: &str) -> bool {
        matches!(
            header.trim(),
            "/tool" | "/workflow" | "/helper" | "/schema" | "/custom" | "/result" | "/error"
        )
    }

    fn parse_header(&self, header: &str) -> Option<(BlockType, Option<String>, &'static str)> {
        let header = header.trim();

        if header.eq_ignore_ascii_case("result") {
            return Some((BlockType::Result, None, "[/result]"));
        }

        if header.eq_ignore_ascii_case("error") {
            return Some((BlockType::Error, None, "[/error]"));
        }

        let (kind, target) = header.split_once(':')?;
        let kind = kind.trim();
        let target = target.trim();
        if target.is_empty() {
            return None;
        }

        match kind {
            "tool_call" => Some((BlockType::Tool, Some(target.to_string()), "[/tool]")),
            "workflow_call" => Some((BlockType::Workflow, Some(target.to_string()), "[/workflow]")),
            "helper_call" => Some((BlockType::Helper, Some(target.to_string()), "[/helper]")),
            "schema" => Some((BlockType::Out, Some(target.to_string()), "[/schema]")),
            "custom" => Some((
                BlockType::Custom(target.to_string()),
                Some(target.to_string()),
                "[/custom]",
            )),
            _ => None,
        }
    }

    fn read_until_literal_or_eof(&mut self, literal: &str) -> String {
        let mut content = String::new();

        while self.pos < self.chars.len() {
            if self.check_literal(literal) {
                break;
            }

            if self.check_literal("<response_text>") {
                break;
            }

            if let Some(header) = self.try_read_header() {
                if self.parse_header(&header).is_some() || self.is_known_closing_header(&header) {
                    break;
                }
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
            if self.check_literal("<response_text>") {
                if !implicit_chat.trim().is_empty() {
                    blocks.push(Block {
                        block_type: BlockType::Chat,
                        content: implicit_chat.trim().to_string(),
                        target_name: None,
                    });
                    implicit_chat.clear();
                }

                self.consume_literal("<response_text>");
                let content = self.read_until_literal_or_eof("</response_text>");
                self.consume_literal("</response_text>");
                blocks.push(Block {
                    block_type: BlockType::Chat,
                    content,
                    target_name: None,
                });
            } else if let Some(header) = self.try_read_header() {
                if let Some((block_type, target_name, close_literal)) = self.parse_header(&header) {
                    if !implicit_chat.trim().is_empty() {
                        blocks.push(Block {
                            block_type: BlockType::Chat,
                            content: implicit_chat.trim().to_string(),
                            target_name: None,
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
                    });
                } else if self.is_known_closing_header(&header) {
                    self.consume_header();
                } else if let Some(ch) = self.advance() {
                    implicit_chat.push(ch);
                }
            } else {
                if let Some(ch) = self.advance() {
                    implicit_chat.push(ch);
                }
            }
        }

        // Flush any remaining implicit chat
        if !implicit_chat.trim().is_empty() {
            blocks.push(Block {
                block_type: BlockType::Chat,
                content: implicit_chat.trim().to_string(),
                target_name: None,
            });
        }

        blocks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_response_text_block() {
        let input = "<response_text>Hello world</response_text>";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, BlockType::Chat);
        assert_eq!(blocks[0].content, "Hello world");
    }

    #[test]
    fn test_tool_block() {
        let input = "[tool_call: fetch_user]\nid: \"123\"\n[/tool]";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, BlockType::Tool);
        assert_eq!(blocks[0].target_name, Some("fetch_user".to_string()));
        assert_eq!(blocks[0].content, "id: \"123\"");
    }

    #[test]
    fn test_implicit_chat() {
        let input = "Hello\n[tool_call: fetch]\nid: \"123\"\n[/tool]\nGoodbye";
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
        let input = "<response_text>Hello\n[tool_call: fetch_user]\nid: \"123\"\n[/tool]";
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
        let input = "[tool_call: fetch_user]\nid: \"123\"\n[/workflow]\n<response_text>Done</response_text>";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].block_type, BlockType::Tool);
        assert_eq!(blocks[0].content, "id: \"123\"");
        assert_eq!(blocks[1].block_type, BlockType::Chat);
        assert_eq!(blocks[1].content, "Done");
    }
}
