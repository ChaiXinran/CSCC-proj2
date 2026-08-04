//! Stable token types shared by the lexer and parser.

use crate::runtime::JsString;

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
    Do,
    While,
    For,
    Break,
    Continue,
    Debugger,
    Throw,
    Try,
    Catch,
    Finally,
    Switch,
    Case,
    Default,
    New,
    TypeOf,
    Void,
    Delete,
    In,
    InstanceOf,
    True,
    False,
    Null,
    Class,
    Extends,
    Static,
    Super,
    This,
    With,
    Import,
    Export,
    Enum,
    // V9-A: generator / async
    Yield,
    Await,
}

impl Keyword {
    /// Returns the exact ECMAScript source spelling of this keyword.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Let => "let",
            Self::Const => "const",
            Self::Var => "var",
            Self::Function => "function",
            Self::Return => "return",
            Self::If => "if",
            Self::Else => "else",
            Self::Do => "do",
            Self::While => "while",
            Self::For => "for",
            Self::Break => "break",
            Self::Continue => "continue",
            Self::Debugger => "debugger",
            Self::Throw => "throw",
            Self::Try => "try",
            Self::Catch => "catch",
            Self::Finally => "finally",
            Self::Switch => "switch",
            Self::Case => "case",
            Self::Default => "default",
            Self::New => "new",
            Self::TypeOf => "typeof",
            Self::Void => "void",
            Self::Delete => "delete",
            Self::In => "in",
            Self::InstanceOf => "instanceof",
            Self::True => "true",
            Self::False => "false",
            Self::Null => "null",
            Self::Class => "class",
            Self::Extends => "extends",
            Self::Static => "static",
            Self::Super => "super",
            Self::This => "this",
            Self::With => "with",
            Self::Import => "import",
            Self::Export => "export",
            Self::Enum => "enum",
            Self::Yield => "yield",
            Self::Await => "await",
        }
    }
}

/// Storage for token text that is either source-backed or decoded.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenText {
    /// The semantic text is a delimiter-adjusted slice of [`Token::span`].
    SourceSlice,
    /// Escaped or normalized text that differs from the original source.
    Cooked(JsString),
}

impl From<String> for TokenText {
    fn from(value: String) -> Self {
        Self::Cooked(value.into())
    }
}

impl From<&str> for TokenText {
    fn from(value: &str) -> Self {
        Self::Cooked(value.into())
    }
}

/// Lexical token payload.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Eof,
    Identifier(TokenText),
    Number(f64),
    BigInt(TokenText),
    String(TokenText),
    /// No-substitution template literal: `` `text` ``.
    TemplateLiteral(TokenText),
    /// Start of a template with substitutions: `` `text${ ``.
    TemplateHead(TokenText),
    /// Middle part between two substitutions: `}text${`.
    TemplateMiddle(TokenText),
    /// End of a template with substitutions: `}text`` `.
    TemplateTail(TokenText),
    Keyword(Keyword),
    Punctuator(char),
    Operator(&'static str),
    /// `#name` — private class field/method identifier.
    PrivateName(TokenText),
}

/// One token and its location in source text.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    /// Raw source text for template token segments, excluding delimiters.
    /// `None` for every non-template token.
    pub template_raw: Option<TokenText>,
    /// Whether an ECMAScript line terminator appeared between the end of the
    /// previous token and the start of this one, counting terminators inside
    /// skipped comments. The parser uses this for restricted productions such
    /// as `throw`, which forbids a newline before its expression.
    pub line_terminator_before: bool,
    /// Set on `String` and `TemplateLiteral` tokens that contain a legacy
    /// octal escape sequence (`\1`–`\7`, `\00` followed by a digit) or a
    /// non-octal decimal escape (`\8`, `\9`). These sequences are forbidden
    /// inside strict-mode code; the parser checks this flag after determining
    /// whether the enclosing function or script is strict.
    pub has_legacy_escape: bool,
    /// Set on `Identifier` tokens that contain a Unicode escape sequence
    /// (`\uXXXX` or `\u{XXXX}`). Per spec, identifiers with escape sequences
    /// cannot serve as contextual keywords such as `async` or `let`, even when
    /// their decoded value matches the keyword spelling.
    pub has_identifier_escape: bool,
    /// Set on `Number` tokens that are legacy octal integer literals (`012`)
    /// or non-octal decimal integer literals (`08`). Both are forbidden in
    /// strict mode and the parser rejects them after determining strict context.
    pub has_legacy_numeric: bool,
}

