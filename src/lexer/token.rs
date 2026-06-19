//! Stable token types shared by the lexer and parser.

/// Half-open byte range in the original UTF-8 source.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// Reserved words recognized by the lexer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Keyword {
    Let,
    Const,
    Var,
    Function,
    Return,
    If,
    Else,
    While,
    For,
    True,
    False,
    Null,
}

/// Lexical token payload.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Eof,
    Identifier(String),
    Number(f64),
    String(String),
    Keyword(Keyword),
    Punctuator(char),
    Operator(String),
}

/// One token and its location in source text.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    #[must_use]
    pub const fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}
