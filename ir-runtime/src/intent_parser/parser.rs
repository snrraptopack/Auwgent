use super::tokenizer::Tokenizer;
use super::types::*;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

// ═══════════════════════════════════════════════════════════════════════════
// PARSER STATE
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct ParserFrame {
    pub frame_type: FrameType,
    pub node: FrameNode,
    pub indent: usize,
    /// Current key waiting for value (mapping only)
    pub pending_key: Option<String>,
    pub pending_key_pos: Option<Position>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FrameType {
    Mapping,
    Sequence,
}

#[derive(Debug)]
pub enum FrameNode {
    Mapping(MappingNode),
    Sequence(SequenceNode),
}

impl FrameNode {
    fn as_ast_node(self) -> ASTNode {
        match self {
            FrameNode::Mapping(n) => ASTNode::Mapping(n),
            FrameNode::Sequence(n) => ASTNode::Sequence(n),
        }
    }

    pub fn to_ast_node(&self) -> ASTNode {
        match self {
            FrameNode::Mapping(n) => ASTNode::Mapping(n.clone()),
            FrameNode::Sequence(n) => ASTNode::Sequence(n.clone()),
        }
    }
}

pub type ParserHandler = Arc<dyn Fn(Value) + Send + Sync>;

// ═══════════════════════════════════════════════════════════════════════════
// PARSER CLASS
// ═══════════════════════════════════════════════════════════════════════════

pub struct Parser {
    tokenizer: Tokenizer,
    tokens: Vec<Token>,
    pos: usize,
    stack: Vec<ParserFrame>,
    errors: Vec<ParseError>,
    listeners: HashMap<ParserEventType, Vec<ParserHandler>>,
    options: ParserOptions,
}

impl Parser {
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
            tokenizer: Tokenizer::new(Some(opts.clone())),
            tokens: Vec::new(),
            pos: 0,
            stack: Vec::new(),
            errors: Vec::new(),
            listeners: HashMap::new(),
            options: opts,
        }
    }

    pub fn reset(&mut self) {
        let listeners = std::mem::take(&mut self.listeners);
        self.tokenizer.reset();
        self.tokens.clear();
        self.pos = 0;
        self.stack.clear();
        self.errors.clear();
        self.listeners = listeners;
    }

    pub fn on(&mut self, event: ParserEventType, handler: ParserHandler) {
        self.listeners.entry(event).or_default().push(handler);
    }

    pub fn get_root_mapping(&self) -> Option<&MappingNode> {
        if let Some(frame) = self.stack.first() {
            if let FrameNode::Mapping(m) = &frame.node {
                return Some(m);
            }
        }
        None
    }

    pub fn stack_depth(&self) -> usize {
        self.stack.len()
    }

    pub fn stack(&self) -> &Vec<ParserFrame> {
        &self.stack
    }

    pub fn get_partial_token(&self) -> String {
        self.tokenizer.get_partial_token()
    }

    /// Get a reference to the node in a specific frame
    pub fn get_frame_node(&self, _depth: usize) -> Option<&ASTNode> {
        // This is tricky because FrameNode is not ASTNode.
        // Let's just expose a way to see what's currently being built.
        None
    }

    pub fn off(&mut self, event: ParserEventType, handler: &ParserHandler) {
        if let Some(list) = self.listeners.get_mut(&event) {
            list.retain(|h| !Arc::ptr_eq(h, handler));
        }
    }

    fn emit(&self, event_type: ParserEventType, data: Value, position: Position) {
        let event = ParserEvent {
            event_type: event_type.clone(),
            data: data.clone(),
            position,
        };
        if let Some(handlers) = self.listeners.get(&event_type) {
            for handler in handlers {
                handler(data.clone());
            }
        }
        if let Some(middleware) = &self.options.middleware {
            for mw in middleware {
                mw(event.clone());
            }
        }
    }

    pub fn write(&mut self, chunk: &str) {
        self.tokenizer.write(chunk);
        while let Some(token) = self.tokenizer.next_token() {
            self.tokens.push(token);
        }
        self.parse_tokens(false);
    }

    pub fn peek(&mut self) -> ParseResult {
        while let Some(token) = self.tokenizer.next_token() {
            self.tokens.push(token);
        }
        self.parse_tokens(false)
    }

    pub fn end(&mut self) -> ParseResult {
        while let Some(token) = self.tokenizer.next_token() {
            self.tokens.push(token);
        }
        let final_tokens = self.tokenizer.finalize();
        self.tokens.extend(final_tokens);
        self.parse_tokens(true)
    }

    fn parse_tokens(&mut self, complete: bool) -> ParseResult {
        if self.stack.is_empty() {
            let first_non_comment_token = self
                .tokens
                .iter()
                .find(|t| t.kind != TokenType::Comment && t.kind != TokenType::Newline);

            let is_root_sequence =
                matches!(first_non_comment_token, Some(t) if t.kind == TokenType::Dash);

            if is_root_sequence {
                self.push_frame(FrameType::Sequence, 0, 0, 0);
            } else {
                self.push_frame(FrameType::Mapping, 0, 0, 0);
            }
        }

        while self.pos < self.tokens.len() {
            let token = self.tokens[self.pos].clone();
            self.process_token(token);
            self.pos += 1;
        }

        let ast = self.stack.first().map(|f| match &f.node {
            FrameNode::Mapping(n) => ASTNode::Mapping(n.clone()),
            FrameNode::Sequence(n) => ASTNode::Sequence(n.clone()),
        });

        ParseResult {
            ast,
            errors: self.errors.clone(),
            complete,
        }
    }

    fn process_token(&mut self, token: Token) {
        let pos = Position {
            line: token.line,
            column: token.column,
            offset: 0,
        };

        match token.kind {
            TokenType::Key => self.handle_key(token, pos),
            TokenType::Colon => {}
            TokenType::Scalar | TokenType::Quoted => self.handle_value(token, pos),
            TokenType::Dash => self.handle_dash(token, pos),
            TokenType::Indent => self.handle_indent(token, pos),
            TokenType::Dedent => self.handle_dedent(token, pos),
            TokenType::Newline => self.handle_newline(token, pos),
            TokenType::Eof => self.handle_eof(pos),
            TokenType::Comment => {
                self.emit(
                    ParserEventType::Line,
                    json!({ "comment": token.value }),
                    pos,
                );
            }
        }
    }

    fn handle_key(&mut self, token: Token, pos: Position) {
        self.emit(
            ParserEventType::Key,
            json!({ "key": token.value }),
            pos.clone(),
        );

        if self.current_frame().frame_type != FrameType::Mapping {
            self.push_frame(FrameType::Mapping, token.indent, token.line, token.column);
        }

        let frame = self.current_frame_mut();
        if let Some(key) = frame.pending_key.take() {
            if let Some(key_pos) = frame.pending_key_pos.take() {
                let empty_node = EmptyNode {
                    kind: "empty".to_string(),
                    hint: Some("mapping".to_string()),
                    line: key_pos.line,
                    column: key_pos.column,
                };
                Self::add_mapping_entry_node(
                    match &mut frame.node {
                        FrameNode::Mapping(m) => m,
                        _ => unreachable!(),
                    },
                    key,
                    ASTNode::Empty(empty_node),
                    key_pos.line,
                    key_pos.column,
                );
            }
        }

        frame.pending_key = Some(token.value);
        frame.pending_key_pos = Some(pos);
    }

    fn handle_value(&mut self, token: Token, pos: Position) {
        let value = token.value.clone();
        let quoted = token.kind == TokenType::Quoted;

        self.emit(
            ParserEventType::Value,
            json!({ "value": value, "quoted": quoted }),
            pos.clone(),
        );

        let scalar_node = ScalarNode {
            kind: "scalar".to_string(),
            value,
            quoted,
            line: token.line,
            column: token.column,
        };

        let frame = self.current_frame_mut();
        match frame.frame_type {
            FrameType::Mapping => {
                if let Some(key) = frame.pending_key.take() {
                    let key_pos = frame.pending_key_pos.take().unwrap();
                    Self::add_mapping_entry_node(
                        match &mut frame.node {
                            FrameNode::Mapping(m) => m,
                            _ => unreachable!(),
                        },
                        key,
                        ASTNode::Scalar(scalar_node),
                        key_pos.line,
                        key_pos.column,
                    );
                }
            }
            FrameType::Sequence => {
                if let FrameNode::Sequence(seq) = &mut frame.node {
                    seq.items.push(ASTNode::Scalar(scalar_node));
                }
            }
        }
    }

    fn handle_dash(&mut self, token: Token, pos: Position) {
        self.emit(
            ParserEventType::BlockStart,
            json!({ "type": "sequence_item" }),
            pos.clone(),
        );

        while self.stack.len() > 1 {
            if self.current_frame().indent <= token.indent {
                break;
            }
            self.pop_frame(pos.clone());
        }

        let next_token = self.tokens.get(self.pos + 1);
        let is_key_item = matches!(next_token, Some(t) if t.kind == TokenType::Key);

        if self.current_frame().frame_type == FrameType::Mapping
            && self.current_frame().pending_key.is_some()
        {
            self.push_frame(FrameType::Sequence, token.indent, token.line, token.column);
        }

        if self.current_frame().frame_type == FrameType::Sequence {
            if is_key_item {
                self.push_frame(
                    FrameType::Mapping,
                    token.indent + 1,
                    token.line,
                    token.column,
                );
            }
        }
    }

    fn handle_indent(&mut self, token: Token, pos: Position) {
        self.emit(
            ParserEventType::Indent,
            json!({ "level": token.indent }),
            pos.clone(),
        );

        if self.current_frame().frame_type == FrameType::Mapping
            && self.current_frame().pending_key.is_some()
        {
            let next_token = self.tokens.get(self.pos + 1);
            if matches!(next_token, Some(t) if t.kind != TokenType::Dash) {
                self.push_frame(FrameType::Mapping, token.indent, token.line, token.column);
            }
        }
    }

    fn handle_dedent(&mut self, token: Token, pos: Position) {
        self.emit(
            ParserEventType::Dedent,
            json!({ "level": token.indent }),
            pos.clone(),
        );

        while self.stack.len() > 1 {
            if self.current_frame().indent <= token.indent {
                break;
            }
            self.pop_frame(pos.clone());
        }
    }

    fn handle_newline(&mut self, _token: Token, pos: Position) {
        self.emit(ParserEventType::Line, json!({}), pos.clone());

        let action = {
            let frame = self.current_frame();
            if frame.frame_type == FrameType::Mapping && frame.pending_key.is_some() {
                let next_token = self.tokens.get(self.pos + 1);
                matches!(next_token, Some(t) if t.kind == TokenType::Dedent || t.kind == TokenType::Eof)
            } else {
                false
            }
        };

        if action {
            let frame = self.current_frame_mut();
            let key = frame.pending_key.take().unwrap();
            let key_pos = frame.pending_key_pos.take().unwrap();
            let empty_node = EmptyNode {
                kind: "empty".to_string(),
                hint: Some("mapping".to_string()),
                line: pos.line,
                column: pos.column,
            };
            Self::add_mapping_entry_node(
                match &mut frame.node {
                    FrameNode::Mapping(m) => m,
                    _ => unreachable!(),
                },
                key,
                ASTNode::Empty(empty_node),
                key_pos.line,
                key_pos.column,
            );
        }
    }

    fn handle_eof(&mut self, pos: Position) {
        while self.stack.len() > 1 {
            self.pop_frame(pos.clone());
        }

        let root_frame = self.current_frame_mut();
        if let Some(key) = root_frame.pending_key.take() {
            let key_pos = root_frame.pending_key_pos.take().unwrap();
            let empty_node = EmptyNode {
                kind: "empty".to_string(),
                hint: Some("mapping".to_string()),
                line: pos.line,
                column: pos.column,
            };
            Self::add_mapping_entry_node(
                match &mut root_frame.node {
                    FrameNode::Mapping(m) => m,
                    _ => unreachable!(),
                },
                key,
                ASTNode::Empty(empty_node),
                key_pos.line,
                key_pos.column,
            );
        }
    }

    fn push_frame(&mut self, frame_type: FrameType, indent: usize, line: usize, col: usize) {
        let node = match frame_type {
            FrameType::Mapping => FrameNode::Mapping(MappingNode {
                kind: "mapping".to_string(),
                entries: Vec::new(),
                line,
                column: col,
            }),
            FrameType::Sequence => FrameNode::Sequence(SequenceNode {
                kind: "sequence".to_string(),
                items: Vec::new(),
                line,
                column: col,
            }),
        };

        self.stack.push(ParserFrame {
            frame_type,
            node,
            indent,
            pending_key: None,
            pending_key_pos: None,
        });
    }

    fn pop_frame(&mut self, pos: Position) {
        if let Some(frame) = self.stack.pop() {
            self.emit(
                ParserEventType::BlockEnd,
                json!({ "type": format!("{:?}", frame.frame_type).to_lowercase() }),
                pos,
            );

            if !self.stack.is_empty() {
                let parent = self.stack.last_mut().unwrap();
                let child_node = frame.node.as_ast_node();

                match &mut parent.node {
                    FrameNode::Mapping(m) => {
                        if let Some(key) = parent.pending_key.take() {
                            let key_pos = parent.pending_key_pos.take().unwrap();
                            Self::add_mapping_entry_node(
                                m,
                                key,
                                child_node,
                                key_pos.line,
                                key_pos.column,
                            );
                        }
                    }
                    FrameNode::Sequence(s) => {
                        s.items.push(child_node);
                    }
                }
            }
        }
    }

    fn current_frame(&self) -> &ParserFrame {
        self.stack.last().expect("Stack underflow")
    }

    fn current_frame_mut(&mut self) -> &mut ParserFrame {
        self.stack.last_mut().expect("Stack underflow")
    }

    fn add_mapping_entry_node(
        mapping: &mut MappingNode,
        key: String,
        value: ASTNode,
        line: usize,
        column: usize,
    ) {
        let entry_value = if key == "ref" {
            if let ASTNode::Scalar(scalar) = &value {
                ASTNode::Ref(RefNode {
                    kind: "ref".to_string(),
                    target: scalar.value.clone(),
                    line: scalar.line,
                    column: scalar.column,
                })
            } else {
                value
            }
        } else {
            value
        };

        mapping.entries.push(MappingEntry {
            key,
            value: entry_value,
            line,
            column,
        });
    }
}

pub fn parse(input: &str, options: Option<ParserOptions>) -> ParseResult {
    let mut parser = Parser::new(options);
    parser.write(input);
    parser.end()
}
