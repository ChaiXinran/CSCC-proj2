//! Expression parsing using Pratt precedence rules.
//!
//! Keeping expression parsing in its own module lets one contributor extend
//! precedence handling without editing statement parsing. The precedence
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
    ast::{
        ArrayElement, BinaryOperator, Expression, FunctionLiteral, Literal, LogicalOperator,
        ObjectProperty, PropertyName, UnaryOperator,
    },
    lexer::{Keyword, TokenKind},
    parser::{ParseError, Parser, describe},
};

impl Parser {
    /// Parses a full expression, including assignment.
    pub(super) fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        self.parse_assignment()
    }

    /// Parses `target = value`. Assignment is right associative and binds looser
    /// than the conditional operator and every binary operator.
    pub(super) fn parse_assignment(&mut self) -> Result<Expression, ParseError> {
        let left = self.parse_conditional()?;
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

    /// Parses `test ? consequent : alternate`.
    fn parse_conditional(&mut self) -> Result<Expression, ParseError> {
        let test = self.parse_binary(0)?;
        if self.eat_punctuator('?') {
            let consequent = self.parse_assignment()?;
            self.expect_punctuator(':')?;
            let alternate = self.parse_assignment()?;
            Ok(Expression::Conditional {
                test: Box::new(test),
                consequent: Box::new(consequent),
                alternate: Box::new(alternate),
            })
        } else {
            Ok(test)
        }
    }

    /// Precedence-climbing parser for binary and logical operators.
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

    fn peek_binary_operator(&self) -> Option<(u8, String)> {
        match &self.peek().kind {
            TokenKind::Operator(operator) => {
                binary_precedence(operator).map(|precedence| (precedence, operator.clone()))
            }
            TokenKind::Keyword(Keyword::In) => Some((4, "in".into())),
            TokenKind::Keyword(Keyword::InstanceOf) => Some((4, "instanceof".into())),
            _ => None,
        }
    }

    /// Parses prefix unary operators (`+`, `-`, `!`, `typeof`).
    fn parse_unary(&mut self) -> Result<Expression, ParseError> {
        if self.check_keyword(Keyword::TypeOf) {
            self.advance();
            let argument = self.parse_unary()?;
            return Ok(Expression::Unary {
                operator: UnaryOperator::TypeOf,
                argument: Box::new(argument),
            });
        }
        if self.check_keyword(Keyword::Delete) {
            self.advance();
            let argument = self.parse_unary()?;
            return Ok(Expression::Unary {
                operator: UnaryOperator::Delete,
                argument: Box::new(argument),
            });
        }
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

    /// Parses the highest-precedence postfix forms: member access and calls.
    fn parse_call_member(&mut self) -> Result<Expression, ParseError> {
        let mut expression = if self.check_keyword(Keyword::New) {
            self.parse_new()?
        } else {
            self.parse_primary()?
        };
        loop {
            if self.eat_punctuator('.') {
                let property = self.expect_identifier()?;
                expression = Expression::Member {
                    object: Box::new(expression),
                    property: Box::new(Expression::Identifier(property)),
                    computed: false,
                };
            } else if self.eat_punctuator('[') {
                // Computed member access: object[expression]
                let key = self.parse_assignment()?;
                self.expect_punctuator(']')?;
                expression = Expression::Member {
                    object: Box::new(expression),
                    property: Box::new(key),
                    computed: true,
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

    /// Parses `new callee` and `new callee(args)`.
    fn parse_new(&mut self) -> Result<Expression, ParseError> {
        self.advance(); // `new`
        let mut callee = self.parse_primary()?;
        while self.eat_punctuator('.') {
            let property = self.expect_identifier()?;
            callee = Expression::Member {
                object: Box::new(callee),
                property: Box::new(Expression::Identifier(property)),
                computed: false,
            };
        }
        let arguments = if self.check_punctuator('(') {
            self.parse_arguments()?
        } else {
            Vec::new()
        };
        Ok(Expression::Construct {
            callee: Box::new(callee),
            arguments,
        })
    }

    /// Parses a parenthesized, comma-separated argument list.
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

    /// Parses literals, identifiers, parenthesized groups, array literals,
    /// object literals, and function expressions.
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
                let inner = self.parse_expression()?;
                self.expect_punctuator(')')?;
                Ok(inner)
            }
            TokenKind::Punctuator('[') => self.parse_array_literal(),
            TokenKind::Punctuator('{') => self.parse_object_literal(),
            TokenKind::Keyword(Keyword::Function) => self.parse_function_expression(),
            other => Err(self.error(format!("unexpected {}", describe(&other)))),
        }
    }

    /// Parses `[element, element, ...]`.
    fn parse_array_literal(&mut self) -> Result<Expression, ParseError> {
        self.expect_punctuator('[')?;
        let mut elements = Vec::new();
        while !self.check_punctuator(']') {
            if self.eat_punctuator(',') {
                elements.push(ArrayElement::Hole);
                continue;
            }
            elements.push(ArrayElement::Expression(self.parse_assignment()?));
            if !self.eat_punctuator(',') {
                break;
            }
        }
        self.expect_punctuator(']')?;
        Ok(Expression::Array(elements))
    }

    /// Parses `{ key: value, ... }` as an object literal.
    ///
    /// Note: `{` at the start of a statement is a block; object literals must
    /// appear in an expression context where `{` cannot start a block.
    fn parse_object_literal(&mut self) -> Result<Expression, ParseError> {
        self.expect_punctuator('{')?;
        let mut properties = Vec::new();
        let mut has_prototype_setter = false;
        while !self.check_punctuator('}') && !self.at_eof() {
            if self.is_accessor_start("get") {
                self.advance();
                let key = self.parse_property_name()?;
                let params = self.parse_param_list()?;
                if !params.is_empty() {
                    return Err(self.error("getter must not have parameters".into()));
                }
                let body = self.parse_function_body()?;
                properties.push(ObjectProperty::Getter { key, body });
                if !self.eat_punctuator(',') {
                    break;
                }
                continue;
            }
            if self.is_accessor_start("set") {
                self.advance();
                let key = self.parse_property_name()?;
                let mut params = self.parse_param_list()?;
                if params.len() != 1 {
                    return Err(self.error("setter must have exactly one parameter".into()));
                }
                let parameter = params.remove(0);
                let body = self.parse_function_body()?;
                properties.push(ObjectProperty::Setter {
                    key,
                    parameter,
                    body,
                });
                if !self.eat_punctuator(',') {
                    break;
                }
                continue;
            }

            let key = self.parse_property_name()?;
            self.expect_punctuator(':')?;
            let value = self.parse_assignment()?;
            if matches!(&key, PropertyName::Identifier(name) if name == "__proto__") {
                if has_prototype_setter {
                    return Err(self.error("duplicate __proto__ setter".into()));
                }
                has_prototype_setter = true;
                properties.push(ObjectProperty::PrototypeSetter { value });
            } else {
                properties.push(ObjectProperty::Data { key, value });
            }
            if !self.eat_punctuator(',') {
                break;
            }
        }
        self.expect_punctuator('}')?;
        Ok(Expression::Object(properties))
    }

    fn is_accessor_start(&self, name: &str) -> bool {
        matches!(&self.peek().kind, TokenKind::Identifier(value) if value == name)
            && !matches!(self.peek_n(1).kind, TokenKind::Punctuator(':'))
            && matches!(self.peek_n(2).kind, TokenKind::Punctuator('('))
    }

    /// Parses a property key: identifier, string literal, or number literal.
    fn parse_property_name(&mut self) -> Result<PropertyName, ParseError> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Identifier(name) => {
                self.advance();
                Ok(PropertyName::Identifier(name))
            }
            TokenKind::String(s) => {
                self.advance();
                Ok(PropertyName::String(s))
            }
            TokenKind::Number(n) => {
                self.advance();
                Ok(PropertyName::Number(n))
            }
            // Keywords are also valid as property names (e.g. `{ if: 1 }`)
            TokenKind::Keyword(_) => {
                if let TokenKind::Keyword(kw) = self.advance().kind {
                    Ok(PropertyName::Identifier(format!("{kw:?}").to_lowercase()))
                } else {
                    unreachable!()
                }
            }
            other => Err(self.error(format!("expected property name, got {}", describe(&other)))),
        }
    }

    /// Parses `function(params) { body }` or `function name(params) { body }`
    /// as an expression.
    fn parse_function_expression(&mut self) -> Result<Expression, ParseError> {
        self.advance(); // `function`
        // Optional name for named function expressions
        let name = if matches!(self.peek().kind, TokenKind::Identifier(_)) {
            if let TokenKind::Identifier(name) = self.advance().kind {
                Some(name)
            } else {
                unreachable!()
            }
        } else {
            None
        };
        let params = self.parse_param_list()?;
        let body = self.parse_function_body()?;
        Ok(Expression::Function(FunctionLiteral { name, params, body }))
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
        "in" | "instanceof" => 4,
        "+" | "-" => 5,
        "*" | "/" | "%" => 6,
        _ => return None,
    })
}

