//! Program and statement nodes.

use super::expression::{Expression, FunctionBody, FunctionParam};

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
}
