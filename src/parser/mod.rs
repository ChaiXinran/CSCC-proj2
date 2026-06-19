//! Token-to-AST parser.

mod expression;
mod statement;

use std::fmt;

use crate::{
    ast::Program,
    lexer::{Span, Token, TokenKind},
};

/// Syntax error with source location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub span: Span,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at bytes {}..{}",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for ParseError {}

/// Recursive-descent parser state.
pub struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
}

impl Parser {
    #[must_use]
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, cursor: 0 }
    }

    /// Parses a script. The scaffold currently accepts only an empty program.
    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let token = self.current().ok_or_else(|| ParseError {
            span: Span::default(),
            message: "token stream must end with EOF".into(),
        })?;

        if token.kind == TokenKind::Eof {
            self.cursor += 1;
            Ok(Program::default())
        } else {
            Err(ParseError {
                span: token.span,
                message: "syntax is not implemented by the native parser yet".into(),
            })
        }
    }

    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.cursor)
    }
}

#[cfg(test)]
mod tests {
    use crate::lexer::{Span, Token, TokenKind};

    use super::Parser;

    #[test]
    fn parses_empty_program() {
        let mut parser = Parser::new(vec![Token::new(TokenKind::Eof, Span::new(0, 0))]);
        assert!(parser.parse_program().unwrap().body.is_empty());
    }
}
