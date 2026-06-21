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
    Break,
    Continue,
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
            Self::While => "while",
            Self::For => "for",
            Self::Break => "break",
            Self::Continue => "continue",
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
        }
    }
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
    /// Whether an ECMAScript line terminator appeared between the end of the
    /// previous token and the start of this one, counting terminators inside
    /// skipped comments. The parser uses this for restricted productions such
    /// as `throw`, which forbids a newline before its expression.
    pub line_terminator_before: bool,
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
            line_terminator_before: false,
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
            line_terminator_before,
        }
    }
}
