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

/// Statement subset implemented incrementally by AgentJS.
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Empty,
    Expression(Expression),
    Block(Vec<Statement>),
    VariableDeclaration {
        kind: VariableKind,
        name: String,
        initializer: Option<Expression>,
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
