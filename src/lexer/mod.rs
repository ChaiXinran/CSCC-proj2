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

/// Operators recognized by the V1 lexer, ordered so that maximal munch is a
/// simple linear scan: longer operators precede their shorter prefixes.
const OPERATORS: &[&str] = &[
    "===", "!==", "<=", ">=", "&&", "||", "+", "-", "*", "/", "%", "!", "=", "<", ">",
];

/// Punctuators recognized by the V1 lexer.
const PUNCTUATORS: &[char] = &['(', ')', '{', '}', ';', ',', '.'];

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

    /// Converts source text into a token stream terminated by [`TokenKind::Eof`].
    ///
    /// The V1 grammar covers whitespace and line terminators, line and block
    /// comments, ASCII identifiers, decimal number literals, single- and
    /// double-quoted strings, the keywords `var`/`true`/`false`/`null`, the
    /// punctuators `(){};,.`, and the operator set required by the expression
    /// milestone. Unsupported input produces a [`LexError`] carrying its span.
    pub fn tokenize(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_trivia()?;
            let start = self.cursor.offset();
            let Some(ch) = self.cursor.peek() else {
                tokens.push(Token::new(TokenKind::Eof, Span::new(start, start)));
                return Ok(tokens);
            };

            let token = if is_identifier_start(ch) {
                self.read_identifier_or_keyword()
            } else if ch.is_ascii_digit()
                || (ch == '.' && self.cursor.second().is_some_and(|c| c.is_ascii_digit()))
            {
                self.read_number()?
            } else if ch == '"' || ch == '\'' {
                self.read_string()?
            } else {
                self.read_operator_or_punctuator()?
            };
            tokens.push(token);
        }
    }

    /// Consumes whitespace, line terminators, and comments between tokens.
    fn skip_trivia(&mut self) -> Result<(), LexError> {
        loop {
            self.cursor.skip_while(is_whitespace);
            let rest = self.cursor.rest();
            if rest.starts_with("//") {
                self.cursor.bump();
                self.cursor.bump();
                self.cursor.skip_while(|c| !is_line_terminator(c));
            } else if rest.starts_with("/*") {
                let start = self.cursor.offset();
                self.cursor.bump();
                self.cursor.bump();
                loop {
                    if self.cursor.rest().starts_with("*/") {
                        self.cursor.bump();
                        self.cursor.bump();
                        break;
                    }
                    if self.cursor.bump().is_none() {
                        return Err(LexError {
                            span: Span::new(start, self.cursor.offset()),
                            message: "unterminated block comment".into(),
                        });
                    }
                }
            } else {
                return Ok(());
            }
        }
    }

    fn read_identifier_or_keyword(&mut self) -> Token {
        let start = self.cursor.offset();
        self.cursor.bump();
        self.cursor.skip_while(is_identifier_part);
        let end = self.cursor.offset();
        let text = self.cursor.slice(Span::new(start, end));
        let kind = match text {
            "var" => TokenKind::Keyword(Keyword::Var),
            "true" => TokenKind::Keyword(Keyword::True),
            "false" => TokenKind::Keyword(Keyword::False),
            "null" => TokenKind::Keyword(Keyword::Null),
            _ => TokenKind::Identifier(text.to_owned()),
        };
        Token::new(kind, Span::new(start, end))
    }

    fn read_number(&mut self) -> Result<Token, LexError> {
        let start = self.cursor.offset();
        self.cursor.skip_while(|c| c.is_ascii_digit());
        if self.cursor.peek() == Some('.') {
            self.cursor.bump();
            self.cursor.skip_while(|c| c.is_ascii_digit());
        }
        if matches!(self.cursor.peek(), Some('e' | 'E')) {
            self.cursor.bump();
            if matches!(self.cursor.peek(), Some('+' | '-')) {
                self.cursor.bump();
            }
            let exponent_start = self.cursor.offset();
            self.cursor.skip_while(|c| c.is_ascii_digit());
            if self.cursor.offset() == exponent_start {
                let end = self.cursor.offset();
                return Err(LexError {
                    span: Span::new(start, end),
                    message: "missing exponent digits in number literal".into(),
                });
            }
        }

        let end = self.cursor.offset();
        let text = self.cursor.slice(Span::new(start, end));
        let value = text.parse::<f64>().map_err(|_| LexError {
            span: Span::new(start, end),
            message: format!("invalid number literal `{text}`"),
        })?;
        Ok(Token::new(TokenKind::Number(value), Span::new(start, end)))
    }

    fn read_string(&mut self) -> Result<Token, LexError> {
        let start = self.cursor.offset();
        let quote = self
            .cursor
            .bump()
            .expect("string literal opens with a quote");
        let mut value = String::new();
        loop {
            match self.cursor.bump() {
                None => {
                    return Err(LexError {
                        span: Span::new(start, self.cursor.offset()),
                        message: "unterminated string literal".into(),
                    });
                }
                Some(ch) if ch == quote => {
                    let end = self.cursor.offset();
                    return Ok(Token::new(TokenKind::String(value), Span::new(start, end)));
                }
                Some('\\') => self.read_string_escape(start, &mut value)?,
                Some(ch) if is_line_terminator(ch) => {
                    return Err(LexError {
                        span: Span::new(start, self.cursor.offset()),
                        message: "unterminated string literal".into(),
                    });
                }
                Some(ch) => value.push(ch),
            }
        }
    }

    fn read_string_escape(&mut self, start: usize, value: &mut String) -> Result<(), LexError> {
        let escape = self.cursor.bump().ok_or_else(|| LexError {
            span: Span::new(start, self.cursor.offset()),
            message: "unterminated string literal".into(),
        })?;
        match escape {
            'n' => value.push('\n'),
            't' => value.push('\t'),
            'r' => value.push('\r'),
            'b' => value.push('\u{0008}'),
            'f' => value.push('\u{000C}'),
            'v' => value.push('\u{000B}'),
            '0' => value.push('\0'),
            '\\' => value.push('\\'),
            '\'' => value.push('\''),
            '"' => value.push('"'),
            // Line continuations produce no character. A CRLF pair counts once.
            '\r' => {
                if self.cursor.peek() == Some('\n') {
                    self.cursor.bump();
                }
            }
            '\n' | '\u{2028}' | '\u{2029}' => {}
            // Any other escaped character denotes itself (identity escape).
            other => value.push(other),
        }
        Ok(())
    }

    fn read_operator_or_punctuator(&mut self) -> Result<Token, LexError> {
        let start = self.cursor.offset();
        let ch = self.cursor.peek().expect("peeked character exists");

        if PUNCTUATORS.contains(&ch) {
            self.cursor.bump();
            return Ok(Token::new(
                TokenKind::Punctuator(ch),
                Span::new(start, self.cursor.offset()),
            ));
        }

        let rest = self.cursor.rest();
        if let Some(operator) = OPERATORS.iter().find(|op| rest.starts_with(*op)) {
            // Operators are ASCII, so byte length equals character count.
            for _ in 0..operator.len() {
                self.cursor.bump();
            }
            return Ok(Token::new(
                TokenKind::Operator((*operator).to_owned()),
                Span::new(start, self.cursor.offset()),
            ));
        }

        Err(LexError {
            span: Span::new(start, start + ch.len_utf8()),
            message: format!("unexpected character {ch:?}"),
        })
    }
}

