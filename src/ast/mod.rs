//! Backend-independent abstract syntax tree.

mod expression;
mod statement;

pub use expression::{
    BinaryOperator, Expression, FunctionBody, FunctionLiteral, FunctionParam, Literal,
    LogicalOperator, ObjectProperty, PropertyName, UnaryOperator,
};
pub use statement::{Program, Statement, VariableDeclarator, VariableKind};
