use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
    pub index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ASTValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Array(Vec<ASTValue>),
    Object(HashMap<String, ASTValue>),
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub value: ASTValue,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Intent {
    pub name: String,
    pub fields: HashMap<String, ASTValue>,
    pub position: Position,
    pub is_complete: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Identifier(String),
    Equals,
    StringLiteral(String),
    NumberLiteral(f64),
    BooleanLiteral(bool),
    OpenParen,
    CloseParen,
    OpenBrace,
    CloseBrace,
    OpenBracket,
    CloseBracket,
    Comma,
    Null,
    EOF,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub position: Position,
}