/// ECMAScript line terminators.
fn is_line_terminator(ch: char) -> bool {
    matches!(ch, '\u{000A}' | '\u{000D}' | '\u{2028}' | '\u{2029}')
}

/// Whitespace skipped between tokens. V1 performs no automatic semicolon
/// insertion, so line terminators are treated as ordinary whitespace.
fn is_whitespace(ch: char) -> bool {
    matches!(
        ch,
        '\u{0009}' | '\u{000B}' | '\u{000C}' | '\u{0020}' | '\u{00A0}' | '\u{FEFF}'
    ) || is_line_terminator(ch)
        || ch.is_whitespace()
}

/// ASCII identifier start characters (`$` and `_` are permitted by ECMAScript).
fn is_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_' || ch == '$'
}

/// ASCII identifier continuation characters.
fn is_identifier_part(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '$'
}

#[cfg(test)]
mod tests {
    use super::{Keyword, Lexer, Span, Token, TokenKind};

    fn kinds(source: &str) -> Vec<TokenKind> {
        Lexer::new(source)
            .tokenize()
            .unwrap()
            .into_iter()
            .map(|token| token.kind)
            .collect()
    }

    #[test]
    fn tokenizes_empty_program() {
        assert_eq!(
            Lexer::new(" \n\t").tokenize().unwrap(),
            [Token::new(TokenKind::Eof, Span::new(3, 3))]
        );
    }

