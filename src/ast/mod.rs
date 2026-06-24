//! Backend-independent abstract syntax tree.

mod expression;
mod statement;

pub use expression::{
    ArrayElement, AssignmentOperator, BinaryOperator, BindingPattern, CallArgument,
    ClassDeclaration, ClassElement, ClassExpression, Expression, FunctionBody, FunctionLiteral,
    FunctionParam, Literal, LogicalOperator, ObjectProperty, PropertyName, TemplateLiteral,
    UnaryOperator, UpdateOperator,
};
pub use statement::{
    CatchClause, ForBinding, Program, Statement, SwitchCase, VariableDeclarator, VariableKind,
};
