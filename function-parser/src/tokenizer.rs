use crate::ast::{Position, Token, TokenKind};

pub struct Tokenizer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
}

impl Tokenizer {
    pub fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
            line: 1,
            column: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += 1;
        if ch == '\n' {
            self.line += 1;
            self.column = 0;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        let start_pos = Position {
            line: self.line,
            column: self.column,
            index: self.pos,
        };

        if let Some(ch) = self.advance() {
            match ch {
                '(' => Token { kind: TokenKind::OpenParen, position: start_pos },
                ')' => Token { kind: TokenKind::CloseParen, position: start_pos },
                '{' => Token { kind: TokenKind::OpenBrace, position: start_pos },
                '}' => Token { kind: TokenKind::CloseBrace, position: start_pos },
                '[' => Token { kind: TokenKind::OpenBracket, position: start_pos },
                ']' => Token { kind: TokenKind::CloseBracket, position: start_pos },
                ',' => Token { kind: TokenKind::Comma, position: start_pos },
                '=' | ':' => Token { kind: TokenKind::Equals, position: start_pos }, // Treat : and = interchangeably as assignment
                '"' => self.read_string(start_pos),
                c if c.is_ascii_alphabetic() || c == '_' => {
                    self.pos -= 1; // back up
                    if ch == '\n' { self.line -= 1; } else { self.column -= 1; }
                    self.read_identifier_or_keyword(start_pos)
                }
                c if c.is_ascii_digit() || c == '-' || c == '.' => {
                    self.pos -= 1;
                    if ch == '\n' { self.line -= 1; } else { self.column -= 1; }
                    self.read_number(start_pos)
                }
                _ => self.next_token(), // Skip unknown characters for error recovery
            }
        } else {
            Token { kind: TokenKind::EOF, position: start_pos }
        }
    }

    fn read_string(&mut self, start_pos: Position) -> Token {
        let mut value = String::new();
        let mut escape = false;

        while let Some(ch) = self.advance() {
            if escape {
                value.push(ch);
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                break;
            } else {
                value.push(ch);
            }
        }

        Token {
            kind: TokenKind::StringLiteral(value),
            position: start_pos,
        }
    }

    fn read_identifier_or_keyword(&mut self, start_pos: Position) -> Token {
        let mut value = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                value.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        let kind = match value.as_str() {
            "true" => TokenKind::BooleanLiteral(true),
            "false" => TokenKind::BooleanLiteral(false),
            "null" => TokenKind::Null,
            _ => TokenKind::Identifier(value),
        };

        Token { kind, position: start_pos }
    }

    fn read_number(&mut self, start_pos: Position) -> Token {
        let mut value = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() || ch == '.' || ch == '-' {
                value.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        if let Ok(num) = value.parse::<f64>() {
            Token { kind: TokenKind::NumberLiteral(num), position: start_pos }
        } else {
            // Fallback to identifier if it's malformed like a weird version string
            Token { kind: TokenKind::Identifier(value), position: start_pos }
        }
    }
}
