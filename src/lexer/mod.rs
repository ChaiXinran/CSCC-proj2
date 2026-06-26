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
    ">>>=", "&&=", "||=", "??=", // 3-char
    "===", "!==", ">>=", ">>>", "<<=", "**=", // 2-char
    "=>", "==", "!=", "<=", ">=", "&&", "||", "??", "++", "--", "+=", "-=", "**", "*=", "/=", "%=", "|=",
    "^=", "&=", ">>", "<<", // 1-char
    "+", "-", "*", "/", "%", "!", "=", "<", ">", "|", "^", "&", "~", "?",
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
        // Tracks brace depth inside each open template substitution expression.
        // One entry per nesting level; entry = how many unmatched '{' we've seen
        // inside that substitution (so we know when the matching '}' closes it).
        let mut tpl_stack: Vec<u32> = Vec::new();

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

            let mut token = if ch == '}' && !tpl_stack.is_empty() {
                if *tpl_stack.last().expect("checked non-empty") == 0 {
                    // This `}` closes the innermost template substitution.
                    tpl_stack.pop();
                    self.read_template_middle_or_tail()?
                } else {
                    // A `}` that matches an earlier `{` inside the expression.
                    *tpl_stack.last_mut().expect("checked non-empty") -= 1;
                    self.read_operator_or_punctuator()?
                }
            } else if ch == '#'
                && self
                    .cursor
                    .rest()
                    .chars()
                    .nth(1)
                    .is_some_and(is_identifier_start)
            {
                self.read_private_name()?
            } else if is_identifier_start(ch)
                || (self.cursor.rest().starts_with("\\u")
                    && !tokens.last().is_some_and(|token| {
                        matches!(&token.kind, TokenKind::Operator(op) if op == "/" || op == "/=")
                    }))
            {
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

            // Track `{` depth inside template substitution expressions.
            if !tpl_stack.is_empty()
                && let TokenKind::Punctuator('{') = &token.kind
            {
                *tpl_stack.last_mut().expect("checked non-empty") += 1;
            }
            // A TemplateHead or TemplateMiddle opens a new substitution scope.
            if matches!(
                token.kind,
                TokenKind::TemplateHead(_) | TokenKind::TemplateMiddle(_)
            ) {
                tpl_stack.push(0);
            }

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

    fn read_private_name(&mut self) -> Result<Token, LexError> {
        let start = self.cursor.offset();
        self.cursor.bump(); // consume `#`
        let mut name = String::new();
        // Private names follow the same IdentifierName rules, including Unicode escapes.
        loop {
            if self.cursor.rest().starts_with("\\u") {
                let saved = self.cursor.clone();
                match self.read_identifier_escape(start) {
                    Ok(ch) if is_identifier_part(ch) => name.push(ch),
                    _ => {
                        self.cursor = saved;
                        break;
                    }
                }
            } else if self.cursor.peek().is_some_and(is_identifier_part) {
                name.push(self.cursor.bump().expect("identifier part exists"));
            } else {
                break;
            }
        }
        let end = self.cursor.offset();
        Ok(Token::new(TokenKind::PrivateName(name), Span::new(start, end)))
    }

    fn read_identifier_or_keyword(&mut self) -> Result<Token, LexError> {
        let start = self.cursor.offset();
        let mut text = String::new();
        let mut had_escape = false;

        if self.cursor.rest().starts_with("\\u") {
            let saved = self.cursor.clone();
            had_escape = true;
            match self.read_identifier_escape(start) {
                Ok(character) if is_identifier_start(character) => text.push(character),
                _ => {
                    self.cursor = saved;
                    return self.read_operator_or_punctuator();
                }
            }
        } else {
            text.push(self.cursor.bump().expect("identifier start exists"));
        }

        loop {
            if self.cursor.rest().starts_with("\\u") {
                let saved = self.cursor.clone();
                had_escape = true;
                match self.read_identifier_escape(start) {
                    Ok(character) if is_identifier_part(character) => text.push(character),
                    _ => {
                        self.cursor = saved;
                        break;
                    }
                }
            } else if self.cursor.peek().is_some_and(is_identifier_part) {
                text.push(self.cursor.bump().expect("identifier part exists"));
            } else {
                break;
            }
        }
        let end = self.cursor.offset();
        let kind = if had_escape {
            // Identifiers containing Unicode escapes cannot be contextual keywords.
            let mut tok = Token::new(TokenKind::Identifier(text), Span::new(start, end));
            tok.has_identifier_escape = true;
            return Ok(tok);
        } else {
            match text.as_str() {
                "let" => TokenKind::Keyword(Keyword::Let),
                "const" => TokenKind::Keyword(Keyword::Const),
                "var" => TokenKind::Keyword(Keyword::Var),
                "function" => TokenKind::Keyword(Keyword::Function),
                "return" => TokenKind::Keyword(Keyword::Return),
                "if" => TokenKind::Keyword(Keyword::If),
                "else" => TokenKind::Keyword(Keyword::Else),
                "do" => TokenKind::Keyword(Keyword::Do),
                "while" => TokenKind::Keyword(Keyword::While),
                "for" => TokenKind::Keyword(Keyword::For),
                "break" => TokenKind::Keyword(Keyword::Break),
                "continue" => TokenKind::Keyword(Keyword::Continue),
                "debugger" => TokenKind::Keyword(Keyword::Debugger),
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
                "class" => TokenKind::Keyword(Keyword::Class),
                "extends" => TokenKind::Keyword(Keyword::Extends),
                "static" => TokenKind::Keyword(Keyword::Static),
                "super" => TokenKind::Keyword(Keyword::Super),
                "this" => TokenKind::Keyword(Keyword::This),
                "with" => TokenKind::Keyword(Keyword::With),
                "import" => TokenKind::Keyword(Keyword::Import),
                "export" => TokenKind::Keyword(Keyword::Export),
                "enum" => TokenKind::Keyword(Keyword::Enum),
                "yield" => TokenKind::Keyword(Keyword::Yield),
                "await" => TokenKind::Keyword(Keyword::Await),
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
                let digits = self.read_number_digits(radix, start)?;
                let digits_end = self.cursor.offset();
                if digits.is_empty() {
                    return Err(LexError {
                        span: Span::new(start, digits_end),
                        message: format!("missing base-{radix} digits in number literal"),
                    });
                }
                if self.cursor.peek() == Some('n') {
                    self.cursor.bump();
                    let raw = self.cursor.slice(Span::new(start, self.cursor.offset()));
                    return Ok(Token::new(
                        TokenKind::BigInt(raw.into()),
                        Span::new(start, self.cursor.offset()),
                    ));
                }
                let value = u64::from_str_radix(&digits, radix).map_err(|_| LexError {
                    span: Span::new(start, digits_end),
                    message: format!("invalid base-{radix} number literal"),
                })? as f64;
                return Ok(Token::new(
                    TokenKind::Number(value),
                    Span::new(start, self.cursor.offset()),
                ));
            }
        }
        let mut text = if self.cursor.peek() == Some('.') {
            "0".into()
        } else {
            self.read_number_digits(10, start)?
        };
        let integer_end = self.cursor.offset();
        let integer_raw = self.cursor.slice(Span::new(start, integer_end));
        let leading_zero_with_separator = integer_raw.starts_with('0') && integer_raw.contains('_');
        if leading_zero_with_separator {
            return Err(self.invalid_numeric_separator(start));
        }
        let mut is_integer_literal = true;
        if self.cursor.peek() == Some('.') {
            is_integer_literal = false;
            text.push('.');
            self.cursor.bump();
            if self.cursor.peek() == Some('_') {
                return Err(self.invalid_numeric_separator(start));
            }
            text.push_str(&self.read_optional_number_digits(10, start)?);
        }
        if matches!(self.cursor.peek(), Some('e' | 'E')) {
            is_integer_literal = false;
            text.push('e');
            self.cursor.bump();
            if matches!(self.cursor.peek(), Some('+' | '-')) {
                text.push(self.cursor.bump().expect("exponent sign exists"));
            }
            let exponent_start = self.cursor.offset();
            let exponent_digits = self.read_number_digits(10, start)?;
            if self.cursor.offset() == exponent_start {
                let end = self.cursor.offset();
                return Err(LexError {
                    span: Span::new(start, end),
                    message: "missing exponent digits in number literal".into(),
                });
            }
            text.push_str(&exponent_digits);
        }

        let end = self.cursor.offset();
        // Detect legacy octal (012) and non-octal decimal (08, 09) integer literals.
        // These start with `0` and have more than one digit with no radix prefix.
        // They are valid in sloppy mode but SyntaxErrors in strict mode.
        let is_legacy_numeric = is_integer_literal
            && integer_raw.len() > 1
            && integer_raw.starts_with('0')
            && !integer_raw.starts_with("0_");
        let value = text.parse::<f64>().map_err(|_| LexError {
            span: Span::new(start, end),
            message: format!("invalid number literal `{text}`"),
        })?;
        if is_integer_literal && self.cursor.peek() == Some('n') {
            if integer_raw.starts_with('0') && text.len() > 1 {
                return Err(LexError {
                    span: Span::new(start, self.cursor.offset() + 1),
                    message: "BigInt literals cannot use legacy octal-like decimal syntax".into(),
                });
            }
            self.cursor.bump();
            let raw = self.cursor.slice(Span::new(start, self.cursor.offset()));
            return Ok(Token::new(
                TokenKind::BigInt(raw.into()),
                Span::new(start, self.cursor.offset()),
            ));
        }
        let mut tok = Token::new(TokenKind::Number(value), Span::new(start, end));
        tok.has_legacy_numeric = is_legacy_numeric;
        Ok(tok)
    }

    fn read_number_digits(&mut self, radix: u32, literal_start: usize) -> Result<String, LexError> {
        let digits = self.read_optional_number_digits(radix, literal_start)?;
        if digits.is_empty() {
            return Err(LexError {
                span: Span::new(literal_start, self.cursor.offset()),
                message: format!("missing base-{radix} digits in number literal"),
            });
        }
        Ok(digits)
    }

    fn read_optional_number_digits(
        &mut self,
        radix: u32,
        literal_start: usize,
    ) -> Result<String, LexError> {
        let mut digits = String::new();
        let mut previous_was_digit = false;
        while let Some(character) = self.cursor.peek() {
            if character.is_digit(radix) {
                digits.push(character);
                previous_was_digit = true;
                self.cursor.bump();
            } else if character == '_' {
                if !previous_was_digit
                    || !self
                        .cursor
                        .second()
                        .is_some_and(|next| next.is_digit(radix))
                {
                    return Err(self.invalid_numeric_separator(literal_start));
                }
                previous_was_digit = false;
                self.cursor.bump();
            } else {
                break;
            }
        }
        Ok(digits)
    }

    fn invalid_numeric_separator(&self, literal_start: usize) -> LexError {
        LexError {
            span: Span::new(literal_start, self.cursor.offset()),
            message: "numeric separators may only appear between digits".into(),
        }
    }
    /// Reads a template literal starting at the opening backtick.
    /// Returns `TemplateLiteral` for no-substitution templates, or `TemplateHead`
    /// when the first `${` is encountered.
    fn read_template_literal(&mut self) -> Result<Token, LexError> {
        let start = self.cursor.offset();
        let saved = self.cursor.clone();
        self.cursor
            .bump()
            .expect("template literal opens with a backtick");
        let mut value = String::new();
        loop {
            match self.cursor.bump() {
                None => {
                    if self.char_can_be_regex_body(start) {
                        self.cursor = saved;
                        return Ok(self.read_regex_body_placeholder());
                    }
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
                Some('\\') => {
                    if let Err(error) = self.read_string_escape(start, &mut value) {
                        if self.char_can_be_regex_body(start) {
                            self.cursor = saved;
                            return Ok(self.read_regex_body_placeholder());
                        }
                        return Err(error);
                    }
                }
                Some('$') if self.cursor.peek() == Some('{') => {
                    self.cursor.bump(); // consume '{'
                    let end = self.cursor.offset();
                    return Ok(Token::new(
                        TokenKind::TemplateHead(value),
                        Span::new(start, end),
                    ));
                }
                Some(ch) => value.push(ch),
            }
        }
    }

    /// Reads the text after a `}` that closes a template substitution, up to
    /// the next `${` (returning `TemplateMiddle`) or closing backtick
    /// (returning `TemplateTail`). The `}` itself has already been detected
    /// (peeked) by `tokenize`; this method consumes it.
    fn read_template_middle_or_tail(&mut self) -> Result<Token, LexError> {
        let start = self.cursor.offset();
        self.cursor.bump().expect("} was peeked");
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
                        TokenKind::TemplateTail(value),
                        Span::new(start, end),
                    ));
                }
                Some('\\') => self.read_string_escape(start, &mut value)?,
                Some('$') if self.cursor.peek() == Some('{') => {
                    self.cursor.bump(); // consume '{'
                    let end = self.cursor.offset();
                    return Ok(Token::new(
                        TokenKind::TemplateMiddle(value),
                        Span::new(start, end),
                    ));
                }
                Some(ch) => value.push(ch),
            }
        }
    }
    fn read_string(&mut self) -> Result<Token, LexError> {
        let start = self.cursor.offset();
        let saved = self.cursor.clone();
        let quote = self
            .cursor
            .bump()
            .expect("string literal opens with a quote");
        let mut value = String::new();
        self.string_has_legacy_escape = false;
        loop {
            match self.cursor.bump() {
                None => {
                    if self.char_can_be_regex_body(start) {
                        self.cursor = saved;
                        return Ok(self.read_regex_body_placeholder());
                    }
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
                Some('\\') => {
                    if let Err(error) = self.read_string_escape(start, &mut value) {
                        if self.char_can_be_regex_body(start) {
                            self.cursor = saved;
                            return Ok(self.read_regex_body_placeholder());
                        }
                        return Err(error);
                    }
                }
                // ES2019+: U+000A (LF) and U+000D (CR) terminate string literals,
                // but U+2028 (LS) and U+2029 (PS) are now valid string content.
                Some('\n' | '\r') => {
                    if self.char_can_be_regex_body(start) {
                        self.cursor = saved;
                        return Ok(self.read_regex_body_placeholder());
                    }
                    return Err(LexError {
                        span: Span::new(start, self.cursor.offset()),
                        message: "unterminated string literal".into(),
                    });
                }
                Some(ch) => value.push(ch),
            }
        }
    }

    fn char_can_be_regex_body(&self, quote_start: usize) -> bool {
        let before = self.cursor.slice(Span::new(0, quote_start));
        let line_start = before
            .char_indices()
            .filter_map(|(index, ch)| is_line_terminator(ch).then_some(index + ch.len_utf8()))
            .next_back()
            .unwrap_or(0);
        if !before[line_start..].contains('/') {
            return false;
        }

        let quote_end = quote_start
            + self
                .cursor
                .slice(Span::new(quote_start, quote_start + 1))
                .len();
        let after = self
            .cursor
            .slice(Span::new(quote_end, self.cursor.offset()));
        for ch in after.chars() {
            if is_line_terminator(ch) {
                return false;
            }
            if ch == '/' {
                return true;
            }
        }
        false
    }

    fn read_regex_body_placeholder(&mut self) -> Token {
        let start = self.cursor.offset();
        self.cursor.bump();
        Token::new(
            TokenKind::Operator("\0".to_owned()),
            Span::new(start, self.cursor.offset()),
        )
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

        // Handle `...` (spread / rest) before the single-`.` punctuator check.
        if ch == '.' && self.cursor.rest().starts_with("...") {
            self.cursor.bump();
            self.cursor.bump();
            self.cursor.bump();
            return Ok(Token::new(
                TokenKind::Operator("...".to_owned()),
                Span::new(start, self.cursor.offset()),
            ));
        }

        // `??=`, `??`, `?.` must be tried before the single-`?` punctuator.
        if ch == '?' {
            let rest = self.cursor.rest();
            let (op_str, len) = if rest.starts_with("??=") {
                ("??=", 3)
            } else if rest.starts_with("??") {
                ("??", 2)
            } else if rest.starts_with("?.") {
                ("?.", 2)
            } else {
                ("?", 1)
            };
            for _ in 0..len {
                self.cursor.bump();
            }
            let kind = if len == 1 {
                TokenKind::Punctuator('?')
            } else {
                TokenKind::Operator(op_str.to_owned())
            };
            return Ok(Token::new(kind, Span::new(start, self.cursor.offset())));
        }

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

/// Validates the body of a regex literal for ES early errors:
///  - inline modifier groups `(?flags:...)` — flags must be subset of {i,m,s}, no dups
///  - arithmetic modifier groups `(?add-remove:...)` — always rejected (ES2025 unsupported)
///  - named capture groups `(?<name>...)` — name must be valid identifier, no duplicates
///  - named backreferences `\k<name>` — must reference a defined capture group
///
/// The body is the raw pattern string (between `/` delimiters) already collected by the
/// lexer. `lex_start` is the source offset of the opening `/` for error spans.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RegexClassAtomKind {
    Single,
    Multi,
}

fn is_regex_hex_digit(ch: char) -> bool {
    ch.is_ascii_hexdigit()
}

fn is_regex_syntax_char(ch: char) -> bool {
    matches!(
        ch,
        '^' | '$' | '\\' | '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
    )
}

fn parse_regex_decimal(chars: &[char], start: usize) -> (usize, usize) {
    let mut i = start;
    let mut value = 0usize;
    while i < chars.len() && chars[i].is_ascii_digit() {
        value = value
            .saturating_mul(10)
            .saturating_add(chars[i].to_digit(10).unwrap_or(0) as usize);
        i += 1;
    }
    (value, i)
}

fn parse_braced_quantifier(chars: &[char], start: usize) -> Option<(usize, Option<usize>, usize)> {
    if chars.get(start) != Some(&'{') {
        return None;
    }

    let mut i = start + 1;
    if !chars.get(i).is_some_and(|ch| ch.is_ascii_digit()) {
        return None;
    }

    let (min, next) = parse_regex_decimal(chars, i);
    i = next;
    let max = if chars.get(i) == Some(&',') {
        i += 1;
        if chars.get(i).is_some_and(|ch| ch.is_ascii_digit()) {
            let (max, next) = parse_regex_decimal(chars, i);
            i = next;
            Some(max)
        } else {
            None
        }
    } else {
        Some(min)
    };

    if chars.get(i) == Some(&'}') {
        Some((min, max, i + 1))
    } else {
        None
    }
}

fn is_regex_quantifier_at(chars: &[char], index: usize) -> bool {
    matches!(chars.get(index), Some('*' | '+' | '?'))
        || parse_braced_quantifier(chars, index).is_some()
}

fn find_regex_group_end(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start + 1;
    let mut depth = 1usize;
    let mut in_class = false;

    while i < chars.len() {
        match chars[i] {
            '\\' => i += 2,
            '[' if !in_class => {
                in_class = true;
                i += 1;
            }
            ']' if in_class => {
                in_class = false;
                i += 1;
            }
            '(' if !in_class => {
                depth += 1;
                i += 1;
            }
            ')' if !in_class => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
                i += 1;
            }
            _ => i += 1,
        }
    }

    None
}

