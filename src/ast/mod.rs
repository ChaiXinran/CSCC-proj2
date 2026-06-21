//! Backend-independent abstract syntax tree.

mod expression;
mod statement;

pub use expression::{
    ArrayElement, BinaryOperator, Expression, FunctionBody, FunctionLiteral, FunctionParam,
    Literal, LogicalOperator, ObjectProperty, PropertyName, UnaryOperator,
};
pub use statement::{
    CatchClause, Program, Statement, SwitchCase, VariableDeclarator, VariableKind,
};
