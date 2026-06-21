//! Token-to-AST parser.

mod expression;
mod statement;
#[cfg(test)]
mod token_tests;

use std::fmt;

use crate::{
    ast::Program,
    lexer::{Keyword, Span, Token, TokenKind},
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

/// Recursive-descent parser with Pratt expression precedence.
///
/// The token stream must be terminated by a single [`TokenKind::Eof`], which is
/// the contract produced by [`crate::lexer::Lexer::tokenize`]. The parser never
/// advances past that terminator, so [`Parser::peek`] always resolves.
pub struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
    /// Number of enclosing loops. Used to reject `break`/`continue` that appear
    /// outside any iteration statement.
    loop_depth: usize,
    /// Number of enclosing function bodies. Used to reject `return` that appears
    /// outside any function.
    function_depth: usize,
}

impl Parser {
    #[must_use]
    pub fn new(tokens: Vec<Token>) -> Self {
        debug_assert!(
            matches!(tokens.last().map(|token| &token.kind), Some(TokenKind::Eof)),
            "token stream must be terminated by Eof"
        );
        Self {
            tokens,
            cursor: 0,
            loop_depth: 0,
            function_depth: 0,
        }
    }

    /// Parses a complete script, consuming every token up to and including EOF.
    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut body = Vec::new();
        while !self.at_eof() {
            body.push(self.parse_statement()?);
        }
        Ok(Program { body })
    }

    /// Returns the token at the cursor. The EOF terminator keeps this in bounds.
    fn peek(&self) -> &Token {
        &self.tokens[self.cursor.min(self.tokens.len() - 1)]
    }

    /// Consumes and returns the current token, never moving past EOF.
    fn advance(&mut self) -> Token {
        let token = self.peek().clone();
        if !matches!(token.kind, TokenKind::Eof) {
            self.cursor += 1;
        }
        token
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }

    fn check_punctuator(&self, ch: char) -> bool {
        matches!(self.peek().kind, TokenKind::Punctuator(value) if value == ch)
    }

    fn eat_punctuator(&mut self, ch: char) -> bool {
        if self.check_punctuator(ch) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_punctuator(&mut self, ch: char) -> Result<(), ParseError> {
        if self.eat_punctuator(ch) {
            Ok(())
        } else {
            Err(self.error(format!(
                "expected `{ch}` but found {}",
                describe(&self.peek().kind)
            )))
        }
    }

    fn eat_operator(&mut self, op: &str) -> bool {
        if matches!(&self.peek().kind, TokenKind::Operator(value) if value == op) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check_keyword(&self, keyword: Keyword) -> bool {
        matches!(&self.peek().kind, TokenKind::Keyword(value) if *value == keyword)
    }

    fn eat_keyword(&mut self, keyword: Keyword) -> bool {
        if self.check_keyword(keyword) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_identifier(&mut self) -> Result<String, ParseError> {
        if let TokenKind::Identifier(name) = &self.peek().kind {
            let name = name.clone();
            self.advance();
            Ok(name)
        } else {
            Err(self.error(format!(
                "expected identifier but found {}",
                describe(&self.peek().kind)
            )))
        }
    }

    /// Consumes an IdentifierName, which also permits keywords after `.` and
    /// in property-name positions.
    fn expect_identifier_name(&mut self) -> Result<String, ParseError> {
        match self.peek().kind.clone() {
            TokenKind::Identifier(name) => {
                self.advance();
                Ok(name)
            }
            TokenKind::Keyword(keyword) => {
                self.advance();
                Ok(keyword.as_str().into())
            }
            _ => Err(self.error(format!(
                "expected identifier name but found {}",
                describe(&self.peek().kind)
            ))),
        }
    }

    /// Consumes a statement terminator: an explicit `;`, or end of input.
    ///
    /// V1 does not implement automatic semicolon insertion at line terminators;
    /// the only implicit terminator is EOF.
    fn expect_semicolon(&mut self) -> Result<(), ParseError> {
        match &self.peek().kind {
            TokenKind::Punctuator(';') => {
                self.advance();
                Ok(())
            }
            TokenKind::Eof => Ok(()),
            other => Err(self.error(format!("expected `;` but found {}", describe(other)))),
        }
    }

    /// Builds a [`ParseError`] anchored at the current token's span.
    fn error(&self, message: String) -> ParseError {
        ParseError {
            span: self.peek().span,
            message,
        }
    }
}

/// Renders a token kind for human-readable error messages.
fn describe(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Eof => "end of input".into(),
        TokenKind::Identifier(name) => format!("identifier `{name}`"),
        TokenKind::Number(_) => "number".into(),
        TokenKind::String(_) => "string".into(),
        TokenKind::Keyword(keyword) => format!("keyword `{keyword:?}`"),
        TokenKind::Punctuator(ch) => format!("`{ch}`"),
        TokenKind::Operator(op) => format!("`{op}`"),
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
