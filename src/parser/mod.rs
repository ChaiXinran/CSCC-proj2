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

/// Maximum nesting depth for parenthesized expressions, unary chains, and
/// statement blocks. Inputs that exceed this limit receive a `SyntaxError`
/// instead of overflowing the Rust call stack.
///
/// Kept at 50 so that even debug builds on Windows (1 MB default thread stack,
/// ~1 KB per recursive frame) stay well within the stack budget. Real-world
/// JavaScript rarely exceeds 20 levels of expression nesting.
pub(crate) const MAX_PARSE_DEPTH: usize = 50;

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
    /// Number of enclosing switches. A switch permits `break`, but not
    /// `continue`.
    switch_depth: usize,
    /// Number of enclosing function bodies. Used to reject `return` that appears
    /// outside any function.
    function_depth: usize,
    /// When true, the relational `in` operator is not consumed by expression
    /// parsing. Used while parsing a `for` header so `for (x in obj)` can be
    /// disambiguated from `for (x in y; …)`. Reset on entry to any nested
    /// bracketed sub-expression.
    no_in: bool,
    /// Original source text kept for regex literal relexing. When the parser
    /// encounters `/` in a primary-expression position it uses this to re-read
    /// the bytes that the context-free lexer split into separate tokens.
    source: Option<Box<str>>,
    /// Current recursive nesting depth across parenthesized expressions, unary
    /// chains, and statement blocks. Checked against [`MAX_PARSE_DEPTH`].
    nesting_depth: usize,
    /// Whether the current parsing context is strict-mode code. Set when a
    /// `"use strict"` directive prologue is detected in a script or function
    /// body. Used to enforce strict-mode early errors (e.g. legacy octal
    /// escapes in string literals, `delete` of an unqualified identifier).
    pub(super) is_strict: bool,
    /// Whether the innermost enclosing function is an async function. Used to
    /// reject `await` as a binding identifier in async function parameter lists
    /// and bodies (including escaped forms like `await`).
    pub(super) is_async_context: bool,
    /// Whether the innermost enclosing function is a generator. Used to reject
    /// `yield` as a binding identifier in generator parameter lists and bodies.
    pub(super) is_generator_context: bool,
    /// Stack of active labels. Each entry is `(label_name, is_iteration)` where
    /// `is_iteration` is true when the label directly or transitively labels a
    /// `while`, `for`, `for-in`, or `do-while` statement. Used to validate
    /// `break label` and `continue label` references.
    pub(super) label_stack: Vec<(String, bool)>,
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
            switch_depth: 0,
            function_depth: 0,
            no_in: false,
            source: None,
            nesting_depth: 0,
            is_strict: false,
            is_async_context: false,
            is_generator_context: false,
            label_stack: Vec::new(),
        }
    }

    /// Like [`Parser::new`] but retains the original source text so that regex
    /// literals can be relexed when `/` appears in a primary-expression context.
    #[must_use]
    pub fn with_source(tokens: Vec<Token>, source: &str) -> Self {
        let mut parser = Self::new(tokens);
        parser.source = Some(source.into());
        parser
    }

    /// Increments the nesting counter and returns `Err` if the limit is exceeded.
    /// Call [`Parser::leave_depth`] after the nested sub-parse completes.
    pub(super) fn enter_depth(&mut self) -> Result<(), ParseError> {
        self.nesting_depth += 1;
        if self.nesting_depth > MAX_PARSE_DEPTH {
            Err(self.error(format!("nesting depth exceeds limit of {MAX_PARSE_DEPTH}")))
        } else {
            Ok(())
        }
    }

    /// Decrements the nesting counter after a nested sub-parse.
    pub(super) fn leave_depth(&mut self) {
        self.nesting_depth = self.nesting_depth.saturating_sub(1);
    }

    /// Parses a complete script, consuming every token up to and including EOF.
    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        self.consume_directive_prologue()?;
        let mut body = Vec::new();
        while !self.at_eof() {
            body.push(self.parse_statement()?);
        }
        self.validate_lexical_declarations(&body)?;
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

    /// Returns `true` if the next token is `...` (spread / rest operator).
    pub(super) fn check_spread(&self) -> bool {
        matches!(&self.peek().kind, TokenKind::Operator(v) if v == "...")
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
        let tok = self.peek();
        // `await` is a reserved word inside async functions (including escaped forms).
        if self.is_async_context {
            match &tok.kind {
                TokenKind::Keyword(Keyword::Await) => {
                    return Err(self
                        .error("`await` is not allowed as an identifier in async context".into()));
                }
                TokenKind::Identifier(n) if n == "await" => {
                    return Err(self
                        .error("`await` is not allowed as an identifier in async context".into()));
                }
                _ => {}
            }
        }
        // `yield` is a reserved word inside generator functions.
        if self.is_generator_context {
            match &tok.kind {
                TokenKind::Keyword(Keyword::Yield) => {
                    return Err(self.error(
                        "`yield` is not allowed as an identifier in generator context".into(),
                    ));
                }
                TokenKind::Identifier(n) if n == "yield" => {
                    return Err(self.error(
                        "`yield` is not allowed as an identifier in generator context".into(),
                    ));
                }
                _ => {}
            }
        }
        // In non-strict non-generator contexts, `yield` is a contextual keyword usable
        // as an identifier.  In non-strict non-async contexts, `await` is similarly
        // contextual.  The early-error guards above already returned `Err` for the
        // cases where these keywords ARE reserved.
        match &self.peek().kind {
            TokenKind::Keyword(Keyword::Yield) if !self.is_generator_context && !self.is_strict => {
                self.advance();
                return Ok("yield".into());
            }
            TokenKind::Keyword(Keyword::Await) if !self.is_async_context => {
                self.advance();
                return Ok("await".into());
            }
            _ => {}
        }
        if let TokenKind::Identifier(name) = &self.peek().kind {
            if is_reserved_identifier_name(name) {
                return Err(self.error(format!("reserved word `{name}` cannot be an identifier")));
            }
            // In strict mode, `arguments`, `eval`, and future reserved words
            // cannot be used as binding identifiers.
            if self.is_strict
                && (matches!(name.as_str(), "arguments" | "eval")
                    || is_strict_future_reserved(name))
            {
                return Err(self.error(format!(
                    "`{name}` cannot be used as a binding identifier in strict mode"
                )));
            }
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

    /// Consumes a statement terminator, including the common automatic
    /// semicolon insertion boundaries at a line terminator and before `}`.
    fn expect_semicolon(&mut self) -> Result<(), ParseError> {
        match &self.peek().kind {
            TokenKind::Punctuator(';') => {
                self.advance();
                Ok(())
            }
            TokenKind::Eof | TokenKind::Punctuator('}') => Ok(()),
            _ if self.peek().line_terminator_before => Ok(()),
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
        TokenKind::BigInt(_) => "bigint".into(),
        TokenKind::String(_) | TokenKind::TemplateLiteral(_) => "string".into(),
        TokenKind::Keyword(keyword) => format!("keyword `{keyword:?}`"),
        TokenKind::Punctuator(ch) => format!("`{ch}`"),
        TokenKind::Operator(op) => format!("`{op}`"),
        TokenKind::PrivateName(name) => format!("`#{name}`"),
        TokenKind::TemplateHead(_) => "template literal head".into(),
        TokenKind::TemplateMiddle(_) => "template literal middle".into(),
        TokenKind::TemplateTail(_) => "template literal tail".into(),
    }
}

/// Returns true if `name` is a future reserved word in strict mode
/// that is tokenized as an `Identifier` (not a keyword).
pub(super) fn is_strict_future_reserved(name: &str) -> bool {
    matches!(
        name,
        "implements" | "interface" | "package" | "private" | "protected" | "public"
    )
}

/// Returns true if `name` is a strict-mode reserved word that IS
/// tokenized as a keyword in our lexer (e.g. `let`, `static`, `yield`).
/// Used for checking escaped identifier sequences.
pub(super) fn is_strict_future_reserved_keyword(name: &str) -> bool {
    matches!(name, "let" | "static" | "yield")
}

/// Returns true if `name` is unconditionally reserved in all modes/contexts.
/// Used for checking escaped identifier sequences — an escaped form of a
/// reserved word (e.g. `case` = `case`) is always a SyntaxError.
/// Contextual keywords (`yield`, `await`, `async`, `let`, `static`, `of`)
/// are NOT included here; they are handled by context-specific checks.
pub(super) fn is_keyword_name(name: &str) -> bool {
    is_reserved_identifier_name(name)
}

pub(super) fn is_reserved_identifier_name(name: &str) -> bool {
    matches!(
        name,
        "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "new"
            | "null"
            | "return"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
    )
}

#[cfg(test)]
mod tests {
    use crate::lexer::{Lexer, Span, Token, TokenKind};

    use super::Parser;

    #[test]
    fn parses_empty_program() {
        let mut parser = Parser::new(vec![Token::new(TokenKind::Eof, Span::new(0, 0))]);
        assert!(parser.parse_program().unwrap().body.is_empty());
    }

    #[test]
    fn inserts_semicolon_at_line_terminator() {
        let tokens = Lexer::new("var first = 1\nvar second = 2")
            .tokenize()
            .unwrap();
        assert_eq!(Parser::new(tokens).parse_program().unwrap().body.len(), 2);
    }

    #[test]
    fn inserts_semicolon_before_closing_brace() {
        let tokens = Lexer::new("function value() { return 1 }")
            .tokenize()
            .unwrap();
        assert_eq!(Parser::new(tokens).parse_program().unwrap().body.len(), 1);
    }
}
