use super::types::*;

// ═══════════════════════════════════════════════════════════════════════════
// TOKENIZER STATE
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct TokenizerState {
    /// Current position in input (index in chars vector)
    pos: usize,
    /// Current line (1-indexed)
    line: usize,
    /// Current column (1-indexed)
    column: usize,
    /// Stack of indent levels
    indent_stack: Vec<usize>,
    /// Current indent level
    current_indent: usize,
    /// Pending tokens to emit
    pending: Vec<Token>,
    /// Whether we're at line start
    at_line_start: bool,
    after_colon: bool,
    pub partial_token: String,
}

impl Default for TokenizerState {
    fn default() -> Self {
        Self {
            pos: 0,
            line: 1,
            column: 1,
            indent_stack: vec![0],
            current_indent: 0,
            pending: Vec::new(),
            at_line_start: true,
            after_colon: false,
            partial_token: String::new(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TOKENIZER CLASS
// ═══════════════════════════════════════════════════════════════════════════

pub struct Tokenizer {
    input: Vec<char>,
    state: TokenizerState,
    options: ParserOptions,
    finishing: bool,
}

impl Tokenizer {
    pub fn new(options: Option<ParserOptions>) -> Self {
        let opts = options.unwrap_or(ParserOptions {
            indent_size: Some(2),
            allow_tabs: Some(false),
            preserve_comments: Some(false),
            strict: Some(false),
            intent_schema: None,
            intent_key: None,
            middleware: None,
        });

        Self {
            input: Vec::new(),
            state: TokenizerState::default(),
            options: opts,
            finishing: false,
        }
    }

    /// Reset tokenizer state
    pub fn reset(&mut self) {
        self.input.clear();
        self.state = TokenizerState::default();
        self.finishing = false;
    }

    /// Add input to tokenize
    pub fn write(&mut self, chunk: &str) {
        self.input.extend(chunk.chars());
    }

    /// Get current position
    pub fn get_position(&self) -> Position {
        Position {
            line: self.state.line,
            column: self.state.column,
            offset: self.state.pos,
        }
    }

    /// Check if more input is available
    pub fn has_more(&self) -> bool {
        self.state.pos < self.input.len() || !self.state.pending.is_empty()
    }

    /// Get next token (or None if exhausted)
    pub fn next_token(&mut self) -> Option<Token> {
        // Return pending tokens first
        if !self.state.pending.is_empty() {
            return Some(self.state.pending.remove(0));
        }

        // Check if we have more input
        if self.state.pos >= self.input.len() {
            return None;
        }

        // At line start, handle indentation
        if self.state.at_line_start {
            return self.tokenize_line_start();
        }

        // Otherwise, tokenize content
        self.tokenize_content()
    }

    /// Tokenize all remaining input
    pub fn tokenize_all(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        while let Some(token) = self.next_token() {
            tokens.push(token);
        }
        // Emit remaining DEDENTs and EOF
        tokens.extend(self.finalize());
        tokens
    }

    /// Finalize tokenization (emit closing DEDENTs and EOF)
    pub fn finalize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();

        // Flush any pending partial tokens
        self.finishing = true;
        while let Some(token) = self.next_token() {
            tokens.push(token);
        }
        self.finishing = false;

        // Emit DEDENTs for all open blocks
        while self.state.indent_stack.len() > 1 {
            self.state.indent_stack.pop();
            tokens.push(self.create_token(TokenType::Dedent, String::new()));
        }

        // Emit EOF
        tokens.push(self.create_token(TokenType::Eof, String::new()));

        tokens
    }

    pub fn get_partial_token(&self) -> String {
        self.state.partial_token.clone()
    }

    // ─────────────────────────────────────────────────────────────────────────
    // PRIVATE METHODS
    // ─────────────────────────────────────────────────────────────────────────

    fn tokenize_line_start(&mut self) -> Option<Token> {
        let indent = self.consume_indent();
        let indent_size = self.options.indent_size.unwrap_or(2);
        let indent_level = indent / indent_size;
        let current_level = *self.state.indent_stack.last().unwrap_or(&0);

        self.state.at_line_start = false;

        // Check for blank line or comment-only line
        let peek = self.peek_char(0);
        if peek == '\n' || peek == '\r' {
            return Some(self.tokenize_newline());
        }

        if peek == '#' {
            return Some(self.tokenize_comment());
        }

        // Handle indent level changes
        if indent_level > current_level {
            // Indent increase
            self.state.indent_stack.push(indent_level);
            self.state.current_indent = indent_level;

            // Should properly stringify the indent, currently just simulating spaces
            let indent_str = " ".repeat(indent);
            return Some(self.create_token(TokenType::Indent, indent_str));
        } else if indent_level < current_level {
            // Dedent
            self.state.current_indent = indent_level;

            while self.state.indent_stack.len() > 1
                && *self.state.indent_stack.last().unwrap_or(&0) > indent_level
            {
                self.state.indent_stack.pop();
                self.state
                    .pending
                    .push(self.create_token(TokenType::Dedent, String::new()));
            }

            if !self.state.pending.is_empty() {
                return Some(self.state.pending.remove(0));
            }
            return self.tokenize_content();
        }

        // Same level, continue
        self.state.current_indent = indent_level;
        self.tokenize_content()
    }

    fn tokenize_content(&mut self) -> Option<Token> {
        self.skip_spaces();

        let char = self.peek_char(0);
        if char == '\0' {
            return None;
        }

        // Newline
        if char == '\n' || char == '\r' {
            return Some(self.tokenize_newline());
        }

        // Comment
        if char == '#' {
            return Some(self.tokenize_comment());
        }

        // Dash (sequence item)
        if char == '-' && self.peek_char(1) == ' ' {
            self.advance();
            return Some(self.create_token(TokenType::Dash, "-".to_string()));
        }

        // Quoted string
        if char == '"' || char == '\'' {
            return self.tokenize_quoted_string(char);
        }

        // Colon
        if char == ':' {
            self.advance();
            self.state.after_colon = true;
            return Some(self.create_token(TokenType::Colon, ":".to_string()));
        }

        if char == '|' {
            return self.tokenize_multiline_string();
        }

        // Flow collections { } [ ]
        if char == '{' || char == '[' {
            return Some(self.tokenize_flow_collection(char));
        }

        // Key or Scalar
        self.tokenize_key_or_scalar()
    }

    fn tokenize_flow_collection(&mut self, open_char: char) -> Token {
        let start_col = self.state.column;
        let close_char = if open_char == '{' { '}' } else { ']' };
        let mut value = String::new();
        let mut depth = 0;

        while self.state.pos < self.input.len() {
            let c = self.peek_char(0);

            if c == open_char {
                depth += 1;
            }
            if c == close_char {
                depth -= 1;
            }

            value.push(c);
            self.advance();

            if depth == 0 {
                break;
            }
            // Temporarily allow spanning lines for inline objects if needed,
            // but the original had a break here. Flow collections are limited.
            if c == '\n' || c == '\r' {
                break;
            }
        }

        if depth != 0 && !self.finishing {
            self.state.partial_token = value.clone();
        } else {
            self.state.partial_token.clear();
        }

        self.state.after_colon = false;
        Token {
            kind: TokenType::Scalar,
            value: value.trim().to_string(),
            line: self.state.line,
            column: start_col,
            indent: self.state.current_indent,
        }
    }

    fn tokenize_key_or_scalar(&mut self) -> Option<Token> {
        let start_col = self.state.column;
        let start_pos = self.state.pos;
        let start_line = self.state.line;
        let mut value = String::new();

        while self.state.pos < self.input.len() {
            let c = self.peek_char(0);

            if c == '\n' || c == '\r' || c == '#' {
                break;
            }

            if c == ':' {
                let next = self.peek_char(1);
                if !self.state.after_colon
                    && (next == ' ' || next == '\n' || next == '\r' || next == '\0')
                {
                    break;
                }
            }

            value.push(c);
            self.advance();
        }

        if self.state.pos >= self.input.len() && !self.finishing {
            // Incomplete, rewind
            self.state.partial_token = value;
            self.state.pos = start_pos;
            self.state.column = start_col;
            self.state.line = start_line;
            return None;
        }

        self.state.partial_token.clear();

        let value = value.trim().to_string();

        if self.peek_char(0) == ':' {
            return Some(Token {
                kind: TokenType::Key,
                value,
                line: self.state.line,
                column: start_col,
                indent: self.state.current_indent,
            });
        }

        self.state.after_colon = false;
        Some(Token {
            kind: TokenType::Scalar,
            value,
            line: self.state.line,
            column: start_col,
            indent: self.state.current_indent,
        })
    }

    fn tokenize_quoted_string(&mut self, quote: char) -> Option<Token> {
        let start_col = self.state.column;
        let start_pos = self.state.pos;
        let start_line = self.state.line;

        self.advance(); // consume open quote

        let mut value = String::new();
        let mut closed = false;

        while self.state.pos < self.input.len() {
            let c = self.peek_char(0);

            if c == quote {
                closed = true;
                self.advance();
                break;
            }

            if c == '\\' {
                self.advance();
                let escaped = self.peek_char(0);
                match escaped {
                    'n' => value.push('\n'),
                    't' => value.push('\t'),
                    'r' => value.push('\r'),
                    '\\' => value.push('\\'),
                    '"' => value.push('"'),
                    '\'' => value.push('\''),
                    _ => value.push(escaped),
                }
                self.advance();
                continue;
            }

            value.push(c);
            self.advance();
        }

        if !closed && !self.finishing {
            self.state.partial_token = value;
            self.state.pos = start_pos;
            self.state.column = start_col;
            self.state.line = start_line;
            return None;
        }

        self.state.partial_token.clear();
        self.state.after_colon = false;
        Some(Token {
            kind: TokenType::Quoted,
            value,
            line: self.state.line,
            column: start_col,
            indent: self.state.current_indent,
        })
    }

    fn tokenize_comment(&mut self) -> Token {
        let start_col = self.state.column;
        let mut value = String::new();

        self.advance(); // consume #

        while self.state.pos < self.input.len() {
            let c = self.peek_char(0);
            if c == '\n' || c == '\r' {
                break;
            }
            value.push(c);
            self.advance();
        }

        let preserve = self.options.preserve_comments.unwrap_or(false);
        if !preserve {
            return self.tokenize_newline();
        }

        self.state.after_colon = false;
        Token {
            kind: TokenType::Comment,
            value: value.trim().to_string(),
            line: self.state.line,
            column: start_col,
            indent: self.state.current_indent,
        }
    }

    fn tokenize_newline(&mut self) -> Token {
        let token = self.create_token(TokenType::Newline, "\n".to_string());

        self.state.after_colon = false;
        if self.peek_char(0) == '\r' {
            self.advance();
        }
        if self.peek_char(0) == '\n' {
            self.advance();
        }

        self.state.line += 1;
        self.state.column = 1;
        self.state.at_line_start = true;

        token
    }

    fn consume_indent(&mut self) -> usize {
        let mut spaces = 0;
        let indent_size = self.options.indent_size.unwrap_or(2);
        // let allow_tabs = self.options.allow_tabs.unwrap_or(false); // Unused for now

        while self.state.pos < self.input.len() {
            let c = self.peek_char(0);
            if c == ' ' {
                spaces += 1;
                self.advance();
            } else if c == '\t' {
                spaces += indent_size;
                self.advance();
            } else {
                break;
            }
        }
        spaces
    }

    fn measure_indent(&self) -> usize {
        let mut spaces = 0;
        let mut offset = 0;
        let indent_size = self.options.indent_size.unwrap_or(2);
        let allow_tabs = self.options.allow_tabs.unwrap_or(false);

        loop {
            let c = self.peek_char(offset);
            if c == ' ' {
                spaces += 1;
                offset += 1;
            } else if c == '\t' && allow_tabs {
                spaces += indent_size;
                offset += 1;
            } else {
                break;
            }
        }
        spaces
    }

    fn tokenize_multiline_string(&mut self) -> Option<Token> {
        let start_pos = self.state.pos;
        let start_line = self.state.line;
        let start_col = self.state.column;
        let start_at_line_start = self.state.at_line_start;

        // Save state to rewind if needed
        // Since state is complex, maybe just cloning it is easier if we need full restore?
        // But here we can manually restore fields.

        self.advance(); // consume |

        // Skip to end of line
        while self.state.pos < self.input.len() {
            let c = self.peek_char(0);
            if c == '\n' || c == '\r' {
                break;
            }
            if c != ' ' {
                break;
            } // Ignore trailing spaces/modifiers after | (simplified)
            self.advance();
        }

        if self.state.pos >= self.input.len() && !self.finishing {
            // Restore? The original code says "rewind()"
            // For multiline, partial token is complicated, but we can store raw progress
            self.state.partial_token = String::new(); // Hard to track perfectly, but better than nothing
            self.state.pos = start_pos;
            self.state.line = start_line;
            self.state.column = start_col;
            self.state.at_line_start = start_at_line_start;
            return None;
        }

        // Consume newline
        if self.peek_char(0) == '\r' {
            self.advance();
        }
        if self.peek_char(0) == '\n' {
            self.advance();
            self.state.line += 1;
            self.state.column = 1;
        }

        if self.state.pos >= self.input.len() && !self.finishing {
            self.state.pos = start_pos;
            self.state.line = start_line;
            self.state.column = start_col;
            self.state.at_line_start = start_at_line_start;
            return None;
        }

        // Determine the indent level of the multiline content
        let base_indent = self.measure_indent();
        if base_indent == 0 {
            // No indented content, return empty string
            return Some(Token {
                kind: TokenType::Scalar,
                value: String::new(),
                line: self.state.line,
                column: start_col,
                indent: self.state.current_indent,
            });
        }

        let mut lines = Vec::new();
        let mut incomplete = false;

        while self.state.pos < self.input.len() {
            let line_indent = self.measure_indent();

            // If line is less indented (and not empty), we're done
            let _peek = self.peek_char(line_indent); // Check char after indent without consuming
            // Actually measure_indent returns count, we need to know if line is empty.
            // But we can't easily peek past indent with measure_indent.
            // Let's rely on standard logic:

            // Check if line is empty (just newline)
            let c = self.peek_char(0);
            let _is_eol = c == '\n' || c == '\r';
            // Wait, measure_indent peeks from current pos. So if line is empty, indent is 0 (or until newline).
            // But we need to handle empty lines in block.

            // Re-implementing logic carefully:
            // "If line is less indented (and not empty), we're done"

            // Check if line is effectively empty (whitespace only until newline)
            let mut is_empty_line = true;
            let mut offset = 0;
            loop {
                let c = self.peek_char(offset);
                if c == '\n' || c == '\r' {
                    break;
                }
                if c != ' ' && c != '\t' {
                    is_empty_line = false;
                    break;
                }
                offset += 1;
            }

            if !is_empty_line && line_indent < base_indent {
                break;
            }

            // Skip the base indent
            for _ in 0..base_indent {
                if self.peek_char(0) == ' ' {
                    self.advance();
                } else {
                    break;
                } // Should match unless mixed tabs/spaces
            }

            // Read the line content
            let mut line_content = String::new();
            while self.state.pos < self.input.len() {
                let c = self.peek_char(0);
                if c == '\n' || c == '\r' {
                    break;
                }
                line_content.push(c);
                self.advance();
            }

            if self.state.pos >= self.input.len() && !self.finishing {
                incomplete = true;
                break;
            }

            lines.push(line_content);

            // Consume newline
            if self.peek_char(0) == '\r' {
                self.advance();
            }
            if self.peek_char(0) == '\n' {
                self.advance();
                self.state.line += 1;
                self.state.column = 1;
                self.state.at_line_start = true;
            } else {
                break;
            }
        }

        if (incomplete || self.state.pos >= self.input.len()) && !self.finishing {
            self.state.partial_token = lines.join("\n");
            self.state.pos = start_pos;
            self.state.line = start_line;
            self.state.column = start_col;
            self.state.at_line_start = start_at_line_start;
            return None;
        }

        self.state.partial_token.clear();
        self.state.after_colon = false;
        Some(Token {
            kind: TokenType::Scalar,
            value: lines.join("\n"),
            line: self.state.line,
            column: start_col,
            indent: self.state.current_indent,
        })
    }

    fn skip_spaces(&mut self) {
        while self.peek_char(0) == ' ' {
            self.advance();
        }
    }

    fn peek_char(&self, offset: usize) -> char {
        if self.state.pos + offset < self.input.len() {
            self.input[self.state.pos + offset]
        } else {
            '\0'
        }
    }

    fn advance(&mut self) {
        if self.state.pos < self.input.len() {
            self.state.pos += 1;
            self.state.column += 1;
        }
    }

    fn create_token(&self, kind: TokenType, value: String) -> Token {
        Token {
            kind,
            value,
            line: self.state.line,
            column: self.state.column,
            indent: self.state.current_indent,
        }
    }
}
