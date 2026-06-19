//! Statement parsing helpers.
//!
//! Statement productions stay separate from Pratt expression parsing so both
//! areas can be developed and reviewed independently.

use crate::{
    ast::{Statement, VariableDeclarator, VariableKind},
    lexer::{Keyword, TokenKind},
    parser::{ParseError, Parser, describe},
};

impl Parser {
    /// Parses a single statement.
    ///
    /// The V2 grammar adds blocks, `if`/`else`, `while`, `break`, `continue`,
    /// and `throw` on top of the V1 expression and `var` statements.
    pub(super) fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match &self.peek().kind {
            TokenKind::Punctuator(';') => {
                self.advance();
                Ok(Statement::Empty)
            }
            TokenKind::Punctuator('{') => self.parse_block(),
            TokenKind::Keyword(Keyword::Var) => self.parse_variable_declaration(),
            TokenKind::Keyword(Keyword::If) => self.parse_if(),
            TokenKind::Keyword(Keyword::While) => self.parse_while(),
            TokenKind::Keyword(Keyword::Break) => self.parse_break(),
            TokenKind::Keyword(Keyword::Continue) => self.parse_continue(),
            TokenKind::Keyword(Keyword::Throw) => self.parse_throw(),
            _ => self.parse_expression_statement(),
        }
    }

    /// Parses `{ statement* }`.
    fn parse_block(&mut self) -> Result<Statement, ParseError> {
        self.expect_punctuator('{')?;
        let mut body = Vec::new();
        while !self.check_punctuator('}') && !self.at_eof() {
            body.push(self.parse_statement()?);
        }
        self.expect_punctuator('}')?;
        Ok(Statement::Block(body))
    }

    /// Parses `var name (= expr)? (, name (= expr)?)* ;`.
    ///
    /// The result always carries at least one declarator.
    fn parse_variable_declaration(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `var`
        let mut declarations = Vec::new();
        loop {
            let name = self.expect_identifier()?;
            // Initializers parse at assignment level so a top-level comma ends
            // the current declarator instead of being read as an operator.
            let initializer = if self.eat_operator("=") {
                Some(self.parse_assignment()?)
            } else {
                None
            };
            declarations.push(VariableDeclarator { name, initializer });
            if !self.eat_punctuator(',') {
                break;
            }
        }
        self.expect_semicolon()?;
        Ok(Statement::VariableDeclaration {
            kind: VariableKind::Var,
            declarations,
        })
    }

    /// Parses `if (test) consequent` with an optional `else`.
    ///
    /// `else` is consumed eagerly right after the consequent, so it always binds
    /// to the nearest unmatched `if`.
    fn parse_if(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `if`
        self.expect_punctuator('(')?;
        let test = self.parse_expression()?;
        self.expect_punctuator(')')?;
        let consequent = Box::new(self.parse_statement()?);
        let alternate = if self.eat_keyword(Keyword::Else) {
            Some(Box::new(self.parse_statement()?))
        } else {
            None
        };
        Ok(Statement::If {
            test,
            consequent,
            alternate,
        })
    }

    /// Parses `while (test) body`, tracking loop depth so the body may contain
    /// `break`/`continue`.
    fn parse_while(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `while`
        self.expect_punctuator('(')?;
        let test = self.parse_expression()?;
        self.expect_punctuator(')')?;

        self.loop_depth += 1;
        let body = self.parse_statement();
        self.loop_depth -= 1;

        Ok(Statement::While {
            test,
            body: Box::new(body?),
        })
    }

    /// Parses `break;`, rejecting it outside any loop.
    fn parse_break(&mut self) -> Result<Statement, ParseError> {
        if self.loop_depth == 0 {
            return Err(self.error("illegal `break` statement outside of a loop".into()));
        }
        self.advance(); // `break`
        self.expect_semicolon()?;
        Ok(Statement::Break)
    }

    /// Parses `continue;`, rejecting it outside any loop.
    fn parse_continue(&mut self) -> Result<Statement, ParseError> {
        if self.loop_depth == 0 {
            return Err(self.error("illegal `continue` statement outside of a loop".into()));
        }
        self.advance(); // `continue`
        self.expect_semicolon()?;
        Ok(Statement::Continue)
    }

    /// Parses `throw expression;`.
    ///
    /// ECMAScript forbids a line terminator between `throw` and its expression,
    /// so a newline (recorded by the lexer) is a syntax error rather than an
    /// implicit empty operand.
    fn parse_throw(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `throw`
        if self.peek().line_terminator_before {
            return Err(self.error("illegal newline after `throw`".into()));
        }
        if matches!(
            self.peek().kind,
            TokenKind::Punctuator(';') | TokenKind::Eof
        ) {
            return Err(self.error("`throw` must be followed by an expression".into()));
        }
        let argument = self.parse_expression()?;
        self.expect_semicolon()?;
        Ok(Statement::Throw(argument))
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
        ast::{Expression, Literal, Statement, VariableDeclarator, VariableKind},
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

    fn parse_error(source: &str) -> crate::parser::ParseError {
        let tokens = Lexer::new(source).tokenize().expect("lexing succeeds");
        Parser::new(tokens)
            .parse_program()
            .expect_err("parsing fails")
    }

    fn declarator(name: &str, initializer: Option<Expression>) -> VariableDeclarator {
        VariableDeclarator {
            name: name.into(),
            initializer,
        }
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
                declarations: vec![declarator("x", None)],
            }]
        );
    }

    #[test]
    fn parses_var_with_initializer() {
        assert_eq!(
            parse("var x = 1;"),
            [Statement::VariableDeclaration {
                kind: VariableKind::Var,
                declarations: vec![declarator(
                    "x",
                    Some(Expression::Literal(Literal::Number(1.0))),
                )],
            }]
        );
    }

    #[test]
    fn parses_multiple_declarators() {
        assert_eq!(
            parse("var a, b = 1;"),
            [Statement::VariableDeclaration {
                kind: VariableKind::Var,
                declarations: vec![
                    declarator("a", None),
                    declarator("b", Some(Expression::Literal(Literal::Number(1.0)))),
                ],
            }]
        );
    }

    #[test]
    fn parses_block_with_statements() {
        assert_eq!(
            parse("{ ; 1; }"),
            [Statement::Block(vec![
                Statement::Empty,
                Statement::Expression(Expression::Literal(Literal::Number(1.0))),
            ])]
        );
    }

    #[test]
    fn dangling_else_binds_to_nearest_if() {
        let body = parse("if (1) if (2) 3; else 4;");
        let Statement::If {
            consequent,
            alternate,
            ..
        } = &body[0]
        else {
            panic!("expected an if statement");
        };
        // The outer `if` has no `else`; the `else` attaches to the inner `if`.
        assert!(alternate.is_none());
        assert!(matches!(
            consequent.as_ref(),
            Statement::If {
                alternate: Some(_),
                ..
            }
        ));
    }

    #[test]
    fn parses_while_with_break_and_continue() {
        let body = parse("while (1) { break; continue; }");
        let Statement::While { body, .. } = &body[0] else {
            panic!("expected a while statement");
        };
        assert_eq!(
            body.as_ref(),
            &Statement::Block(vec![Statement::Break, Statement::Continue])
        );
    }

    #[test]
    fn rejects_break_outside_loop() {
        assert!(parse_error("break;").message.contains("break"));
    }

    #[test]
    fn rejects_continue_outside_loop() {
        assert!(parse_error("continue;").message.contains("continue"));
    }

    #[test]
    fn parses_throw_statement() {
        assert_eq!(
            parse("throw 1;"),
            [Statement::Throw(Expression::Literal(Literal::Number(1.0)))]
        );
    }

    #[test]
    fn rejects_newline_between_throw_and_expression() {
        assert!(parse_error("throw\n1;").message.contains("throw"));
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
