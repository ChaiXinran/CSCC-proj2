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

/// Operators recognized by the lexer, ordered so that maximal munch is a
/// simple linear scan: longer operators precede their shorter prefixes.
/// Includes bitwise/shift/exponentiation operators so that regex literal bodies
/// containing `|`, `^`, `&`, `~`, `**`, `>>`, `<<` tokenize without lex errors.
const OPERATORS: &[&str] = &[
    // 4-char
    ">>>=", // 3-char
    "===", "!==", ">>=", ">>>", "<<=", "**=", // 2-char
    "=>", "==", "!=", "<=", ">=", "&&", "||", "++", "--", "+=", "-=", "**", "*=", "/=", "%=", "|=",
    "^=", "&=", ">>", "<<", // 1-char
    "+", "-", "*", "/", "%", "!", "=", "<", ">", "|", "^", "&", "~",
];

/// Punctuators recognized by the lexer. V2 adds `?` and `:` for the conditional
/// operator. V3 adds `[` and `]` for array literals and computed member access.
const PUNCTUATORS: &[char] = &['(', ')', '{', '}', '[', ']', ';', ',', '.', '?', ':'];

/// Stateful tokenizer for AgentJS source text.
pub struct Lexer<'source> {
    cursor: Cursor<'source>,
    /// Set to `true` inside `read_string_escape` when a legacy octal or
    /// non-octal decimal escape is encountered. Consumed by `read_string` to
    /// stamp the resulting token with `has_legacy_escape`.
    string_has_legacy_escape: bool,
}

impl<'source> Lexer<'source> {
    #[must_use]
    pub fn new(source: &'source str) -> Self {
        Self {
            cursor: Cursor::new(source),
            string_has_legacy_escape: false,
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
            let line_terminator_before = self.skip_trivia()?;
            let start = self.cursor.offset();
            let Some(ch) = self.cursor.peek() else {
                tokens.push(Token::with_line_terminator_before(
                    TokenKind::Eof,
                    Span::new(start, start),
                    line_terminator_before,
                ));
                return Ok(tokens);
            };

            let mut token = if is_identifier_start(ch) || self.cursor.rest().starts_with("\\u") {
                self.read_identifier_or_keyword()?
            } else if ch.is_ascii_digit()
                || (ch == '.' && self.cursor.second().is_some_and(|c| c.is_ascii_digit()))
            {
                self.read_number()?
            } else if ch == '`' {
                self.read_template_literal()?
            } else if ch == '"' || ch == '\'' {
                self.read_string()?
            } else {
                self.read_operator_or_punctuator()?
            };
            token.line_terminator_before = line_terminator_before;
            tokens.push(token);
        }
    }

