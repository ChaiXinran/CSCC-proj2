//! Token-driven parser unit tests.
//!
//! Every test here constructs a `Vec<Token>` by hand and feeds it directly to
//! `Parser::new`, bypassing the lexer entirely.  This makes the parser's
//! behaviour observable in isolation: a failing test here means the parser is
//! wrong, never the lexer.

use crate::{
    ast::{
        BinaryOperator, Expression, Literal, LogicalOperator, Statement, UnaryOperator,
        VariableKind,
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

/// `1 + 2 * 3` must produce `Add(1, Mul(2, 3))` — multiplication binds tighter.
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

/// `18 / 2 / 3` must produce `Div(Div(18, 2), 3)` — left associativity.
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

/// `1 === 2` → Binary { StrictEqual }.
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

/// `1 !== 2` → Binary { StrictNotEqual }.
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

/// `1 <= 2` → Binary { LessThanOrEqual }.
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

/// `1 >= 2` → Binary { GreaterThanOrEqual }.
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
// Logical (short-circuit) operators — must emit Logical, not Binary
// ---------------------------------------------------------------------------

/// `a && b` → Logical { And }.
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

/// `a || b` → Logical { Or }.
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

/// `a || b && c` — `&&` binds tighter than `||` so the top node must be `||`.
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

/// `- 5` → Unary { Minus, Number(5) }.
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

/// `+ ""` → Unary { Plus, String("") }.
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

/// `! true` → Unary { Not }.
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

/// `a . b` → Member { computed: false }.
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

/// `f ( )` → Call with zero arguments.
#[test]
fn call_with_no_arguments() {
    let tokens = vec![ident("f"), punc('('), punc(')'), punc(';'), eof()];
    let expr = parse_expr(tokens);
    assert!(matches!(
        expr,
        Expression::Call { arguments, .. } if arguments.is_empty()
    ));
}

/// `assert . sameValue ( x , 1 )` → Call with two arguments.
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

/// `x = 1` — identifier is a valid assignment target.
#[test]
fn assignment_to_identifier() {
    let tokens = vec![ident("x"), op("="), num(1.0), punc(';'), eof()];
    let expr = parse_expr(tokens);
    assert!(matches!(expr, Expression::Assignment { .. }));
}

/// `1 = 2` — number literal is not a valid assignment target.
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

/// `var x ;` → VariableDeclaration with no initializer.
#[test]
fn var_without_initializer() {
    let tokens = vec![kw(Keyword::Var), ident("x"), punc(';'), eof()];
    assert_eq!(
        parse_stmt(tokens),
        Statement::VariableDeclaration {
            kind: VariableKind::Var,
            name: "x".into(),
            initializer: None,
        }
    );
}

/// `var y = 42 ;` → VariableDeclaration with number initializer.
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
            name: "y".into(),
            initializer: Some(Expression::Literal(Literal::Number(42.0))),
        }
    );
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

/// `1 + EOF` — missing right operand produces ParseError.
#[test]
fn missing_right_operand_is_a_parse_error() {
    let tokens = vec![num(1.0), op("+"), eof()];
    parse_err(tokens);
}

/// `; ; ;` — multiple empty statements parse cleanly.
#[test]
fn multiple_empty_statements() {
    let tokens = vec![punc(';'), punc(';'), punc(';'), eof()];
    let program = Parser::new(tokens).parse_program().unwrap();
    assert_eq!(program.body.len(), 3);
    assert!(program.body.iter().all(|s| *s == Statement::Empty));
}
