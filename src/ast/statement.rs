//! Program and statement nodes.

use super::Expression;

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
