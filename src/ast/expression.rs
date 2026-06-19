//! Expression nodes.

/// Literal values represented directly in the AST.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
}

/// Unary expression operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Plus,
    Minus,
    Not,
    TypeOf,
}

/// Binary expression operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Equal,
    StrictEqual,
    LessThan,
    GreaterThan,
}

/// Expression subset implemented incrementally by AgentJS.
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Literal(Literal),
    Identifier(String),
    Unary {
        operator: UnaryOperator,
        argument: Box<Expression>,
    },
    Binary {
        operator: BinaryOperator,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Assignment {
        target: Box<Expression>,
        value: Box<Expression>,
    },
    Call {
        callee: Box<Expression>,
        arguments: Vec<Expression>,
    },
    Member {
        object: Box<Expression>,
        property: Box<Expression>,
        computed: bool,
    },
    Array(Vec<Expression>),
    Object(Vec<(String, Expression)>),
}
