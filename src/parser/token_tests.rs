//! Token-driven parser unit tests.
//!
//! Every test here constructs a `Vec<Token>` by hand and feeds it directly to
//! `Parser::new`, bypassing the lexer entirely.  This makes the parser's
//! behaviour observable in isolation: a failing test here means the parser is
//! wrong, never the lexer.

use crate::{
    ast::{
        BinaryOperator, Expression, Literal, LogicalOperator, Statement, UnaryOperator,
        VariableDeclarator, VariableKind,
    },
    lexer::{Keyword, Span, Token, TokenKind},
    parser::Parser,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Creates a token with a dummy span. Parser logic is span-agnostic so the
/// exact byte offsets do not matter in isolation tests.
fn tok(kind: TokenKind) -> Token {
    Token::new(kind, Span::new(0, 0))
}

fn num(value: f64) -> Token {
    tok(TokenKind::Number(value))
}

fn op(spelling: &str) -> Token {
    tok(TokenKind::Operator(spelling.to_owned()))
}

fn ident(name: &str) -> Token {
    tok(TokenKind::Identifier(name.to_owned()))
}

fn punc(ch: char) -> Token {
    tok(TokenKind::Punctuator(ch))
}

fn kw(keyword: Keyword) -> Token {
    tok(TokenKind::Keyword(keyword))
}

fn eof() -> Token {
    tok(TokenKind::Eof)
}

/// Like [`tok`], but marks the token as preceded by a line terminator. Used to
/// exercise restricted productions such as `throw` without involving the lexer.
fn tok_nl(kind: TokenKind) -> Token {
    Token::with_line_terminator_before(kind, Span::new(0, 0), true)
}

/// Parses a token stream and returns the single expression it contains.
fn parse_expr(tokens: Vec<Token>) -> Expression {
    let mut program = Parser::new(tokens).parse_program().expect("parse succeeds");
    assert_eq!(program.body.len(), 1, "expected exactly one statement");
    match program.body.remove(0) {
        Statement::Expression(expr) => expr,
        other => panic!("expected an expression statement, got {other:?}"),
    }
}

/// Parses a token stream and returns the single statement it contains.
fn parse_stmt(tokens: Vec<Token>) -> Statement {
    let mut program = Parser::new(tokens).parse_program().expect("parse succeeds");
    assert_eq!(program.body.len(), 1);
    program.body.remove(0)
}

fn parse_err(tokens: Vec<Token>) -> crate::parser::ParseError {
    Parser::new(tokens)
        .parse_program()
        .expect_err("parse should fail")
}

// ---------------------------------------------------------------------------
// Operator precedence
// ---------------------------------------------------------------------------

/// `1 + 2 * 3` must produce `Add(1, Mul(2, 3))` �?multiplication binds tighter.
#[test]
fn addition_binds_looser_than_multiplication() {
    let tokens = vec![
        num(1.0),
        op("+"),
        num(2.0),
        op("*"),
        num(3.0),
        punc(';'),
        eof(),
    ];
    let expr = parse_expr(tokens);
    assert_eq!(
        expr,
        Expression::Binary {
            operator: BinaryOperator::Add,
            left: Box::new(Expression::Literal(Literal::Number(1.0))),
            right: Box::new(Expression::Binary {
                operator: BinaryOperator::Multiply,
                left: Box::new(Expression::Literal(Literal::Number(2.0))),
                right: Box::new(Expression::Literal(Literal::Number(3.0))),
            }),
        }
    );
}

/// `18 / 2 / 3` must produce `Div(Div(18, 2), 3)` �?left associativity.
#[test]
fn division_is_left_associative() {
    let tokens = vec![
        num(18.0),
        op("/"),
        num(2.0),
        op("/"),
        num(3.0),
        punc(';'),
        eof(),
    ];
    let expr = parse_expr(tokens);
    assert_eq!(
        expr,
        Expression::Binary {
            operator: BinaryOperator::Divide,
            left: Box::new(Expression::Binary {
                operator: BinaryOperator::Divide,
                left: Box::new(Expression::Literal(Literal::Number(18.0))),
                right: Box::new(Expression::Literal(Literal::Number(2.0))),
            }),
            right: Box::new(Expression::Literal(Literal::Number(3.0))),
        }
    );
}

/// `1 === 2` �?Binary { StrictEqual }.
#[test]
fn strict_equal_produces_binary_node() {
    let tokens = vec![num(1.0), op("==="), num(2.0), punc(';'), eof()];
    let expr = parse_expr(tokens);
    assert!(matches!(
        expr,
        Expression::Binary {
            operator: BinaryOperator::StrictEqual,
            ..
        }
    ));
}

/// `1 !== 2` �?Binary { StrictNotEqual }.
#[test]
fn strict_not_equal_produces_binary_node() {
    let tokens = vec![num(1.0), op("!=="), num(2.0), punc(';'), eof()];
    let expr = parse_expr(tokens);
    assert!(matches!(
        expr,
        Expression::Binary {
            operator: BinaryOperator::StrictNotEqual,
            ..
        }
    ));
}

/// `1 <= 2` �?Binary { LessThanOrEqual }.
#[test]
fn less_than_or_equal_produces_binary_node() {
    let tokens = vec![num(1.0), op("<="), num(2.0), punc(';'), eof()];
    let expr = parse_expr(tokens);
    assert!(matches!(
        expr,
        Expression::Binary {
            operator: BinaryOperator::LessThanOrEqual,
            ..
        }
    ));
}

/// `1 >= 2` �?Binary { GreaterThanOrEqual }.
#[test]
fn greater_than_or_equal_produces_binary_node() {
    let tokens = vec![num(1.0), op(">="), num(2.0), punc(';'), eof()];
    let expr = parse_expr(tokens);
    assert!(matches!(
        expr,
        Expression::Binary {
            operator: BinaryOperator::GreaterThanOrEqual,
            ..
        }
    ));
}

// ---------------------------------------------------------------------------
// Logical (short-circuit) operators �?must emit Logical, not Binary
// ---------------------------------------------------------------------------

/// `a && b` �?Logical { And }.
#[test]
fn logical_and_produces_logical_node_not_binary() {
    let tokens = vec![ident("a"), op("&&"), ident("b"), punc(';'), eof()];
    let expr = parse_expr(tokens);
    assert!(
        matches!(
            expr,
            Expression::Logical {
                operator: LogicalOperator::And,
                ..
            }
        ),
        "expected Logical(And), got {expr:?}"
    );
}

/// `a || b` �?Logical { Or }.
#[test]
fn logical_or_produces_logical_node_not_binary() {
    let tokens = vec![ident("a"), op("||"), ident("b"), punc(';'), eof()];
    let expr = parse_expr(tokens);
    assert!(
        matches!(
            expr,
            Expression::Logical {
                operator: LogicalOperator::Or,
                ..
            }
        ),
        "expected Logical(Or), got {expr:?}"
    );
}

/// `a || b && c` �?`&&` binds tighter than `||` so the top node must be `||`.
#[test]
fn and_binds_tighter_than_or() {
    let tokens = vec![
        ident("a"),
        op("||"),
        ident("b"),
        op("&&"),
        ident("c"),
        punc(';'),
        eof(),
    ];
    let expr = parse_expr(tokens);
    let Expression::Logical {
        operator, right, ..
    } = expr
    else {
        panic!("expected top-level Logical, got {expr:?}");
    };
    assert_eq!(operator, LogicalOperator::Or);
    assert!(matches!(
        *right,
        Expression::Logical {
            operator: LogicalOperator::And,
            ..
        }
    ));
}

// ---------------------------------------------------------------------------
// Unary operators
// ---------------------------------------------------------------------------

/// `- 5` �?Unary { Minus, Number(5) }.
#[test]
fn unary_minus_wraps_operand() {
    let tokens = vec![op("-"), num(5.0), punc(';'), eof()];
    let expr = parse_expr(tokens);
    assert_eq!(
        expr,
        Expression::Unary {
            operator: UnaryOperator::Minus,
            argument: Box::new(Expression::Literal(Literal::Number(5.0))),
        }
    );
}

/// `+ ""` �?Unary { Plus, String("") }.
#[test]
fn unary_plus_on_empty_string() {
    let tokens = vec![
        op("+"),
        tok(TokenKind::String(String::new())),
        punc(';'),
        eof(),
    ];
    let expr = parse_expr(tokens);
    assert_eq!(
        expr,
        Expression::Unary {
            operator: UnaryOperator::Plus,
            argument: Box::new(Expression::Literal(Literal::String(String::new()))),
        }
    );
}

/// `! true` �?Unary { Not }.
#[test]
fn logical_not_wraps_boolean() {
    let tokens = vec![op("!"), kw(Keyword::True), punc(';'), eof()];
    let expr = parse_expr(tokens);
    assert!(matches!(
        expr,
        Expression::Unary {
            operator: UnaryOperator::Not,
            ..
        }
    ));
}

// ---------------------------------------------------------------------------
// Member access and function call
// ---------------------------------------------------------------------------

/// `a . b` �?Member { computed: false }.
#[test]
fn member_access_with_dot() {
    let tokens = vec![ident("a"), punc('.'), ident("b"), punc(';'), eof()];
    let expr = parse_expr(tokens);
    assert_eq!(
        expr,
        Expression::Member {
            object: Box::new(Expression::Identifier("a".into())),
            property: Box::new(Expression::Identifier("b".into())),
            computed: false,
        }
    );
}

/// `f ( )` �?Call with zero arguments.
#[test]
fn call_with_no_arguments() {
    let tokens = vec![ident("f"), punc('('), punc(')'), punc(';'), eof()];
    let expr = parse_expr(tokens);
    assert!(matches!(
        expr,
        Expression::Call { arguments, .. } if arguments.is_empty()
    ));
}

/// `assert . sameValue ( x , 1 )` �?Call with two arguments.
#[test]
fn call_with_two_arguments() {
    let tokens = vec![
        ident("assert"),
        punc('.'),
        ident("sameValue"),
        punc('('),
        ident("x"),
        punc(','),
        num(1.0),
        punc(')'),
        punc(';'),
        eof(),
    ];
    let expr = parse_expr(tokens);
    let Expression::Call { callee, arguments } = expr else {
        panic!("expected Call, got {expr:?}");
    };
    assert_eq!(arguments.len(), 2);
    assert!(matches!(
        *callee,
        Expression::Member {
            computed: false,
            ..
        }
    ));
}

// ---------------------------------------------------------------------------
// Assignment
// ---------------------------------------------------------------------------

/// `x = 1` �?identifier is a valid assignment target.
#[test]
fn assignment_to_identifier() {
    let tokens = vec![ident("x"), op("="), num(1.0), punc(';'), eof()];
    let expr = parse_expr(tokens);
    assert!(matches!(expr, Expression::Assignment { .. }));
}

/// `1 = 2` �?number literal is not a valid assignment target.
#[test]
fn assignment_to_literal_is_a_parse_error() {
    let tokens = vec![num(1.0), op("="), num(2.0), punc(';'), eof()];
    let err = parse_err(tokens);
    assert!(
        err.message.contains("assignment target"),
        "unexpected message: {}",
        err.message
    );
}

// ---------------------------------------------------------------------------
// Variable declaration
// ---------------------------------------------------------------------------

/// `var x ;` �?VariableDeclaration with no initializer.
#[test]
fn var_without_initializer() {
    let tokens = vec![kw(Keyword::Var), ident("x"), punc(';'), eof()];
    assert_eq!(
        parse_stmt(tokens),
        Statement::VariableDeclaration {
            kind: VariableKind::Var,
            declarations: vec![VariableDeclarator {
                name: "x".into(),
                pattern: None,
                initializer: None,
            }],
        }
    );
}

/// `var y = 42 ;` �?VariableDeclaration with number initializer.
#[test]
fn var_with_number_initializer() {
    let tokens = vec![
        kw(Keyword::Var),
        ident("y"),
        op("="),
        num(42.0),
        punc(';'),
        eof(),
    ];
    assert_eq!(
        parse_stmt(tokens),
        Statement::VariableDeclaration {
            kind: VariableKind::Var,
            declarations: vec![VariableDeclarator {
                name: "y".into(),
                pattern: None,
                initializer: Some(Expression::Literal(Literal::Number(42.0))),
            }],
        }
    );
}

/// `var a , b = 1 ;` �?two declarators, only the second initialized.
#[test]
fn var_with_multiple_declarators() {
    let tokens = vec![
        kw(Keyword::Var),
        ident("a"),
        punc(','),
        ident("b"),
        op("="),
        num(1.0),
        punc(';'),
        eof(),
    ];
    assert_eq!(
        parse_stmt(tokens),
        Statement::VariableDeclaration {
            kind: VariableKind::Var,
            declarations: vec![
                VariableDeclarator {
                    name: "a".into(),
                    pattern: None,
                    initializer: None,
                },
                VariableDeclarator {
                    name: "b".into(),
                    pattern: None,
                    initializer: Some(Expression::Literal(Literal::Number(1.0))),
                },
            ],
        }
    );
}

// ---------------------------------------------------------------------------
// V2 control flow
// ---------------------------------------------------------------------------

/// `{ ; }` �?an empty block holding one empty statement.
#[test]
fn block_groups_statements() {
    let tokens = vec![punc('{'), punc(';'), punc('}'), eof()];
    assert_eq!(parse_stmt(tokens), Statement::Block(vec![Statement::Empty]));
}

/// `if ( 1 ) 2 ; else 3 ;` �?if/else with both branches present.
#[test]
fn if_else_parses_both_branches() {
    let tokens = vec![
        kw(Keyword::If),
        punc('('),
        num(1.0),
        punc(')'),
        num(2.0),
        punc(';'),
        kw(Keyword::Else),
        num(3.0),
        punc(';'),
        eof(),
    ];
    assert_eq!(
        parse_stmt(tokens),
        Statement::If {
            test: Expression::Literal(Literal::Number(1.0)),
            consequent: Box::new(Statement::Expression(Expression::Literal(Literal::Number(
                2.0
            )))),
            alternate: Some(Box::new(Statement::Expression(Expression::Literal(
                Literal::Number(3.0)
            )))),
        }
    );
}

/// `if ( 1 ) if ( 2 ) 3 ; else 4 ;` �?the `else` binds to the inner `if`.
#[test]
fn dangling_else_binds_to_innermost_if() {
    let tokens = vec![
        kw(Keyword::If),
        punc('('),
        num(1.0),
        punc(')'),
        kw(Keyword::If),
        punc('('),
        num(2.0),
        punc(')'),
        num(3.0),
        punc(';'),
        kw(Keyword::Else),
        num(4.0),
        punc(';'),
        eof(),
    ];
    let Statement::If {
        consequent,
        alternate,
        ..
    } = parse_stmt(tokens)
    else {
        panic!("expected an if statement");
    };
    assert!(alternate.is_none(), "outer if must not own the else");
    assert!(matches!(
        consequent.as_ref(),
        Statement::If {
            alternate: Some(_),
            ..
        }
    ));
}

/// `while ( 1 ) { break ; }` �?break is legal inside the loop body.
#[test]
fn while_body_allows_break() {
    let tokens = vec![
        kw(Keyword::While),
        punc('('),
        num(1.0),
        punc(')'),
        punc('{'),
        kw(Keyword::Break),
        punc(';'),
        punc('}'),
        eof(),
    ];
    assert_eq!(
        parse_stmt(tokens),
        Statement::While {
            test: Expression::Literal(Literal::Number(1.0)),
            body: Box::new(Statement::Block(vec![Statement::Break])),
        }
    );
}

/// `break ;` at the top level is a parse error.
#[test]
fn top_level_break_is_a_parse_error() {
    let tokens = vec![kw(Keyword::Break), punc(';'), eof()];
    assert!(parse_err(tokens).message.contains("break"));
}

/// `continue ;` at the top level is a parse error.
#[test]
fn top_level_continue_is_a_parse_error() {
    let tokens = vec![kw(Keyword::Continue), punc(';'), eof()];
    assert!(parse_err(tokens).message.contains("continue"));
}

/// `throw 1 ;` �?a throw statement carrying the literal.
#[test]
fn throw_carries_its_expression() {
    let tokens = vec![kw(Keyword::Throw), num(1.0), punc(';'), eof()];
    assert_eq!(
        parse_stmt(tokens),
        Statement::Throw(Expression::Literal(Literal::Number(1.0)))
    );
}

/// A line terminator between `throw` and its operand is a parse error, even
/// though the same tokens without the newline are valid.
#[test]
fn newline_after_throw_is_a_parse_error() {
    let tokens = vec![
        kw(Keyword::Throw),
        tok_nl(TokenKind::Number(1.0)),
        punc(';'),
        eof(),
    ];
    assert!(parse_err(tokens).message.contains("throw"));
}

/// `a ? b : c` �?a conditional expression.
#[test]
fn conditional_expression_builds_conditional_node() {
    let tokens = vec![
        ident("a"),
        punc('?'),
        ident("b"),
        punc(':'),
        ident("c"),
        punc(';'),
        eof(),
    ];
    assert_eq!(
        parse_expr(tokens),
        Expression::Conditional {
            test: Box::new(Expression::Identifier("a".into())),
            consequent: Box::new(Expression::Identifier("b".into())),
            alternate: Box::new(Expression::Identifier("c".into())),
        }
    );
}

/// `typeof x` �?a unary `typeof` expression.
#[test]
fn typeof_builds_unary_node() {
    let tokens = vec![kw(Keyword::TypeOf), ident("x"), punc(';'), eof()];
    assert_eq!(
        parse_expr(tokens),
        Expression::Unary {
            operator: UnaryOperator::TypeOf,
            argument: Box::new(Expression::Identifier("x".into())),
        }
    );
}

/// `new E ( 1 )` �?a construct expression with one argument.
#[test]
fn new_builds_construct_node() {
    let tokens = vec![
        kw(Keyword::New),
        ident("E"),
        punc('('),
        num(1.0),
        punc(')'),
        punc(';'),
        eof(),
    ];
    assert_eq!(
        parse_expr(tokens),
        Expression::Construct {
            callee: Box::new(Expression::Identifier("E".into())),
            arguments: vec![crate::ast::CallArgument::Expression(Expression::Literal(
                Literal::Number(1.0)
            ))],
        }
    );
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

/// `1 + EOF` �?missing right operand produces ParseError.
#[test]
fn missing_right_operand_is_a_parse_error() {
    let tokens = vec![num(1.0), op("+"), eof()];
    parse_err(tokens);
}

/// `; ; ;` �?multiple empty statements parse cleanly.
#[test]
fn multiple_empty_statements() {
    let tokens = vec![punc(';'), punc(';'), punc(';'), eof()];
    let program = Parser::new(tokens).parse_program().unwrap();
    assert_eq!(program.body.len(), 3);
    assert!(program.body.iter().all(|s| *s == Statement::Empty));
}