/// Builds the AST node for an infix operator.
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
        "in" => BinaryOperator::In,
        "instanceof" => BinaryOperator::InstanceOf,
        other => unreachable!("`{other}` is not a binary operator"),
    }
}

/// Valid assignment targets in V3: identifiers and member expressions.
fn is_assignment_target(expression: &Expression) -> bool {
    matches!(
        expression,
        Expression::Identifier(_) | Expression::Member { .. }
    )
}

#[cfg(test)]
mod tests {
    use crate::{
        ast::{
            ArrayElement, BinaryOperator, Expression, FunctionLiteral, FunctionParam, Literal,
            LogicalOperator, ObjectProperty, PropertyName, Statement, UnaryOperator,
        },
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
            Statement::Expression(expression) => expression,
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

    fn param(name: &str) -> FunctionParam {
        FunctionParam { name: name.into() }
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

    #[test]
    fn conditional_is_right_associative() {
        let expression = parse_expression("a ? b : c ? d : e");
        let Expression::Conditional {
            test, alternate, ..
        } = expression
        else {
            panic!("expected a conditional");
        };
        assert_eq!(*test, Expression::Identifier("a".into()));
        assert!(matches!(*alternate, Expression::Conditional { .. }));
    }

    #[test]
    fn conditional_binds_looser_than_binary() {
        let expression = parse_expression("1 < 2 ? 3 : 4");
        let Expression::Conditional { test, .. } = expression else {
            panic!("expected a conditional");
        };
        assert!(matches!(
            *test,
            Expression::Binary {
                operator: BinaryOperator::LessThan,
                ..
            }
        ));
    }

    #[test]
    fn typeof_parses_as_unary() {
        assert_eq!(
            parse_expression("typeof x"),
            Expression::Unary {
                operator: UnaryOperator::TypeOf,
                argument: Box::new(Expression::Identifier("x".into())),
            }
        );
    }

    #[test]
    fn new_with_arguments_builds_construct() {
        let expression = parse_expression("new Test262Error(\"boom\")");
        let Expression::Construct { callee, arguments } = expression else {
            panic!("expected a construct expression");
        };
        assert_eq!(*callee, Expression::Identifier("Test262Error".into()));
        assert_eq!(
            arguments,
            [Expression::Literal(Literal::String("boom".into()))]
        );
    }

    #[test]
    fn new_without_arguments_has_empty_argument_list() {
        let expression = parse_expression("new Widget");
        let Expression::Construct { callee, arguments } = expression else {
            panic!("expected a construct expression");
        };
        assert_eq!(*callee, Expression::Identifier("Widget".into()));
        assert!(arguments.is_empty());
    }

    // -----------------------------------------------------------------------
    // V3 expression tests
    // -----------------------------------------------------------------------

    #[test]
    fn parses_empty_array_literal() {
        assert_eq!(parse_expression("[]"), Expression::Array(vec![]));
    }

    #[test]
    fn parses_array_literal_with_elements() {
        assert_eq!(
            parse_expression("[1, 2, 3]"),
            Expression::Array(vec![
                ArrayElement::Expression(number(1.0)),
                ArrayElement::Expression(number(2.0)),
                ArrayElement::Expression(number(3.0)),
            ])
        );
    }

    #[test]
    fn parses_empty_object_literal() {
        assert_eq!(parse_expression("({})"), Expression::Object(vec![]));
    }

    #[test]
    fn parses_object_literal_with_identifier_keys() {
        let expr = parse_expression("({ a: 1, b: 2 })");
        assert_eq!(
            expr,
            Expression::Object(vec![
                ObjectProperty::Data {
                    key: PropertyName::Identifier("a".into()),
                    value: number(1.0),
                },
                ObjectProperty::Data {
                    key: PropertyName::Identifier("b".into()),
                    value: number(2.0),
                },
            ])
        );
    }

    #[test]
    fn parses_object_literal_with_string_and_number_keys() {
        let expr = parse_expression("({ \"x\": 1, 0: 2 })");
        let Expression::Object(props) = expr else {
            panic!("expected object");
        };
        assert!(matches!(
            props[0],
            ObjectProperty::Data {
                key: PropertyName::String(_),
                ..
            }
        ));
        assert!(matches!(
            props[1],
            ObjectProperty::Data {
                key: PropertyName::Number(_),
                ..
            }
        ));
    }

    #[test]
    fn parses_computed_member_access() {
        let expr = parse_expression("arr[0]");
        assert_eq!(
            expr,
            Expression::Member {
                object: Box::new(Expression::Identifier("arr".into())),
                property: Box::new(number(0.0)),
                computed: true,
            }
        );
    }

    #[test]
    fn computed_member_access_chained_with_calls() {
        // arr[0].length
        let expr = parse_expression("arr[0].length");
        assert!(matches!(
            expr,
            Expression::Member {
                computed: false,
                ..
            }
        ));
    }

    #[test]
    fn parses_anonymous_function_expression() {
        let expr = parse_expression("(function() { })");
        let Expression::Function(FunctionLiteral { name, params, body }) = expr else {
            panic!("expected function expression");
        };
        assert!(name.is_none());
        assert!(params.is_empty());
        assert!(body.statements.is_empty());
    }

    #[test]
    fn parses_named_function_expression() {
        let expr = parse_expression("(function add(a, b) { return a + b; })");
        let Expression::Function(FunctionLiteral { name, params, .. }) = expr else {
            panic!("expected function expression");
        };
        assert_eq!(name, Some("add".into()));
        assert_eq!(params, [param("a"), param("b")]);
    }

    #[test]
    fn parses_member_assignment() {
        // obj.x = 5 should parse as an assignment with a Member target
        let expr = parse_expression("obj.x = 5");
        let Expression::Assignment { target, value } = expr else {
            panic!("expected assignment");
        };
        assert!(matches!(
            *target,
            Expression::Member {
                computed: false,
                ..
            }
        ));
        assert_eq!(*value, number(5.0));
    }

    #[test]
    fn parses_computed_member_assignment() {
        let expr = parse_expression("arr[0] = 99");
        let Expression::Assignment { target, .. } = expr else {
            panic!("expected assignment");
        };
        assert!(matches!(*target, Expression::Member { computed: true, .. }));
    }

    #[test]
    fn function_expression_with_this_access() {
        // Parse to make sure `this` keyword is treated as an identifier for now
        // (full `this` support is a V3 runtime concern)
        let expr = parse_expression("(function() { return this; })");
        assert!(matches!(expr, Expression::Function(_)));
    }
}
