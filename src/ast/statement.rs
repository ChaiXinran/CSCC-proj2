//! Program and statement nodes.

use super::expression::{
    BindingPattern, ClassDeclaration, Expression, FunctionBody, FunctionParam,
};

/// Complete script AST.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Program {
    pub body: Vec<Statement>,
}

/// Variable declaration category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableKind {
    Var,
    Let,
    Const,
}

/// One binding inside a variable declaration, e.g. the `b = 1` in
/// `var a, b = 1;`.
#[derive(Debug, Clone, PartialEq)]
pub struct VariableDeclarator {
    pub name: String,
    pub initializer: Option<Expression>,
}

/// One `catch` clause. V5 Core supports an optional identifier binding.
#[derive(Debug, Clone, PartialEq)]
pub struct CatchClause {
    pub parameter: Option<String>,
    pub body: Vec<Statement>,
}

/// One clause in a `switch` statement. `None` denotes `default`.
#[derive(Debug, Clone, PartialEq)]
pub struct SwitchCase {
    pub test: Option<Expression>,
    pub consequent: Vec<Statement>,
}

/// Statement subset implemented incrementally by AgentJS.
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Empty,
    Expression(Expression),
    Block(Vec<Statement>),
    VariableDeclaration {
        kind: VariableKind,
        /// Always holds at least one declarator; the parser never produces an
        /// empty list.
        declarations: Vec<VariableDeclarator>,
    },
    /// `function name(params) { body }` — hoisted function declaration.
    FunctionDeclaration {
        name: String,
        params: Vec<FunctionParam>,
        body: FunctionBody,
    },
    Return(Option<Expression>),
    If {
        test: Expression,
        consequent: Box<Statement>,
        alternate: Option<Box<Statement>>,
    },
    While {
        test: Expression,
        body: Box<Statement>,
    },
    Break,
    Continue,
    Throw(Expression),
    Try {
        block: Vec<Statement>,
        handler: Option<CatchClause>,
        finalizer: Option<Vec<Statement>>,
    },
    Switch {
        discriminant: Expression,
        cases: Vec<SwitchCase>,
    },
    /// C-style `for (init; test; update) body`. Each clause is optional.
    /// `init` is a variable declaration or an expression statement.
    For {
        init: Option<Box<Statement>>,
        test: Option<Expression>,
        update: Option<Expression>,
        body: Box<Statement>,
    },
    /// `for (left in right) body`. `declaration` is `Some` for
    /// `for (var/let/const x in obj)`, `None` when `target` is an existing
    /// assignment target (`for (x in obj)`).
    ForIn {
        declaration: Option<VariableKind>,
        target: Expression,
        right: Expression,
        body: Box<Statement>,
    },
    /// `class Name [extends Super] { ... }` — class declaration.
    ClassDeclaration(ClassDeclaration),
    /// `var/let/const [a, b] = expr` or `var/let/const { x, y } = expr`.
    DestructuringDeclaration {
        kind: VariableKind,
        pattern: BindingPattern,
        initializer: Expression,
    },
}
