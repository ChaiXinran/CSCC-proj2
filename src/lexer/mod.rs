//! Source text tokenization.

mod cursor;
mod token;

use std::fmt;

pub use cursor::Cursor;
pub use token::{Keyword, Span, Token, TokenKind};

/// Error produced while converting source text into tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub span: Span,
    pub message: String,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at bytes {}..{}",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for LexError {}

/// Stateful tokenizer for AgentJS source text.
pub struct Lexer<'source> {
    cursor: Cursor<'source>,
}

impl<'source> Lexer<'source> {
    #[must_use]
    pub fn new(source: &'source str) -> Self {
        Self {
            cursor: Cursor::new(source),
        }
    }

    /// Tokenizes source text.
    ///
    /// The initial scaffold accepts whitespace-only input. Language tokens are
    /// added incrementally without changing the parser-facing token contract.
    pub fn tokenize(mut self) -> Result<Vec<Token>, LexError> {
        self.cursor.skip_while(char::is_whitespace);
        if let Some(ch) = self.cursor.peek() {
            let start = self.cursor.offset();
            return Err(LexError {
                span: Span::new(start, start + ch.len_utf8()),
                message: format!("unsupported token {ch:?}"),
            });
        }

        let end = self.cursor.offset();
        Ok(vec![Token::new(TokenKind::Eof, Span::new(end, end))])
    }
}

#[cfg(test)]
mod tests {
    use super::{Lexer, Span, Token, TokenKind};

    #[test]
    fn tokenizes_empty_program() {
        assert_eq!(
            Lexer::new(" \n\t").tokenize().unwrap(),
            [Token::new(TokenKind::Eof, Span::new(3, 3))]
        );
    }

    #[test]
    fn rejects_tokens_not_implemented_yet() {
        let error = Lexer::new("let x").tokenize().unwrap_err();
        assert_eq!(error.span, Span::new(0, 1));
    }
}