fn validate_regex_property_escape(chars: &[char], index: &mut usize) -> Result<(), &'static str> {
    if chars.get(*index) != Some(&'{') {
        return Err("Unicode property escape must use braces");
    }
    *index += 1;
    let start = *index;
    while *index < chars.len() && chars[*index] != '}' {
        let ch = chars[*index];
        if !ch.is_ascii_alphanumeric() && ch != '_' && ch != '=' {
            return Err("invalid character in Unicode property escape");
        }
        *index += 1;
    }
    if chars.get(*index) != Some(&'}') {
        return Err("unterminated Unicode property escape");
    }
    if start == *index {
        return Err("empty Unicode property escape");
    }

    let spec: String = chars[start..*index].iter().collect();
    if spec.starts_with("In") || spec.starts_with('^') || spec.matches('=').count() > 1 {
        return Err("invalid Unicode property escape");
    }

    *index += 1;
    Ok(())
}

fn validate_unicode_regex_escape(
    chars: &[char],
    index: &mut usize,
    escape: char,
    in_class: bool,
) -> Result<RegexClassAtomKind, &'static str> {
    match escape {
        'f' | 'n' | 'r' | 't' | 'v' | 'b' | 'B' => Ok(RegexClassAtomKind::Single),
        'd' | 'D' | 's' | 'S' | 'w' | 'W' => Ok(RegexClassAtomKind::Multi),
        'p' | 'P' => {
            validate_regex_property_escape(chars, index)?;
            Ok(RegexClassAtomKind::Multi)
        }
        '0' => {
            if chars.get(*index).is_some_and(|next| next.is_ascii_digit()) {
                Err("legacy octal escape is invalid in Unicode regular expression")
            } else {
                Ok(RegexClassAtomKind::Single)
            }
        }
        'x' => {
            if *index + 1 < chars.len()
                && is_regex_hex_digit(chars[*index])
                && is_regex_hex_digit(chars[*index + 1])
            {
                *index += 2;
                Ok(RegexClassAtomKind::Single)
            } else {
                Err("invalid hexadecimal escape in regular expression")
            }
        }
        'u' => {
            if chars.get(*index) == Some(&'{') {
                *index += 1;
                let hex_start = *index;
                let mut value = 0u32;
                while *index < chars.len() && chars[*index] != '}' {
                    let Some(digit) = chars[*index].to_digit(16) else {
                        return Err("invalid Unicode escape in regular expression");
                    };
                    value = value.saturating_mul(16).saturating_add(digit);
                    *index += 1;
                }
                if chars.get(*index) != Some(&'}') || hex_start == *index || value > 0x10ffff {
                    return Err("invalid Unicode escape in regular expression");
                }
                *index += 1;
                Ok(RegexClassAtomKind::Single)
            } else if *index + 3 < chars.len()
                && chars[*index..*index + 4]
                    .iter()
                    .copied()
                    .all(is_regex_hex_digit)
            {
                *index += 4;
                Ok(RegexClassAtomKind::Single)
            } else {
                Err("invalid Unicode escape in regular expression")
            }
        }
        'c' => {
            if chars
                .get(*index)
                .is_some_and(|next| next.is_ascii_alphabetic())
            {
                *index += 1;
                Ok(RegexClassAtomKind::Single)
            } else {
                Err("invalid control escape in regular expression")
            }
        }
        '/' => Ok(RegexClassAtomKind::Single),
        '-' if in_class => Ok(RegexClassAtomKind::Single),
        ch if is_regex_syntax_char(ch) => Ok(RegexClassAtomKind::Single),
        _ => Err("invalid identity escape in Unicode regular expression"),
    }
}