    /// Consumes whitespace, line terminators, and comments between tokens.
    ///
    /// Returns whether any ECMAScript line terminator was skipped, including
    /// terminators inside comments. The parser relies on this for restricted
    /// productions such as `throw expression`.
    fn skip_trivia(&mut self) -> Result<bool, LexError> {
        let mut saw_line_terminator = false;
        loop {
            while let Some(ch) = self.cursor.peek() {
                if is_line_terminator(ch) {
                    saw_line_terminator = true;
                    self.cursor.bump();
                } else if is_whitespace(ch) {
                    self.cursor.bump();
                } else {
                    break;
                }
            }
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
                    match self.cursor.bump() {
                        Some(ch) if is_line_terminator(ch) => saw_line_terminator = true,
                        Some(_) => {}
                        None => {
                            return Err(LexError {
                                span: Span::new(start, self.cursor.offset()),
                                message: "unterminated block comment".into(),
                            });
                        }
                    }
                }
            } else {
                return Ok(saw_line_terminator);
            }
        }
    }

    fn read_identifier_or_keyword(&mut self) -> Result<Token, LexError> {
        let start = self.cursor.offset();
        let mut text = String::new();
        let mut had_escape = false;

        if self.cursor.rest().starts_with("\\u") {
            had_escape = true;
            let character = self.read_identifier_escape(start)?;
            if !is_identifier_start(character) {
                return Err(self.invalid_identifier_escape(start));
            }
            text.push(character);
        } else {
            text.push(self.cursor.bump().expect("identifier start exists"));
        }

        loop {
            if self.cursor.rest().starts_with("\\u") {
                had_escape = true;
                let character = self.read_identifier_escape(start)?;
                if !is_identifier_part(character) {
                    return Err(self.invalid_identifier_escape(start));
                }
                text.push(character);
            } else if self.cursor.peek().is_some_and(is_identifier_part) {
                text.push(self.cursor.bump().expect("identifier part exists"));
            } else {
                break;
            }
        }
        let end = self.cursor.offset();
        let kind = if had_escape {
            TokenKind::Identifier(text)
        } else {
            match text.as_str() {
                "let" => TokenKind::Keyword(Keyword::Let),
                "const" => TokenKind::Keyword(Keyword::Const),
                "var" => TokenKind::Keyword(Keyword::Var),
                "function" => TokenKind::Keyword(Keyword::Function),
                "return" => TokenKind::Keyword(Keyword::Return),
                "if" => TokenKind::Keyword(Keyword::If),
                "else" => TokenKind::Keyword(Keyword::Else),
                "while" => TokenKind::Keyword(Keyword::While),
                "for" => TokenKind::Keyword(Keyword::For),
                "break" => TokenKind::Keyword(Keyword::Break),
                "continue" => TokenKind::Keyword(Keyword::Continue),
                "throw" => TokenKind::Keyword(Keyword::Throw),
                "try" => TokenKind::Keyword(Keyword::Try),
                "catch" => TokenKind::Keyword(Keyword::Catch),
                "finally" => TokenKind::Keyword(Keyword::Finally),
                "switch" => TokenKind::Keyword(Keyword::Switch),
                "case" => TokenKind::Keyword(Keyword::Case),
                "default" => TokenKind::Keyword(Keyword::Default),
                "new" => TokenKind::Keyword(Keyword::New),
                "typeof" => TokenKind::Keyword(Keyword::TypeOf),
                "void" => TokenKind::Keyword(Keyword::Void),
                "delete" => TokenKind::Keyword(Keyword::Delete),
                "in" => TokenKind::Keyword(Keyword::In),
                "instanceof" => TokenKind::Keyword(Keyword::InstanceOf),
                "true" => TokenKind::Keyword(Keyword::True),
                "false" => TokenKind::Keyword(Keyword::False),
                "null" => TokenKind::Keyword(Keyword::Null),
                _ => TokenKind::Identifier(text),
            }
        };
        Ok(Token::new(kind, Span::new(start, end)))
    }

    fn read_number(&mut self) -> Result<Token, LexError> {
        let start = self.cursor.offset();
        if self.cursor.peek() == Some('0') {
            let radix = match self.cursor.second() {
                Some('x' | 'X') => Some(16),
                Some('b' | 'B') => Some(2),
                Some('o' | 'O') => Some(8),
                _ => None,
            };
            if let Some(radix) = radix {
                self.cursor.bump();
                self.cursor.bump();
                let digits_start = self.cursor.offset();
                self.cursor
                    .skip_while(|character| character.is_digit(radix));
                let digits_end = self.cursor.offset();
                if digits_end == digits_start {
                    return Err(LexError {
                        span: Span::new(start, digits_end),
                        message: format!("missing base-{radix} digits in number literal"),
                    });
                }
                let digits = self.cursor.slice(Span::new(digits_start, digits_end));
                let value = u64::from_str_radix(digits, radix).map_err(|_| LexError {
                    span: Span::new(start, digits_end),
                    message: format!("invalid base-{radix} number literal"),
                })? as f64;
                if self.cursor.peek() == Some('n') {
                    self.cursor.bump();
                    return Ok(Token::new(
                        TokenKind::BigInt(value),
                        Span::new(start, self.cursor.offset()),
                    ));
                }
                return Ok(Token::new(
                    TokenKind::Number(value),
                    Span::new(start, digits_end),
                ));
            }
        }
        self.cursor.skip_while(|c| c.is_ascii_digit());
        let mut is_integer_literal = true;
        if self.cursor.peek() == Some('.') {
            is_integer_literal = false;
            self.cursor.bump();
            self.cursor.skip_while(|c| c.is_ascii_digit());
        }
        if matches!(self.cursor.peek(), Some('e' | 'E')) {
            is_integer_literal = false;
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
        if is_integer_literal && self.cursor.peek() == Some('n') {
            self.cursor.bump();
            return Ok(Token::new(
                TokenKind::BigInt(value),
                Span::new(start, self.cursor.offset()),
            ));
        }
        Ok(Token::new(TokenKind::Number(value), Span::new(start, end)))
    }
    fn read_template_literal(&mut self) -> Result<Token, LexError> {
        let start = self.cursor.offset();
        self.cursor
            .bump()
            .expect("template literal opens with a backtick");
        let mut value = String::new();
        loop {
            match self.cursor.bump() {
                None => {
                    return Err(LexError {
                        span: Span::new(start, self.cursor.offset()),
                        message: "unterminated template literal".into(),
                    });
                }
                Some('`') => {
                    let end = self.cursor.offset();
                    return Ok(Token::new(
                        TokenKind::TemplateLiteral(value),
                        Span::new(start, end),
                    ));
                }
                Some('\\') => self.read_string_escape(start, &mut value)?,
                Some('$') if self.cursor.peek() == Some('{') => {
                    return Err(LexError {
                        span: Span::new(start, self.cursor.offset()),
                        message: "template substitutions are not supported".into(),
                    });
                }
                Some(ch) => value.push(ch),
            }
        }
    }
    fn read_string(&mut self) -> Result<Token, LexError> {
        let start = self.cursor.offset();
        let quote = self
            .cursor
            .bump()
            .expect("string literal opens with a quote");
        let mut value = String::new();
        self.string_has_legacy_escape = false;
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
                    let mut token = Token::new(TokenKind::String(value), Span::new(start, end));
                    token.has_legacy_escape = self.string_has_legacy_escape;
                    return Ok(token);
                }
                Some('\\') => self.read_string_escape(start, &mut value)?,
                // ES2019+: U+000A (LF) and U+000D (CR) terminate string literals,
                // but U+2028 (LS) and U+2029 (PS) are now valid string content.
                Some('\n' | '\r') => {
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
            '0' => {
                // `\0` is a null escape. If followed by a decimal digit it
                // becomes a legacy octal escape sequence (`\00`, `\01`, …),
                // which is forbidden in strict-mode code.
                if self.cursor.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.string_has_legacy_escape = true;
                    // Read the additional octal digits (up to 2 more for \0NN).
                    let mut octal = 0u32;
                    for _ in 0..2 {
                        match self.cursor.peek() {
                            Some(d @ '0'..='7') => {
                                octal = octal * 8 + (d as u32 - '0' as u32);
                                self.cursor.bump();
                            }
                            _ => break,
                        }
                    }
                    let ch = char::from_u32(octal).unwrap_or('\0');
                    value.push(ch);
                } else {
                    value.push('\0');
                }
            }
            // Legacy octal escape sequences \1–\7 (single-digit).
            d @ '1'..='7' => {
                self.string_has_legacy_escape = true;
                let mut octal = d as u32 - '0' as u32;
                // Two-digit octal: \NM where N is 1–3 and M is 0–7.
                if d <= '3' {
                    if let Some(m @ '0'..='7') = self.cursor.peek() {
                        octal = octal * 8 + (m as u32 - '0' as u32);
                        self.cursor.bump();
                        // Three-digit octal: \NML.
                        if let Some(l @ '0'..='7') = self.cursor.peek() {
                            octal = octal * 8 + (l as u32 - '0' as u32);
                            self.cursor.bump();
                        }
                    }
                } else if let Some(m @ '0'..='7') = self.cursor.peek() {
                    octal = octal * 8 + (m as u32 - '0' as u32);
                    self.cursor.bump();
                }
                let ch = char::from_u32(octal).unwrap_or('\u{FFFD}');
                value.push(ch);
            }
            // Non-octal decimal escapes \8 and \9 — also forbidden in strict mode.
            '8' | '9' => {
                self.string_has_legacy_escape = true;
                value.push(escape);
            }
            'x' => {
                let code_point = self.read_hex_escape(start, 2)?;
                value.push(char::from_u32(code_point).expect("two hex digits form a scalar value"));
            }
            'u' => {
                let first = self.read_unicode_escape_value(start)?;
                let code_point = if (0xD800..=0xDBFF).contains(&first)
                    && self.cursor.rest().starts_with("\\u")
                {
                    self.cursor.bump();
                    self.cursor.bump();
                    let second = self.read_unicode_escape_value(start)?;
                    if !(0xDC00..=0xDFFF).contains(&second) {
                        return Err(self.invalid_unicode_escape(start));
                    }
                    0x1_0000 + ((first - 0xD800) << 10) + (second - 0xDC00)
                } else {
                    first
                };
                // AgentJS currently stores strings as UTF-8, so an isolated
                // UTF-16 surrogate cannot be represented losslessly yet.
                let character = char::from_u32(code_point).unwrap_or(char::REPLACEMENT_CHARACTER);
                value.push(character);
            }
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

    fn read_hex_escape(&mut self, string_start: usize, digits: usize) -> Result<u32, LexError> {
        let mut value = 0_u32;
        for _ in 0..digits {
            let digit = self
                .cursor
                .bump()
                .and_then(|ch| ch.to_digit(16))
                .ok_or_else(|| self.invalid_unicode_escape(string_start))?;
            value = (value << 4) | digit;
        }
        Ok(value)
    }

    fn read_identifier_escape(&mut self, identifier_start: usize) -> Result<char, LexError> {
        debug_assert!(self.cursor.rest().starts_with("\\u"));
        self.cursor.bump();
        self.cursor.bump();
        let code_point = self.read_unicode_escape_value(identifier_start)?;
        char::from_u32(code_point).ok_or_else(|| self.invalid_identifier_escape(identifier_start))
    }

    fn read_unicode_escape_value(&mut self, start: usize) -> Result<u32, LexError> {
        if self.cursor.peek() != Some('{') {
            return self.read_hex_escape(start, 4);
        }
        self.cursor.bump();
        let mut value = 0_u32;
        let mut digits = 0;
        while let Some(character) = self.cursor.peek() {
            if character == '}' {
                break;
            }
            let digit = character
                .to_digit(16)
                .ok_or_else(|| self.invalid_unicode_escape(start))?;
            value = value
                .checked_mul(16)
                .and_then(|current| current.checked_add(digit))
                .ok_or_else(|| self.invalid_unicode_escape(start))?;
            digits += 1;
            self.cursor.bump();
        }
        if digits == 0 || self.cursor.bump() != Some('}') || value > 0x10_FFFF {
            return Err(self.invalid_unicode_escape(start));
        }
        Ok(value)
    }

    fn invalid_identifier_escape(&self, identifier_start: usize) -> LexError {
        LexError {
            span: Span::new(identifier_start, self.cursor.offset()),
            message: "invalid Unicode escape in identifier".into(),
        }
    }

    fn invalid_unicode_escape(&self, string_start: usize) -> LexError {
        LexError {
            span: Span::new(string_start, self.cursor.offset()),
            message: "invalid hexadecimal escape sequence".into(),
        }
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

        // Fallback: consume the unknown character (and for backslash, also the
        // escaped character) as a placeholder operator token. This prevents lex
        // errors for characters that appear inside regex literal bodies such as
        // `\d`, `\s`, `|`, `^`, etc. The regex relexer reads from the original
        // source bytes and ignores these placeholder tokens entirely.
        self.cursor.bump();
        if ch == '\\' && self.cursor.peek().is_some() {
            self.cursor.bump();
        }
        Ok(Token::new(
            TokenKind::Operator("\0".to_owned()),
            Span::new(start, self.cursor.offset()),
        ))
    }
}

