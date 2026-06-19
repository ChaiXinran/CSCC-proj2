//! Expression parsing using Pratt precedence rules.
//!
//! Keeping expression parsing in its own module lets one contributor extend
//! precedence handling without editing statement parsing. The V1 precedence
//! ladder, from lowest to highest binding power, is:
//!
//! ```text
//! assignment  (=)           right associative, handled separately
//! ||                        level 1
//! &&                        level 2
//! === !==                   level 3
//! < <= > >=                 level 4
//! + -                       level 5
//! * / %                     level 6
//! unary + - !               prefix, right associative
//! call / member             postfix, highest
//! primary
//! ```

use crate::{
    ast::{BinaryOperator, Expression, Literal, LogicalOperator, UnaryOperator},
    lexer::{Keyword, TokenKind},
    parser::{ParseError, Parser, describe},
};

impl Parser {
    /// Parses a full expression, including assignment.
    pub(super) fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        self.parse_assignment()
    }

    /// Parses `target = value`. Assignment is right associative and binds looser
    /// than every binary operator.
    fn parse_assignment(&mut self) -> Result<Expression, ParseError> {
        let left = self.parse_binary(0)?;
        if self.eat_operator("=") {
            if !is_assignment_target(&left) {
                return Err(self.error("invalid assignment target".into()));
            }
            let value = self.parse_assignment()?;
            Ok(Expression::Assignment {
                target: Box::new(left),
                value: Box::new(value),
            })
        } else {
            Ok(left)
        }
    }

    /// Precedence-climbing parser for binary and logical operators.
    ///
    /// `min_binding_power` is the lowest precedence this call may consume.
    /// Recursing with `precedence + 1` yields left associativity.
    fn parse_binary(&mut self, min_binding_power: u8) -> Result<Expression, ParseError> {
        let mut left = self.parse_unary()?;
        while let Some((precedence, operator)) = self.peek_binary_operator() {
            if precedence < min_binding_power {
                break;
            }
            self.advance();
            let right = self.parse_binary(precedence + 1)?;
            left = combine(&operator, left, right);
        }
        Ok(left)
    }

    /// Returns the precedence and spelling of the current binary/logical
    /// operator, if any, without consuming it.
    fn peek_binary_operator(&self) -> Option<(u8, String)> {
        if let TokenKind::Operator(operator) = &self.peek().kind {
            binary_precedence(operator).map(|precedence| (precedence, operator.clone()))
        } else {
            None
        }
    }

    /// Parses prefix unary operators (`+`, `-`, `!`), which are right
    /// associative so chains such as `- - x` and `!!x` nest correctly.
    fn parse_unary(&mut self) -> Result<Expression, ParseError> {
        if let TokenKind::Operator(operator) = &self.peek().kind {
            let operator = match operator.as_str() {
                "+" => Some(UnaryOperator::Plus),
                "-" => Some(UnaryOperator::Minus),
                "!" => Some(UnaryOperator::Not),
                _ => None,
            };
            if let Some(operator) = operator {
                self.advance();
                let argument = self.parse_unary()?;
                return Ok(Expression::Unary {
                    operator,
                    argument: Box::new(argument),
                });
            }
        }
        self.parse_call_member()
    }

    /// Parses the highest-precedence postfix forms: member access `a.b` and
    /// calls `f(args)`, applied left to right.
    fn parse_call_member(&mut self) -> Result<Expression, ParseError> {
        let mut expression = self.parse_primary()?;
        loop {
            if self.eat_punctuator('.') {
                let property = self.expect_identifier()?;
                expression = Expression::Member {
                    object: Box::new(expression),
                    property: Box::new(Expression::Identifier(property)),
                    computed: false,
                };
            } else if self.check_punctuator('(') {
                let arguments = self.parse_arguments()?;
                expression = Expression::Call {
                    callee: Box::new(expression),
                    arguments,
                };
            } else {
                break;
            }
        }
        Ok(expression)
    }

    /// Parses a parenthesized, comma-separated argument list. Each argument is an
    /// assignment-level expression, so a top-level comma ends the argument.
    fn parse_arguments(&mut self) -> Result<Vec<Expression>, ParseError> {
        self.expect_punctuator('(')?;
        let mut arguments = Vec::new();
        if !self.check_punctuator(')') {
            loop {
                arguments.push(self.parse_assignment()?);
                if !self.eat_punctuator(',') {
                    break;
                }
            }
        }
        self.expect_punctuator(')')?;
        Ok(arguments)
    }

    /// Parses literals, identifiers, and parenthesized groups.
    fn parse_primary(&mut self) -> Result<Expression, ParseError> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Number(value) => {
                self.advance();
                Ok(Expression::Literal(Literal::Number(value)))
            }
            TokenKind::String(value) => {
                self.advance();
                Ok(Expression::Literal(Literal::String(value)))
            }
            TokenKind::Identifier(name) => {
                self.advance();
                Ok(Expression::Identifier(name))
            }
            TokenKind::Keyword(Keyword::True) => {
                self.advance();
                Ok(Expression::Literal(Literal::Boolean(true)))
            }
            TokenKind::Keyword(Keyword::False) => {
                self.advance();
                Ok(Expression::Literal(Literal::Boolean(false)))
            }
            TokenKind::Keyword(Keyword::Null) => {
                self.advance();
                Ok(Expression::Literal(Literal::Null))
            }
            TokenKind::Punctuator('(') => {
                self.advance();
                // A group is transparent: precedence is already captured by the
                // tree shape of the inner expression.
                let inner = self.parse_expression()?;
                self.expect_punctuator(')')?;
                Ok(inner)
            }
            other => Err(self.error(format!("unexpected {}", describe(&other)))),
        }
    }
}

