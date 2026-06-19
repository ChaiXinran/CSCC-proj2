//! Statement parsing helpers.
//!
//! Statement productions stay separate from Pratt expression parsing so both
//! areas can be developed and reviewed independently.

use crate::{
    ast::{Statement, VariableKind},
    lexer::{Keyword, TokenKind},
    parser::{ParseError, Parser, describe},
};

impl Parser {
    /// Parses a single statement from the V1 grammar.
    pub(super) fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match &self.peek().kind {
            TokenKind::Punctuator(';') => {
                self.advance();
                Ok(Statement::Empty)
            }
            TokenKind::Keyword(Keyword::Var) => self.parse_variable_declaration(),
            _ => self.parse_expression_statement(),
        }
    }

    /// Parses `var name;` and `var name = expression;`.
    fn parse_variable_declaration(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `var`
        let name = self.expect_identifier()?;
        let initializer = if self.eat_operator("=") {
            Some(self.parse_expression()?)
        } else {
            None
        };
        self.expect_semicolon()?;
        Ok(Statement::VariableDeclaration {
            kind: VariableKind::Var,
            name,
            initializer,
        })
    }

    fn parse_expression_statement(&mut self) -> Result<Statement, ParseError> {
        // Reject obvious non-starters up front so the error points at the token
        // rather than failing deeper inside expression parsing.
        if self.at_eof() {
            return Err(self.error(format!(
                "expected a statement but found {}",
                describe(&self.peek().kind)
            )));
        }
        let expression = self.parse_expression()?;
        self.expect_semicolon()?;
        Ok(Statement::Expression(expression))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ast::{Expression, Literal, Statement, VariableKind},
        lexer::Lexer,
        parser::Parser,
    };

    fn parse(source: &str) -> Vec<Statement> {
        let tokens = Lexer::new(source).tokenize().expect("lexing succeeds");
        Parser::new(tokens)
            .parse_program()
            .expect("parsing succeeds")
            .body
    }

    #[test]
    fn parses_empty_statement() {
        assert_eq!(parse(";"), [Statement::Empty]);
    }

    #[test]
    fn parses_var_without_initializer() {
        assert_eq!(
            parse("var x;"),
            [Statement::VariableDeclaration {
                kind: VariableKind::Var,
                name: "x".into(),
                initializer: None,
            }]
        );
    }

    #[test]
    fn parses_var_with_initializer() {
        assert_eq!(
            parse("var x = 1;"),
            [Statement::VariableDeclaration {
                kind: VariableKind::Var,
                name: "x".into(),
                initializer: Some(Expression::Literal(Literal::Number(1.0))),
            }]
        );
    }

    #[test]
    fn allows_trailing_statement_without_semicolon() {
        assert_eq!(
            parse("1"),
            [Statement::Expression(Expression::Literal(Literal::Number(
                1.0
            )))]
        );
    }

    #[test]
    fn requires_separator_between_statements() {
        let tokens = Lexer::new("1 2").tokenize().unwrap();
        assert!(Parser::new(tokens).parse_program().is_err());
    }
}
