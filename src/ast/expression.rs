//! Expression nodes.

use super::statement::Statement;

/// Literal values represented directly in the AST.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    BigInt(String),
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
    BitwiseNot,
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
    Exponentiation,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    LeftShift,
    RightShift,
    UnsignedRightShift,
    LogicalAnd,
    LogicalOr,
    NullishCoalescing,
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
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    LeftShift,
    RightShift,
    UnsignedRightShift,
    Exponentiation,
    NullishCoalescing,
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
    Nullish,
}

// ---------------------------------------------------------------------------
// V3 function-related AST types
// ---------------------------------------------------------------------------

/// One formal parameter in a function definition.
#[derive(Debug, Clone, PartialEq)]
pub enum FunctionParam {
    /// A simple identifier parameter: `function f(x)`.
    Simple(String),
    /// A simple parameter with a default value: `function f(x = 1)`.
    Default(String, Box<Expression>),
    /// A destructuring parameter with an optional default: `function f({a, b})`.
    Pattern(BindingPattern, Option<Box<Expression>>),
    /// A rest parameter collecting remaining arguments into a named array: `function f(...rest)`.
    Rest(String),
    /// A rest parameter with a destructuring pattern: `function f(...[a, b])`.
    RestPattern(BindingPattern),
}

impl FunctionParam {
    /// Returns the binding name for simple/rest/default parameters.
    /// Returns `""` for pattern-based parameters.
    pub fn name(&self) -> &str {
        match self {
            Self::Simple(name) | Self::Rest(name) | Self::Default(name, _) => name.as_str(),
            Self::Pattern(..) | Self::RestPattern(..) => "",
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
    /// `true` for arrow functions (`=>`). Arrow functions do not have their own
    /// `arguments` binding or `this`, unlike regular function expressions.
    pub is_arrow: bool,
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
    /// `...expr` — spread element in an object literal.
    Spread(Expression),
}

/// The key of an object property or class member.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyName {
    Identifier(String),
    String(String),
    Number(f64),
    /// `#name` — private class field/method identifier.
    PrivateName(String),
    /// `[expr]` — computed class member key (not valid in plain object literals).
    Computed(Box<Expression>),
}

/// Convert a numeric property key to the ECMAScript canonical string (ToString algorithm).
fn js_number_to_property_key(n: f64) -> String {
    if n.fract() == 0.0 && n.is_finite() && n >= 0.0 && n < 9.007_199_254_740_992e15 {
        return format!("{}", n as i64);
    }
    if n.is_nan() {
        return "NaN".into();
    }
    if n.is_infinite() {
        return if n > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    let magnitude = n.abs();
    if !(1e-6..1e21).contains(&magnitude) {
        // Scientific notation.
        let sign = if n.is_sign_negative() { "-" } else { "" };
        let raw = format!("{:e}", n.abs());
        if let Some((mantissa, exp)) = raw.split_once('e') {
            let exp_i = exp.parse::<i32>().unwrap_or(0);
            return format!("{sign}{mantissa}e{exp_i:+}");
        }
    }
    n.to_string()
}

impl PropertyName {
    /// Converts the property name to the string key used in the object's
    /// property table.
    pub fn to_key_string(&self) -> String {
        match self {
            Self::Identifier(name) | Self::String(name) => name.clone(),
            Self::PrivateName(name) => format!("#{name}"),
            Self::Number(n) => js_number_to_property_key(*n),
            Self::Computed(_) => "__computed__".into(),
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
        is_getter: bool,
        is_setter: bool,
    },
    /// Instance or static field declaration: `[static] name [= expr]`.
    Field {
        name: PropertyName,
        is_static: bool,
        initializer: Option<Box<Expression>>,
    },
    /// `static { ... }` class initialization block.
    StaticBlock(Vec<Statement>),
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

/// One element in an array binding pattern, pairing a sub-pattern with an
/// optional initialiser: `[a = 1]`.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayBindingElement {
    pub pattern: BindingPattern,
    pub default: Option<Box<Expression>>,
}

/// The key of a property in an object binding pattern.
#[derive(Debug, Clone, PartialEq)]
pub enum ObjectBindingKey {
    /// A static property name (identifier, string, or number key).
    Static(PropertyName),
    /// A computed property key: `[expr]`.
    Computed(Box<Expression>),
}

/// One property binding in an object binding pattern.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectBindingProp {
    pub key: ObjectBindingKey,
    pub value: BindingPattern,
    pub default: Option<Box<Expression>>,
}

/// A binding pattern used in variable declarations and parameter lists.
#[derive(Debug, Clone, PartialEq)]
pub enum BindingPattern {
    /// Plain identifier binding: `x`.
    Identifier(String),
    /// Array destructuring: `[a, b, , c, ...rest]`.
    /// `elements` entries are `None` for holes; `rest` is an optional trailing rest.
    Array {
        elements: Vec<Option<ArrayBindingElement>>,
        rest: Option<Box<BindingPattern>>,
    },
    /// Object destructuring: `{ x, y: z, [k]: w, ...rest }`.
    Object {
        props: Vec<ObjectBindingProp>,
        rest: Option<Box<BindingPattern>>,
    },
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
    /// `import(specifier[, options])` dynamic import expression.
    DynamicImport {
        specifier: Box<Expression>,
        options: Option<Box<Expression>>,
    },
    /// `new.target` — the constructor or function that was invoked with `new`.
    /// Returns `undefined` in regular calls; the constructor function in `new` calls.
    NewTarget,
    /// `import.meta` — the module meta-object. Only valid inside module code.
    ImportMeta,
    /// `#name` used as a member-access property (e.g. `this.#x`, `obj.#method()`).
    PrivateName(String),
    /// Comma operator: evaluates each expression and returns the last.
    Sequence(Vec<Expression>),
    /// `base?.step…` — optional chaining expression.
    ///
    /// If any step marked `optional: true` is applied to `null` or `undefined`,
    /// the entire chain short-circuits to `undefined`.
    OptionalChain {
        base: Box<Expression>,
        steps: Vec<OptionalChainStep>,
    },
}

/// One step inside an optional chain expression.
#[derive(Debug, Clone, PartialEq)]
pub enum OptionalChainStep {
    /// `?.prop`, `?.[key]`, `.prop`, or `[key]` within a chain.
    Member {
        property: Box<Expression>,
        computed: bool,
        /// `true` for `?.`, `false` for `.` / `[]` following a previous `?.`.
        optional: bool,
    },
    /// `?.()` or `()` within a chain.
    Call {
        arguments: Vec<CallArgument>,
        optional: bool,
    },
}

/// One argument in a call or construct expression.
#[derive(Debug, Clone, PartialEq)]
pub enum CallArgument {
    /// A regular expression argument.
    Expression(Expression),
    /// A spread argument: `...expr`.
    Spread(Expression),
}
