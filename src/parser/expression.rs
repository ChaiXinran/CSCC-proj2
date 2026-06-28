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
        ArrayElement, AssignmentOperator, BinaryOperator, CallArgument, ClassElement,
        ClassExpression, Expression, FunctionLiteral, FunctionParam, Literal, LogicalOperator,
        ObjectProperty, OptionalChainStep, PropertyName, Statement, TemplateLiteral, UnaryOperator,
        UpdateOperator,
    },
    lexer::{Keyword, TokenKind},
    parser::{
        ParseError, Parser, describe, is_keyword_name, is_reserved_identifier_name,
        is_strict_future_reserved, is_strict_future_reserved_keyword,
    },
};

impl Parser {
    /// Parses a full expression, including the comma/sequence operator.
    pub(super) fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        let first = self.parse_assignment()?;
        if !self.check_punctuator(',') {
            return Ok(first);
        }
        let mut exprs = vec![first];
        while self.eat_punctuator(',') {
            exprs.push(self.parse_assignment()?);
        }
        Ok(Expression::Sequence(exprs))
    }

    /// Runs `parse` with the relational `in` operator re-enabled, restoring the
    /// previous `no_in` state afterwards. Used at bracketed sub-expression
    /// boundaries inside a `for` header (where `in` is otherwise suppressed).
    pub(super) fn allowing_in<T>(
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
        // V9-A: `yield [*] [expr]` — only valid inside a generator function.
        if self.is_generator_context && self.check_keyword(Keyword::Yield) {
            self.advance();
            let delegate = self.eat_operator("*");
            // `yield` can be followed by a line terminator, in which case the
            // argument is absent.
            let argument = if !self.peek().line_terminator_before
                && !matches!(
                    self.peek().kind,
                    TokenKind::Punctuator(';')
                        | TokenKind::Punctuator(',')
                        | TokenKind::Punctuator(')')
                        | TokenKind::Punctuator(']')
                        | TokenKind::Punctuator('}')
                )
                && !self.at_eof()
            {
                Some(Box::new(self.parse_assignment()?))
            } else {
                None
            };
            return Ok(Expression::Yield { argument, delegate });
        }

        // V9-A: `await expr` — only valid inside an async function.
        if self.is_async_context && self.check_keyword(Keyword::Await) {
            self.advance();
            let value = self.parse_unary()?;
            return Ok(Expression::Await(Box::new(value)));
        }

        if let Some(arrow) = self.try_parse_arrow_function()? {
            return Ok(arrow);
        }
        let left = self.parse_conditional()?;
        if self.eat_operator("=") {
            // Array/Object literals can be reinterpreted as destructuring assignment patterns.
            if !is_assignment_target(&left) && !is_destructuring_target(&left) {
                return Err(self.error("invalid assignment target".into()));
            }
            self.check_strict_assignment_target(&left)?;
            validate_destructuring_assignment_target(&left).map_err(|msg| self.error(msg))?;
            let value = self.parse_assignment()?;
            Ok(Expression::Assignment {
                target: Box::new(left),
                value: Box::new(value),
            })
        } else if let TokenKind::Operator(operator_text) = self.peek().kind.clone()
            && let Some(operator) = compound_assignment_operator(&operator_text)
        {
            if !is_assignment_target(&left) {
                return Err(self.error("invalid assignment target".into()));
            }
            self.check_strict_assignment_target(&left)?;
            self.advance();
            let value = self.parse_assignment()?;
            Ok(Expression::CompoundAssignment {
                operator,
                target: Box::new(left),
                value: Box::new(value),
            })
        } else {
            Ok(left)
        }
    }

    /// In strict mode, `eval` and `arguments` cannot be assignment targets.
    pub(super) fn check_strict_assignment_target(
        &self,
        expr: &Expression,
    ) -> Result<(), ParseError> {
        if !self.is_strict {
            return Ok(());
        }
        match expr {
            Expression::Identifier(name) if name == "eval" || name == "arguments" => {
                Err(self.error(format!("cannot assign to '{name}' in strict mode")))
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        ArrayElement::Expression(target) | ArrayElement::Spread(target) => {
                            self.check_strict_assignment_target(target)?;
                        }
                        ArrayElement::Hole => {}
                    }
                }
                Ok(())
            }
            Expression::Object(properties) => {
                for property in properties {
                    match property {
                        ObjectProperty::Data { value, .. }
                        | ObjectProperty::ComputedData { value, .. }
                        | ObjectProperty::PrototypeSetter { value }
                        | ObjectProperty::Spread(value) => {
                            self.check_strict_assignment_target(value)?;
                        }
                        ObjectProperty::Getter { .. } | ObjectProperty::Setter { .. } => {}
                    }
                }
                Ok(())
            }
            Expression::Assignment { target, .. } => self.check_strict_assignment_target(target),
            _ => Ok(()),
        }
    }

    fn try_parse_arrow_function(&mut self) -> Result<Option<Expression>, ParseError> {
        let saved = self.cursor;
        // Helper closure to detect `next-token-is =>` without line terminator.
        let next_is_arrow = |tokens: &[crate::lexer::Token], cursor: usize| {
            matches!(
                tokens.get(cursor + 1),
                Some(crate::lexer::Token {
                    kind: TokenKind::Operator(op),
                    line_terminator_before: false,
                    ..
                }) if op == "=>"
            )
        };
        let params = match self.peek().kind.clone() {
            TokenKind::Identifier(name) if next_is_arrow(&self.tokens, self.cursor) => {
                // In strict mode, binding identifiers like `eval` and `arguments`
                // are forbidden as arrow function parameter names.
                if self.is_strict
                    && (matches!(name.as_str(), "eval" | "arguments")
                        || is_strict_future_reserved(&name))
                {
                    return Err(self.error(format!(
                        "`{name}` cannot be used as a parameter name in strict mode"
                    )));
                }
                self.advance();
                vec![FunctionParam::Simple(name)]
            }
            // `yield` as a single arrow param is valid in non-strict, non-generator context.
            TokenKind::Keyword(Keyword::Yield)
                if !self.is_strict
                    && !self.is_generator_context
                    && next_is_arrow(&self.tokens, self.cursor) =>
            {
                self.advance();
                vec![FunctionParam::Simple("yield".into())]
            }
            // `await` as a single arrow param is valid in non-async context.
            TokenKind::Keyword(Keyword::Await)
                if !self.is_async_context && next_is_arrow(&self.tokens, self.cursor) =>
            {
                self.advance();
                vec![FunctionParam::Simple("await".into())]
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

        // NSPL + "use strict" is forbidden for non-async arrows too.
        let is_nspl = Self::params_are_non_simple(&params);
        let body_strict = self.check_punctuator('{') && self.peek_body_has_use_strict();
        if is_nspl && body_strict {
            return Err(self.error(
                "\"use strict\" directive is not allowed in function with non-simple parameters"
                    .into(),
            ));
        }
        // Arrow functions always require unique formal parameters (UniqueFormalParameters).
        self.check_duplicate_params(&params)?;
        if self.is_strict || body_strict {
            self.check_strict_params(&params)?;
        }

        let body = if self.check_punctuator('{') {
            self.parse_function_body()?
        } else {
            let value = self.parse_assignment()?;
            crate::ast::FunctionBody {
                statements: vec![Statement::Return(Some(value))],
                is_strict: self.is_strict,
            }
        };
        Ok(Some(Expression::Function(FunctionLiteral {
            name: None,
            params,
            body,
            is_async: false,
            is_generator: false,
            is_arrow: true,
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
            // Spec: it is a SyntaxError if the left operand of `**` is a
            // UnaryExpression with a prefix unary operator (not an update expr).
            if operator == "**" {
                if let Expression::Unary { .. } = &left {
                    return Err(self.error(
                        "unary expression cannot be used directly as left operand of `**`; wrap in parentheses".into(),
                    ));
                }
            }
            self.advance();
            // `**` is right-associative: right operand uses same precedence
            let right_min = if operator == "**" {
                precedence
            } else {
                precedence + 1
            };
            let right = self.parse_binary(right_min)?;
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
            TokenKind::Keyword(Keyword::In) if !self.no_in => {
                binary_precedence("in").map(|p| (p, "in".into()))
            }
            TokenKind::Keyword(Keyword::InstanceOf) => {
                binary_precedence("instanceof").map(|p| (p, "instanceof".into()))
            }
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
            self.enter_depth()?;
            let argument = self.parse_unary()?;
            self.leave_depth();
            if !is_assignment_target(&argument) {
                return Err(self.error("invalid operand for `++`/`--`".into()));
            }
            self.check_strict_assignment_target(&argument)?;
            return Ok(Expression::Update {
                operator,
                prefix: true,
                argument: Box::new(argument),
            });
        }
        if self.check_keyword(Keyword::TypeOf) {
            self.advance();
            self.enter_depth()?;
            let argument = self.parse_unary()?;
            self.leave_depth();
            return Ok(Expression::Unary {
                operator: UnaryOperator::TypeOf,
                argument: Box::new(argument),
            });
        }
        if self.check_keyword(Keyword::Delete) {
            self.advance();
            self.enter_depth()?;
            let argument = self.parse_unary()?;
            self.leave_depth();
            // Strict-mode early error: `delete` of an unqualified identifier.
            if self.is_strict && matches!(argument, Expression::Identifier(_)) {
                return Err(
                    self.error("cannot delete an unqualified identifier in strict mode".into())
                );
            }
            // Strict-mode early error: `delete expr.#privateName` (spec: MemberExpression.PrivateName).
            if self.is_strict && is_private_member_access(&argument) {
                return Err(self.error("cannot delete a private member access expression".into()));
            }
            return Ok(Expression::Unary {
                operator: UnaryOperator::Delete,
                argument: Box::new(argument),
            });
        }
        if self.check_keyword(Keyword::Void) {
            self.advance();
            self.enter_depth()?;
            let argument = self.parse_unary()?;
            self.leave_depth();
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
                "~" => Some(UnaryOperator::BitwiseNot),
                _ => None,
            };
            if let Some(operator) = operator {
                self.advance();
                self.enter_depth()?;
                let argument = self.parse_unary()?;
                self.leave_depth();
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
            self.check_strict_assignment_target(&expression)?;
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
                if let TokenKind::PrivateName(name) = self.peek().kind.clone() {
                    self.advance();
                    expression = Expression::Member {
                        object: Box::new(expression),
                        property: Box::new(Expression::PrivateName(name)),
                        computed: false,
                    };
                } else {
                    let property = self.expect_identifier_name()?;
                    expression = Expression::Member {
                        object: Box::new(expression),
                        property: Box::new(Expression::Identifier(property)),
                        computed: false,
                    };
                }
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
            } else if self.eat_operator("?.") {
                // Optional chaining: `base?.prop`, `base?.[key]`, `base?.(args)`.
                // Collect all subsequent chain steps (both optional and mandatory).
                let first_step = self.parse_optional_chain_first_step()?;
                let mut steps = vec![first_step];
                loop {
                    if self.eat_operator("?.") {
                        steps.push(self.parse_optional_chain_first_step()?);
                    } else if self.eat_punctuator('.') {
                        if let TokenKind::PrivateName(name) = self.peek().kind.clone() {
                            self.advance();
                            steps.push(OptionalChainStep::Member {
                                property: Box::new(Expression::PrivateName(name)),
                                computed: false,
                                optional: false,
                            });
                        } else {
                            let prop = self.expect_identifier_name()?;
                            steps.push(OptionalChainStep::Member {
                                property: Box::new(Expression::Identifier(prop)),
                                computed: false,
                                optional: false,
                            });
                        }
                    } else if self.eat_punctuator('[') {
                        let key = self.allowing_in(|p| p.parse_assignment())?;
                        self.expect_punctuator(']')?;
                        steps.push(OptionalChainStep::Member {
                            property: Box::new(key),
                            computed: true,
                            optional: false,
                        });
                    } else if self.check_punctuator('(') {
                        let arguments = self.parse_arguments()?;
                        steps.push(OptionalChainStep::Call {
                            arguments,
                            optional: false,
                        });
                    } else {
                        break;
                    }
                }
                expression = Expression::OptionalChain {
                    base: Box::new(expression),
                    steps,
                };
            } else {
                break;
            }
        }
        Ok(expression)
    }

    /// Parses the first step after `?.` has already been consumed.
    fn parse_optional_chain_first_step(&mut self) -> Result<OptionalChainStep, ParseError> {
        if self.check_punctuator('(') {
            let arguments = self.parse_arguments()?;
            Ok(OptionalChainStep::Call {
                arguments,
                optional: true,
            })
        } else if self.eat_punctuator('[') {
            let key = self.allowing_in(|p| p.parse_assignment())?;
            self.expect_punctuator(']')?;
            Ok(OptionalChainStep::Member {
                property: Box::new(key),
                computed: true,
                optional: true,
            })
        } else if let TokenKind::PrivateName(name) = self.peek().kind.clone() {
            self.advance();
            Ok(OptionalChainStep::Member {
                property: Box::new(Expression::PrivateName(name)),
                computed: false,
                optional: true,
            })
        } else {
            let prop = self.expect_identifier_name()?;
            Ok(OptionalChainStep::Member {
                property: Box::new(Expression::Identifier(prop)),
                computed: false,
                optional: true,
            })
        }
    }

    /// Parses `new callee` and `new callee(args)`.
    fn parse_new(&mut self) -> Result<Expression, ParseError> {
        self.advance(); // `new`
        // `new.target` is a MetaProperty — handle before trying to parse a callee.
        if self.eat_punctuator('.') {
            let prop = self.expect_identifier_name()?;
            if prop == "target" {
                return Ok(Expression::NewTarget);
            }
            return Err(self.error(format!("`new.{prop}` is not a valid meta-property")));
        }
        let mut callee = self.parse_primary()?;
        while self.eat_punctuator('.') {
            if let TokenKind::PrivateName(name) = self.peek().kind.clone() {
                self.advance();
                callee = Expression::Member {
                    object: Box::new(callee),
                    property: Box::new(Expression::PrivateName(name)),
                    computed: false,
                };
            } else {
                let property = self.expect_identifier_name()?;
                callee = Expression::Member {
                    object: Box::new(callee),
                    property: Box::new(Expression::Identifier(property)),
                    computed: false,
                };
            }
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
    fn parse_arguments(&mut self) -> Result<Vec<CallArgument>, ParseError> {
        self.expect_punctuator('(')?;
        let mut arguments = Vec::new();
        if !self.check_punctuator(')') {
            loop {
                if self.check_spread() {
                    self.advance(); // consume `...`
                    let expr = self.allowing_in(|p| p.parse_assignment())?;
                    arguments.push(CallArgument::Spread(expr));
                } else {
                    let expr = self.allowing_in(|p| p.parse_assignment())?;
                    arguments.push(CallArgument::Expression(expr));
                }
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
                if self.is_strict && token.has_legacy_numeric {
                    return Err(ParseError {
                        span: token.span,
                        message: "legacy octal and non-octal decimal integer literals are not allowed in strict mode".into(),
                    });
                }
                self.advance();
                Ok(Expression::Literal(Literal::Number(value)))
            }
            TokenKind::BigInt(raw) => {
                self.advance();
                Ok(Expression::Literal(Literal::BigInt(raw)))
            }
            TokenKind::String(value) => {
                // Strict-mode early error: legacy octal/non-octal decimal
                // escapes are forbidden inside strict-mode string literals.
                if self.is_strict && token.has_legacy_escape {
                    return Err(ParseError {
                        span: token.span,
                        message:
                            "octal escape sequences are not allowed in strict mode string literals"
                                .into(),
                    });
                }
                self.advance();
                Ok(Expression::Literal(Literal::String(value)))
            }
            TokenKind::Identifier(ref name) => {
                if is_reserved_identifier_name(name) {
                    return Err(
                        self.error(format!("reserved word `{name}` cannot be an identifier"))
                    );
                }
                // An escaped identifier whose StringValue is a keyword is always
                // a SyntaxError (e.g. `let` = `let`).
                if token.has_identifier_escape
                    && (is_keyword_name(name)
                        || (self.is_strict && is_strict_future_reserved_keyword(name)))
                {
                    return Err(self.error(format!(
                        "identifier escape sequence resolves to reserved word `{name}`"
                    )));
                }
                // In strict mode, future reserved words cannot be used as identifier references.
                if self.is_strict && is_strict_future_reserved(name) {
                    return Err(self.error(format!("`{name}` is a reserved word in strict mode")));
                }
                // Unicode-escaped forms of context-reserved words (e.g. `await`
                // resolving to `await`) are SyntaxErrors in the relevant context.
                if self.is_async_context && name == "await" {
                    return Err(self
                        .error("`await` is not allowed as an identifier in async context".into()));
                }
                if (self.is_generator_context || self.is_strict) && name == "yield" {
                    return Err(self
                        .error("`yield` is not allowed as an identifier in this context".into()));
                }
                // V9-A: `async` is a contextual keyword when followed (on the same line)
                // by `function`, `(`, or a simple identifier — in that case try to parse
                // an async function expression or async arrow.
                // Per spec, identifiers with Unicode escapes cannot serve as contextual
                // keywords, so `async` must NOT be treated as async.
                if name == "async" && !self.peek().has_identifier_escape {
                    let next_on_same_line = self
                        .tokens
                        .get(self.cursor + 1)
                        .is_some_and(|t| !t.line_terminator_before);
                    if next_on_same_line
                        && matches!(
                            self.tokens.get(self.cursor + 1).map(|t| &t.kind),
                            Some(TokenKind::Keyword(Keyword::Function))
                                | Some(TokenKind::Punctuator('('))
                                | Some(TokenKind::Identifier(_))
                        )
                    {
                        return self.parse_async_expression();
                    }
                }
                let name = name.clone();
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
                self.enter_depth()?;
                let inner = self.allowing_in(|parser| parser.parse_expression())?;
                self.leave_depth();
                self.expect_punctuator(')')?;
                Ok(inner)
            }
            TokenKind::Punctuator('[') => self.parse_array_literal(),
            TokenKind::Punctuator('{') => self.parse_object_literal(),
            TokenKind::Keyword(Keyword::Function) => self.parse_function_expression(),
            TokenKind::Keyword(Keyword::Import) => self.parse_dynamic_import(),
            // In a primary-expression position, `/` and `/=` introduce a regex
            // literal, not a division operator. Use the source text to re-read the
            // full `/pattern/flags` sequence, then skip the tokens that the
            // context-free lexer split it into.
            TokenKind::Operator(ref op) if op == "/" || op == "/=" => self.parse_regex_literal(),
            // V8-A: template literals
            TokenKind::TemplateLiteral(value) => {
                self.advance();
                // No-substitution template: `...text...`
                Ok(Expression::TemplateLiteral(TemplateLiteral {
                    quasis: vec![value],
                    expressions: vec![],
                }))
            }
            TokenKind::TemplateHead(_) => self.parse_template_literal(),
            // V8-A: class expressions
            TokenKind::Keyword(Keyword::Class) => self.parse_class_expression(),
            // `this` / `super`
            TokenKind::Keyword(Keyword::This) => {
                self.advance();
                Ok(Expression::This)
            }
            TokenKind::Keyword(Keyword::Super) => {
                self.advance();
                Ok(Expression::Super)
            }
            // Contextual keywords usable as identifiers when not in the relevant context.
            TokenKind::Keyword(Keyword::Yield) if !self.is_generator_context && !self.is_strict => {
                self.advance();
                Ok(Expression::Identifier("yield".into()))
            }
            TokenKind::Keyword(Keyword::Await) if !self.is_async_context => {
                self.advance();
                Ok(Expression::Identifier("await".into()))
            }
            // `let` is not a reserved word; in sloppy mode it can appear as an
            // identifier expression in contexts where it is not starting a
            // lexical declaration (e.g. after ASI disambiguation in statement.rs).
            TokenKind::Keyword(Keyword::Let) if !self.is_strict => {
                self.advance();
                Ok(Expression::Identifier("let".into()))
            }
            other => Err(self.error(format!("unexpected {}", describe(&other)))),
        }
    }

    fn parse_dynamic_import(&mut self) -> Result<Expression, ParseError> {
        self.advance(); // consume `import`
        // `import.meta` — module meta-object (§13.3.12). Only valid in module code.
        if self.eat_punctuator('.') {
            match &self.peek().kind.clone() {
                TokenKind::Identifier(name) if name == "meta" => {
                    self.advance();
                    return Ok(Expression::ImportMeta);
                }
                _ => return Err(self.error("expected `meta` after `import.`".into())),
            }
        }
        self.expect_punctuator('(')?;
        if self.check_punctuator(')') {
            return Err(self.error("dynamic import requires a specifier".into()));
        }
        let specifier = self.allowing_in(|parser| parser.parse_assignment())?;
        let options = if self.eat_punctuator(',') {
            if self.check_punctuator(')') {
                None
            } else {
                Some(Box::new(
                    self.allowing_in(|parser| parser.parse_assignment())?,
                ))
            }
        } else {
            None
        };
        self.expect_punctuator(')')?;
        Ok(Expression::DynamicImport {
            specifier: Box::new(specifier),
            options,
        })
    }

    fn parse_regex_literal(&mut self) -> Result<Expression, ParseError> {
        let start = self.peek().span.start;
        let Some(ref source) = self.source.clone() else {
            return Err(self.error("unexpected `/`".into()));
        };
        match crate::lexer::read_regex_literal_at(source, start) {
            Err(lex_err) => Err(ParseError {
                span: lex_err.span,
                message: lex_err.message,
            }),
            Ok((pattern, flags, end_offset)) => {
                // Consume the leading '/' (or '/=') token.
                self.advance();
                // Skip all subsequent tokens whose span lies inside the regex body.
                while !matches!(self.peek().kind, TokenKind::Eof)
                    && self.peek().span.start < end_offset
                {
                    self.advance();
                }
                Ok(Expression::Literal(Literal::RegExp { pattern, flags }))
            }
        }
    }

    /// Parses `[element, element, ...]`, including sparse holes and spread elements.
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
            } else if self.check_spread() {
                self.advance(); // consume `...`
                let expr = self.parse_assignment()?;
                elements.push(ArrayElement::Spread(expr));
                if !self.eat_punctuator(',') {
                    break;
                }
                if self.check_punctuator(']') {
                    break;
                }
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
            // Spread element: `...expr`
            if self.check_spread() {
                self.advance(); // consume `...`
                let expr = self.parse_assignment()?;
                properties.push(ObjectProperty::Spread(expr));
            } else {
                let property = self.parse_object_property(&mut has_proto_setter)?;
                properties.push(property);
            }
            if !self.eat_punctuator(',') {
                break;
            }
        }
        self.expect_punctuator('}')?;
        Ok(Expression::Object(properties))
    }

    /// Parses a single object property: data, getter, setter, method, or `__proto__`.
    fn parse_object_property(
        &mut self,
        has_proto_setter: &mut bool,
    ) -> Result<ObjectProperty, ParseError> {
        // Generator method: `*name() {}` or `*[expr]() {}`
        if self.eat_operator("*") {
            return self.parse_object_method(false, true);
        }

        // Async method (contextual): `async [*] name() {}` or `async [*] [expr]() {}`.
        // Detection: `async` identifier not followed by a newline, followed by a valid
        // method key token (not `:`, `,`, `}`, `(`). This avoids consuming `async` when
        // it's used as a plain property name (`{ async: 1 }`) or shorthand (`{ async }`).
        if matches!(&self.peek().kind, TokenKind::Identifier(s) if s == "async")
            && !self.peek().has_identifier_escape
        {
            let next = self.tokens.get(self.cursor + 1);
            let next_is_method_key = next.is_some_and(|t| {
                !t.line_terminator_before
                    && matches!(
                        &t.kind,
                        TokenKind::Operator(op) if op == "*"
                    )
                    || next.is_some_and(|t| {
                        !t.line_terminator_before
                            && matches!(
                                &t.kind,
                                TokenKind::Punctuator('[')
                                    | TokenKind::Identifier(_)
                                    | TokenKind::Keyword(_)
                                    | TokenKind::String(_)
                                    | TokenKind::Number(_)
                            )
                    })
            });
            if next_is_method_key {
                self.advance(); // consume `async`
                let is_generator = self.eat_operator("*");
                return self.parse_object_method(true, is_generator);
            }
        }

        // Computed key: `[expr]: value` or `[expr]() {}`
        if self.eat_punctuator('[') {
            let key = self.allowing_in(|parser| parser.parse_assignment())?;
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
                        is_async: false,
                        is_generator: false,
                        is_arrow: false,
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
        // `get` followed immediately by `(` is a method named `get`, not a getter.
        if is_get {
            let saved = self.cursor;
            self.advance(); // consume `get`
            if !self.check_punctuator(':')
                && !self.check_punctuator(',')
                && !self.check_punctuator('}')
                && !self.check_punctuator('(') // `get(` = method named `get`, not getter
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
        // `set` followed immediately by `(` is a method named `set`, not a setter.
        if is_set {
            let saved = self.cursor;
            self.advance(); // consume `set`
            if !self.check_punctuator(':')
                && !self.check_punctuator(',')
                && !self.check_punctuator('}')
                && !self.check_punctuator('(') // `set(` = method named `set`, not setter
                && !self.at_eof()
            {
                let key = self.parse_property_name()?;
                if self.check_punctuator('(') {
                    self.expect_punctuator('(')?;
                    let param_name = self.expect_identifier()?;
                    self.expect_punctuator(')')?;
                    let body = self.parse_function_body()?;
                    // Retroactive strict check: if the setter body is strict,
                    // `eval` and `arguments` are forbidden as parameter names.
                    if body.is_strict && matches!(param_name.as_str(), "eval" | "arguments") {
                        return Err(self.error(format!(
                            "`{param_name}` cannot be used as a parameter name in strict mode"
                        )));
                    }
                    return Ok(ObjectProperty::Setter {
                        key,
                        parameter: FunctionParam::Simple(param_name),
                        body,
                    });
                }
            }
            self.cursor = saved;
        }

        // General property: parse the key first, then decide. Contextual
        // keywords can be shorthand identifiers where they are not reserved.
        let key_can_be_shorthand = match &self.peek().kind {
            TokenKind::Identifier(_) => true,
            TokenKind::Keyword(Keyword::Yield) => !self.is_generator_context && !self.is_strict,
            TokenKind::Keyword(Keyword::Await) => !self.is_async_context,
            TokenKind::Keyword(Keyword::Let) => !self.is_strict,
            _ => false,
        };
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
                    is_async: false,
                    is_generator: false,
                    is_arrow: false,
                }),
            });
        }

        // Shorthand property: `{name}` or `{name = default}`.
        // Only when: token was Identifier (not keyword), and name is not a reserved word.
        if key_can_be_shorthand && !self.check_punctuator(':') {
            if let PropertyName::Identifier(ref ident) = key {
                // Truly reserved words can never appear as shorthand.
                if is_reserved_identifier_name(ident)
                    || (self.is_strict
                        && (ident == "eval"
                            || ident == "arguments"
                            || is_strict_future_reserved(ident)
                            || is_strict_future_reserved_keyword(ident)))
                {
                    return Err(self.error(format!(
                        "reserved word `{ident}` cannot be used as shorthand property"
                    )));
                }
                // Context-sensitive keywords (yield/await) are not valid shorthands
                // inside generator/async functions but are fine elsewhere.
                let is_context_keyword = (self.is_generator_context && ident == "yield")
                    || (self.is_async_context && ident == "await");
                if !is_context_keyword {
                    let ident = ident.clone();
                    let default_expr = if self.eat_operator("=") {
                        Some(self.parse_assignment()?)
                    } else {
                        None
                    };
                    let value = if let Some(def) = default_expr {
                        Expression::Assignment {
                            target: Box::new(Expression::Identifier(ident.clone())),
                            value: Box::new(def),
                        }
                    } else {
                        Expression::Identifier(ident.clone())
                    };
                    return Ok(ObjectProperty::Data {
                        key: PropertyName::Identifier(ident),
                        value,
                    });
                }
            }
        }

        self.expect_punctuator(':')?;
        let value = self.parse_assignment()?;
        Ok(ObjectProperty::Data { key, value })
    }

    /// Helper: parses a method key and body after `[*]` and optional `async` flag are
    /// already consumed. Handles both `name(){}` and `[computed](){}` forms.
    fn parse_object_method(
        &mut self,
        is_async: bool,
        is_generator: bool,
    ) -> Result<ObjectProperty, ParseError> {
        let outer_async = self.is_async_context;
        let outer_generator = self.is_generator_context;
        self.is_async_context = is_async;
        self.is_generator_context = is_generator;

        let result = (|| -> Result<ObjectProperty, ParseError> {
            if self.eat_punctuator('[') {
                let key = self.parse_assignment()?;
                self.expect_punctuator(']')?;
                let params = self.parse_param_list()?;
                let is_nspl = Self::params_are_non_simple(&params);
                let body_strict = self.peek_body_has_use_strict();
                if is_nspl && body_strict {
                    return Err(self.error(
                        "\"use strict\" directive is not allowed in function with non-simple parameters".into(),
                    ));
                }
                if is_async || is_generator || self.is_strict || body_strict || is_nspl {
                    self.check_duplicate_params(&params)?;
                }
                let body = self.parse_function_body()?;
                return Ok(ObjectProperty::ComputedData {
                    key,
                    value: Expression::Function(FunctionLiteral {
                        name: None,
                        params,
                        body,
                        is_async,
                        is_generator,
                        is_arrow: false,
                    }),
                });
            }
            let key = self.parse_property_name()?;
            let name = key.to_key_string();
            let params = self.parse_param_list()?;
            let is_nspl = Self::params_are_non_simple(&params);
            let body_strict = self.peek_body_has_use_strict();
            if is_nspl && body_strict {
                return Err(self.error(
                    "\"use strict\" directive is not allowed in function with non-simple parameters".into(),
                ));
            }
            if is_async || is_generator || self.is_strict || body_strict || is_nspl {
                self.check_duplicate_params(&params)?;
            }
            let body = self.parse_function_body()?;
            Ok(ObjectProperty::Data {
                key,
                value: Expression::Function(FunctionLiteral {
                    name: Some(name),
                    params,
                    body,
                    is_async,
                    is_generator,
                    is_arrow: false,
                }),
            })
        })();

        self.is_async_context = outer_async;
        self.is_generator_context = outer_generator;
        result
    }

    /// Parses a property key: identifier, string literal, or number literal.
    fn parse_property_name(&mut self) -> Result<PropertyName, ParseError> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Identifier(name) => {
                self.advance();
                Ok(PropertyName::Identifier(name))
            }
            TokenKind::String(s) | TokenKind::TemplateLiteral(s) => {
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
            // Computed property name: `[expr]`
            TokenKind::Punctuator('[') => {
                self.advance(); // consume `[`
                let expr = self.allowing_in(|parser| parser.parse_assignment())?;
                self.expect_punctuator(']')?;
                Ok(PropertyName::Computed(Box::new(expr)))
            }
            other => Err(self.error(format!("expected property name, got {}", describe(&other)))),
        }
    }

    /// Parses `function [*] [name] (params) { body }` as an expression.
    fn parse_function_expression(&mut self) -> Result<Expression, ParseError> {
        self.advance(); // `function`
        let is_generator = self.eat_operator("*");
        let outer_generator = self.is_generator_context;
        self.is_generator_context = is_generator;
        // Optional name for named function expressions (also accept keywords as names)
        let name = match self.peek().kind.clone() {
            TokenKind::Identifier(n) => {
                self.advance();
                Some(n)
            }
            TokenKind::Keyword(kw) if !matches!(kw, Keyword::Function) => {
                self.advance();
                Some(kw.as_str().to_owned())
            }
            _ => None,
        };
        let params = self.parse_param_list()?;
        if is_generator {
            Self::check_generator_params_no_yield(&params)?;
        }
        if is_generator || self.is_strict {
            self.check_duplicate_params(&params)?;
        }
        let is_nspl = Self::params_are_non_simple(&params);
        if is_nspl && self.peek_body_has_use_strict() {
            self.is_generator_context = outer_generator;
            return Err(self.error(
                "\"use strict\" directive is not allowed in function with non-simple parameters"
                    .into(),
            ));
        }
        let body = self.parse_function_body()?;
        self.is_generator_context = outer_generator;
        // Retroactive strict checks: if the body is strict, validate name and params.
        let effective_strict = self.is_strict || body.is_strict;
        if effective_strict {
            if let Some(ref n) = name {
                if matches!(n.as_str(), "eval" | "arguments") || is_strict_future_reserved(n) {
                    return Err(
                        self.error(format!("function name `{n}` is not allowed in strict mode"))
                    );
                }
            }
            if !is_generator && !self.is_strict {
                // Outer context wasn't strict, but body is — re-check params.
                self.check_duplicate_params(&params)?;
                self.check_strict_params(&params)?;
            }
        }
        // Check for param/lexical conflicts.
        self.validate_params_vs_lexical(&params, &body.statements)?;
        Ok(Expression::Function(FunctionLiteral {
            name,
            params,
            body,
            is_async: false,
            is_generator,
            is_arrow: false,
        }))
    }

    /// Parses `async function [*] [name] (params) { body }` or
    /// `async (params) => body` as an expression.
    pub(super) fn parse_async_expression(&mut self) -> Result<Expression, ParseError> {
        // consume `async` (which was peeked as Identifier("async"))
        self.advance();

        // `async function` → async function expression
        if self.check_keyword(Keyword::Function) {
            self.advance(); // `function`
            let is_generator = self.eat_operator("*");
            let outer_async = self.is_async_context;
            let outer_generator = self.is_generator_context;
            self.is_async_context = true;
            self.is_generator_context = is_generator;
            let name = match self.peek().kind.clone() {
                TokenKind::Identifier(n) => {
                    self.advance();
                    Some(n)
                }
                _ => None,
            };
            let params = self.parse_param_list()?;
            if is_generator {
                Self::check_generator_params_no_yield(&params)?;
            }
            self.check_duplicate_params(&params)?;
            let is_nspl = Self::params_are_non_simple(&params);
            if is_nspl && self.peek_body_has_use_strict() {
                self.is_async_context = outer_async;
                self.is_generator_context = outer_generator;
                return Err(self.error(
                    "\"use strict\" directive is not allowed in function with non-simple parameters"
                        .into(),
                ));
            }
            let body = self.parse_function_body()?;
            self.is_async_context = outer_async;
            self.is_generator_context = outer_generator;
            // Check for param/lexical conflicts.
            self.validate_params_vs_lexical(&params, &body.statements)?;
            return Ok(Expression::Function(FunctionLiteral {
                name,
                params,
                body,
                is_async: true,
                is_generator,
                is_arrow: false,
            }));
        }

        // `async (params) =>` or `async param =>` — async arrow function.
        // We need lookahead to distinguish from a call expression like `async(x)`.
        let saved = self.cursor;
        let params = match self.peek().kind.clone() {
            TokenKind::Identifier(p)
                if matches!(
                    self.tokens.get(self.cursor + 1),
                    Some(crate::lexer::Token {
                        kind: TokenKind::Operator(op),
                        line_terminator_before: false,
                        ..
                    }) if op == "=>"
                ) =>
            {
                self.advance(); // param name
                vec![FunctionParam::Simple(p)]
            }
            TokenKind::Punctuator('(') => {
                let Ok(p) = self.parse_param_list() else {
                    self.cursor = saved;
                    // Not an async arrow: treat `async` as a regular identifier
                    return Ok(Expression::Identifier("async".into()));
                };
                p
            }
            _ => {
                self.cursor = saved;
                return Ok(Expression::Identifier("async".into()));
            }
        };

        if self.peek().line_terminator_before || !self.eat_operator("=>") {
            self.cursor = saved;
            return Ok(Expression::Identifier("async".into()));
        }

        // Now confirmed it's an async arrow — set async context for body parsing.
        let outer_async = self.is_async_context;
        self.is_async_context = true;
        let is_nspl = Self::params_are_non_simple(&params);
        if is_nspl && self.check_punctuator('{') && self.peek_body_has_use_strict() {
            self.is_async_context = outer_async;
            return Err(self.error(
                "\"use strict\" directive is not allowed in function with non-simple parameters"
                    .into(),
            ));
        }
        let body = if self.check_punctuator('{') {
            let b = self.parse_function_body()?;
            self.is_async_context = outer_async;
            b
        } else {
            self.is_async_context = outer_async;
            let value = self.parse_assignment()?;
            crate::ast::FunctionBody {
                statements: vec![Statement::Return(Some(value))],
                is_strict: self.is_strict,
            }
        };
        Ok(Expression::Function(FunctionLiteral {
            name: None,
            params,
            body,
            is_async: true,
            is_generator: false,
            is_arrow: true,
        }))
    }

    // -----------------------------------------------------------------------
    // V8-A: template literals
    // -----------------------------------------------------------------------

    /// Parses a template literal that starts with a `TemplateHead` token.
    fn parse_template_literal(&mut self) -> Result<Expression, ParseError> {
        let mut quasis = Vec::new();
        let mut expressions = Vec::new();

        // Consume the TemplateHead token.
        let head_text = match self.advance().kind {
            TokenKind::TemplateHead(text) => text,
            _ => unreachable!("parse_template_literal called on non-TemplateHead"),
        };
        quasis.push(head_text);

        loop {
            // Parse the substitution expression.
            let expr = self.parse_expression()?;
            expressions.push(expr);

            // Next must be TemplateMiddle or TemplateTail.
            match self.peek().kind.clone() {
                TokenKind::TemplateMiddle(text) => {
                    self.advance();
                    quasis.push(text);
                }
                TokenKind::TemplateTail(text) => {
                    self.advance();
                    quasis.push(text);
                    break;
                }
                other => {
                    return Err(self.error(format!(
                        "expected template continuation but found {}",
                        crate::parser::describe(&other)
                    )));
                }
            }
        }

        Ok(Expression::TemplateLiteral(TemplateLiteral {
            quasis,
            expressions,
        }))
    }

    // -----------------------------------------------------------------------
    // V8-A: class expressions
    // -----------------------------------------------------------------------

    fn parse_class_expression(&mut self) -> Result<Expression, ParseError> {
        self.advance(); // `class`
        let name = if matches!(self.peek().kind, TokenKind::Identifier(_)) {
            if let TokenKind::Identifier(n) = self.advance().kind {
                Some(n)
            } else {
                unreachable!()
            }
        } else {
            None
        };
        let super_class = if self.eat_keyword(Keyword::Extends) {
            Some(Box::new(self.parse_assignment()?))
        } else {
            None
        };
        let elements = self.parse_class_body()?;
        Ok(Expression::Class(ClassExpression {
            name,
            super_class,
            elements,
        }))
    }

    /// Parses `{ constructor() {...} method() {...} static foo() {...} #priv = 0; }`.
    pub(super) fn parse_class_body(&mut self) -> Result<Vec<ClassElement>, ParseError> {
        self.expect_punctuator('{')?;
        // Class bodies are always strict mode per ECMAScript specification.
        let outer_strict = self.is_strict;
        self.is_strict = true;
        let mut elements = Vec::new();
        let result = self.parse_class_body_elements(&mut elements);
        self.is_strict = outer_strict;
        result?;
        Ok(elements)
    }

    fn parse_class_body_elements(
        &mut self,
        elements: &mut Vec<ClassElement>,
    ) -> Result<(), ParseError> {
        // Track seen private names to detect duplicates.
        // Getters and setters with the same name form a valid accessor pair.
        let mut seen_private_names: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut seen_private_getters: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut seen_private_setters: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        while !self.check_punctuator('}') && !self.at_eof() {
            // Skip empty class elements (bare semicolons).
            if self.eat_punctuator(';') {
                continue;
            }

            // Optional `static` keyword.
            // Disambiguate: `static` followed by `;`, `=`, `}`, or `(` means it
            // is a field/method *name*, not the `static` modifier keyword.
            // Note: `=` is Operator("="), not Punctuator('=').
            let is_static = if self.peek().kind == TokenKind::Keyword(Keyword::Static) {
                let after = self.tokens.get(self.cursor + 1);
                let next_is_field_or_method_name = after.is_some_and(|t| {
                    matches!(&t.kind, TokenKind::Punctuator(';' | '}' | '('))
                        || matches!(&t.kind, TokenKind::Operator(op) if op == "=")
                });
                if next_is_field_or_method_name {
                    false
                } else {
                    self.eat_keyword(Keyword::Static)
                }
            } else {
                false
            };
            if is_static && self.check_punctuator('{') {
                let outer_async = self.is_async_context;
                let outer_generator = self.is_generator_context;
                let outer_function_depth = self.function_depth;
                let outer_loop_depth = self.loop_depth;
                let outer_switch_depth = self.switch_depth;
                let outer_labels = std::mem::take(&mut self.label_stack);
                self.is_async_context = false;
                self.is_generator_context = false;
                self.function_depth = 0;
                self.loop_depth = 0;
                self.switch_depth = 0;
                let block = self.parse_block();
                self.is_async_context = outer_async;
                self.is_generator_context = outer_generator;
                self.function_depth = outer_function_depth;
                self.loop_depth = outer_loop_depth;
                self.switch_depth = outer_switch_depth;
                self.label_stack = outer_labels;
                let Statement::Block(body) = block? else {
                    unreachable!("parse_block returns a block statement")
                };
                self.check_static_block_early_errors(&body)?;
                elements.push(ClassElement::StaticBlock(body));
                while self.eat_punctuator(';') {}
                continue;
            }

            // Check for `async` contextual keyword (method modifier).
            // Only treat it as a modifier when the token after `async` on the SAME LINE
            // looks like the start of a method key, AND the `async` token has no escape seqs.
            let is_async = if matches!(&self.peek().kind, TokenKind::Identifier(s) if s == "async")
                && !self.peek().has_identifier_escape
            {
                let next = self.tokens.get(self.cursor + 1);
                let next_is_key = next.is_some_and(|t| {
                    !t.line_terminator_before
                        && !matches!(t.kind, TokenKind::Punctuator(';' | '(' | '}'))
                });
                if next_is_key {
                    self.advance(); // consume `async`
                    true
                } else {
                    false
                }
            } else {
                false
            };

            // Check for `get`/`set` accessor keywords.
            let is_getter =
                !is_async && matches!(&self.peek().kind, TokenKind::Identifier(s) if s == "get");
            let is_setter =
                !is_async && matches!(&self.peek().kind, TokenKind::Identifier(s) if s == "set");

            // For get/set: peek at the token after to distinguish `get(){}` (method named
            // "get") from `get foo(){}` (getter). Treat as accessor only when next-next
            // token is NOT `(`, `;`, `}`, or `=`.
            let is_accessor = (is_getter || is_setter) && {
                let next = self.tokens.get(self.cursor + 1);
                next.is_some_and(|t| {
                    !t.line_terminator_before
                        && !matches!(t.kind, TokenKind::Punctuator('(' | ';' | '}' | '='))
                })
            };
            if is_accessor {
                self.advance(); // consume `get` or `set`
            }

            // Optional `*` for generator methods.
            let is_generator_method = !is_accessor && self.eat_operator("*");

            // Class member name: identifier, string, number, `#private`, or `[computed]`.
            let prop_name = self.parse_class_member_name()?;

            // Private name duplicate check.
            // Getter + setter with the same name is a valid accessor pair.
            if let PropertyName::PrivateName(pn) = &prop_name {
                let prefix = if is_static { "s" } else { "i" };
                let key = format!("{}#{}", prefix, pn);
                if is_accessor && is_getter {
                    if !seen_private_getters.insert(key) {
                        return Err(self.error(format!("duplicate private getter `#{pn}`")));
                    }
                } else if is_accessor && is_setter {
                    if !seen_private_setters.insert(key) {
                        return Err(self.error(format!("duplicate private setter `#{pn}`")));
                    }
                } else if !seen_private_names.insert(key) {
                    return Err(self.error(format!("duplicate private name `#{pn}`")));
                }
                // `#constructor` is forbidden (per spec, in any position).
                if pn == "constructor" {
                    return Err(self.error("`#constructor` is a reserved private name".into()));
                }
            }

            // Semicolons can terminate standalone fields with no initializer.
            if !is_async && !is_accessor && !is_generator_method && self.eat_punctuator(';') {
                if Self::is_forbidden_field_name(&prop_name, is_static) {
                    return Err(self.error(format!(
                        "`{}` is not a valid class field name",
                        prop_name.to_key_string()
                    )));
                }
                elements.push(ClassElement::Field {
                    name: prop_name,
                    is_static,
                    initializer: None,
                });
                continue;
            }

            // Field declaration: `[static] name = expr ;` or `[static] name;`
            if !is_async && !is_accessor && !is_generator_method && self.eat_operator("=") {
                if Self::is_forbidden_field_name(&prop_name, is_static) {
                    return Err(self.error(format!(
                        "`{}` is not a valid class field name",
                        prop_name.to_key_string()
                    )));
                }
                let init_expr = self.parse_assignment()?;
                self.check_field_init_early_errors(&init_expr)?;
                let initializer = Some(Box::new(init_expr));
                // ASI: after field initializer, if no `;`, next token must be on new line or `}`.
                if !self.check_punctuator(';')
                    && !self.check_punctuator('}')
                    && !self.peek().line_terminator_before
                {
                    return Err(self.error(format!(
                        "unexpected token {} after class field initializer",
                        crate::parser::describe(&self.peek().kind)
                    )));
                }
                self.eat_punctuator(';');
                elements.push(ClassElement::Field {
                    name: prop_name,
                    is_static,
                    initializer,
                });
                while self.eat_punctuator(';') {}
                continue;
            }

            // Accessor getter: `get name() { body }` — no params.
            if is_getter && is_accessor {
                // Early error: `get constructor(){}` and `static get prototype(){}` are forbidden.
                if Self::is_forbidden_method_name(&prop_name, is_static, false, false, true, false)
                {
                    return Err(self.error(format!(
                        "`{}` is not a valid class method name in this position",
                        prop_name.to_key_string()
                    )));
                }
                self.expect_punctuator('(')?;
                self.expect_punctuator(')')?;
                let body = self.parse_function_body()?;
                self.check_non_ctor_super_call(&body)?;
                elements.push(ClassElement::Method {
                    name: prop_name,
                    function: FunctionLiteral {
                        name: None,
                        params: vec![],
                        body,
                        is_async: false,
                        is_generator: false,
                        is_arrow: false,
                    },
                    is_static,
                    is_getter: true,
                    is_setter: false,
                });
                while self.eat_punctuator(';') {}
                continue;
            }

            // Accessor setter: `set name(param) { body }` — one param.
            if is_setter && is_accessor {
                if Self::is_forbidden_method_name(&prop_name, is_static, false, false, false, true)
                {
                    return Err(self.error(format!(
                        "`{}` is not a valid class method name in this position",
                        prop_name.to_key_string()
                    )));
                }
                let params = self.parse_param_list()?;
                // Check for non-simple params + "use strict" in setter body.
                let is_nspl = Self::params_are_non_simple(&params);
                if is_nspl && self.peek_body_has_use_strict() {
                    return Err(self.error(
                        "\"use strict\" directive is not allowed in function with non-simple parameters".into(),
                    ));
                }
                let body = self.parse_function_body()?;
                self.check_non_ctor_super_call(&body)?;
                elements.push(ClassElement::Method {
                    name: prop_name,
                    function: FunctionLiteral {
                        name: None,
                        params,
                        body,
                        is_async: false,
                        is_generator: false,
                        is_arrow: false,
                    },
                    is_static,
                    is_getter: false,
                    is_setter: true,
                });
                while self.eat_punctuator(';') {}
                continue;
            }

            // Method (constructor, regular, async, generator, async-generator).
            if self.check_punctuator('(') {
                let is_ctor = !is_static
                    && !is_generator_method
                    && !is_async
                    && matches!(&prop_name, PropertyName::Identifier(n) if n == "constructor");

                // Early error: async/generator/getter/setter constructor.
                if (is_async || is_generator_method || is_accessor)
                    && matches!(&prop_name, PropertyName::Identifier(n) if n == "constructor")
                    && !is_static
                {
                    return Err(self.error(
                        "class constructor may not be an async method, generator, getter, or setter"
                            .into(),
                    ));
                }

                // Early error: static method named "prototype".
                if is_static
                    && matches!(&prop_name,
                        PropertyName::Identifier(n) | PropertyName::String(n) if n == "prototype")
                {
                    return Err(
                        self.error("a static class method may not be named `prototype`".into())
                    );
                }

                // Early error: private method named "#constructor".
                if matches!(&prop_name, PropertyName::PrivateName(n) if n == "constructor") {
                    return Err(
                        self.error("`#constructor` is a reserved private method name".into())
                    );
                }

                // Set async/generator context for parameter and body parsing.
                let outer_async = self.is_async_context;
                let outer_generator = self.is_generator_context;
                self.is_async_context = is_async;
                self.is_generator_context = is_generator_method;

                let params = self.parse_param_list()?;
                if is_generator_method {
                    Self::check_generator_params_no_yield(&params)?;
                }

                // Strict mode: duplicate simple param names are a SyntaxError.
                // (All class methods are strict since the class body is strict.)
                self.check_duplicate_params(&params)?;

                // Non-simple parameter list + explicit "use strict" in body = SyntaxError.
                let is_nspl = Self::params_are_non_simple(&params);
                if is_nspl && self.peek_body_has_use_strict() {
                    self.is_async_context = outer_async;
                    self.is_generator_context = outer_generator;
                    return Err(self.error(
                        "\"use strict\" directive is not allowed in function with non-simple parameters".into(),
                    ));
                }

                let body = self.parse_function_body()?;
                self.is_async_context = outer_async;
                self.is_generator_context = outer_generator;

                // Non-constructor methods: super() call is a SyntaxError.
                if !is_ctor {
                    self.check_non_ctor_super_call(&body)?;
                }
                let function = FunctionLiteral {
                    name: if is_ctor {
                        Some("constructor".into())
                    } else {
                        Some(prop_name.to_key_string())
                    },
                    params,
                    body,
                    is_async,
                    is_generator: is_generator_method,
                    is_arrow: false,
                };
                if is_ctor {
                    elements.push(ClassElement::Constructor(function));
                } else {
                    elements.push(ClassElement::Method {
                        name: prop_name,
                        function,
                        is_static,
                        is_getter: false,
                        is_setter: false,
                    });
                }
            } else {
                // Field without initializer (no `=` and no `;` was eaten above).
                // ASI rule: next token must be `}`, `;`, or on a new line. Otherwise SyntaxError.
                let next = self.peek();
                if !self.check_punctuator('}')
                    && !self.check_punctuator(';')
                    && !next.line_terminator_before
                {
                    return Err(self.error(format!(
                        "unexpected token {} after class field",
                        crate::parser::describe(&next.kind)
                    )));
                }
                if Self::is_forbidden_field_name(&prop_name, is_static) {
                    return Err(self.error(format!(
                        "`{}` is not a valid class field name",
                        prop_name.to_key_string()
                    )));
                }
                self.eat_punctuator(';');
                elements.push(ClassElement::Field {
                    name: prop_name,
                    is_static,
                    initializer: None,
                });
            }

            // Optional semicolons between class elements.
            while self.eat_punctuator(';') {}
        }
        self.expect_punctuator('}')
    }

    /// Parses a class member name: identifier, string, number, `#private`, or `[computed]`.
    fn parse_class_member_name(&mut self) -> Result<PropertyName, ParseError> {
        if let TokenKind::PrivateName(name) = self.peek().kind.clone() {
            self.advance();
            return Ok(PropertyName::PrivateName(name));
        }
        if self.eat_punctuator('[') {
            let expr = self.allowing_in(|parser| parser.parse_assignment())?;
            self.expect_punctuator(']')?;
            return Ok(PropertyName::Computed(Box::new(expr)));
        }
        self.parse_property_name()
    }

    /// Returns `true` if the given property name is forbidden as a class field name.
    ///
    /// - `constructor` is forbidden as a field name for both instance and static fields.
    /// - `prototype` is forbidden only for static fields.
    /// - `#constructor` is forbidden as a private field name.
    fn is_forbidden_field_name(name: &PropertyName, is_static: bool) -> bool {
        match name {
            PropertyName::Identifier(n) | PropertyName::String(n) => {
                n == "constructor" || (is_static && n == "prototype")
            }
            PropertyName::PrivateName(n) => n == "constructor",
            _ => false,
        }
    }

    /// Returns `true` if the given property name is forbidden as a class method name
    /// given the method's modifiers. The `is_getter`/`is_setter` flags refer to
    /// accessor methods (parsed via `get`/`set`).
    fn is_forbidden_method_name(
        name: &PropertyName,
        is_static: bool,
        _is_async: bool,
        _is_generator: bool,
        is_getter: bool,
        is_setter: bool,
    ) -> bool {
        // `get constructor(){}` and `set constructor(){}` are SyntaxErrors.
        if (is_getter || is_setter)
            && !is_static
            && matches!(name, PropertyName::Identifier(n) if n == "constructor")
        {
            return true;
        }
        // Static methods named "prototype" are forbidden.
        if is_static
            && matches!(name, PropertyName::Identifier(n) | PropertyName::String(n) if n == "prototype")
        {
            return true;
        }
        false
    }

    /// Checks that a non-constructor method body does not directly contain a
    /// `super()` call (`Contains SuperCall` abstract operation). Stops recursion
    /// at nested function expressions (they have their own super binding) but
    /// recurses into arrow functions.
    fn check_non_ctor_super_call(&self, body: &crate::ast::FunctionBody) -> Result<(), ParseError> {
        for stmt in &body.statements {
            self.check_super_call_stmt(stmt)?;
        }
        Ok(())
    }

    fn check_super_call_stmt(&self, stmt: &crate::ast::Statement) -> Result<(), ParseError> {
        use crate::ast::Statement;
        match stmt {
            Statement::Expression(e) => self.check_super_call_expr(e),
            Statement::Return(Some(e)) => self.check_super_call_expr(e),
            Statement::Block(stmts) => {
                for s in stmts {
                    self.check_super_call_stmt(s)?;
                }
                Ok(())
            }
            Statement::If {
                test,
                consequent,
                alternate,
            } => {
                self.check_super_call_expr(test)?;
                self.check_super_call_stmt(consequent)?;
                if let Some(alt) = alternate {
                    self.check_super_call_stmt(alt)?;
                }
                Ok(())
            }
            Statement::While { test, body } => {
                self.check_super_call_expr(test)?;
                self.check_super_call_stmt(body)
            }
            Statement::For {
                init,
                test,
                update,
                body,
            } => {
                if let Some(e) = init.as_ref().and_then(|i| {
                    if let Statement::Expression(e) = i.as_ref() {
                        Some(e)
                    } else {
                        None
                    }
                }) {
                    self.check_super_call_expr(e)?;
                }
                if let Some(e) = test {
                    self.check_super_call_expr(e)?;
                }
                if let Some(e) = update {
                    self.check_super_call_expr(e)?;
                }
                self.check_super_call_stmt(body)
            }
            Statement::Throw(e) => self.check_super_call_expr(e),
            Statement::Try {
                block,
                handler,
                finalizer,
            } => {
                for s in block {
                    self.check_super_call_stmt(s)?;
                }
                if let Some(clause) = handler {
                    for s in &clause.body {
                        self.check_super_call_stmt(s)?;
                    }
                }
                if let Some(fin) = finalizer {
                    for s in fin {
                        self.check_super_call_stmt(s)?;
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn check_super_call_expr(&self, expr: &Expression) -> Result<(), ParseError> {
        match expr {
            Expression::Call { callee, arguments } => {
                if matches!(**callee, Expression::Super) {
                    return Err(ParseError {
                        span: self.peek().span,
                        message: "`super()` is not allowed in non-constructor class methods".into(),
                    });
                }
                self.check_super_call_expr(callee)?;
                for arg in arguments {
                    let e = match arg {
                        crate::ast::CallArgument::Expression(e) => e,
                        crate::ast::CallArgument::Spread(e) => e,
                    };
                    self.check_super_call_expr(e)?;
                }
                Ok(())
            }
            // Arrow functions share the outer super context — recurse.
            Expression::Function(fl) if fl.is_arrow => {
                for s in &fl.body.statements {
                    self.check_super_call_stmt(s)?;
                }
                Ok(())
            }
            // Regular function expressions have their own super binding — stop.
            Expression::Function(_) => Ok(()),
            // Class expressions introduce a new class — stop.
            Expression::Class(_) => Ok(()),
            Expression::Binary { left, right, .. } => {
                self.check_super_call_expr(left)?;
                self.check_super_call_expr(right)
            }
            Expression::Logical { left, right, .. } => {
                self.check_super_call_expr(left)?;
                self.check_super_call_expr(right)
            }
            Expression::Assignment { target, value } => {
                self.check_super_call_expr(target)?;
                self.check_super_call_expr(value)
            }
            Expression::CompoundAssignment { target, value, .. } => {
                self.check_super_call_expr(target)?;
                self.check_super_call_expr(value)
            }
            Expression::Unary { argument, .. } | Expression::Update { argument, .. } => {
                self.check_super_call_expr(argument)
            }
            Expression::Member {
                object, property, ..
            } => {
                self.check_super_call_expr(object)?;
                self.check_super_call_expr(property)
            }
            Expression::Conditional {
                test,
                consequent,
                alternate,
            } => {
                self.check_super_call_expr(test)?;
                self.check_super_call_expr(consequent)?;
                self.check_super_call_expr(alternate)
            }
            Expression::Construct { callee, arguments } => {
                self.check_super_call_expr(callee)?;
                for arg in arguments {
                    let e = match arg {
                        crate::ast::CallArgument::Expression(e) => e,
                        crate::ast::CallArgument::Spread(e) => e,
                    };
                    self.check_super_call_expr(e)?;
                }
                Ok(())
            }
            Expression::Array(elements) => {
                for el in elements {
                    if let crate::ast::ArrayElement::Expression(e)
                    | crate::ast::ArrayElement::Spread(e) = el
                    {
                        self.check_super_call_expr(e)?;
                    }
                }
                Ok(())
            }
            Expression::Object(props) => {
                for prop in props {
                    match prop {
                        ObjectProperty::Data { value, .. } => self.check_super_call_expr(value)?,
                        ObjectProperty::ComputedData { key, value } => {
                            self.check_super_call_expr(key)?;
                            self.check_super_call_expr(value)?;
                        }
                        _ => {}
                    }
                }
                Ok(())
            }
            Expression::TemplateLiteral(tl) => {
                for e in &tl.expressions {
                    self.check_super_call_expr(e)?;
                }
                Ok(())
            }
            Expression::Spread(e)
            | Expression::Await(e)
            | Expression::Yield {
                argument: Some(e), ..
            } => self.check_super_call_expr(e),
            _ => Ok(()),
        }
    }

    /// Early-error check for class field initializers.
    ///
    /// - `arguments` identifier is forbidden (no `arguments` binding in field context)
    /// - `super()` calls are forbidden (`Contains SuperCall` must be false)
    ///
    /// Recurses into arrow functions but stops at regular function expressions.
    fn check_field_init_early_errors(&self, expr: &Expression) -> Result<(), ParseError> {
        self.walk_field_init(expr, false)
    }

    fn walk_field_init(&self, expr: &Expression, in_super_call: bool) -> Result<(), ParseError> {
        use crate::ast::{ArrayElement, CallArgument, ObjectProperty};
        match expr {
            Expression::Identifier(name) if name == "arguments" => {
                return Err(ParseError {
                    span: self.peek().span,
                    message: "`arguments` is not allowed in class field initializers".into(),
                });
            }
            Expression::Call { callee, arguments } => {
                if matches!(**callee, Expression::Super) {
                    return Err(ParseError {
                        span: self.peek().span,
                        message: "`super()` is not allowed in class field initializers".into(),
                    });
                }
                self.walk_field_init(callee, false)?;
                for arg in arguments {
                    match arg {
                        CallArgument::Expression(e) | CallArgument::Spread(e) => {
                            self.walk_field_init(e, false)?;
                        }
                    }
                }
            }
            // Regular function expressions: stop recursion (they have own `arguments`)
            Expression::Function(fl) if !fl.is_arrow => {}
            Expression::Function(fl) => {
                // Arrow function: recurse into body (no own `arguments`)
                for stmt in &fl.body.statements {
                    self.walk_field_init_stmt(stmt)?;
                }
            }
            Expression::Unary { argument, .. } | Expression::Await(argument) => {
                self.walk_field_init(argument, false)?;
            }
            Expression::Update { argument, .. } => {
                self.walk_field_init(argument, false)?;
            }
            Expression::Binary { left, right, .. }
            | Expression::Logical { left, right, .. }
            | Expression::Assignment {
                target: left,
                value: right,
            }
            | Expression::CompoundAssignment {
                target: left,
                value: right,
                ..
            } => {
                self.walk_field_init(left, false)?;
                self.walk_field_init(right, false)?;
            }
            Expression::Conditional {
                test,
                consequent,
                alternate,
            } => {
                self.walk_field_init(test, false)?;
                self.walk_field_init(consequent, false)?;
                self.walk_field_init(alternate, false)?;
            }
            Expression::Construct { callee, arguments } => {
                self.walk_field_init(callee, false)?;
                for arg in arguments {
                    match arg {
                        CallArgument::Expression(e) | CallArgument::Spread(e) => {
                            self.walk_field_init(e, false)?;
                        }
                    }
                }
            }
            Expression::Member {
                object, property, ..
            } => {
                self.walk_field_init(object, false)?;
                self.walk_field_init(property, false)?;
            }
            Expression::Array(elements) => {
                for el in elements {
                    match el {
                        ArrayElement::Expression(e) | ArrayElement::Spread(e) => {
                            self.walk_field_init(e, false)?;
                        }
                        ArrayElement::Hole => {}
                    }
                }
            }
            Expression::Object(props) => {
                for prop in props {
                    match prop {
                        ObjectProperty::Data { value, .. }
                        | ObjectProperty::PrototypeSetter { value } => {
                            self.walk_field_init(value, false)?;
                        }
                        ObjectProperty::ComputedData { key, value } => {
                            self.walk_field_init(key, false)?;
                            self.walk_field_init(value, false)?;
                        }
                        ObjectProperty::Getter { .. } | ObjectProperty::Setter { .. } => {}
                        ObjectProperty::Spread(e) => self.walk_field_init(e, false)?,
                    }
                }
            }
            Expression::Spread(e) => self.walk_field_init(e, false)?,
            Expression::Yield { argument, .. } => {
                if let Some(a) = argument {
                    self.walk_field_init(a, false)?;
                }
            }
            Expression::TemplateLiteral(tl) => {
                for e in &tl.expressions {
                    self.walk_field_init(e, false)?;
                }
            }
            Expression::Sequence(exprs) => {
                for e in exprs {
                    self.walk_field_init(e, false)?;
                }
            }
            Expression::DynamicImport { specifier, options } => {
                self.walk_field_init(specifier, false)?;
                if let Some(options) = options {
                    self.walk_field_init(options, false)?;
                }
            }
            // Class expressions: stop recursion (own class scope)
            Expression::Class(_) => {}
            Expression::OptionalChain { base, steps } => {
                self.walk_field_init(base, false)?;
                for step in steps {
                    match step {
                        OptionalChainStep::Member { property, .. } => {
                            self.walk_field_init(property, false)?
                        }
                        OptionalChainStep::Call { arguments, .. } => {
                            for arg in arguments {
                                match arg {
                                    CallArgument::Expression(e) | CallArgument::Spread(e) => {
                                        self.walk_field_init(e, false)?;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Terminals: nothing to recurse into
            Expression::Literal(_)
            | Expression::This
            | Expression::Super
            | Expression::NewTarget
            | Expression::ImportMeta
            | Expression::Identifier(_)
            | Expression::PrivateName(_) => {}
        }
        let _ = in_super_call;
        Ok(())
    }

    fn walk_field_init_stmt(&self, stmt: &crate::ast::Statement) -> Result<(), ParseError> {
        use crate::ast::Statement;
        match stmt {
            Statement::Expression(e) | Statement::Return(Some(e)) | Statement::Throw(e) => {
                self.walk_field_init(e, false)
            }
            Statement::Block(stmts) => {
                for s in stmts {
                    self.walk_field_init_stmt(s)?;
                }
                Ok(())
            }
            Statement::VariableDeclaration { declarations, .. } => {
                for d in declarations {
                    if let Some(init) = &d.initializer {
                        self.walk_field_init(init, false)?;
                    }
                }
                Ok(())
            }
            Statement::If {
                test,
                consequent,
                alternate,
            } => {
                self.walk_field_init(test, false)?;
                self.walk_field_init_stmt(consequent)?;
                if let Some(alt) = alternate {
                    self.walk_field_init_stmt(alt)?;
                }
                Ok(())
            }
            Statement::While { test, body } => {
                self.walk_field_init(test, false)?;
                self.walk_field_init_stmt(body)
            }
            Statement::For {
                init: _,
                test,
                update,
                body,
            } => {
                if let Some(e) = test {
                    self.walk_field_init(e, false)?;
                }
                if let Some(e) = update {
                    self.walk_field_init(e, false)?;
                }
                self.walk_field_init_stmt(body)
            }
            Statement::FunctionDeclaration { .. } => Ok(()),
            _ => Ok(()),
        }
    }

    fn check_static_block_early_errors(
        &self,
        statements: &[crate::ast::Statement],
    ) -> Result<(), ParseError> {
        for stmt in statements {
            self.walk_static_block_stmt(stmt)?;
        }
        Ok(())
    }

    fn walk_static_block_stmt(&self, stmt: &crate::ast::Statement) -> Result<(), ParseError> {
        use crate::ast::Statement;
        match stmt {
            Statement::Expression(expr) | Statement::Throw(expr) => {
                self.walk_static_block_expr(expr)
            }
            Statement::Return(_) => Err(ParseError {
                span: self.peek().span,
                message: "`return` is not allowed in class static blocks".into(),
            }),
            Statement::Block(statements) => {
                for stmt in statements {
                    self.walk_static_block_stmt(stmt)?;
                }
                Ok(())
            }
            Statement::VariableDeclaration { declarations, .. } => {
                for declaration in declarations {
                    if let Some(pattern) = &declaration.pattern {
                        self.walk_static_block_binding_pattern(pattern)?;
                    }
                    if let Some(initializer) = &declaration.initializer {
                        self.walk_static_block_expr(initializer)?;
                    }
                }
                Ok(())
            }
            Statement::DestructuringDeclaration {
                pattern,
                initializer,
                ..
            } => {
                self.walk_static_block_binding_pattern(pattern)?;
                self.walk_static_block_expr(initializer)
            }
            Statement::If {
                test,
                consequent,
                alternate,
            } => {
                self.walk_static_block_expr(test)?;
                self.walk_static_block_stmt(consequent)?;
                if let Some(alternate) = alternate {
                    self.walk_static_block_stmt(alternate)?;
                }
                Ok(())
            }
            Statement::While { test, body } | Statement::DoWhile { test, body } => {
                self.walk_static_block_expr(test)?;
                self.walk_static_block_stmt(body)
            }
            Statement::For {
                init,
                test,
                update,
                body,
            } => {
                if let Some(init) = init {
                    self.walk_static_block_stmt(init)?;
                }
                if let Some(test) = test {
                    self.walk_static_block_expr(test)?;
                }
                if let Some(update) = update {
                    self.walk_static_block_expr(update)?;
                }
                self.walk_static_block_stmt(body)
            }
            Statement::ForIn { left, right, body }
            | Statement::ForOf {
                left, right, body, ..
            } => {
                self.walk_static_block_for_binding(left)?;
                self.walk_static_block_expr(right)?;
                self.walk_static_block_stmt(body)
            }
            Statement::Labelled { body, .. } => self.walk_static_block_stmt(body),
            Statement::Try {
                block,
                handler,
                finalizer,
            } => {
                for stmt in block {
                    self.walk_static_block_stmt(stmt)?;
                }
                if let Some(handler) = handler {
                    for stmt in &handler.body {
                        self.walk_static_block_stmt(stmt)?;
                    }
                }
                if let Some(finalizer) = finalizer {
                    for stmt in finalizer {
                        self.walk_static_block_stmt(stmt)?;
                    }
                }
                Ok(())
            }
            Statement::Switch {
                discriminant,
                cases,
            } => {
                self.walk_static_block_expr(discriminant)?;
                for case in cases {
                    if let Some(test) = &case.test {
                        self.walk_static_block_expr(test)?;
                    }
                    for stmt in &case.consequent {
                        self.walk_static_block_stmt(stmt)?;
                    }
                }
                Ok(())
            }
            Statement::ClassDeclaration(class) => {
                if class.name == "await" {
                    return Err(ParseError {
                        span: self.peek().span,
                        message: "`await` is not allowed as a class static block binding".into(),
                    });
                }
                if let Some(super_class) = &class.super_class {
                    self.walk_static_block_expr(super_class)?;
                }
                for element in &class.elements {
                    self.walk_static_block_class_element(element)?;
                }
                Ok(())
            }
            Statement::With { object, body } => {
                self.walk_static_block_expr(object)?;
                self.walk_static_block_stmt(body)
            }
            // Function bodies and module declarations form their own syntax boundaries here.
            Statement::FunctionDeclaration { .. }
            | Statement::ModuleDeclaration(_)
            | Statement::Empty
            | Statement::Break(_)
            | Statement::Continue(_) => Ok(()),
        }
    }

    fn walk_static_block_expr(&self, expr: &Expression) -> Result<(), ParseError> {
        use crate::ast::{ArrayElement, CallArgument, ObjectProperty};
        match expr {
            Expression::Identifier(name) if name == "arguments" || name == "await" => {
                Err(ParseError {
                    span: self.peek().span,
                    message: format!("`{name}` is not allowed in class static blocks"),
                })
            }
            Expression::Await(_) => Err(ParseError {
                span: self.peek().span,
                message: "`await` is not allowed in class static blocks".into(),
            }),
            Expression::Call { callee, arguments } => {
                if matches!(**callee, Expression::Super) {
                    return Err(ParseError {
                        span: self.peek().span,
                        message: "`super()` is not allowed in class static blocks".into(),
                    });
                }
                self.walk_static_block_expr(callee)?;
                for arg in arguments {
                    match arg {
                        CallArgument::Expression(expr) | CallArgument::Spread(expr) => {
                            self.walk_static_block_expr(expr)?;
                        }
                    }
                }
                Ok(())
            }
            Expression::Function(_) => Ok(()),
            Expression::Class(class) => {
                if class.name.as_deref() == Some("await") {
                    return Err(ParseError {
                        span: self.peek().span,
                        message: "`await` is not allowed as a class static block binding".into(),
                    });
                }
                if let Some(super_class) = &class.super_class {
                    self.walk_static_block_expr(super_class)?;
                }
                for element in &class.elements {
                    self.walk_static_block_class_element(element)?;
                }
                Ok(())
            }
            Expression::Unary { argument, .. } | Expression::Update { argument, .. } => {
                self.walk_static_block_expr(argument)
            }
            Expression::Binary { left, right, .. }
            | Expression::Logical { left, right, .. }
            | Expression::Assignment {
                target: left,
                value: right,
            }
            | Expression::CompoundAssignment {
                target: left,
                value: right,
                ..
            }
            | Expression::Member {
                object: left,
                property: right,
                ..
            } => {
                self.walk_static_block_expr(left)?;
                self.walk_static_block_expr(right)
            }
            Expression::Conditional {
                test,
                consequent,
                alternate,
            } => {
                self.walk_static_block_expr(test)?;
                self.walk_static_block_expr(consequent)?;
                self.walk_static_block_expr(alternate)
            }
            Expression::Construct { callee, arguments } => {
                self.walk_static_block_expr(callee)?;
                for arg in arguments {
                    match arg {
                        CallArgument::Expression(expr) | CallArgument::Spread(expr) => {
                            self.walk_static_block_expr(expr)?;
                        }
                    }
                }
                Ok(())
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        ArrayElement::Expression(expr) | ArrayElement::Spread(expr) => {
                            self.walk_static_block_expr(expr)?;
                        }
                        ArrayElement::Hole => {}
                    }
                }
                Ok(())
            }
            Expression::Object(properties) => {
                for property in properties {
                    match property {
                        ObjectProperty::Data { value, .. }
                        | ObjectProperty::PrototypeSetter { value } => {
                            self.walk_static_block_expr(value)?;
                        }
                        ObjectProperty::ComputedData { key, value } => {
                            self.walk_static_block_expr(key)?;
                            self.walk_static_block_expr(value)?;
                        }
                        ObjectProperty::Spread(expr) => self.walk_static_block_expr(expr)?,
                        ObjectProperty::Getter { .. } | ObjectProperty::Setter { .. } => {}
                    }
                }
                Ok(())
            }
            Expression::Spread(expr) => self.walk_static_block_expr(expr),
            Expression::Yield { argument, .. } => {
                if let Some(argument) = argument {
                    self.walk_static_block_expr(argument)?;
                }
                Ok(())
            }
            Expression::TemplateLiteral(template) => {
                for expr in &template.expressions {
                    self.walk_static_block_expr(expr)?;
                }
                Ok(())
            }
            Expression::Sequence(expressions) => {
                for expr in expressions {
                    self.walk_static_block_expr(expr)?;
                }
                Ok(())
            }
            Expression::DynamicImport { specifier, options } => {
                self.walk_static_block_expr(specifier)?;
                if let Some(options) = options {
                    self.walk_static_block_expr(options)?;
                }
                Ok(())
            }
            Expression::OptionalChain { base, steps } => {
                self.walk_static_block_expr(base)?;
                for step in steps {
                    match step {
                        OptionalChainStep::Member { property, .. } => {
                            self.walk_static_block_expr(property)?;
                        }
                        OptionalChainStep::Call { arguments, .. } => {
                            for arg in arguments {
                                match arg {
                                    CallArgument::Expression(e) | CallArgument::Spread(e) => {
                                        self.walk_static_block_expr(e)?;
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(())
            }
            Expression::Literal(_)
            | Expression::This
            | Expression::Super
            | Expression::NewTarget
            | Expression::ImportMeta
            | Expression::Identifier(_)
            | Expression::PrivateName(_) => Ok(()),
        }
    }

    fn walk_static_block_class_element(&self, element: &ClassElement) -> Result<(), ParseError> {
        match element {
            ClassElement::Constructor(_) => Ok(()),
            ClassElement::Method { name, .. } => self.walk_static_block_property_name(name),
            ClassElement::Field {
                name, initializer, ..
            } => {
                self.walk_static_block_property_name(name)?;
                if let Some(initializer) = initializer {
                    self.walk_static_block_expr(initializer)?;
                }
                Ok(())
            }
            ClassElement::StaticBlock(_) => Ok(()),
        }
    }

    fn walk_static_block_property_name(&self, name: &PropertyName) -> Result<(), ParseError> {
        if let PropertyName::Computed(expr) = name {
            self.walk_static_block_expr(expr)?;
        }
        Ok(())
    }

    fn walk_static_block_for_binding(
        &self,
        binding: &crate::ast::ForBinding,
    ) -> Result<(), ParseError> {
        match binding {
            crate::ast::ForBinding::Target(expr) => self.walk_static_block_expr(expr),
            crate::ast::ForBinding::Declaration { pattern, .. } => {
                self.walk_static_block_binding_pattern(pattern)
            }
        }
    }

    fn walk_static_block_binding_pattern(
        &self,
        pattern: &crate::ast::BindingPattern,
    ) -> Result<(), ParseError> {
        match pattern {
            crate::ast::BindingPattern::Identifier(name)
                if name == "arguments" || name == "await" =>
            {
                Err(ParseError {
                    span: self.peek().span,
                    message: format!("`{name}` is not allowed as a class static block binding"),
                })
            }
            crate::ast::BindingPattern::Identifier(_) => Ok(()),
            crate::ast::BindingPattern::Array { elements, rest } => {
                for element in elements.iter().flatten() {
                    self.walk_static_block_binding_pattern(&element.pattern)?;
                    if let Some(default) = &element.default {
                        self.walk_static_block_expr(default)?;
                    }
                }
                if let Some(rest) = rest {
                    self.walk_static_block_binding_pattern(rest)?;
                }
                Ok(())
            }
            crate::ast::BindingPattern::Object { props, rest } => {
                for prop in props {
                    if let crate::ast::ObjectBindingKey::Computed(key) = &prop.key {
                        self.walk_static_block_expr(key)?;
                    }
                    self.walk_static_block_binding_pattern(&prop.value)?;
                    if let Some(default) = &prop.default {
                        self.walk_static_block_expr(default)?;
                    }
                }
                if let Some(rest) = rest {
                    self.walk_static_block_binding_pattern(rest)?;
                }
                Ok(())
            }
        }
    }
}

/// Maps an operator spelling to its precedence, or `None` if it is not a binary
/// or logical operator.
fn binary_precedence(operator: &str) -> Option<u8> {
    Some(match operator {
        "??" => 1,
        "||" => 2,
        "&&" => 3,
        "|" => 4,
        "^" => 5,
        "&" => 6,
        "==" | "!=" | "===" | "!==" => 7,
        "<" | "<=" | ">" | ">=" => 8,
        "in" | "instanceof" => 8,
        "<<" | ">>" | ">>>" => 9,
        "+" | "-" => 10,
        "*" | "/" | "%" => 11,
        "**" => 12, // right-associative; handled specially in parse_binary
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
        "??" => Some(LogicalOperator::Nullish),
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
        "**" => BinaryOperator::Exponentiation,
        "&" => BinaryOperator::BitwiseAnd,
        "|" => BinaryOperator::BitwiseOr,
        "^" => BinaryOperator::BitwiseXor,
        "<<" => BinaryOperator::LeftShift,
        ">>" => BinaryOperator::RightShift,
        ">>>" => BinaryOperator::UnsignedRightShift,
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

/// Validates that an array/object destructuring assignment target obeys ES spec early errors.
pub(super) fn validate_destructuring_assignment_target(
    expr: &Expression,
) -> Result<(), String> {
    match expr {
        Expression::Array(elements) => {
            let len = elements.len();
            for (i, el) in elements.iter().enumerate() {
                match el {
                    ArrayElement::Spread(inner) => {
                        // Rest element cannot have an initializer.
                        if matches!(inner, Expression::Assignment { .. }) {
                            return Err("rest element may not have a default value".into());
                        }
                        // Rest element must be last — nothing (including holes) may follow.
                        if i + 1 < len {
                            return Err("rest element must be last element".into());
                        }
                        validate_destructuring_assignment_subtarget(inner)?;
                    }
                    ArrayElement::Expression(inner) => {
                        validate_destructuring_assignment_element(inner)?;
                    }
                    ArrayElement::Hole => {}
                }
            }
        }
        Expression::Object(props) => {
            let len = props.len();
            for (i, prop) in props.iter().enumerate() {
                match prop {
                    ObjectProperty::Data { value, .. }
                    | ObjectProperty::ComputedData { value, .. } => {
                        validate_destructuring_assignment_element(value)?;
                    }
                    ObjectProperty::Spread(inner) => {
                        if matches!(inner, Expression::Assignment { .. }) {
                            return Err("rest element may not have a default value".into());
                        }
                        if i + 1 < len {
                            return Err("rest element must be last element".into());
                        }
                        validate_destructuring_assignment_subtarget(inner)?;
                    }
                    ObjectProperty::Getter { .. }
                    | ObjectProperty::Setter { .. }
                    | ObjectProperty::PrototypeSetter { .. } => {
                        return Err("invalid destructuring assignment target".into());
                    }
                }
            }
        }
        Expression::Assignment { target, .. } => {
            validate_destructuring_assignment_subtarget(target)?;
        }
        Expression::Identifier(_) | Expression::Member { .. } => {}
        _ => return Err("invalid destructuring assignment target".into()),
    }
    Ok(())
}

fn validate_destructuring_assignment_element(expr: &Expression) -> Result<(), String> {
    match expr {
        Expression::Assignment { target, .. } => {
            validate_destructuring_assignment_subtarget(target)
        }
        _ => validate_destructuring_assignment_subtarget(expr),
    }
}

fn validate_destructuring_assignment_subtarget(expr: &Expression) -> Result<(), String> {
    match expr {
        Expression::Identifier(_) | Expression::Member { .. } => Ok(()),
        Expression::Array(_) | Expression::Object(_) => {
            validate_destructuring_assignment_target(expr)
        }
        _ => Err("invalid destructuring assignment target".into()),
    }
}

fn is_assignment_target(expression: &Expression) -> bool {
    matches!(
        expression,
        Expression::Identifier(_) | Expression::Member { .. }
    )
}

/// Returns true for array/object literals that can serve as destructuring assignment targets.
fn is_destructuring_target(expression: &Expression) -> bool {
    matches!(expression, Expression::Array(_) | Expression::Object(_))
}

/// Maps compound assignment lexemes onto [`AssignmentOperator`].
fn compound_assignment_operator(operator: &str) -> Option<AssignmentOperator> {
    match operator {
        "+=" => Some(AssignmentOperator::Add),
        "-=" => Some(AssignmentOperator::Subtract),
        "*=" => Some(AssignmentOperator::Multiply),
        "/=" => Some(AssignmentOperator::Divide),
        "%=" => Some(AssignmentOperator::Remainder),
        "**=" => Some(AssignmentOperator::Exponentiation),
        "&=" => Some(AssignmentOperator::BitwiseAnd),
        "|=" => Some(AssignmentOperator::BitwiseOr),
        "^=" => Some(AssignmentOperator::BitwiseXor),
        "<<=" => Some(AssignmentOperator::LeftShift),
        ">>=" => Some(AssignmentOperator::RightShift),
        ">>>=" => Some(AssignmentOperator::UnsignedRightShift),
        "&&=" => Some(AssignmentOperator::LogicalAnd),
        "||=" => Some(AssignmentOperator::LogicalOr),
        "??=" => Some(AssignmentOperator::NullishCoalescing),
        _ => None,
    }
}

/// Maps the `++` / `--` operator lexemes onto [`UpdateOperator`].
fn update_operator(operator: &str) -> Option<UpdateOperator> {
    match operator {
        "++" => Some(UpdateOperator::Increment),
        "--" => Some(UpdateOperator::Decrement),
        _ => None,
    }
}

/// Returns `true` if `expr` (or its recursive MemberExpression base) accesses a private name
/// via non-computed member access — e.g. `expr.#x` or `(a.b).#x`.
fn is_private_member_access(expr: &crate::ast::Expression) -> bool {
    match expr {
        crate::ast::Expression::Member {
            property,
            computed: false,
            ..
        } => {
            matches!(property.as_ref(), crate::ast::Expression::PrivateName(_))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ast::{
            ArrayElement, AssignmentOperator, BinaryOperator, CallArgument, Expression,
            FunctionLiteral, FunctionParam, Literal, LogicalOperator, ObjectProperty, PropertyName,
            Statement, UnaryOperator,
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

    fn parse_program_ok(source: &str) {
        let tokens = Lexer::new(source).tokenize().expect("lexing succeeds");
        Parser::new(tokens)
            .parse_program()
            .expect("parsing succeeds");
    }

    fn parse_program_err(source: &str) {
        let tokens = Lexer::new(source).tokenize().expect("lexing succeeds");
        Parser::new(tokens)
            .parse_program()
            .expect_err("parsing fails");
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
        FunctionParam::Simple(name.into())
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
    fn parses_compound_assignment() {
        let expression = parse_expression("total += value");
        let Expression::CompoundAssignment {
            operator,
            target,
            value,
        } = expression
        else {
            panic!("expected compound assignment");
        };
        assert_eq!(operator, AssignmentOperator::Add);
        assert_eq!(*target, Expression::Identifier("total".into()));
        assert_eq!(*value, Expression::Identifier("value".into()));
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
            [CallArgument::Expression(Expression::Literal(
                Literal::String("boom".into())
            ))]
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
        let Expression::Function(FunctionLiteral {
            name, params, body, ..
        }) = expr
        else {
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
    fn class_computed_member_name_allows_in_inside_for_header() {
        let source = "for (C = class { get ['x' in empty]() { return 1; } }; ; ) { break; }";
        let tokens = Lexer::new(source).tokenize().expect("lexing succeeds");
        Parser::new(tokens)
            .parse_program()
            .expect("computed class member names re-enable `in`");
    }

    #[test]
    fn object_computed_property_name_allows_in_inside_for_header() {
        parse_program_ok("for (obj = { get ['x' in empty]() { return 1; } }; ; ) { break; }");
        parse_program_ok("for (obj = { ['x' in empty]: 1 }; ; ) { break; }");
    }

    #[test]
    fn static_block_rejects_direct_arguments_and_super_call() {
        parse_program_err("class C { static { arguments; } }");
        parse_program_err("class C { static { super(); } }");
        parse_program_err("class C { static { (class { [arguments]() {} }); } }");
    }

    #[test]
    fn static_block_rejects_return_and_outer_loop_control() {
        parse_program_err("function f() { class C { static { return; } } }");
        parse_program_err("while (true) { class C { static { break; } } }");
        parse_program_err("while (true) { class C { static { continue; } } }");
    }

    #[test]
    fn static_block_keeps_function_arguments_boundary() {
        parse_program_ok(
            "class C { static { \
                (function(x = arguments) { return arguments; }); \
                (class { method(x = arguments) { return arguments; } }); \
            } }",
        );
    }

    #[test]
    fn generator_method_rejects_yield_in_parameter_defaults() {
        parse_program_err("class C { static *g(x = yield) {} }");
        parse_program_err("0, class { static *g(x = yield) {} };");
    }

    #[test]
    fn strict_destructuring_rejects_eval_arguments_targets() {
        parse_program_err("\"use strict\"; [arguments] = [];");
        parse_program_err("\"use strict\"; ({ eval } = {});");
        parse_program_ok("\"use strict\"; var x; [x = arguments] = [];");
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

        let expr = parse_expression("value instanceof Type !== true");
        let Expression::Binary {
            operator: BinaryOperator::StrictNotEqual,
            left,
            ..
        } = expr
        else {
            panic!("expected strict inequality at top level");
        };
        assert!(matches!(
            *left,
            Expression::Binary {
                operator: BinaryOperator::InstanceOf,
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
        assert_eq!(parameter.name(), "v");
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
    fn parses_object_property_shorthand() {
        let Expression::Object(properties) = parse_expression("({ length })") else {
            panic!("expected object literal");
        };
        assert!(matches!(
            &properties[0],
            ObjectProperty::Data {
                key: PropertyName::Identifier(key),
                value: Expression::Identifier(value),
            } if key == "length" && value == "length"
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
