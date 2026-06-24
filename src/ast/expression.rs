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
pub enum FunctionParam {
    /// A simple identifier parameter: `function f(x)`.
    Simple(String),
    /// A rest parameter collecting remaining arguments: `function f(...rest)`.
    Rest(String),
}

impl FunctionParam {
    /// Returns the binding name for this parameter.
    pub fn name(&self) -> &str {
        match self {
            Self::Simple(name) | Self::Rest(name) => name.as_str(),
        }
    }
}

/// The body of a function: a list of statements.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionBody {
    pub statements: Vec<Statement>,
    pub is_strict: bool,
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
    /// `async function` — body may contain `await` expressions.
    pub is_async: bool,
    /// `function*` — body may contain `yield` expressions.
    pub is_generator: bool,
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

/// One element slot in an array literal.
#[derive(Debug, Clone, PartialEq)]
pub enum ArrayElement {
    /// An elision (empty slot between commas): `[1, , 3]`.
    Hole,
    /// A concrete expression that evaluates to the element value.
    Expression(Expression),
    /// A spread element that expands an iterable: `[a, ...b]`.
    Spread(Expression),
}

// ---------------------------------------------------------------------------
// V8 template literal
// ---------------------------------------------------------------------------

/// An untagged template literal with zero or more substitutions.
///
/// `quasis.len() == expressions.len() + 1` always holds.
/// The cooked string text alternates with expressions:
/// quasis[0] + toString(expressions[0]) + quasis[1] + ...
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateLiteral {
    pub quasis: Vec<String>,
    pub expressions: Vec<Expression>,
}

// ---------------------------------------------------------------------------
// V8 class nodes
// ---------------------------------------------------------------------------

/// One element of a class body.
#[derive(Debug, Clone, PartialEq)]
pub enum ClassElement {
    Constructor(FunctionLiteral),
    Method {
        name: PropertyName,
        function: FunctionLiteral,
        is_static: bool,
    },
}

/// A class expression: `class [Name] [extends Super] { ... }`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassExpression {
    pub name: Option<String>,
    pub super_class: Option<Box<Expression>>,
    pub elements: Vec<ClassElement>,
}

/// A class declaration: `class Name [extends Super] { ... }`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassDeclaration {
    pub name: String,
    pub super_class: Option<Expression>,
    pub elements: Vec<ClassElement>,
}

// ---------------------------------------------------------------------------
// V8 binding patterns (destructuring)
// ---------------------------------------------------------------------------

/// A binding pattern used in variable declarations and parameter lists.
#[derive(Debug, Clone, PartialEq)]
pub enum BindingPattern {
    /// Plain identifier binding: `x`.
    Identifier(String),
    /// Array destructuring: `[a, b, , c]`. `None` entries are holes (elisions).
    Array(Vec<Option<BindingPattern>>),
    /// Object destructuring: `{ x, y: z }`. Each entry is (key, local_pattern).
    Object(Vec<(PropertyName, BindingPattern)>),
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
        arguments: Vec<CallArgument>,
    },
    Member {
        object: Box<Expression>,
        property: Box<Expression>,
        computed: bool,
    },
    /// `test ? consequent : alternate`, right associative.
    Conditional {
        test: Box<Expression>,
        consequent: Box<Expression>,
        alternate: Box<Expression>,
    },
    /// `new callee(arguments)`.
    Construct {
        callee: Box<Expression>,
        arguments: Vec<CallArgument>,
    },
    /// `[element, ...]` array literal, potentially sparse.
    Array(Vec<ArrayElement>),
    /// `{ key: value, ... }` object literal.
    Object(Vec<ObjectProperty>),
    /// Function expression or named function expression.
    Function(FunctionLiteral),
    /// Untagged template literal: `` `hello ${name}` ``.
    TemplateLiteral(TemplateLiteral),
    /// Spread expression used inside call arg lists: `...expr`.
    Spread(Box<Expression>),
    /// Class expression: `class [Name] { ... }`.
    Class(ClassExpression),
    /// `this` keyword.
    This,
    /// `super` keyword (as a base for property access in a method).
    Super,
    /// `yield [*] [expr]` inside a generator function.
    Yield {
        argument: Option<Box<Expression>>,
        /// `true` for `yield*` (delegate to another iterable).
        delegate: bool,
    },
    /// `await expr` inside an async function.
    Await(Box<Expression>),
}

/// One argument in a call or construct expression.
#[derive(Debug, Clone, PartialEq)]
pub enum CallArgument {
    /// A regular expression argument.
    Expression(Expression),
    /// A spread argument: `...expr`.
    Spread(Expression),
}
