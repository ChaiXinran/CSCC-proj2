//! Backend-independent abstract syntax tree.

mod expression;
mod statement;

pub use expression::{
<<<<<<< HEAD
    ArrayElement, BinaryOperator, Expression, FunctionBody, FunctionLiteral, FunctionParam,
    Literal, LogicalOperator, ObjectProperty, PropertyName, UnaryOperator,
=======
    ArrayElement, AssignmentOperator, BinaryOperator, Expression, FunctionBody, FunctionLiteral,
    FunctionParam, Literal, LogicalOperator, ObjectProperty, PropertyName, UnaryOperator,
    UpdateOperator,
>>>>>>> ebc5479 (Bug fix phase 1 trail C finished.)
};
pub use statement::{
    CatchClause, Program, Statement, SwitchCase, VariableDeclarator, VariableKind,
};