impl Token {
    /// Builds a token with no preceding line terminator.
    ///
    /// This keeps hand-written tokens in tests concise; only the lexer needs to
    /// record real newline information via [`Token::with_line_terminator_before`].
    #[must_use]
    pub const fn new(kind: TokenKind, span: Span) -> Self {
        Self {
            kind,
            span,
            template_raw: None,
            line_terminator_before: false,
            has_legacy_escape: false,
            has_identifier_escape: false,
            has_legacy_numeric: false,
        }
    }

    /// Builds a token, recording whether a line terminator preceded it.
    #[must_use]
    pub const fn with_line_terminator_before(
        kind: TokenKind,
        span: Span,
        line_terminator_before: bool,
    ) -> Self {
        Self {
            kind,
            span,
            template_raw: None,
            line_terminator_before,
            has_legacy_escape: false,
            has_identifier_escape: false,
            has_legacy_numeric: false,
        }
    }

    #[must_use]
    pub fn text<'a>(&'a self, source: &'a str) -> &'a str {
        let text = match &self.kind {
            TokenKind::Identifier(text)
            | TokenKind::BigInt(text)
            | TokenKind::String(text)
            | TokenKind::TemplateLiteral(text)
            | TokenKind::TemplateHead(text)
            | TokenKind::TemplateMiddle(text)
            | TokenKind::TemplateTail(text)
            | TokenKind::PrivateName(text) => text,
            _ => return "",
        };
        match text {
            TokenText::Cooked(value) => value.as_str(),
            TokenText::SourceSlice => {
                let span = match self.kind {
                    TokenKind::Identifier(_) | TokenKind::BigInt(_) => self.span,
                    TokenKind::PrivateName(_) => Span::new(self.span.start + 1, self.span.end),
                    TokenKind::String(_) | TokenKind::TemplateLiteral(_) => {
                        Span::new(self.span.start + 1, self.span.end.saturating_sub(1))
                    }
                    TokenKind::TemplateHead(_) | TokenKind::TemplateMiddle(_) => {
                        Span::new(self.span.start + 1, self.span.end.saturating_sub(2))
                    }
                    TokenKind::TemplateTail(_) => {
                        Span::new(self.span.start + 1, self.span.end.saturating_sub(1))
                    }
                    _ => self.span,
                };
                source.get(span.start..span.end).unwrap_or("")
            }
        }
    }

    #[must_use]
    pub fn text_owned(&self, source: &str) -> String {
        self.text(source).to_owned()
    }

    #[must_use]
    pub fn text_shared(&self, source: &str) -> JsString {
        match &self.kind {
            TokenKind::Identifier(TokenText::Cooked(value))
            | TokenKind::BigInt(TokenText::Cooked(value))
            | TokenKind::String(TokenText::Cooked(value))
            | TokenKind::TemplateLiteral(TokenText::Cooked(value))
            | TokenKind::TemplateHead(TokenText::Cooked(value))
            | TokenKind::TemplateMiddle(TokenText::Cooked(value))
            | TokenKind::TemplateTail(TokenText::Cooked(value))
            | TokenKind::PrivateName(TokenText::Cooked(value)) => value.clone(),
            _ => self.text(source).into(),
        }
    }

    #[must_use]
    pub fn template_raw_text<'a>(&'a self, source: &'a str) -> Option<&'a str> {
        match self.template_raw.as_ref()? {
            TokenText::Cooked(value) => Some(value.as_str()),
            TokenText::SourceSlice => {
                let span = match self.kind {
                    TokenKind::TemplateLiteral(_) => {
                        Span::new(self.span.start + 1, self.span.end.saturating_sub(1))
                    }
                    TokenKind::TemplateHead(_) | TokenKind::TemplateMiddle(_) => {
                        Span::new(self.span.start + 1, self.span.end.saturating_sub(2))
                    }
                    TokenKind::TemplateTail(_) => {
                        Span::new(self.span.start + 1, self.span.end.saturating_sub(1))
                    }
                    _ => return None,
                };
                source.get(span.start..span.end)
            }
        }
    }

    #[must_use]
    pub fn requires_source(&self) -> bool {
        matches!(
            self.kind,
            TokenKind::Identifier(TokenText::SourceSlice)
                | TokenKind::BigInt(TokenText::SourceSlice)
                | TokenKind::String(TokenText::SourceSlice)
                | TokenKind::TemplateLiteral(TokenText::SourceSlice)
                | TokenKind::TemplateHead(TokenText::SourceSlice)
                | TokenKind::TemplateMiddle(TokenText::SourceSlice)
                | TokenKind::TemplateTail(TokenText::SourceSlice)
                | TokenKind::PrivateName(TokenText::SourceSlice)
        ) || matches!(self.template_raw, Some(TokenText::SourceSlice))
    }
}
