//! Backend-independent abstract syntax tree.

mod expression;
mod statement;

pub use expression::{
    ArrayBindingElement, ArrayElement, AssignmentOperator, BinaryOperator, BindingPattern,
    CallArgument, ClassDeclaration, ClassElement, ClassExpression, Expression, FunctionBody,
    FunctionLiteral, FunctionParam, Literal, LogicalOperator, ObjectBindingKey, ObjectBindingProp,
    ObjectProperty, PropertyName, TemplateLiteral, UnaryOperator, UpdateOperator,
};
pub use statement::{
    CatchClause, ExportDeclaration, ExportEntry, ForBinding, ImportDeclaration, ImportEntry,
    ModuleDeclaration, Program, Statement, SwitchCase, VariableDeclarator, VariableKind,
};