/// Reads a regex literal from `source` starting at byte offset `start`.
///
/// `source[start]` must be the opening `/`. Returns `(pattern, flags, end_offset)`
/// where `end_offset` is the byte position immediately after the closing flags.
/// Call this from the parser when `/` appears in a primary-expression position.
pub fn read_regex_literal_at(
    source: &str,
    start: usize,
) -> Result<(String, String, usize), LexError> {
    let bytes = source.as_bytes();
    let mut i = start;

    // Consume the opening '/'
    debug_assert_eq!(bytes.get(i), Some(&b'/'));
    i += 1;

    // The very first char cannot be '/' (empty body would close immediately and
    // re-open a comment) or '*' (would start a block comment).
    match bytes.get(i) {
        None => {
            return Err(LexError {
                span: Span::new(start, i),
                message: "unterminated regex literal".into(),
            });
        }
        Some(b'*') => {
            return Err(LexError {
                span: Span::new(start, i + 1),
                message: "regex literal body cannot begin with `*`".into(),
            });
        }
        _ => {}
    }

    let mut pattern = String::new();
    let mut in_class = false;

    loop {
        let Some(&byte) = bytes.get(i) else {
            return Err(LexError {
                span: Span::new(start, i),
                message: "unterminated regex literal".into(),
            });
        };
        match byte {
            b'\n' | b'\r' => {
                return Err(LexError {
                    span: Span::new(start, i),
                    message: "unterminated regex literal".into(),
                });
            }
            b'\\' => {
                pattern.push('\\');
                i += 1;
                if let Some(ch) = source[i..].chars().next() {
                    pattern.push(ch);
                    i += ch.len_utf8();
                }
            }
            b'[' => {
                in_class = true;
                pattern.push('[');
                i += 1;
            }
            b']' => {
                in_class = false;
                pattern.push(']');
                i += 1;
            }
            b'/' if !in_class => {
                i += 1;
                break;
            }
            _ => {
                let ch = source[i..].chars().next().unwrap_or('\0');
                pattern.push(ch);
                i += ch.len_utf8().max(1);
            }
        }
    }

    // Read flags: identifier-like characters after the closing '/'.
    let mut flags = String::new();
    while let Some(ch) = source[i..].chars().next() {
        if ch.is_alphabetic() || ch == '_' || ch == '$' {
            flags.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }

    Ok((pattern, flags, i))
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

/// Unicode identifier start characters (`$` and `_` are permitted by ECMAScript).
fn is_identifier_start(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_' || ch == '$'
}

/// Unicode identifier continuation characters.
fn is_identifier_part(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '$' | '\u{200C}' | '\u{200D}')
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
            [Token::with_line_terminator_before(
                TokenKind::Eof,
                Span::new(3, 3),
                true,
            )]
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
            kinds("0 18 3.5 .5 1e3 2.0e-2 0x2a 0b101 0o17"),
            [
                TokenKind::Number(0.0),
                TokenKind::Number(18.0),
                TokenKind::Number(3.5),
                TokenKind::Number(0.5),
                TokenKind::Number(1000.0),
                TokenKind::Number(0.02),
                TokenKind::Number(42.0),
                TokenKind::Number(5.0),
                TokenKind::Number(15.0),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn tokenizes_bigint_literals_as_temporary_numeric_payloads() {
        assert_eq!(
            kinds("1n 0xfn 0b101n 0o7n"),
            [
                TokenKind::BigInt(1.0),
                TokenKind::BigInt(15.0),
                TokenKind::BigInt(5.0),
                TokenKind::BigInt(7.0),
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
    fn tokenizes_no_substitution_template_literals() {
        assert_eq!(
            kinds("`a\\n\\${nope}`"),
            [
                TokenKind::TemplateLiteral("a\n${nope}".into()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn reports_template_substitutions_as_unsupported() {
        let error = Lexer::new("`a${b}`").tokenize().unwrap_err();
        assert_eq!(error.message, "template substitutions are not supported");
    }
    #[test]
    fn tokenizes_hexadecimal_and_unicode_string_escapes() {
        assert_eq!(
            kinds(r#""\x41\u0042\uD83D\uDE00\u{1F642}""#),
            [TokenKind::String("AB😀🙂".into()), TokenKind::Eof]
        );
    }

    #[test]
    fn tokenizes_unicode_identifiers() {
        assert_eq!(
            kinds("var 𠮷 = 1; 𠮷"),
            [
                TokenKind::Keyword(Keyword::Var),
                TokenKind::Identifier("𠮷".into()),
                TokenKind::Operator("=".into()),
                TokenKind::Number(1.0),
                TokenKind::Punctuator(';'),
                TokenKind::Identifier("𠮷".into()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn tokenizes_unicode_escapes_in_identifiers() {
        assert_eq!(
            kinds(r"var \u0061 = 1; a"),
            [
                TokenKind::Keyword(Keyword::Var),
                TokenKind::Identifier("a".into()),
                TokenKind::Operator("=".into()),
                TokenKind::Number(1.0),
                TokenKind::Punctuator(';'),
                TokenKind::Identifier("a".into()),
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
            kinds("=== !== <= >= && || += -= *= /= %= + ="),
            [
                TokenKind::Operator("===".into()),
                TokenKind::Operator("!==".into()),
                TokenKind::Operator("<=".into()),
                TokenKind::Operator(">=".into()),
                TokenKind::Operator("&&".into()),
                TokenKind::Operator("||".into()),
                TokenKind::Operator("+=".into()),
                TokenKind::Operator("-=".into()),
                TokenKind::Operator("*=".into()),
                TokenKind::Operator("/=".into()),
                TokenKind::Operator("%=".into()),
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
    fn unknown_character_produces_placeholder_not_lex_error() {
        // Unknown characters (e.g. `@`) that can appear in regex literal bodies
        // are now tokenized as placeholder Operator("\0") tokens instead of
        // causing an immediate lex error. A parse error is raised later if the
        // token appears outside a regex context.
        let tokens = Lexer::new("@").tokenize().unwrap();
        assert!(matches!(&tokens[0].kind, TokenKind::Operator(op) if op == "\0"));
    }

    #[test]
    fn tokenizes_v2_keywords_and_conditional_punctuators() {
        assert_eq!(
            kinds("if else while break continue throw new typeof void ? :"),
            [
                TokenKind::Keyword(Keyword::If),
                TokenKind::Keyword(Keyword::Else),
                TokenKind::Keyword(Keyword::While),
                TokenKind::Keyword(Keyword::Break),
                TokenKind::Keyword(Keyword::Continue),
                TokenKind::Keyword(Keyword::Throw),
                TokenKind::Keyword(Keyword::New),
                TokenKind::Keyword(Keyword::TypeOf),
                TokenKind::Keyword(Keyword::Void),
                TokenKind::Punctuator('?'),
                TokenKind::Punctuator(':'),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn tokenizes_v3_keywords_and_bracket_punctuators() {
        assert_eq!(
            kinds("function return [ ]"),
            [
                TokenKind::Keyword(Keyword::Function),
                TokenKind::Keyword(Keyword::Return),
                TokenKind::Punctuator('['),
                TokenKind::Punctuator(']'),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn tokenizes_v4_keywords() {
        assert_eq!(
            kinds("delete in instanceof"),
            [
                TokenKind::Keyword(Keyword::Delete),
                TokenKind::Keyword(Keyword::In),
                TokenKind::Keyword(Keyword::InstanceOf),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn tokenizes_v5_keywords() {
        assert_eq!(
            kinds("let const try catch finally switch case default"),
            [
                TokenKind::Keyword(Keyword::Let),
                TokenKind::Keyword(Keyword::Const),
                TokenKind::Keyword(Keyword::Try),
                TokenKind::Keyword(Keyword::Catch),
                TokenKind::Keyword(Keyword::Finally),
                TokenKind::Keyword(Keyword::Switch),
                TokenKind::Keyword(Keyword::Case),
                TokenKind::Keyword(Keyword::Default),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn tokenizes_array_literal_syntax() {
        assert_eq!(
            kinds("[1, 2, 3]"),
            [
                TokenKind::Punctuator('['),
                TokenKind::Number(1.0),
                TokenKind::Punctuator(','),
                TokenKind::Number(2.0),
                TokenKind::Punctuator(','),
                TokenKind::Number(3.0),
                TokenKind::Punctuator(']'),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn records_line_terminator_before_each_token() {
        let tokens = Lexer::new("throw\nx").tokenize().unwrap();
        // `throw` is preceded only by start-of-input.
        assert!(!tokens[0].line_terminator_before);
        // `x` sits on the next line, so the parser can reject `throw \n x`.
        assert!(tokens[1].line_terminator_before);
    }

    #[test]
    fn counts_line_terminators_inside_block_comments() {
        let tokens = Lexer::new("a /* \n */ b").tokenize().unwrap();
        assert!(!tokens[0].line_terminator_before);
        assert!(tokens[1].line_terminator_before);
    }
}