/// Maps an operator spelling to its precedence, or `None` if it is not a binary
/// or logical operator.
fn binary_precedence(operator: &str) -> Option<u8> {
    Some(match operator {
        "||" => 1,
        "&&" => 2,
        "===" | "!==" => 3,
        "<" | "<=" | ">" | ">=" => 4,
        "+" | "-" => 5,
        "*" | "/" | "%" => 6,
        _ => return None,
    })
}

/// Builds the AST node for an infix operator, distinguishing short-circuiting
/// logical operators from ordinary binary operators.
fn combine(operator: &str, left: Expression, right: Expression) -> Expression {
    if let Some(logical) = logical_operator(operator) {
        Expression::Logical {
            operator: logical,
            left: Box::new(left),
            right: Box::new(right),
        }
    } else {
        Expression::Binary {
            operator: binary_operator(operator),
            left: Box::new(left),
            right: Box::new(right),
        }
    }
}

fn logical_operator(operator: &str) -> Option<LogicalOperator> {
    match operator {
        "&&" => Some(LogicalOperator::And),
        "||" => Some(LogicalOperator::Or),
        _ => None,
    }
}

fn binary_operator(operator: &str) -> BinaryOperator {
    match operator {
        "+" => BinaryOperator::Add,
        "-" => BinaryOperator::Subtract,
        "*" => BinaryOperator::Multiply,
        "/" => BinaryOperator::Divide,
        "%" => BinaryOperator::Remainder,
        "===" => BinaryOperator::StrictEqual,
        "!==" => BinaryOperator::StrictNotEqual,
        "<" => BinaryOperator::LessThan,
        "<=" => BinaryOperator::LessThanOrEqual,
        ">" => BinaryOperator::GreaterThan,
        ">=" => BinaryOperator::GreaterThanOrEqual,
        other => unreachable!("`{other}` is not a binary operator"),
    }
}

/// Only identifiers and member expressions are valid assignment targets in V1.
fn is_assignment_target(expression: &Expression) -> bool {
    matches!(
        expression,
        Expression::Identifier(_) | Expression::Member { .. }
    )
}

#[cfg(test)]
mod tests {
    use crate::{
        ast::{BinaryOperator, Expression, Literal, LogicalOperator, UnaryOperator},
        lexer::Lexer,
        parser::Parser,
    };

