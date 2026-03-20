use crate::ast::*;
use crate::tokenizer::*;

pub struct Parser {
    tokenizer: Tokenizer,
    current_token: Token,
}

impl Parser {
    pub fn new(input: &str) -> Self {
        let mut tokenizer = Tokenizer::new(input);
        let current_token = tokenizer.next_token();
        Self {
            tokenizer,
            current_token,
        }
    }

    fn advance(&mut self) {
        self.current_token = self.tokenizer.next_token();
    }

    pub fn parse(&mut self) -> Vec<Intent> {
        let mut intents = Vec::new();

        while self.current_token.kind != TokenKind::EOF {
            match &self.current_token.kind {
                TokenKind::Identifier(name) => {
                    let intent_name = name.clone();
                    let pos = self.current_token.position.clone();
                    self.advance();

                    if self.current_token.kind == TokenKind::OpenParen {
                        self.advance(); // consume '('
                        let fields = self.parse_fields();
                        
                        // Expect ')'
                        let mut is_complete = false;
                        if self.current_token.kind == TokenKind::CloseParen {
                            self.advance();
                            is_complete = true;
                        }

                        intents.push(Intent {
                            name: intent_name,
                            fields,
                            position: pos,
                            is_complete,
                        });
                    }
                }
                _ => {
                    // Skip tokens looking for the next intent identifier
                    self.advance();
                }
            }
        }

        intents
    }

    fn parse_fields(&mut self) -> std::collections::HashMap<String, ASTValue> {
        let mut fields = std::collections::HashMap::new();

        while self.current_token.kind != TokenKind::EOF 
            && self.current_token.kind != TokenKind::CloseParen 
            && self.current_token.kind != TokenKind::CloseBrace {
            match &self.current_token.kind {
                TokenKind::Identifier(name) => {
                    let field_name = name.clone();
                    self.advance(); // consume identifier

                    if self.current_token.kind == TokenKind::Equals {
                        self.advance(); // consume '=' or ':'
                        if let Some(val) = self.parse_value() {
                            fields.insert(field_name, val);
                        }
                    }
                }
                TokenKind::Comma => {
                    self.advance(); // skip commas between fields if they exist
                }
                _ => {
                    // Skip unknown tokens inside scope to recover gracefully
                    self.advance();
                }
            }
        }

        fields
    }

    fn parse_value(&mut self) -> Option<ASTValue> {
        match &self.current_token.kind {
            TokenKind::StringLiteral(s) => {
                let val = ASTValue::String(s.clone());
                self.advance();
                Some(val)
            }
            TokenKind::NumberLiteral(n) => {
                let val = ASTValue::Number(*n);
                self.advance();
                Some(val)
            }
            TokenKind::BooleanLiteral(b) => {
                let val = ASTValue::Boolean(*b);
                self.advance();
                Some(val)
            }
            TokenKind::Null => {
                let val = ASTValue::Null;
                self.advance();
                Some(val)
            }
            TokenKind::OpenBrace => {
                self.advance(); // consume '{'
                let obj = self.parse_fields();
                if self.current_token.kind == TokenKind::CloseBrace {
                    self.advance();
                }
                Some(ASTValue::Object(obj))
            }
            TokenKind::OpenBracket => {
                self.advance(); // consume '['
                let mut arr = Vec::new();
                while self.current_token.kind != TokenKind::EOF && self.current_token.kind != TokenKind::CloseBracket {
                    if self.current_token.kind == TokenKind::Comma {
                        self.advance();
                        continue;
                    }
                    if let Some(val) = self.parse_value() {
                        arr.push(val);
                    } else {
                        break;
                    }
                }
                if self.current_token.kind == TokenKind::CloseBracket {
                    self.advance();
                }
                Some(ASTValue::Array(arr))
            }
            _ => None
        }
    }
}