    #[test]
    fn skips_line_and_block_comments() {
        assert_eq!(
            kinds("// a\n/* b */ 1"),
            [TokenKind::Number(1.0), TokenKind::Eof]
        );
    }

    #[test]
    fn reports_unterminated_block_comment() {
        let error = Lexer::new("/* open").tokenize().unwrap_err();
        assert_eq!(error.message, "unterminated block comment");
    }

    #[test]
    fn tokenizes_keywords_and_identifiers() {
        assert_eq!(
            kinds("var truthy null true false"),
            [
                TokenKind::Keyword(Keyword::Var),
                TokenKind::Identifier("truthy".into()),
                TokenKind::Keyword(Keyword::Null),
                TokenKind::Keyword(Keyword::True),
                TokenKind::Keyword(Keyword::False),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn tokenizes_numbers() {
        assert_eq!(
            kinds("0 18 3.5 .5 1e3 2.0e-2"),
            [
                TokenKind::Number(0.0),
                TokenKind::Number(18.0),
                TokenKind::Number(3.5),
                TokenKind::Number(0.5),
                TokenKind::Number(1000.0),
                TokenKind::Number(0.02),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn tokenizes_strings_with_escapes() {
        assert_eq!(
            kinds(r#""a\n\"b" 'c\'d' """#),
            [
                TokenKind::String("a\n\"b".into()),
                TokenKind::String("c'd".into()),
                TokenKind::String(String::new()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn reports_unterminated_string() {
        let error = Lexer::new("\"open").tokenize().unwrap_err();
        assert_eq!(error.message, "unterminated string literal");
    }

    #[test]
    fn applies_maximal_munch_to_operators() {
        assert_eq!(
            kinds("=== !== <= >= && || + ="),
            [
                TokenKind::Operator("===".into()),
                TokenKind::Operator("!==".into()),
                TokenKind::Operator("<=".into()),
                TokenKind::Operator(">=".into()),
                TokenKind::Operator("&&".into()),
                TokenKind::Operator("||".into()),
                TokenKind::Operator("+".into()),
                TokenKind::Operator("=".into()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn treats_division_as_operator_not_comment() {
        assert_eq!(
            kinds("a/b/c"),
            [
                TokenKind::Identifier("a".into()),
                TokenKind::Operator("/".into()),
                TokenKind::Identifier("b".into()),
                TokenKind::Operator("/".into()),
                TokenKind::Identifier("c".into()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn records_spans_as_byte_ranges() {
        let tokens = Lexer::new("  18").tokenize().unwrap();
        assert_eq!(tokens[0].span, Span::new(2, 4));
        assert_eq!(tokens[1].span, Span::new(4, 4));
    }

    #[test]
    fn rejects_unsupported_character() {
        let error = Lexer::new("@").tokenize().unwrap_err();
        assert_eq!(error.span, Span::new(0, 1));
    }
}
