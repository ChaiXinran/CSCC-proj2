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
        ArrayElement, AssignmentOperator, BinaryOperator, Expression, FunctionLiteral,
        FunctionParam, Literal, LogicalOperator, ObjectProperty, PropertyName, Statement,
        UnaryOperator, UpdateOperator,
    },
    lexer::{Keyword, TokenKind},
    parser::{ParseError, Parser, describe},
};

impl Parser {
    /// Parses a full expression, including assignment.
    pub(super) fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        self.parse_assignment()
    }

    /// Runs `parse` with the relational `in` operator re-enabled, restoring the
    /// previous `no_in` state afterwards. Used at bracketed sub-expression
    /// boundaries inside a `for` header (where `in` is otherwise suppressed).
    fn allowing_in<T>(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<T, ParseError>,
    ) -> Result<T, ParseError> {
        let saved = std::mem::replace(&mut self.no_in, false);
        let result = parse(self);
        self.no_in = saved;
        result
    }

    /// Parses `target = value` and compound assignments `+= -= *= /= %=`.
    /// Assignment is right associative and binds looser than every binary operator.
    pub(super) fn parse_assignment(&mut self) -> Result<Expression, ParseError> {
        if let Some(arrow) = self.try_parse_arrow_function()? {
            return Ok(arrow);
        }
        let left = self.parse_conditional()?;
        let operator = if self.eat_operator("=") {
            AssignmentOperator::Assign
        } else if self.eat_operator("+=") {
            AssignmentOperator::PlusAssign
        } else if self.eat_operator("-=") {
            AssignmentOperator::MinusAssign
        } else if self.eat_operator("*=") {
            AssignmentOperator::MulAssign
        } else if self.eat_operator("/=") {
            AssignmentOperator::DivAssign
        } else if self.eat_operator("%=") {
            AssignmentOperator::ModAssign
        } else {
            return Ok(left);
        };
        if !is_assignment_target(&left) {
            return Err(self.error("invalid assignment target".into()));
        }
        let value = self.parse_assignment()?;
        Ok(Expression::Assignment {
            operator,
            target: Box::new(left),
            value: Box::new(value),
        })
    }

    fn try_parse_arrow_function(&mut self) -> Result<Option<Expression>, ParseError> {
        let saved = self.cursor;
        let params = match self.peek().kind.clone() {
            TokenKind::Identifier(name)
                if matches!(
                    self.tokens.get(self.cursor + 1),
                    Some(crate::lexer::Token {
                        kind: TokenKind::Operator(operator),
                        line_terminator_before: false,
                        ..
                    }) if operator == "=>"
                ) =>
            {
                self.advance();
                vec![FunctionParam { name }]
            }
            TokenKind::Punctuator('(') => {
                let Ok(params) = self.parse_param_list() else {
                    self.cursor = saved;
                    return Ok(None);
                };
                params
            }
            _ => return Ok(None),
        };

        if self.peek().line_terminator_before || !self.eat_operator("=>") {
            self.cursor = saved;
            return Ok(None);
        }

        let body = if self.check_punctuator('{') {
            self.parse_function_body()?
        } else {
            let value = self.parse_assignment()?;
            crate::ast::FunctionBody {
                statements: vec![Statement::Return(Some(value))],
            }
        };
        Ok(Some(Expression::Function(FunctionLiteral {
            name: None,
            params,
            body,
        })))
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
                binary_precedence(operator).map(|p| (p, operator.clone()))
            }
            // `in` and `instanceof` are keyword binary operators at relational precedence.
            // `in` is suppressed inside a `for` header (`no_in`) so the header can be
            // disambiguated as a for-in statement.
            TokenKind::Keyword(Keyword::In) if !self.no_in => Some((4, "in".into())),
            TokenKind::Keyword(Keyword::InstanceOf) => Some((4, "instanceof".into())),
            _ => None,
        }
    }

    /// Parses prefix unary operators (`++`, `--`, `+`, `-`, `!`, `typeof`,
    /// `delete`).
    /// Parses prefix unary operators (`+`, `-`, `!`, `typeof`, `void`, `delete`).
    fn parse_unary(&mut self) -> Result<Expression, ParseError> {
        if let TokenKind::Operator(operator) = &self.peek().kind
            && let Some(operator) = update_operator(operator)
        {
            self.advance();
            let argument = self.parse_unary()?;
            if !is_assignment_target(&argument) {
                return Err(self.error("invalid operand for `++`/`--`".into()));
            }
            return Ok(Expression::Update {
                operator,
                prefix: true,
                argument: Box::new(argument),
            });
        }
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
        if self.check_keyword(Keyword::Void) {
            self.advance();
            let argument = self.parse_unary()?;
            return Ok(Expression::Unary {
                operator: UnaryOperator::Void,
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
        self.parse_postfix()
    }

    /// Parses a call/member expression optionally followed by a postfix
    /// `++`/`--`. A line terminator before the operator suppresses it (ASI).
    fn parse_postfix(&mut self) -> Result<Expression, ParseError> {
        let expression = self.parse_call_member()?;
        if let TokenKind::Operator(operator) = &self.peek().kind
            && !self.peek().line_terminator_before
            && let Some(operator) = update_operator(operator)
        {
            if !is_assignment_target(&expression) {
                return Err(self.error("invalid operand for `++`/`--`".into()));
            }
            self.advance();
            return Ok(Expression::Update {
                operator,
                prefix: false,
                argument: Box::new(expression),
            });
        }
        Ok(expression)
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
                let property = self.expect_identifier_name()?;
                expression = Expression::Member {
                    object: Box::new(expression),
                    property: Box::new(Expression::Identifier(property)),
                    computed: false,
                };
            } else if self.eat_punctuator('[') {
                // Computed member access: object[expression]
                let key = self.allowing_in(|parser| parser.parse_assignment())?;
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
            let property = self.expect_identifier_name()?;
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
                arguments.push(self.allowing_in(|parser| parser.parse_assignment())?);
                if !self.eat_punctuator(',') {
                    break;
                }
                if self.check_punctuator(')') {
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
                let inner = self.allowing_in(|parser| parser.parse_expression())?;
                self.expect_punctuator(')')?;
                Ok(inner)
            }
            TokenKind::Punctuator('[') => self.parse_array_literal(),
            TokenKind::Punctuator('{') => self.parse_object_literal(),
            TokenKind::Keyword(Keyword::Function) => self.parse_function_expression(),
            other => Err(self.error(format!("unexpected {}", describe(&other)))),
        }
    }

    /// Parses `[element, element, ...]`, including sparse holes.
    ///
    /// Trailing comma rule: `[1,]` → length 1; `[1,,]` → length 2 (one hole).
    fn parse_array_literal(&mut self) -> Result<Expression, ParseError> {
        self.expect_punctuator('[')?;
        let mut elements: Vec<ArrayElement> = Vec::new();
        while !self.check_punctuator(']') && !self.at_eof() {
            if self.check_punctuator(',') {
                // Elision: a leading or consecutive comma introduces a hole.
                elements.push(ArrayElement::Hole);
                self.advance(); // consume `,`
            } else {
                elements.push(ArrayElement::Expression(self.parse_assignment()?));
                // After an element, consume the separator comma if present.
                if !self.eat_punctuator(',') {
                    break;
                }
                // Trailing comma before `]` — do NOT add another hole.
                if self.check_punctuator(']') {
                    break;
                }
            }
        }
        self.expect_punctuator(']')?;
        Ok(Expression::Array(elements))
    }

    /// Parses `{ key: value, ... }` as an object literal, including V4 getter,
    /// setter, and `__proto__` forms.
    fn parse_object_literal(&mut self) -> Result<Expression, ParseError> {
        self.expect_punctuator('{')?;
        let mut properties: Vec<ObjectProperty> = Vec::new();
        let mut has_proto_setter = false;
        while !self.check_punctuator('}') && !self.at_eof() {
            let property = self.parse_object_property(&mut has_proto_setter)?;
            properties.push(property);
            if !self.eat_punctuator(',') {
                break;
            }
        }
        self.expect_punctuator('}')?;
        Ok(Expression::Object(properties))
    }

    /// Parses a single object property: data, getter, setter, or `__proto__`.
    fn parse_object_property(
        &mut self,
        has_proto_setter: &mut bool,
    ) -> Result<ObjectProperty, ParseError> {
        if self.eat_punctuator('[') {
            let key = self.parse_assignment()?;
            self.expect_punctuator(']')?;
            if self.check_punctuator('(') {
                let params = self.parse_param_list()?;
                let body = self.parse_function_body()?;
                return Ok(ObjectProperty::ComputedData {
                    key,
                    value: Expression::Function(FunctionLiteral {
                        name: None,
                        params,
                        body,
                    }),
                });
            }
            self.expect_punctuator(':')?;
            let value = self.parse_assignment()?;
            return Ok(ObjectProperty::ComputedData { key, value });
        }

        // Detect `get` / `set` context keywords.  They are only context keywords
        // here — they remain valid as regular identifier property names too.
        let is_get = matches!(&self.peek().kind, TokenKind::Identifier(s) if s == "get");
        let is_set = matches!(&self.peek().kind, TokenKind::Identifier(s) if s == "set");

        // `get key() { body }` — accessor getter (0 params)
        if is_get {
            // Peek ahead: if the next token after `get` is a property name followed
            // by `(`, this is a getter.  Otherwise fall through to a data property.
            let saved = self.cursor;
            self.advance(); // consume `get`
            if !self.check_punctuator(':')
                && !self.check_punctuator(',')
                && !self.check_punctuator('}')
                && !self.at_eof()
            {
                let key = self.parse_property_name()?;
                if self.check_punctuator('(') {
                    self.expect_punctuator('(')?;
                    self.expect_punctuator(')')?;
                    let body = self.parse_function_body()?;
                    return Ok(ObjectProperty::Getter { key, body });
                }
            }
            // Not a getter syntax — rewind and parse as data property named "get".
            self.cursor = saved;
        }

        // `set key(param) { body }` — accessor setter (1 param)
        if is_set {
            let saved = self.cursor;
            self.advance(); // consume `set`
            if !self.check_punctuator(':')
                && !self.check_punctuator(',')
                && !self.check_punctuator('}')
                && !self.at_eof()
            {
                let key = self.parse_property_name()?;
                if self.check_punctuator('(') {
                    self.expect_punctuator('(')?;
                    let param_name = self.expect_identifier()?;
                    self.expect_punctuator(')')?;
                    let body = self.parse_function_body()?;
                    return Ok(ObjectProperty::Setter {
                        key,
                        parameter: FunctionParam { name: param_name },
                        body,
                    });
                }
            }
            self.cursor = saved;
        }

        // General property: parse the key first, then decide.
        let key = self.parse_property_name()?;

        // `__proto__: value` — PrototypeSetter (only the non-computed shorthand)
        if matches!(
            &key,
            PropertyName::Identifier(name) | PropertyName::String(name)
                if name == "__proto__"
        ) && self.check_punctuator(':')
        {
            self.advance(); // consume `:`
            let value = self.parse_assignment()?;
            if *has_proto_setter {
                return Err(self.error("duplicate `__proto__` setter in object literal".into()));
            }
            *has_proto_setter = true;
            return Ok(ObjectProperty::PrototypeSetter { value });
        }

        if self.check_punctuator('(') {
            let name = key.to_key_string();
            let params = self.parse_param_list()?;
            let body = self.parse_function_body()?;
            return Ok(ObjectProperty::Data {
                key,
                value: Expression::Function(FunctionLiteral {
                    name: Some(name),
                    params,
                    body,
                }),
            });
        }

        self.expect_punctuator(':')?;
        let value = self.parse_assignment()?;
        Ok(ObjectProperty::Data { key, value })
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
            TokenKind::Keyword(keyword) => {
                self.advance();
                Ok(PropertyName::Identifier(keyword.as_str().into()))
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
        "==" | "!=" | "===" | "!==" => 3,
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
        "==" => BinaryOperator::Equal,
        "!=" => BinaryOperator::NotEqual,
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

/// Maps the `++` / `--` operator lexemes onto [`UpdateOperator`].
fn update_operator(operator: &str) -> Option<UpdateOperator> {
    match operator {
        "++" => Some(UpdateOperator::Increment),
        "--" => Some(UpdateOperator::Decrement),
        _ => None,
    }
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
        let Expression::Assignment { target, value, .. } = expression else {
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
    fn void_parses_as_unary() {
        assert_eq!(
            parse_expression("void 0"),
            Expression::Unary {
                operator: UnaryOperator::Void,
                argument: Box::new(Expression::Literal(Literal::Number(0.0))),
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
    fn parses_arrow_function_forms() {
        for source in [
            "value => value",
            "(left, right) => left + right",
            "() => {}",
        ] {
            assert!(
                matches!(parse_expression(source), Expression::Function(_)),
                "expected arrow function for {source}"
            );
        }

        let Expression::Function(FunctionLiteral { body, .. }) =
            parse_expression("value => value + 1")
        else {
            panic!("expected arrow function");
        };
        assert!(matches!(body.statements[0], Statement::Return(Some(_))));
    }

    #[test]
    fn parses_member_assignment() {
        // obj.x = 5 should parse as an assignment with a Member target
        let expr = parse_expression("obj.x = 5");
        let Expression::Assignment { target, value, .. } = expr else {
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

    // -----------------------------------------------------------------------
    // V4 expression tests
    // -----------------------------------------------------------------------

    #[test]
    fn delete_parses_as_unary() {
        let expr = parse_expression("delete obj.x");
        assert!(matches!(
            expr,
            Expression::Unary {
                operator: UnaryOperator::Delete,
                ..
            }
        ));
    }

    #[test]
    fn delete_computed_member_parses_as_unary() {
        let expr = parse_expression("delete obj[key]");
        let Expression::Unary { operator, argument } = expr else {
            panic!("expected unary delete");
        };
        assert_eq!(operator, UnaryOperator::Delete);
        assert!(matches!(
            *argument,
            Expression::Member { computed: true, .. }
        ));
    }

    #[test]
    fn in_operator_parses_as_binary() {
        let expr = parse_expression("\"x\" in obj");
        assert!(matches!(
            expr,
            Expression::Binary {
                operator: BinaryOperator::In,
                ..
            }
        ));
    }

    #[test]
    fn instanceof_operator_parses_as_binary() {
        let expr = parse_expression("p instanceof Point");
        assert!(matches!(
            expr,
            Expression::Binary {
                operator: BinaryOperator::InstanceOf,
                ..
            }
        ));
    }

    #[test]
    fn in_and_instanceof_bind_at_relational_precedence() {
        // `a + b instanceof C` should parse as `(a + b) instanceof C`
        let expr = parse_expression("a + b instanceof C");
        let Expression::Binary {
            operator: BinaryOperator::InstanceOf,
            left,
            ..
        } = expr
        else {
            panic!("expected instanceof at top level");
        };
        assert!(matches!(
            *left,
            Expression::Binary {
                operator: BinaryOperator::Add,
                ..
            }
        ));
    }

    #[test]
    fn parses_getter_in_object_literal() {
        let expr = parse_expression("({ get x() { return 7; } })");
        let Expression::Object(props) = expr else {
            panic!("expected object literal");
        };
        assert_eq!(props.len(), 1);
        assert!(matches!(
            props[0],
            ObjectProperty::Getter {
                key: PropertyName::Identifier(_),
                ..
            }
        ));
    }

    #[test]
    fn parses_setter_in_object_literal() {
        let expr = parse_expression("({ set x(v) { this.saved = v; } })");
        let Expression::Object(props) = expr else {
            panic!("expected object literal");
        };
        assert_eq!(props.len(), 1);
        assert!(matches!(
            props[0],
            ObjectProperty::Setter {
                key: PropertyName::Identifier(_),
                ..
            }
        ));
    }

    #[test]
    fn setter_parameter_is_recorded() {
        let expr = parse_expression("({ set value(v) { } })");
        let Expression::Object(props) = expr else {
            panic!("expected object literal");
        };
        let ObjectProperty::Setter { parameter, .. } = &props[0] else {
            panic!("expected setter");
        };
        assert_eq!(parameter.name, "v");
    }

    #[test]
    fn parses_proto_setter_in_object_literal() {
        let expr = parse_expression("({ __proto__: base })");
        let Expression::Object(props) = expr else {
            panic!("expected object literal");
        };
        assert_eq!(props.len(), 1);
        assert!(matches!(props[0], ObjectProperty::PrototypeSetter { .. }));
    }

    #[test]
    fn rejects_duplicate_proto_setter() {
        let tokens = Lexer::new("({ __proto__: a, __proto__: b })")
            .tokenize()
            .unwrap();
        assert!(Parser::new(tokens).parse_program().is_err());

        let tokens = Lexer::new("({ __proto__: a, '__proto__': b })")
            .tokenize()
            .unwrap();
        assert!(Parser::new(tokens).parse_program().is_err());
    }

    #[test]
    fn parses_object_method_shorthand() {
        let Expression::Object(properties) = parse_expression("({ value() { return 1; } })") else {
            panic!("expected object literal");
        };
        assert!(matches!(
            &properties[0],
            ObjectProperty::Data {
                value: Expression::Function(FunctionLiteral { params, .. }),
                ..
            } if params.is_empty()
        ));
    }

    #[test]
    fn parses_computed_object_property() {
        let Expression::Object(properties) = parse_expression("({ ['x']: 1 })") else {
            panic!("expected object literal");
        };
        assert!(matches!(
            &properties[0],
            ObjectProperty::ComputedData {
                key: Expression::Literal(Literal::String(key)),
                ..
            } if key == "x"
        ));
    }

    #[test]
    fn keyword_identifier_names_are_valid_after_dot() {
        let expr = parse_expression("object.delete");
        assert_eq!(
            expr,
            Expression::Member {
                object: Box::new(Expression::Identifier("object".into())),
                property: Box::new(Expression::Identifier("delete".into())),
                computed: false,
            }
        );

        let expr = parse_expression("object.default");
        assert!(matches!(
            expr,
            Expression::Member {
                property,
                computed: false,
                ..
            } if *property == Expression::Identifier("default".into())
        ));
    }

    #[test]
    fn builtin_call_shapes_parse_without_special_syntax() {
        for source in [
            "Object.create(base)",
            "Object.defineProperty(object, 'x', { value: 1, writable: true })",
            "Array.isArray(array)",
            "array.push(1)",
            "Function.prototype.call.call(fn, receiver, 1)",
            "new Array(3)",
        ] {
            let expression = parse_expression(source);
            assert!(
                matches!(
                    expression,
                    Expression::Call { .. } | Expression::Construct { .. }
                ),
                "expected call or construct for {source}"
            );
        }
    }

    #[test]
    fn call_arguments_allow_one_trailing_comma() {
        let expression = parse_expression("Object.keys(object,)");
        let Expression::Call { arguments, .. } = expression else {
            panic!("expected call expression");
        };
        assert_eq!(arguments.len(), 1);
    }

    #[test]
    fn keyword_property_names_keep_source_spelling() {
        let expression = parse_expression("({ instanceof: 1, typeof: 2, delete: 3 })");
        let Expression::Object(properties) = expression else {
            panic!("expected object literal");
        };
        assert!(matches!(
            &properties[0],
            ObjectProperty::Data {
                key: PropertyName::Identifier(name),
                ..
            } if name == "instanceof"
        ));
        assert!(matches!(
            &properties[1],
            ObjectProperty::Data {
                key: PropertyName::Identifier(name),
                ..
            } if name == "typeof"
        ));
        assert!(matches!(
            &properties[2],
            ObjectProperty::Data {
                key: PropertyName::Identifier(name),
                ..
            } if name == "delete"
        ));
    }

    #[test]
    fn object_literal_with_getter_and_setter() {
        let expr = parse_expression("({ get x() { return 7; }, set x(v) { } })");
        let Expression::Object(props) = expr else {
            panic!("expected object literal");
        };
        assert_eq!(props.len(), 2);
        assert!(matches!(props[0], ObjectProperty::Getter { .. }));
        assert!(matches!(props[1], ObjectProperty::Setter { .. }));
    }

    #[test]
    fn parses_sparse_array_literal() {
        let expr = parse_expression("[1, , 3]");
        let Expression::Array(elements) = expr else {
            panic!("expected array");
        };
        assert_eq!(elements.len(), 3);
        assert!(matches!(elements[0], ArrayElement::Expression(_)));
        assert!(matches!(elements[1], ArrayElement::Hole));
        assert!(matches!(elements[2], ArrayElement::Expression(_)));
    }

    #[test]
    fn sparse_array_length_semantics() {
        // `[1,]` → length 1 (trailing comma, no hole)
        let expr = parse_expression("[1,]");
        let Expression::Array(elements) = expr else {
            panic!("expected array");
        };
        assert_eq!(elements.len(), 1);

        // `[1,,]` → length 2 (one hole then trailing comma)
        let expr2 = parse_expression("[1,,]");
        let Expression::Array(elements2) = expr2 else {
            panic!("expected array");
        };
        assert_eq!(elements2.len(), 2);
        assert!(matches!(elements2[1], ArrayElement::Hole));
    }

    #[test]
    fn parses_leading_hole_in_array() {
        // `[, 1]` has a leading hole at index 0
        let expr = parse_expression("[, 1]");
        let Expression::Array(elements) = expr else {
            panic!("expected array");
        };
        assert_eq!(elements.len(), 2);
        assert!(matches!(elements[0], ArrayElement::Hole));
        assert!(matches!(elements[1], ArrayElement::Expression(_)));
    }

    #[test]
    fn get_and_set_are_valid_property_names() {
        // `get` and `set` are not reserved words — they work as data property keys.
        let expr = parse_expression("({ get: 1, set: 2 })");
        let Expression::Object(props) = expr else {
            panic!("expected object literal");
        };
        assert_eq!(props.len(), 2);
        assert!(matches!(props[0], ObjectProperty::Data { .. }));
        assert!(matches!(props[1], ObjectProperty::Data { .. }));
    }
}