fn regex_class_atom_at(
    chars: &[char],
    start: usize,
    unicode_mode: bool,
) -> Result<Option<(RegexClassAtomKind, usize)>, &'static str> {
    let Some(&ch) = chars.get(start) else {
        return Ok(None);
    };
    if ch == ']' {
        return Ok(None);
    }
    if ch == '\\' {
        let Some(&escape) = chars.get(start + 1) else {
            return Err("unterminated escape in regular expression character class");
        };
        let mut next = start + 2;
        let kind = if unicode_mode {
            validate_unicode_regex_escape(chars, &mut next, escape, true)?
        } else if matches!(escape, 'd' | 'D' | 's' | 'S' | 'w' | 'W') {
            RegexClassAtomKind::Multi
        } else {
            RegexClassAtomKind::Single
        };
        Ok(Some((kind, next)))
    } else {
        Ok(Some((RegexClassAtomKind::Single, start + 1)))
    }
}

fn validate_regex_body(body: &str, flags: &str, lex_start: usize) -> Result<(), LexError> {
    let unicode_mode = flags.contains('u') || flags.contains('v');
    use std::collections::HashSet;

    let make_err = |msg: &str| LexError {
        span: Span::new(lex_start, lex_start + body.len()),
        message: msg.into(),
    };

    let chars: Vec<char> = body.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_class = false;
    let mut class_previous_atom: Option<RegexClassAtomKind> = None;
    let mut can_quantify = false;

    let mut capture_names: HashSet<String> = HashSet::new();
    let mut backref_names: Vec<String> = Vec::new();
    let mut decimal_backrefs: Vec<usize> = Vec::new();
    let mut capture_count = 0usize;
    // \k without <name> when named groups exist → error deferred to after full scan
    let mut bare_k_escape = false;

    if len > 0 && is_regex_quantifier_at(&chars, 0) {
        return Err(make_err(
            "regular expression quantifier has no preceding atom",
        ));
    }

    while i < len {
        if in_class {
            match chars[i] {
                '\\' => {
                    let Some(&escape) = chars.get(i + 1) else {
                        return Err(make_err(
                            "unterminated escape in regular expression character class",
                        ));
                    };
                    i += 2;
                    let atom = if unicode_mode {
                        validate_unicode_regex_escape(&chars, &mut i, escape, true)
                            .map_err(make_err)?
                    } else if matches!(escape, 'd' | 'D' | 's' | 'S' | 'w' | 'W') {
                        RegexClassAtomKind::Multi
                    } else {
                        RegexClassAtomKind::Single
                    };
                    class_previous_atom = Some(atom);
                    continue;
                }
                ']' => {
                    in_class = false;
                    class_previous_atom = None;
                    can_quantify = true;
                    i += 1;
                    continue;
                }
                '-' if unicode_mode && i + 1 < len && chars[i + 1] != ']' => {
                    if let Some(previous) = class_previous_atom {
                        let Some((next_atom, next_i)) =
                            regex_class_atom_at(&chars, i + 1, unicode_mode)
                                .map_err(make_err)?
                        else {
                            class_previous_atom = Some(RegexClassAtomKind::Single);
                            i += 1;
                            continue;
                        };
                        if previous == RegexClassAtomKind::Multi
                            || next_atom == RegexClassAtomKind::Multi
                        {
                            return Err(make_err(
                                "character class range endpoint cannot be a character class escape",
                            ));
                        }
                        class_previous_atom = Some(RegexClassAtomKind::Single);
                        i = next_i;
                        continue;
                    }
                    class_previous_atom = Some(RegexClassAtomKind::Single);
                    i += 1;
                    continue;
                }
                _ => {
                    class_previous_atom = Some(RegexClassAtomKind::Single);
                    i += 1;
                    continue;
                }
            }
        }
        match chars[i] {
            '\\' => {
                i += 1; // skip '\'
                if i < len {
                    let esc = chars[i];
                    i += 1;
                    if esc.is_ascii_digit() {
                        if esc == '0' {
                            if unicode_mode
                                && chars.get(i).is_some_and(|next| next.is_ascii_digit())
                            {
                                return Err(make_err(
                                    "legacy octal escape is invalid in Unicode regular expression",
                                ));
                            }
                        } else {
                            let (value, next) = parse_regex_decimal(&chars, i - 1);
                            i = next;
                            if unicode_mode {
                                decimal_backrefs.push(value);
                            }
                        }
                    } else if esc == 'k' {
                        if chars.get(i) == Some(&'<') {
                            i += 1; // skip '<'
                            let ref_start = i;
                            while i < len && chars[i] != '>' {
                                i += 1;
                            }
                            if chars.get(i) != Some(&'>') {
                                return Err(make_err(
                                    "unterminated named backreference in regular expression",
                                ));
                            }
                            let ref_name: String = chars[ref_start..i].iter().collect();
                            i += 1; // skip '>'
                            backref_names.push(ref_name);
                        } else {
                            // \k not followed by '<': may be invalid if named groups exist
                            bare_k_escape = true;
                        }
                    } else if unicode_mode {
                        validate_unicode_regex_escape(&chars, &mut i, esc, false)
                            .map_err(make_err)?;
                    }
                    can_quantify = true;
                }
            }
            '[' => {
                in_class = true;
                class_previous_atom = None;
                can_quantify = false;
                i += 1;
            }
            '(' if i + 1 < len && chars[i + 1] == '?' => {
                let group_start = i;
                i += 2; // skip (?
                match chars.get(i) {
                    Some(&'=') | Some(&'!') => {
                        if let Some(end) = find_regex_group_end(&chars, group_start)
                            && unicode_mode
                            && is_regex_quantifier_at(&chars, end + 1)
                        {
                            return Err(make_err(
                                "lookahead assertion cannot be quantified in Unicode regular expression",
                            ));
                        }
                        can_quantify = false;
                        i += 1;
                    }
                    Some(&':') => {
                        can_quantify = false;
                        i += 1;
                    }
                    Some(&'<') => {
                        i += 1; // skip '<'
                        let lookbehind_end = find_regex_group_end(&chars, group_start);
                        if matches!(chars.get(i), Some(&'=') | Some(&'!'))
                            && lookbehind_end
                                .is_some_and(|end| is_regex_quantifier_at(&chars, end + 1))
                        {
                            return Err(make_err("lookbehind assertion cannot be quantified"));
                        }
                        if matches!(chars.get(i), Some(&'=') | Some(&'!')) {
                            can_quantify = false;
                        }
                        match chars.get(i) {
                            Some(&'=') | Some(&'!') => {
                                i += 1;
                            } // lookbehind — skip
                            _ => {
                                // Named capture group (?<name>...)
                                capture_count += 1;
                                let name_start = i;
                                while i < len && chars[i] != '>' {
                                    i += 1;
                                }
                                if chars.get(i) != Some(&'>') {
                                    return Err(make_err(
                                        "unterminated named capture group in regular expression",
                                    ));
                                }
                                let name: String = chars[name_start..i].iter().collect();
                                i += 1; // skip '>'
                                // Empty name
                                if name.is_empty() {
                                    return Err(make_err(
                                        "empty named capture group specifier in regular expression",
                                    ));
                                }
                                // Name must be a valid IdentifierName:
                                //   start char: letter, $, _, or Unicode letter (not digit/punct)
                                //   continue chars: letter, digit, $, _, Unicode letter/digit
                                let mut name_chars = name.chars();
                                let first = name_chars.next().unwrap();
                                if first.is_ascii_digit()
                                    || (!first.is_alphabetic() && first != '_' && first != '$')
                                {
                                    return Err(make_err(
                                        "invalid character at start of named capture group specifier",
                                    ));
                                }
                                for c in name_chars {
                                    if !c.is_alphanumeric() && c != '_' && c != '$' {
                                        return Err(make_err(
                                            "invalid character in named capture group specifier",
                                        ));
                                    }
                                }
                                // Duplicate name
                                if !capture_names.insert(name.clone()) {
                                    return Err(make_err(
                                        "duplicate named capture group in regular expression",
                                    ));
                                }
                            }
                        }
                    }
                    _ => {
                        // Potential modifier group: collect flag-like chars.
                        // Any non-ims char, non-ASCII letter, or first char '-' → error.
                        let mut add_flags: Vec<char> = Vec::new();
                        while i < len {
                            let c = chars[i];
                            if c == ':' || c == '-' || c == ')' || c == '(' {
                                break;
                            }
                            if c.is_alphanumeric() || c == '_' || c == '$' {
                                add_flags.push(c);
                                i += 1;
                            } else {
                                return Err(make_err(
                                    "invalid character in regular expression modifier flags",
                                ));
                            }
                        }
                        match chars.get(i) {
                            Some(&'-') => {
                                // Arithmetic modifier — always a parse-phase error.
                                for &c in &add_flags {
                                    if !matches!(c, 'i' | 'm' | 's') {
                                        return Err(make_err(
                                            "invalid modifier flag in regular expression",
                                        ));
                                    }
                                }
                                let mut seen = [false; 3];
                                for &c in &add_flags {
                                    let j = match c {
                                        'i' => 0,
                                        'm' => 1,
                                        _ => 2,
                                    };
                                    if seen[j] {
                                        return Err(make_err(
                                            "duplicate modifier flag in regular expression",
                                        ));
                                    }
                                    seen[j] = true;
                                }
                                return Err(make_err(
                                    "arithmetic modifier groups (?flags-flags:...) are not supported",
                                ));
                            }
                            Some(&':') => {
                                // Inline modifier: flags must be subset of {i,m,s}, no dups.
                                for &c in &add_flags {
                                    if !matches!(c, 'i' | 'm' | 's') {
                                        return Err(make_err(
                                            "invalid modifier flag in regular expression",
                                        ));
                                    }
                                }
                                let mut seen = [false; 3];
                                for &c in &add_flags {
                                    let j = match c {
                                        'i' => 0,
                                        'm' => 1,
                                        _ => 2,
                                    };
                                    if seen[j] {
                                        return Err(make_err(
                                            "duplicate modifier flag in regular expression",
                                        ));
                                    }
                                    seen[j] = true;
                                }
                                i += 1; // skip ':'
                            }
                            Some(&_c) if !add_flags.is_empty() => {
                                // Unexpected char after flags (e.g. space in `(?s :a)`)
                                return Err(make_err(
                                    "unexpected character in regular expression modifier group",
                                ));
                            }
                            Some(&_c) => {}
                            None => {}
                        }
                    }
                }
            }
            '(' => {
                capture_count += 1;
                can_quantify = false;
                i += 1;
            }
            ')' => {
                can_quantify = true;
                i += 1;
            }
            '*' | '+' | '?' => {
                if !can_quantify {
                    return Err(make_err(
                        "regular expression quantifier has no preceding atom",
                    ));
                }
                i += 1;
                if chars.get(i) == Some(&'?') {
                    i += 1;
                }
                can_quantify = false;
            }
            '{' => {
                if let Some((min, max, next)) = parse_braced_quantifier(&chars, i) {
                    if !can_quantify {
                        return Err(make_err(
                            "regular expression quantifier has no preceding atom",
                        ));
                    }
                    if max.is_some_and(|max| max < min) {
                        return Err(make_err("regular expression quantifier range is reversed"));
                    }
                    i = next;
                    if chars.get(i) == Some(&'?') {
                        i += 1;
                    }
                    can_quantify = false;
                } else if unicode_mode {
                    return Err(make_err(
                        "unescaped `{` is invalid in Unicode regular expression",
                    ));
                } else {
                    can_quantify = true;
                    i += 1;
                }
            }
            '}' if unicode_mode => {
                return Err(make_err(
                    "unescaped `}` is invalid in Unicode regular expression",
                ));
            }
            '|' | '^' | '$' => {
                can_quantify = false;
                i += 1;
            }
            _ => {
                can_quantify = true;
                i += 1;
            }
        }
    }

    // Validate named backreferences: each \k<name> must reference a defined group.
    for ref_name in &backref_names {
        if !capture_names.contains(ref_name.as_str()) {
            return Err(make_err(
                "named backreference refers to undefined capture group in regular expression",
            ));
        }
    }

    // \k without <name> is invalid: (a) in Unicode mode always, (b) when named groups exist.
    if bare_k_escape && (unicode_mode || !capture_names.is_empty()) {
        return Err(make_err(
            "\\k escape must be followed by <name> in regular expression",
        ));
    }

    if unicode_mode
        && decimal_backrefs
            .iter()
            .any(|&reference| reference > capture_count)
    {
        return Err(make_err(
            "decimal escape refers to a capture group that does not exist",
        ));
    }

    Ok(())
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
                    if is_line_terminator(ch) {
                        return Err(LexError {
                            span: Span::new(start, i + ch.len_utf8()),
                            message: "unterminated regex literal".into(),
                        });
                    }
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
                // U+2028 LINE SEPARATOR and U+2029 PARAGRAPH SEPARATOR are ES line
                // terminators and are forbidden unescaped in regex body.
                if matches!(ch, '\u{2028}' | '\u{2029}') {
                    return Err(LexError {
                        span: Span::new(start, i),
                        message: "unterminated regex literal".into(),
                    });
                }
                pattern.push(ch);
                i += ch.len_utf8().max(1);
            }
        }
    }

    // Read flags: only ECMAScript-recognized flag letters are valid.
    // Valid set (ES2023+): d g i m s u v y  — no duplicates allowed.
    let mut flags = String::new();
    while let Some(ch) = source[i..].chars().next() {
        if ch.is_alphabetic() || ch == '_' || ch == '$' {
            // Reject unknown flag characters immediately.
            if !matches!(ch, 'd' | 'g' | 'i' | 'm' | 's' | 'u' | 'v' | 'y') {
                return Err(LexError {
                    span: Span::new(start, i + ch.len_utf8()),
                    message: format!("invalid regular expression flag `{ch}`"),
                });
            }
            // Reject duplicate flags.
            if flags.contains(ch) {
                return Err(LexError {
                    span: Span::new(start, i + ch.len_utf8()),
                    message: format!("duplicate regular expression flag `{ch}`"),
                });
            }
            // `u` and `v` are mutually exclusive.
            if (ch == 'u' && flags.contains('v')) || (ch == 'v' && flags.contains('u')) {
                return Err(LexError {
                    span: Span::new(start, i + ch.len_utf8()),
                    message: "flags `u` and `v` cannot be combined".into(),
                });
            }
            flags.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }

    // Validate regex body for ES-specific early errors. Needs flags (e.g. /u mode).
    validate_regex_body(&pattern, &flags, start)?;

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
    ch.is_alphabetic() || matches!(ch, '_' | '$') || is_other_identifier_start(ch)
}

