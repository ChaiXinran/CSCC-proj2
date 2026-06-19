//! Statement parsing helpers.

use crate::{
    ast::{FunctionBody, FunctionParam, Statement, VariableDeclarator, VariableKind},
    lexer::{Keyword, TokenKind},
    parser::{ParseError, Parser, describe},
};

impl Parser {
    /// Parses a single statement.
    pub(super) fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match &self.peek().kind {
            TokenKind::Punctuator(';') => {
                self.advance();
                Ok(Statement::Empty)
            }
            TokenKind::Punctuator('{') => self.parse_block(),
            TokenKind::Keyword(Keyword::Var) => self.parse_variable_declaration(),
            TokenKind::Keyword(Keyword::Function) => self.parse_function_declaration(),
            TokenKind::Keyword(Keyword::Return) => self.parse_return(),
            TokenKind::Keyword(Keyword::If) => self.parse_if(),
            TokenKind::Keyword(Keyword::While) => self.parse_while(),
            TokenKind::Keyword(Keyword::Break) => self.parse_break(),
            TokenKind::Keyword(Keyword::Continue) => self.parse_continue(),
            TokenKind::Keyword(Keyword::Throw) => self.parse_throw(),
            _ => self.parse_expression_statement(),
        }
    }

    /// Parses `{ statement* }`.
    pub(super) fn parse_block(&mut self) -> Result<Statement, ParseError> {
        self.expect_punctuator('{')?;
        let mut body = Vec::new();
        while !self.check_punctuator('}') && !self.at_eof() {
            body.push(self.parse_statement()?);
        }
        self.expect_punctuator('}')?;
        Ok(Statement::Block(body))
    }

    /// Parses `function name(params) { body }`.
    ///
    /// Function declarations are not allowed at statement level inside other
    /// functions in strict mode, but V3 permits them anywhere a statement is
    /// allowed.
    fn parse_function_declaration(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `function`
        let name = self.expect_identifier()?;
        let params = self.parse_param_list()?;
        let body = self.parse_function_body()?;
        Ok(Statement::FunctionDeclaration { name, params, body })
    }

    /// Parses a parameter list `(name, name, ...)`.
    pub(super) fn parse_param_list(&mut self) -> Result<Vec<FunctionParam>, ParseError> {
        self.expect_punctuator('(')?;
        let mut params = Vec::new();
        if !self.check_punctuator(')') {
            loop {
                let name = self.expect_identifier()?;
                params.push(FunctionParam { name });
                if !self.eat_punctuator(',') {
                    break;
                }
            }
        }
        self.expect_punctuator(')')?;
        Ok(params)
    }

    /// Parses `{ statement* }` as a function body, tracking function_depth so
    /// that `return` inside is accepted.
    pub(super) fn parse_function_body(&mut self) -> Result<FunctionBody, ParseError> {
        self.expect_punctuator('{')?;
        self.function_depth += 1;
        let mut statements = Vec::new();
        while !self.check_punctuator('}') && !self.at_eof() {
            statements.push(self.parse_statement()?);
        }
        self.function_depth -= 1;
        self.expect_punctuator('}')?;
        Ok(FunctionBody { statements })
    }

    /// Parses `return;` or `return expression;`.
    ///
    /// ECMAScript treats a line terminator between `return` and its expression
    /// as an implicit semicolon (restricted production). If the next token is on
    /// a new line, `return;` is produced without consuming the expression.
    fn parse_return(&mut self) -> Result<Statement, ParseError> {
        if self.function_depth == 0 {
            return Err(self.error("illegal `return` statement outside of a function".into()));
        }
        self.advance(); // `return`

        // Restricted production: a line terminator after `return` = implicit `;`
        if self.peek().line_terminator_before
            || matches!(
                self.peek().kind,
                TokenKind::Punctuator(';') | TokenKind::Eof
            )
        {
            self.eat_punctuator(';');
            return Ok(Statement::Return(None));
        }

        let value = self.parse_expression()?;
        self.expect_semicolon()?;
        Ok(Statement::Return(Some(value)))
    }

    /// Parses `var name (= expr)? (, name (= expr)?)* ;`.
    fn parse_variable_declaration(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `var`
        let mut declarations = Vec::new();
        loop {
            let name = self.expect_identifier()?;
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
        ast::{
            Expression, FunctionBody, FunctionParam, Literal, Statement, VariableDeclarator,
            VariableKind,
        },
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

    fn param(name: &str) -> FunctionParam {
        FunctionParam { name: name.into() }
    }

    fn body(statements: Vec<Statement>) -> FunctionBody {
        FunctionBody { statements }
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

    // -----------------------------------------------------------------------
    // V3 function declaration tests
    // -----------------------------------------------------------------------

    #[test]
    fn parses_function_declaration_no_params() {
        let stmts = parse("function f() { }");
        assert_eq!(
            stmts,
            [Statement::FunctionDeclaration {
                name: "f".into(),
                params: vec![],
                body: body(vec![]),
            }]
        );
    }

    #[test]
    fn parses_function_declaration_with_params_and_return() {
        let stmts = parse("function add(a, b) { return a + b; }");
        let Statement::FunctionDeclaration {
            name,
            params,
            body: fn_body,
        } = &stmts[0]
        else {
            panic!("expected FunctionDeclaration");
        };
        assert_eq!(name, "add");
        assert_eq!(params, &[param("a"), param("b")]);
        assert_eq!(fn_body.statements.len(), 1);
        assert!(matches!(fn_body.statements[0], Statement::Return(Some(_))));
    }

    #[test]
    fn parses_return_without_value() {
        let stmts = parse("function f() { return; }");
        let Statement::FunctionDeclaration { body: fn_body, .. } = &stmts[0] else {
            panic!();
        };
        assert_eq!(fn_body.statements, [Statement::Return(None)]);
    }

    #[test]
    fn parses_return_with_line_terminator_as_empty_return() {
        // `return\n1` should parse as `return;` then `1;` (restricted production)
        let stmts = parse("function f() { return\n1; }");
        let Statement::FunctionDeclaration { body: fn_body, .. } = &stmts[0] else {
            panic!();
        };
        assert_eq!(fn_body.statements.len(), 2);
        assert_eq!(fn_body.statements[0], Statement::Return(None));
    }

    #[test]
    fn rejects_return_outside_function() {
        let err = parse_error("return 1;");
        assert!(err.message.contains("return"));
    }

    #[test]
    fn rejects_missing_function_name() {
        // anonymous function at statement level is not a valid declaration
        assert!(!parse_error("function () {}").message.is_empty());
    }

    #[test]
    fn rejects_missing_function_body_brace() {
        assert!(!parse_error("function f()").message.is_empty());
    }

    #[test]
    fn parses_nested_function_declarations() {
        let stmts =
            parse("function outer(x) { function inner(y) { return x + y; } return inner(2); }");
        let Statement::FunctionDeclaration {
            body: outer_body, ..
        } = &stmts[0]
        else {
            panic!();
        };
        assert_eq!(outer_body.statements.len(), 2);
        assert!(matches!(
            outer_body.statements[0],
            Statement::FunctionDeclaration { .. }
        ));
    }
}
