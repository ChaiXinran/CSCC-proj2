//! AgentJS: a small, embeddable JavaScript runtime designed for short-lived
//! agent tool executions.

pub mod ast;
pub mod backend;
pub mod builtins;
pub mod bytecode;
pub mod contracts;
pub mod engine;
pub mod lexer;
pub mod parser;
pub mod runtime;
pub mod test262;
pub mod vm;

pub use backend::BackendKind;
pub use contracts::{ChunkExecutor, NativeError, NativePipeline, ProgramCompiler, SourceParser};
pub use engine::{
    Engine, EvalFailure, ExecutionOptions, ExecutionReport, FailureKind, Runtime, RuntimeConfig,
};