/// Unicode identifier continuation characters.
fn is_identifier_part(ch: char) -> bool {
    ch.is_alphanumeric()
        || matches!(ch, '_' | '$' | '\u{200C}' | '\u{200D}')
        || is_other_identifier_start(ch)
        || is_other_identifier_continue(ch)
}

fn is_other_identifier_start(ch: char) -> bool {
    matches!(
        ch,
        '\u{1885}' | '\u{1886}' | '\u{2118}' | '\u{212E}' | '\u{309B}' | '\u{309C}'
    )
}

fn is_other_identifier_continue(ch: char) -> bool {
    matches!(
        ch,
        '\u{00B7}' | '\u{0387}' | '\u{1369}'..='\u{1371}' | '\u{19DA}'
    )
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
    fn tokenizes_numeric_separators() {
        assert_eq!(
            kinds("1_000 12_34.5_6 1_2e3_4 0xFF_FF 0b1010_0101 0o7_7"),
            [
                TokenKind::Number(1000.0),
                TokenKind::Number(1234.56),
                TokenKind::Number(12e34),
                TokenKind::Number(65535.0),
                TokenKind::Number(165.0),
                TokenKind::Number(63.0),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn rejects_invalid_numeric_separators() {
        for source in [
            "1__0", "1_", "1_.0", "1._0", "1e_1", "1e1_", "0x_FF", "0xFF_", "0b1__0", "0o7_",
        ] {
            assert!(
                Lexer::new(source).tokenize().is_err(),
                "{source} should reject misplaced numeric separators"
            );
        }
    }

    #[test]
    fn tokenizes_bigint_literals_as_temporary_numeric_payloads() {
        assert_eq!(
            kinds("1n 1_000n 0xfn 0xf_fn 0b101n 0b1_01n 0o7n 0o7_7n"),
            [
                TokenKind::BigInt("1n".into()),
                TokenKind::BigInt("1_000n".into()),
                TokenKind::BigInt("0xfn".into()),
                TokenKind::BigInt("0xf_fn".into()),
                TokenKind::BigInt("0b101n".into()),
                TokenKind::BigInt("0b1_01n".into()),
                TokenKind::BigInt("0o7n".into()),
                TokenKind::BigInt("0o7_7n".into()),
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
    fn tokenizes_template_substitution_as_head_expr_tail() {
        assert_eq!(
            kinds("`a${b}`"),
            [
                TokenKind::TemplateHead("a".into()),
                TokenKind::Identifier("b".into()),
                TokenKind::TemplateTail("".into()),
                TokenKind::Eof,
            ]
        );
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
            kinds("if else do while break continue debugger throw new typeof void ? :"),
            [
                TokenKind::Keyword(Keyword::If),
                TokenKind::Keyword(Keyword::Else),
                TokenKind::Keyword(Keyword::Do),
                TokenKind::Keyword(Keyword::While),
                TokenKind::Keyword(Keyword::Break),
                TokenKind::Keyword(Keyword::Continue),
                TokenKind::Keyword(Keyword::Debugger),
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
