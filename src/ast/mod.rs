//! Backend-independent abstract syntax tree.

mod expression;
mod statement;

pub use expression::{
    ArrayElement, AssignmentOperator, BinaryOperator, Expression, FunctionBody, FunctionLiteral,
    FunctionParam, Literal, LogicalOperator, ObjectProperty, PropertyName, UnaryOperator,
    UpdateOperator,
};
pub use statement::{
    CatchClause, Program, Statement, SwitchCase, VariableDeclarator, VariableKind,
};
