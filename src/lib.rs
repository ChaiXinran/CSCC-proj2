//! AgentJS: a small, embeddable JavaScript runtime designed for short-lived
//! agent tool executions.

pub mod engine;
pub mod test262;

pub use engine::{
    Engine, EvalFailure, ExecutionOptions, ExecutionReport, FailureKind, Runtime, RuntimeConfig,
};
