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
    /// Abstract equality (`==`), intentionally outside Native V1.
    Equal,
    StrictEqual,
    StrictNotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LogicalAnd,
    LogicalOr,
}

/// Short-circuiting logical operators.
///
/// These are kept separate from [`BinaryOperator`] because the right operand is
/// only evaluated conditionally; the compiler lowers them to jumps rather than a
/// single binary instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOperator {
    And,
    Or,
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
    Logical {
        operator: LogicalOperator,
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
    /// `test ? consequent : alternate`, right associative. Kept distinct from
    /// [`Expression::Logical`] because the compiler lowers it to a balanced
    /// branch that leaves exactly one value on every path.
    Conditional {
        test: Box<Expression>,
        consequent: Box<Expression>,
        alternate: Box<Expression>,
    },
    /// `new callee(arguments)`. V2 only constructs the minimal `Test262Error`,
    /// but the AST does not hard-code the callee name.
    Construct {
        callee: Box<Expression>,
        arguments: Vec<Expression>,
    },
    Array(Vec<Expression>),
    Object(Vec<(String, Expression)>),
}
