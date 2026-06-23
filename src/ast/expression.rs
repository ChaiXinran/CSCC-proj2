//! Expression nodes.

use super::statement::Statement;

/// Literal values represented directly in the AST.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
    /// `/pattern/flags` regex literal.
    RegExp {
        pattern: String,
        flags: String,
    },
}

/// Unary expression operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Plus,
    Minus,
    Not,
    TypeOf,
    Void,
    Delete,
}

/// `++`/`--` update operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOperator {
    Increment,
    Decrement,
}

/// Compound assignment operators such as `+=` and `*=`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
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
    /// Abstract inequality (`!=`).
    NotEqual,
    StrictEqual,
    StrictNotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LogicalAnd,
    LogicalOr,
    In,
    InstanceOf,
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

// ---------------------------------------------------------------------------
// V3 function-related AST types
// ---------------------------------------------------------------------------

/// One formal parameter in a function definition.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionParam {
    pub name: String,
}

/// The body of a function: a list of statements.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionBody {
    pub statements: Vec<Statement>,
}

/// A function value: either a declaration or an expression.
///
/// `name` is `None` for anonymous function expressions; non-empty for
/// declarations and named expressions.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionLiteral {
    pub name: Option<String>,
    pub params: Vec<FunctionParam>,
    pub body: FunctionBody,
}

// ---------------------------------------------------------------------------
// V3/V4 object literal types
// ---------------------------------------------------------------------------

/// One property in an object literal. V4 extends V3's plain data property with
/// getter, setter, and `__proto__` setter forms.
#[derive(Debug, Clone, PartialEq)]
pub enum ObjectProperty {
    /// `key: value` — a plain data property.
    Data {
        key: PropertyName,
        value: Expression,
    },
    /// `[key]: value` — a data property whose key is evaluated at runtime.
    ComputedData { key: Expression, value: Expression },
    /// `get key() { body }` — an accessor getter (0 parameters).
    Getter {
        key: PropertyName,
        body: FunctionBody,
    },
    /// `set key(param) { body }` — an accessor setter (exactly 1 parameter).
    Setter {
        key: PropertyName,
        parameter: FunctionParam,
        body: FunctionBody,
    },
    /// `__proto__: value` — sets the object's prototype. At most one per literal.
    PrototypeSetter { value: Expression },
}

/// The key of an object property. V3 supports identifier, string, and number
/// keys; computed keys (`[expr]: value`) are deferred to a later milestone.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyName {
    Identifier(String),
    String(String),
    Number(f64),
}

impl PropertyName {
    /// Converts the property name to the string key used in the object's
    /// property table.
    pub fn to_key_string(&self) -> String {
        match self {
            Self::Identifier(name) | Self::String(name) => name.clone(),
            Self::Number(n) => {
                if n.fract() == 0.0 && n.is_finite() {
                    format!("{}", *n as i64)
                } else {
                    format!("{n}")
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// V4 array element
// ---------------------------------------------------------------------------

/// One element slot in an array literal. V4 supports sparse arrays via holes.
///
/// Trailing comma rule: `[1,]` has length 1; `[1,,]` has length 2 (one hole).
#[derive(Debug, Clone, PartialEq)]
pub enum ArrayElement {
    /// An elision (empty slot between commas): `[1, , 3]`.
    Hole,
    /// A concrete expression that evaluates to the element value.
    Expression(Expression),
}

// ---------------------------------------------------------------------------
// Expression enum
// ---------------------------------------------------------------------------

/// Expression subset implemented incrementally by AgentJS.
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Literal(Literal),
    Identifier(String),
    Unary {
        operator: UnaryOperator,
        argument: Box<Expression>,
    },
    /// `++x` / `x++` / `--x` / `x--`. `prefix` distinguishes the result value.
    Update {
        operator: UpdateOperator,
        prefix: bool,
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
    CompoundAssignment {
        operator: AssignmentOperator,
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
    /// `new callee(arguments)`.
    Construct {
        callee: Box<Expression>,
        arguments: Vec<Expression>,
    },
    /// `[element, ...]` array literal, potentially sparse.
    Array(Vec<ArrayElement>),
    /// `{ key: value, ... }` object literal.
    Object(Vec<ObjectProperty>),
    /// Function expression or named function expression.
    Function(FunctionLiteral),
}
