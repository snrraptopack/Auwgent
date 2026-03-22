/// Block scanner for @@marker-based protocol
/// Scans for @@chat, @@tool, @@workflow, @@helper, @@out, @@result, @@error, @@end

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
    pub schema_name: Option<String>, // For @@out blocks (schema name on same line)
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

    fn peek_ahead(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += 1;
        Some(ch)
    }

    /// Check if we're at a @@marker
    fn check_marker(&self) -> Option<String> {
        if self.peek() != Some('@') || self.peek_ahead(1) != Some('@') {
            return None;
        }

        let mut marker = String::new();
        let mut i = 2; // Skip @@
        while let Some(ch) = self.chars.get(self.pos + i) {
            if ch.is_ascii_alphabetic() || *ch == '_' {
                marker.push(*ch);
                i += 1;
            } else {
                break;
            }
        }

        if marker.is_empty() {
            None
        } else {
            Some(marker)
        }
    }

    /// Consume a marker (@@word)
    fn consume_marker(&mut self) -> Option<String> {
        let marker = self.check_marker()?;
        self.pos += 2 + marker.len(); // Skip @@ + marker
        Some(marker)
    }

    pub fn scan(&mut self) -> Vec<Block> {
        let mut blocks = Vec::new();
        let mut implicit_chat = String::new();

        while self.pos < self.chars.len() {
            if let Some(marker) = self.check_marker() {
                // Flush any implicit chat text before this marker
                if !implicit_chat.trim().is_empty() {
                    blocks.push(Block {
                        block_type: BlockType::Chat,
                        content: implicit_chat.trim().to_string(),
                        schema_name: None,
                    });
                    implicit_chat.clear();
                }

                self.consume_marker();
                // Don't skip whitespace here - let read_until_marker_or_eof handle it

                match marker.as_str() {
                    "end" => {
                        // @@end just closes the current block, nothing to do
                        continue;
                    }
                    "chat" => {
                        let content = self.read_until_marker_or_eof();
                        blocks.push(Block {
                            block_type: BlockType::Chat,
                            content,
                            schema_name: None,
                        });
                    }
                    "tool" => {
                        let content = self.read_until_marker_or_eof();
                        blocks.push(Block {
                            block_type: BlockType::Tool,
                            content,
                            schema_name: None,
                        });
                    }
                    "workflow" => {
                        let content = self.read_until_marker_or_eof();
                        blocks.push(Block {
                            block_type: BlockType::Workflow,
                            content,
                            schema_name: None,
                        });
                    }
                    "helper" => {
                        let content = self.read_until_marker_or_eof();
                        blocks.push(Block {
                            block_type: BlockType::Helper,
                            content,
                            schema_name: None,
                        });
                    }
                    "out" => {
                        // Read schema name on the same line
                        let schema_name = self.read_schema_name();
                        let content = self.read_until_marker_or_eof();
                        blocks.push(Block {
                            block_type: BlockType::Out,
                            content,
                            schema_name: Some(schema_name),
                        });
                    }
                    "result" => {
                        let content = self.read_until_marker_or_eof();
                        blocks.push(Block {
                            block_type: BlockType::Result,
                            content,
                            schema_name: None,
                        });
                    }
                    "error" => {
                        let content = self.read_until_marker_or_eof();
                        blocks.push(Block {
                            block_type: BlockType::Error,
                            content,
                            schema_name: None,
                        });
                    }
                    _ => {
                        // Unknown marker - could be custom intent
                        // Read content and create Custom block
                        let content = self.read_until_marker_or_eof();
                        blocks.push(Block {
                            block_type: BlockType::Custom(marker),
                            content,
                            schema_name: None,
                        });
                    }
                }
            } else {
                // Not a marker - accumulate as implicit chat
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
                schema_name: None,
            });
        }

        blocks
    }

    /// Read schema name after @@out (on the same line)
    fn read_schema_name(&mut self) -> String {
        let mut name = String::new();
        while let Some(ch) = self.peek() {
            if ch == '\n' {
                self.advance();
                break;
            } else if ch.is_whitespace() {
                self.advance();
                if !name.is_empty() {
                    // Already got the name, skip remaining whitespace
                    continue;
                }
            } else if ch.is_ascii_alphanumeric() || ch == '_' {
                name.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        name
    }

    /// Read content until we hit @@end, another @@marker, or EOF
    fn read_until_marker_or_eof(&mut self) -> String {
        let mut content = String::new();

        while self.pos < self.chars.len() {
            if self.check_marker().is_some() {
                // Hit another marker - auto-close current block
                break;
            }

            if let Some(ch) = self.advance() {
                content.push(ch);
            }
        }

        content.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_chat_block() {
        let input = "@@chat\nHello world\n@@end";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, BlockType::Chat);
        assert_eq!(blocks[0].content, "Hello world");
    }

    #[test]
    fn test_auto_close() {
        let input = "@@chat\nHello\n@@tool\nfetch()";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].block_type, BlockType::Chat);
        assert_eq!(blocks[0].content, "Hello");
        assert_eq!(blocks[1].block_type, BlockType::Tool);
        assert_eq!(blocks[1].content, "fetch()");
    }

    #[test]
    fn test_implicit_chat() {
        let input = "Hello\n@@tool\nfetch()\n@@end\nGoodbye";
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
    fn test_out_with_schema() {
        let input = "@@out MySchema\n{data: \"test\"}";
        let mut scanner = BlockScanner::new(input);
        let blocks = scanner.scan();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, BlockType::Out);
        assert_eq!(blocks[0].schema_name, Some("MySchema".to_string()));
        assert!(blocks[0].content.contains("data"));
    }
}