    fn parse_expression(source: &str) -> Expression {
        let tokens = Lexer::new(source).tokenize().expect("lexing succeeds");
        let mut program = Parser::new(tokens)
            .parse_program()
            .expect("parsing succeeds");
        assert_eq!(program.body.len(), 1, "expected a single statement");
        match program.body.remove(0) {
            crate::ast::Statement::Expression(expression) => expression,
            other => panic!("expected an expression statement, got {other:?}"),
        }
    }

    fn number(value: f64) -> Expression {
        Expression::Literal(Literal::Number(value))
    }

    fn binary(operator: BinaryOperator, left: Expression, right: Expression) -> Expression {
        Expression::Binary {
            operator,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    #[test]
    fn multiplication_binds_tighter_than_addition() {
        assert_eq!(
            parse_expression("1 + 2 * 3"),
            binary(
                BinaryOperator::Add,
                number(1.0),
                binary(BinaryOperator::Multiply, number(2.0), number(3.0)),
            )
        );
    }

    #[test]
    fn grouping_overrides_precedence() {
        assert_eq!(
            parse_expression("(1 + 2) * 3"),
            binary(
                BinaryOperator::Multiply,
                binary(BinaryOperator::Add, number(1.0), number(2.0)),
                number(3.0),
            )
        );
    }

    #[test]
    fn same_precedence_is_left_associative() {
        // (18 / 2) / 3
        assert_eq!(
            parse_expression("18 / 2 / 3"),
            binary(
                BinaryOperator::Divide,
                binary(BinaryOperator::Divide, number(18.0), number(2.0)),
                number(3.0),
            )
        );
    }

    #[test]
    fn unary_operators_nest_right_to_left() {
        assert_eq!(
            parse_expression("- -1"),
            Expression::Unary {
                operator: UnaryOperator::Minus,
                argument: Box::new(Expression::Unary {
                    operator: UnaryOperator::Minus,
                    argument: Box::new(number(1.0)),
                }),
            }
        );
    }

    #[test]
    fn logical_operators_use_dedicated_nodes() {
        assert_eq!(
            parse_expression("false && missing"),
            Expression::Logical {
                operator: LogicalOperator::And,
                left: Box::new(Expression::Literal(Literal::Boolean(false))),
                right: Box::new(Expression::Identifier("missing".into())),
            }
        );
    }

    #[test]
    fn logical_or_binds_looser_than_and() {
        // a || (b && c)
        let expression = parse_expression("a || b && c");
        let Expression::Logical {
            operator: LogicalOperator::Or,
            right,
            ..
        } = expression
        else {
            panic!("expected a top-level `||`");
        };
        assert!(matches!(
            *right,
            Expression::Logical {
                operator: LogicalOperator::And,
                ..
            }
        ));
    }

    #[test]
    fn assignment_is_right_associative() {
        let expression = parse_expression("a = b = 1");
        let Expression::Assignment { target, value } = expression else {
            panic!("expected assignment");
        };
        assert_eq!(*target, Expression::Identifier("a".into()));
        assert!(matches!(*value, Expression::Assignment { .. }));
    }

    #[test]
    fn parses_member_access_and_calls() {
        // assert.sameValue(x, 324)
        let expression = parse_expression("assert.sameValue(x, 324)");
        let Expression::Call { callee, arguments } = expression else {
            panic!("expected a call expression");
        };
        assert_eq!(arguments.len(), 2);
        assert_eq!(
            *callee,
            Expression::Member {
                object: Box::new(Expression::Identifier("assert".into())),
                property: Box::new(Expression::Identifier("sameValue".into())),
                computed: false,
            }
        );
    }

    #[test]
    fn rejects_invalid_assignment_target() {
        let tokens = Lexer::new("1 = 2").tokenize().unwrap();
        assert!(Parser::new(tokens).parse_program().is_err());
    }

    #[test]
    fn reports_missing_operand() {
        let tokens = Lexer::new("1 +").tokenize().unwrap();
        assert!(Parser::new(tokens).parse_program().is_err());
    }
}
